//! Office Open XML (Word .docx / Excel .xlsx / PowerPoint .pptx)。
//!
//! ZIP の中の XML から本文の文字を取り出し、書き戻しでは該当する文字・属性だけを置き換える
//! (書式や未知の要素はバイト単位でそのまま残す。変更しない部品は圧縮済みのまま複製する)。

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::xml::{self, local_name, Tok};
use super::{node_outputs, DocKind, Document, Inner, PropertiesMode, Property, TextBuilder, WriteOptions, WriteReport};
use crate::engine::MaskResult;

/// 1 つの部品として読み込む XML の上限 (壊れたファイル・圧縮爆弾対策)。
const MAX_PART: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone)]
struct Node {
    part: usize,
    span: (usize, usize),
    attr: bool,
}

struct PropNode {
    node: Node,
    label: String,
    value: String,
}

pub(crate) struct OoxmlDoc {
    bytes: Vec<u8>,
    /// 読み込んだ XML 部品 (名前, 内容)
    parts: Vec<(String, String)>,
    nodes: Vec<Node>,
    originals: Vec<String>,
    segs: Vec<(usize, usize, usize)>,
    props: Vec<PropNode>,
    /// Excel のふりがな: 共有文字列の本文 node のどれかがマスクされたら、ふりがなの node を空にする
    phonetic: Vec<(Vec<usize>, Vec<Node>)>,
    /// Excel のシート名 (テキスト上の範囲, 名前)。シート名は数式から参照されるため書き換えない
    sheet_names: Vec<(usize, usize, String)>,
}

struct Ctx {
    names: Vec<String>,
    parts: Vec<(String, String)>,
    nodes: Vec<Node>,
    originals: Vec<String>,
    props: Vec<PropNode>,
    b: TextBuilder,
}

impl Ctx {
    fn load(&mut self, zip: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<usize> {
        if let Some(i) = self.parts.iter().position(|(n, _)| n == name) {
            return Some(i);
        }
        let f = zip.by_name(name).ok()?;
        let mut s = String::new();
        f.take(MAX_PART).read_to_string(&mut s).ok()?;
        self.parts.push((name.to_string(), s));
        Some(self.parts.len() - 1)
    }

    fn add_node(&mut self, part: usize, span: (usize, usize), attr: bool, text: String) -> usize {
        self.nodes.push(Node { part, span, attr });
        self.originals.push(text);
        self.nodes.len() - 1
    }

    fn add_prop(&mut self, part: usize, span: (usize, usize), attr: bool, label: &str, value: String) {
        if value.trim().is_empty() {
            return;
        }
        self.props.push(PropNode { node: Node { part, span, attr }, label: label.to_string(), value });
    }

    /// 名前が条件に合う部品 (自然順: slide2 < slide10)。
    fn matching(&self, pred: impl Fn(&str) -> bool) -> Vec<String> {
        let mut v: Vec<String> = self.names.iter().filter(|n| pred(n)).cloned().collect();
        v.sort_by_key(|n| natural_key(n));
        v
    }

    /// 本文の部品を読む。`text_elems` の中の文字を本文とし、`para_elems` の終わりで改行する。
    /// 作成者名などの属性 (`prop_attrs`) はプロパティとして記録する。
    fn body_part(&mut self, part: usize, loc: &str, text_elems: &[&str], para_elems: &[&str], prop_attrs: &[(&str, &str)]) {
        let xml = self.parts[part].1.clone();
        self.b.newline();
        self.b.location(loc.to_string());
        let mut in_text = false;
        for tok in xml::scan(&xml) {
            match tok {
                Tok::Start { name, tag, self_closing } => {
                    let l = local_name(name);
                    for (an, span) in xml::attrs(&xml, tag) {
                        if let Some((_, label)) = prop_attrs.iter().find(|(a, _)| *a == local_name(an)) {
                            let v = xml::unescape(&xml[span.0..span.1]);
                            self.add_prop(part, span, true, label, v);
                        }
                    }
                    if self_closing {
                        match l {
                            "tab" => self.b.push("\t"),
                            "br" | "cr" => self.b.push("\n"),
                            _ => {}
                        }
                    } else if text_elems.contains(&l) {
                        in_text = true;
                    }
                }
                Tok::Text { span } if in_text => {
                    let t = xml::unescape(&xml[span.0..span.1]);
                    let n = self.add_node(part, span, false, t.clone());
                    self.b.push_node(&t, n);
                }
                Tok::End { name } => {
                    let l = local_name(name);
                    if text_elems.contains(&l) {
                        in_text = false;
                    }
                    // フィールドコード (HYPERLINK "mailto:…" など) と表示文字の間を区切る
                    if l == "instrText" {
                        self.b.push(" ");
                    }
                    if para_elems.contains(&l) {
                        self.b.newline();
                    }
                }
                _ => {}
            }
        }
    }

    /// `*.rels` の外部リンク先 (mailto: や URL) を本文として読む。
    fn rels_part(&mut self, part: usize) {
        let xml = self.parts[part].1.clone();
        for tok in xml::scan(&xml) {
            if let Tok::Start { name, tag, .. } = tok {
                if local_name(name) != "Relationship" || xml::attr_value(&xml, tag, "TargetMode").as_deref() != Some("External") {
                    continue;
                }
                if let Some((_, span)) = xml::attrs(&xml, tag).into_iter().find(|(n, _)| *n == "Target") {
                    let t = xml::unescape(&xml[span.0..span.1]);
                    self.b.newline();
                    self.b.location("リンク");
                    let n = self.add_node(part, span, true, t.clone());
                    self.b.push_node(&t, n);
                    self.b.newline();
                }
            }
        }
    }

    /// 要素の文字をプロパティとして読む (`docProps/core.xml` の作成者など)。
    fn prop_elements(&mut self, part: usize, map: &[(&str, &str)]) {
        let xml = self.parts[part].1.clone();
        let mut cur: Option<&str> = None;
        for tok in xml::scan(&xml) {
            match tok {
                Tok::Start { name, self_closing: false, .. } => cur = map.iter().find(|(e, _)| *e == local_name(name)).map(|(_, l)| *l),
                Tok::Text { span } => {
                    if let Some(label) = cur {
                        let v = xml::unescape(&xml[span.0..span.1]);
                        self.add_prop(part, span, false, label, v);
                    }
                }
                Tok::End { .. } => cur = None,
                _ => {}
            }
        }
    }

    /// `docProps/custom.xml` のユーザー設定のプロパティ。
    fn custom_props(&mut self, part: usize) {
        let xml = self.parts[part].1.clone();
        let mut prop_name: Option<String> = None;
        let mut in_value = false;
        for tok in xml::scan(&xml) {
            match tok {
                Tok::Start { name, tag, self_closing: false } => {
                    if local_name(name) == "property" {
                        prop_name = xml::attr_value(&xml, tag, "name");
                    } else if prop_name.is_some() {
                        in_value = true;
                    }
                }
                Tok::Text { span } if in_value => {
                    let v = xml::unescape(&xml[span.0..span.1]);
                    let label = prop_name.clone().unwrap_or_default();
                    self.add_prop(part, span, false, &label, v);
                }
                Tok::End { name } => {
                    in_value = false;
                    if local_name(name) == "property" {
                        prop_name = None;
                    }
                }
                _ => {}
            }
        }
    }

    fn doc_props(&mut self, zip: &mut ZipArchive<Cursor<&[u8]>>) {
        if let Some(p) = self.load(zip, "docProps/core.xml") {
            self.prop_elements(
                p,
                &[
                    ("creator", "作成者"),
                    ("lastModifiedBy", "最終更新者"),
                    ("title", "タイトル"),
                    ("subject", "件名"),
                    ("keywords", "キーワード"),
                    ("description", "コメント"),
                    ("category", "分類"),
                ],
            );
        }
        if let Some(p) = self.load(zip, "docProps/app.xml") {
            self.prop_elements(p, &[("Company", "会社"), ("Manager", "管理者")]);
        }
        if let Some(p) = self.load(zip, "docProps/custom.xml") {
            self.custom_props(p);
        }
    }

    fn rels(&mut self, zip: &mut ZipArchive<Cursor<&[u8]>>, prefix: &str) {
        for n in self.matching(|n| n.starts_with(prefix) && n.ends_with(".rels")) {
            if let Some(p) = self.load(zip, &n) {
                self.rels_part(p);
            }
        }
    }
}

/// 作成者名の属性 (コメント・変更履歴)。
const AUTHOR_ATTRS: &[(&str, &str)] = &[("author", "コメント・変更履歴の作成者"), ("initials", "コメント・変更履歴のイニシャル")];

fn natural_key(name: &str) -> (String, u64, String) {
    let stem = name.rsplit('/').next().unwrap_or(name);
    let digits: String = stem.chars().rev().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    let prefix = name.trim_end_matches(|c: char| !c.is_ascii_digit()).trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
    (prefix, digits.parse().unwrap_or(0), name.to_string())
}

fn slide_number(name: &str) -> String {
    let (_, n, _) = natural_key(name);
    n.to_string()
}

pub(crate) fn open(kind: DocKind, bytes: Vec<u8>) -> Result<Document, String> {
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        return Err("パスワードで保護された文書、または古い形式 (.doc / .xls / .ppt) のファイルは開けません。保護を解除するか、新しい形式で保存し直してください".into());
    }
    let mut zip = ZipArchive::new(Cursor::new(&bytes[..])).map_err(|e| format!("Office 文書として読み込めません: {e}"))?;
    let names: Vec<String> = (0..zip.len()).filter_map(|i| zip.by_index_raw(i).ok().map(|f| f.name().to_string())).collect();
    let mut c = Ctx { names, parts: vec![], nodes: vec![], originals: vec![], props: vec![], b: TextBuilder::default() };
    let mut warnings = vec![];
    let mut phonetic = vec![];
    let mut sheet_names = vec![];
    match kind {
        DocKind::Docx => {
            let main = c.load(&mut zip, "word/document.xml").ok_or("word/document.xml がありません")?;
            c.body_part(main, "本文", &["t", "delText", "instrText"], &["p"], AUTHOR_ATTRS);
            let extra: [(&str, &str); 6] = [
                ("word/header", "ヘッダー"),
                ("word/footer", "フッター"),
                ("word/footnotes", "脚注"),
                ("word/endnotes", "文末脚注"),
                ("word/comments", "コメント"),
                ("word/charts/chart", "グラフ"),
            ];
            for (prefix, label) in extra {
                for n in c.matching(|n| n.starts_with(prefix) && n.ends_with(".xml")) {
                    if let Some(p) = c.load(&mut zip, &n) {
                        c.body_part(p, label, &["t", "delText", "instrText"], &["p"], AUTHOR_ATTRS);
                    }
                }
            }
            for n in c.matching(|n| n.starts_with("word/diagrams/data") && n.ends_with(".xml")) {
                if let Some(p) = c.load(&mut zip, &n) {
                    c.body_part(p, "図表", &["t"], &["p"], &[]);
                }
            }
            if let Some(p) = c.load(&mut zip, "word/people.xml") {
                c.body_part(p, "作成者一覧", &[], &[], &[("author", "コメント・変更履歴の作成者"), ("userId", "作成者のアカウント")]);
            }
            c.rels(&mut zip, "word/_rels/");
            let doc_xml = &c.parts[main].1;
            if doc_xml.contains("<w:ins ") || doc_xml.contains("<w:del ") {
                warnings.push("変更履歴が残っています。削除した文字もマスクしましたが、共有前に変更履歴を承諾・削除することをおすすめします".to_string());
            }
        }
        DocKind::Pptx => {
            let slides = c.matching(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"));
            if slides.is_empty() && !c.names.iter().any(|n| n == "ppt/presentation.xml") {
                return Err("PowerPoint のプレゼンテーションとして読み込めません".into());
            }
            for n in slides {
                if let Some(p) = c.load(&mut zip, &n) {
                    c.body_part(p, &format!("スライド {}", slide_number(&n)), &["t"], &["p"], &[]);
                }
            }
            for n in c.matching(|n| n.starts_with("ppt/notesSlides/notesSlide") && n.ends_with(".xml")) {
                if let Some(p) = c.load(&mut zip, &n) {
                    c.body_part(p, &format!("ノート {}", slide_number(&n)), &["t"], &["p"], &[]);
                }
            }
            for n in c.matching(|n| n.starts_with("ppt/comments/") && n.ends_with(".xml")) {
                if let Some(p) = c.load(&mut zip, &n) {
                    c.body_part(p, "コメント", &["t", "text"], &["p", "text"], &[]);
                }
            }
            for prefix in ["ppt/charts/chart", "ppt/diagrams/data"] {
                for n in c.matching(|n| n.starts_with(prefix) && n.ends_with(".xml")) {
                    if let Some(p) = c.load(&mut zip, &n) {
                        c.body_part(p, "グラフ・図表", &["t"], &["p"], &[]);
                    }
                }
            }
            for n in ["ppt/commentAuthors.xml", "ppt/authors.xml"] {
                if let Some(p) = c.load(&mut zip, n) {
                    c.body_part(p, "作成者一覧", &[], &[], &[("name", "コメントの作成者"), ("initials", "コメントの作成者のイニシャル"), ("userId", "作成者のアカウント")]);
                }
            }
            c.rels(&mut zip, "ppt/slides/_rels/");
            c.rels(&mut zip, "ppt/notesSlides/_rels/");
        }
        DocKind::Xlsx => xlsx(&mut c, &mut zip, &mut phonetic, &mut sheet_names, &mut warnings)?,
        _ => unreachable!(),
    }
    c.doc_props(&mut zip);
    if c.names.iter().any(|n| n.contains("/media/") || n.contains("/embeddings/")) {
        warnings.push("画像や埋め込まれたファイルの中身はマスクされません (そのまま残ります)".to_string());
    }
    let mut properties: Vec<Property> = vec![];
    for p in &c.props {
        let prop = Property { label: p.label.clone(), value: p.value.clone() };
        if !properties.contains(&prop) {
            properties.push(prop);
        }
    }
    let Ctx { parts, nodes, originals, props, b, .. } = c;
    Ok(Document {
        kind,
        text: b.text,
        encoding_name: kind.label(true).to_string(),
        warnings,
        properties,
        locations: b.locations,
        inner: Inner::Ooxml(OoxmlDoc { bytes, parts, nodes, originals, segs: b.segs, props, phonetic, sheet_names }),
    })
}

/// Excel: 共有文字列の 1 項目。
struct Si {
    /// (node, 文字)
    main: Vec<(usize, String)>,
    phonetic: Vec<Node>,
    used: bool,
}

fn xlsx(
    c: &mut Ctx,
    zip: &mut ZipArchive<Cursor<&[u8]>>,
    phonetic: &mut Vec<(Vec<usize>, Vec<Node>)>,
    sheet_names: &mut Vec<(usize, usize, String)>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let wb = c.load(zip, "xl/workbook.xml").ok_or("Excel のブックとして読み込めません (xl/workbook.xml がありません)")?;
    // シートの一覧と、r:id → 部品名
    let wb_xml = c.parts[wb].1.clone();
    let mut sheets: Vec<(String, String)> = vec![];
    for tok in xml::scan(&wb_xml) {
        if let Tok::Start { name, tag, .. } = tok {
            if local_name(name) == "sheet" {
                let n = xml::attr_value(&wb_xml, tag, "name").unwrap_or_default();
                let rid = xml::attrs(&wb_xml, tag).into_iter().find(|(a, _)| a.ends_with(":id")).map(|(_, s)| wb_xml[s.0..s.1].to_string()).unwrap_or_default();
                sheets.push((n, rid));
            }
        }
    }
    let mut targets: HashMap<String, String> = HashMap::new();
    if let Some(r) = c.load(zip, "xl/_rels/workbook.xml.rels") {
        let rx = c.parts[r].1.clone();
        for tok in xml::scan(&rx) {
            if let Tok::Start { name, tag, .. } = tok {
                if local_name(name) == "Relationship" {
                    if let (Some(id), Some(t)) = (xml::attr_value(&rx, tag, "Id"), xml::attr_value(&rx, tag, "Target")) {
                        let t = if let Some(abs) = t.strip_prefix('/') { abs.to_string() } else { format!("xl/{t}") };
                        targets.insert(id, t);
                    }
                }
            }
        }
    }

    // 共有文字列
    let mut sis: Vec<Si> = vec![];
    if let Some(p) = c.load(zip, "xl/sharedStrings.xml") {
        let x = c.parts[p].1.clone();
        let (mut in_t, mut in_ph) = (false, false);
        let mut cur: Option<Si> = None;
        for tok in xml::scan(&x) {
            match tok {
                Tok::Start { name, self_closing, .. } => match local_name(name) {
                    "si" if !self_closing => cur = Some(Si { main: vec![], phonetic: vec![], used: false }),
                    "si" => sis.push(Si { main: vec![], phonetic: vec![], used: false }),
                    "rPh" if !self_closing => in_ph = true,
                    "t" if !self_closing => in_t = true,
                    _ => {}
                },
                Tok::Text { span } if in_t => {
                    let t = xml::unescape(&x[span.0..span.1]);
                    if let Some(si) = cur.as_mut() {
                        if in_ph {
                            si.phonetic.push(Node { part: p, span, attr: false });
                        } else {
                            let n = c.add_node(p, span, false, t.clone());
                            si.main.push((n, t));
                        }
                    }
                }
                Tok::End { name } => match local_name(name) {
                    "t" => in_t = false,
                    "rPh" => in_ph = false,
                    "si" => sis.extend(cur.take()),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    // シートを行の順に読む (セルは \t、行は改行で区切る)
    let mut has_numbers = false;
    for (sheet_name, rid) in &sheets {
        let Some(target) = targets.get(rid).cloned() else { continue };
        let Some(p) = c.load(zip, &target) else { continue };
        let x = c.parts[p].1.clone();
        c.b.newline();
        c.b.push("[シート: ");
        let s = c.b.text.len();
        c.b.push(sheet_name);
        sheet_names.push((s, c.b.text.len(), sheet_name.clone()));
        c.b.push("]\n");
        let (mut cell_ref, mut cell_type) = (String::new(), String::new());
        let (mut in_v, mut in_is_t) = (false, false);
        let mut v_text = String::new();
        let mut inline: Vec<(usize, String)> = vec![];
        let mut first_in_row = true;
        for tok in xml::scan(&x) {
            match tok {
                Tok::Start { name, tag, self_closing } => match local_name(name) {
                    "row" => first_in_row = true,
                    "c" if !self_closing => {
                        cell_ref = xml::attr_value(&x, tag, "r").unwrap_or_default();
                        cell_type = xml::attr_value(&x, tag, "t").unwrap_or_default();
                        v_text.clear();
                        inline.clear();
                    }
                    "v" if !self_closing => in_v = true,
                    "t" if !self_closing => in_is_t = true,
                    _ => {}
                },
                Tok::Text { span } => {
                    if in_v {
                        v_text.push_str(&xml::unescape(&x[span.0..span.1]));
                    } else if in_is_t {
                        let t = xml::unescape(&x[span.0..span.1]);
                        let n = c.add_node(p, span, false, t.clone());
                        inline.push((n, t));
                    }
                }
                Tok::End { name } => match local_name(name) {
                    "v" => in_v = false,
                    "t" => in_is_t = false,
                    "c" => {
                        let content: Vec<(usize, String)> = match cell_type.as_str() {
                            "s" => match v_text.trim().parse::<usize>().ok().and_then(|i| sis.get_mut(i)) {
                                Some(si) => {
                                    si.used = true;
                                    si.main.clone()
                                }
                                None => vec![],
                            },
                            "inlineStr" => std::mem::take(&mut inline),
                            "" | "n" => {
                                has_numbers |= v_text.trim().len() >= 9 && v_text.trim().chars().all(|ch| ch.is_ascii_digit());
                                vec![]
                            }
                            _ => vec![],
                        };
                        if !content.is_empty() {
                            if !first_in_row {
                                c.b.push("\t");
                            }
                            first_in_row = false;
                            c.b.location(format!("{sheet_name}!{cell_ref}"));
                            for (n, t) in content {
                                c.b.push_node(&t, n);
                            }
                        }
                    }
                    "row" => c.b.newline(),
                    _ => {}
                },
            }
        }
    }
    // どのセルからも使われていない共有文字列もマスクする
    if sis.iter().any(|s| !s.used && !s.main.is_empty()) {
        c.b.newline();
        c.b.location("共有文字列");
        for si in sis.iter().filter(|s| !s.used) {
            for (n, t) in &si.main {
                c.b.push_node(t, *n);
            }
            c.b.newline();
        }
    }
    for si in sis {
        if !si.phonetic.is_empty() {
            phonetic.push((si.main.iter().map(|(n, _)| *n).collect(), si.phonetic));
        }
    }
    if has_numbers {
        warnings.push("数値として入力された長い番号 (電話番号など) はマスクされません。文字列として入力し直すか、確認してください".to_string());
    }

    // コメント・図形・グラフ・リンク
    for n in c.matching(|n| n.starts_with("xl/comments") && n.ends_with(".xml")) {
        if let Some(p) = c.load(zip, &n) {
            c.body_part(p, "コメント", &["t"], &["comment"], &[]);
            c.prop_elements(p, &[("author", "コメントの作成者")]);
        }
    }
    for n in c.matching(|n| n.starts_with("xl/threadedComments/") && n.ends_with(".xml")) {
        if let Some(p) = c.load(zip, &n) {
            c.body_part(p, "コメント", &["text"], &["threadedComment"], &[]);
        }
    }
    for prefix in ["xl/drawings/drawing", "xl/charts/chart"] {
        for n in c.matching(|n| n.starts_with(prefix) && n.ends_with(".xml")) {
            if let Some(p) = c.load(zip, &n) {
                c.body_part(p, "図形・グラフ", &["t"], &["p"], &[]);
            }
        }
    }
    for n in c.matching(|n| n.starts_with("xl/persons/") && n.ends_with(".xml")) {
        if let Some(p) = c.load(zip, &n) {
            c.body_part(p, "作成者一覧", &[], &[], &[("displayName", "コメントの作成者"), ("userId", "作成者のアカウント")]);
        }
    }
    c.rels(zip, "xl/worksheets/_rels/");
    c.rels(zip, "xl/drawings/_rels/");
    Ok(())
}

pub(crate) fn write(doc: &Document, o: &OoxmlDoc, result: &MaskResult, opts: &mut WriteOptions<'_>) -> Result<(Vec<u8>, WriteReport), String> {
    let mut report = WriteReport::default();
    let new_nodes = node_outputs(&doc.text, result, &o.segs, &o.originals);
    let mut edits: HashMap<usize, Vec<((usize, usize), String)>> = HashMap::new();
    let mut edit = |node: &Node, value: &str| {
        let v = if node.attr { xml::escape_attr(value) } else { xml::escape_text(value) };
        edits.entry(node.part).or_default().push((node.span, v));
    };
    for (i, new) in new_nodes.iter().enumerate() {
        if let Some(v) = new {
            edit(&o.nodes[i], v);
        }
    }
    // 共有文字列がマスクされたら、そのふりがなも消す
    for (main, ph) in &o.phonetic {
        if main.iter().any(|n| new_nodes[*n].is_some()) {
            for node in ph {
                edit(node, "");
            }
        }
    }
    // プロパティ (作成者など)
    if !o.props.is_empty() {
        report.properties = Some(match opts.properties {
            PropertiesMode::Keep => "kept",
            PropertiesMode::Clear => {
                for p in &o.props {
                    edit(&p.node, "");
                }
                "cleared"
            }
            PropertiesMode::Mask => {
                for p in &o.props {
                    let masked = opts.engine.mask(&p.value, opts.session).output;
                    if masked != p.value {
                        edit(&p.node, &masked);
                    }
                }
                "masked"
            }
        });
    }
    // シート名は書き換えない (数式から参照されるため)。個人情報らしい場合は知らせる
    for (s, e, name) in &o.sheet_names {
        if result.replacements.iter().any(|r| r.start < *e && r.end > *s) {
            report.warnings.push(format!("シート名「{name}」に個人情報が含まれている可能性があります (シート名は変更していません)"));
        }
    }

    // ZIP を組み立て直す (変更しない部品は圧縮済みのまま複製)
    let mut zin = ZipArchive::new(Cursor::new(&o.bytes[..])).map_err(|e| e.to_string())?;
    let mut zout = ZipWriter::new(Cursor::new(Vec::with_capacity(o.bytes.len())));
    for i in 0..zin.len() {
        let name = zin.by_index_raw(i).map_err(|e| e.to_string())?.name().to_string();
        let part = o.parts.iter().position(|(n, _)| *n == name);
        match part.and_then(|p| edits.remove(&p).map(|e| (p, e))) {
            Some((p, e)) => {
                let method = zin.by_index_raw(i).map(|f| f.compression()).unwrap_or(CompressionMethod::Deflated);
                let method = if method == CompressionMethod::Stored { method } else { CompressionMethod::Deflated };
                let data = xml::apply_edits(&o.parts[p].1, e);
                zout.start_file(name, SimpleFileOptions::default().compression_method(method)).map_err(|e| e.to_string())?;
                zout.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
            }
            None => {
                let f = zin.by_index_raw(i).map_err(|e| e.to_string())?;
                zout.raw_copy_file(f).map_err(|e| e.to_string())?;
            }
        }
    }
    let out = zout.finish().map_err(|e| e.to_string())?.into_inner();
    Ok((out, report))
}
