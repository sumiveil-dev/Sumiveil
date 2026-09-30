//! テキスト以外のファイル形式 (メール・Office 文書・PDF) の読み込みと書き戻し。
//!
//! どの形式も、本文を **一続きのテキスト** として取り出し、既存のエンジンで 1 回だけマスクする
//! (同じ値には同じ連番が付き、項目名などの文脈による検出もそのまま効く)。
//! 書き戻しでは、テキスト上の範囲 (segment) ごとに置換を振り分けて元のファイルの該当箇所だけを書き換える。

mod eml;
mod msg;
mod ooxml;
mod pdf;
mod xml;

use std::path::Path;

use encoding_rs::Encoding;

use crate::encoding;
use crate::engine::{Engine, MaskResult, MaskSession};

/// ファイルの種類 (拡張子で判定)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Text,
    Eml,
    Docx,
    Xlsx,
    Pptx,
    Pdf,
    Msg,
}

impl DocKind {
    pub fn from_path(path: &Path) -> DocKind {
        let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        match ext.as_str() {
            "eml" => DocKind::Eml,
            "docx" | "docm" => DocKind::Docx,
            "xlsx" | "xlsm" => DocKind::Xlsx,
            "pptx" | "pptm" => DocKind::Pptx,
            "pdf" => DocKind::Pdf,
            "msg" => DocKind::Msg,
            _ => DocKind::Text,
        }
    }

    /// テキストとしてしか書き出せない (元の形式には書き戻さない)。
    pub fn writes_text(self) -> bool {
        matches!(self, DocKind::Pdf | DocKind::Msg)
    }

    /// 取り出したテキストを編集してはいけない (書き戻しの対応がずれるため)。
    pub fn is_structured(self) -> bool {
        self != DocKind::Text
    }

    /// 文書のプロパティ (作成者など) を持つ形式。
    pub fn has_properties(self) -> bool {
        matches!(self, DocKind::Docx | DocKind::Xlsx | DocKind::Pptx)
    }

    pub fn label(self, ja: bool) -> &'static str {
        match (self, ja) {
            (DocKind::Text, true) => "テキスト",
            (DocKind::Text, false) => "Text",
            (DocKind::Eml, true) => "メール (.eml)",
            (DocKind::Eml, false) => "Email (.eml)",
            (DocKind::Docx, true) => "Word 文書",
            (DocKind::Docx, false) => "Word document",
            (DocKind::Xlsx, true) => "Excel ブック",
            (DocKind::Xlsx, false) => "Excel workbook",
            (DocKind::Pptx, true) => "PowerPoint プレゼンテーション",
            (DocKind::Pptx, false) => "PowerPoint presentation",
            (DocKind::Pdf, true) => "PDF",
            (DocKind::Pdf, false) => "PDF",
            (DocKind::Msg, true) => "Outlook メール (.msg)",
            (DocKind::Msg, false) => "Outlook message (.msg)",
        }
    }
}

/// 対応している拡張子 (ファイルを開くダイアログ用)。テキスト系は含めない。
pub const DOCUMENT_EXTENSIONS: &[&str] = &["eml", "msg", "docx", "docm", "xlsx", "xlsm", "pptx", "pptm", "pdf"];

/// 文書のプロパティ (作成者など) の扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertiesMode {
    /// 本文と同じ検出でマスクする
    Mask,
    /// 値を空にする
    Clear,
    /// そのまま残す
    Keep,
}

impl PropertiesMode {
    pub fn parse(s: &str) -> Option<PropertiesMode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mask" => Some(PropertiesMode::Mask),
            "clear" => Some(PropertiesMode::Clear),
            "keep" => Some(PropertiesMode::Keep),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PropertiesMode::Mask => "mask",
            PropertiesMode::Clear => "clear",
            PropertiesMode::Keep => "keep",
        }
    }
}

/// 表示用のプロパティ (例: 作成者 = 山田太郎)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub label: String,
    pub value: String,
}

/// 読み込んだファイル。
pub struct Document {
    pub kind: DocKind,
    /// 取り出した本文 (これをマスクする)
    pub text: String,
    /// 表示用の文字コード・形式名
    pub encoding_name: String,
    /// 利用者に知らせること (取り除いた添付ファイル・変更履歴など)
    pub warnings: Vec<String>,
    /// 文書のプロパティ (空でないもの)
    pub properties: Vec<Property>,
    /// テキスト上の位置 → 場所の表示 (例: "Sheet1!B3"、"件名")。開始位置の昇順
    locations: Vec<(usize, String)>,
    inner: Inner,
}

enum Inner {
    Text { encoding: &'static Encoding, bom: bool },
    Eml(eml::EmlDoc),
    Ooxml(ooxml::OoxmlDoc),
    /// テキストとして書き出すだけ (PDF / .msg)
    Plain,
}

/// 書き出した内容。
pub enum Output {
    /// 元と同じ形式のファイル
    Bytes(Vec<u8>),
    /// テキスト (PDF / .msg、またはテキスト形式で文字コードを変えない場合も含む)
    Text(String),
}

/// 書き出しの設定。
pub struct WriteOptions<'a> {
    pub engine: &'a Engine,
    pub session: &'a mut MaskSession,
    pub properties: PropertiesMode,
    /// テキスト形式の出力文字コード (None なら入力と同じ)
    pub text_encoding: Option<(&'static Encoding, bool)>,
}

/// 処理の結果 (レポート用)。
#[derive(Debug, Clone, Default)]
pub struct WriteReport {
    /// プロパティをどう扱ったか ("masked" / "cleared" / "kept"。プロパティがなければ None)
    pub properties: Option<&'static str>,
    /// 書き出し時に分かったこと (Excel のシート名に個人情報らしいものがある、など)
    pub warnings: Vec<String>,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("kind", &self.kind)
            .field("encoding_name", &self.encoding_name)
            .field("text_len", &self.text.len())
            .field("warnings", &self.warnings)
            .field("properties", &self.properties.len())
            .finish()
    }
}

impl Document {
    /// テキスト上の位置の場所表示 (Office 文書のセル・メールのヘッダー・PDF のページなど)。
    pub fn location_at(&self, pos: usize) -> Option<&str> {
        let i = self.locations.partition_point(|(s, _)| *s <= pos);
        (i > 0).then(|| self.locations[i - 1].1.as_str())
    }

    /// 書き出すファイルの拡張子 (PDF / .msg は txt)。
    pub fn output_extension(&self) -> Option<&'static str> {
        self.kind.writes_text().then_some("txt")
    }

    /// テキスト形式の文字コード。
    pub fn text_encoding(&self) -> Option<(&'static Encoding, bool)> {
        match &self.inner {
            Inner::Text { encoding, bom } => Some((*encoding, *bom)),
            _ => None,
        }
    }
}

/// 文字列から (UTF-8 のテキストとして)。
pub fn from_text(text: String) -> Document {
    Document {
        kind: DocKind::Text,
        text,
        encoding_name: "UTF-8".into(),
        warnings: vec![],
        properties: vec![],
        locations: vec![],
        inner: Inner::Text { encoding: encoding_rs::UTF_8, bom: false },
    }
}

/// PDF / .msg のようにテキストで書き出す形式の出力ファイル名 (`report.pdf` → `report.pdf.txt`)。
/// 同じフォルダの `report.txt` の出力と重ならないよう、元の拡張子を残す。
pub fn text_output_name(name: &str) -> String {
    format!("{name}.txt")
}

/// JSON レポートに、検出の場所 (セル・ページなど)・警告・プロパティの扱いを書き加える。
pub fn annotate_report(rep: &mut crate::report::JsonReport, doc: &Document, result: &MaskResult, write: Option<&WriteReport>) {
    if doc.kind.is_structured() {
        for (d, r) in rep.detections.iter_mut().zip(&result.replacements) {
            d.location = doc.location_at(r.start).map(str::to_string);
        }
    }
    rep.warnings = doc.warnings.clone();
    if let Some(w) = write {
        rep.warnings.extend(w.warnings.iter().cloned());
        rep.properties = w.properties.map(str::to_string);
    }
}

/// ファイルを読み込む。テキスト形式は文字コードを自動判定する (`forced` で指定も可)。
pub fn open(path: &Path, bytes: Vec<u8>, forced_encoding: Option<&str>) -> Result<Document, String> {
    let kind = DocKind::from_path(path);
    let doc = match kind {
        DocKind::Text => {
            let dec = encoding::decode(&bytes, forced_encoding);
            let mut warnings = vec![];
            if dec.had_errors {
                warnings.push("文字コードを変換できない文字がありました (置き換え文字になっています)".to_string());
            }
            Document {
                kind,
                encoding_name: dec.display_name(),
                text: dec.text,
                warnings,
                properties: vec![],
                locations: vec![],
                inner: Inner::Text { encoding: dec.encoding, bom: dec.bom },
            }
        }
        DocKind::Eml => eml::open(&bytes)?,
        DocKind::Docx | DocKind::Xlsx | DocKind::Pptx => ooxml::open(kind, bytes)?,
        DocKind::Pdf => pdf::open(&bytes)?,
        DocKind::Msg => msg::open(&bytes)?,
    };
    Ok(doc)
}

/// マスク結果を元の形式で書き出す。`result` は `doc.text` をマスクした結果であること。
pub fn write(doc: &Document, result: &MaskResult, opts: &mut WriteOptions<'_>) -> Result<(Output, WriteReport), String> {
    match &doc.inner {
        Inner::Text { encoding, bom } => {
            let (enc, bom) = opts.text_encoding.unwrap_or((*encoding, *bom));
            Ok((Output::Bytes(encoding::encode(&result.output, enc, bom)), WriteReport::default()))
        }
        Inner::Plain => Ok((Output::Text(result.output.clone()), WriteReport::default())),
        Inner::Eml(e) => Ok((Output::Bytes(eml::write(doc, e, result)?), WriteReport::default())),
        Inner::Ooxml(o) => ooxml::write(doc, o, result, opts).map(|(b, r)| (Output::Bytes(b), r)),
    }
}

// ───────────────────────── 共通部品 ─────────────────────────

/// テキストを組み立てながら、本文の範囲と元の位置 (node) の対応を記録する。
#[derive(Default)]
pub(crate) struct TextBuilder {
    pub text: String,
    /// (開始, 終了, node 番号)
    pub segs: Vec<(usize, usize, usize)>,
    pub locations: Vec<(usize, String)>,
}

impl TextBuilder {
    /// 元の位置 `node` に対応する文字列を追加する。
    pub fn push_node(&mut self, s: &str, node: usize) {
        let start = self.text.len();
        self.text.push_str(s);
        self.segs.push((start, self.text.len(), node));
    }

    /// 書き戻さない文字 (区切りや見出し) を追加する。
    pub fn push(&mut self, s: &str) {
        self.text.push_str(s);
    }

    /// 改行で終わっていなければ改行する。
    pub fn newline(&mut self) {
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.text.push('\n');
        }
    }

    pub fn location(&mut self, label: impl Into<String>) {
        let label = label.into();
        if self.locations.last().map(|(_, l)| l.as_str()) != Some(label.as_str()) {
            self.locations.push((self.text.len(), label));
        }
    }
}

/// 範囲ごとの置換後の文字列を作る。範囲をまたぐ置換は、置換後の文字を開始位置を含む範囲に入れ、
/// 続く範囲からは該当部分を取り除く (Word で「山」「田」が別の run に分かれている場合など)。
pub(crate) fn split_output(text: &str, result: &MaskResult, ranges: &[(usize, usize)]) -> Vec<String> {
    let mut reps: Vec<&crate::engine::Replacement> = result.replacements.iter().collect();
    reps.sort_by_key(|r| r.start);
    ranges
        .iter()
        .map(|&(s, e)| {
            let mut out = String::new();
            let mut pos = s;
            let first = reps.partition_point(|r| r.end <= s);
            for r in reps[first..].iter().take_while(|r| r.start < e) {
                if r.end <= s {
                    continue;
                }
                if r.start >= s {
                    if r.start > pos {
                        out.push_str(&text[pos..r.start]);
                    }
                    out.push_str(&r.replacement);
                }
                pos = pos.max(r.end.min(e));
            }
            if pos < e {
                out.push_str(&text[pos..e]);
            }
            out
        })
        .collect()
}

/// node ごとの置換後の文字列。同じ node に複数の範囲がある場合 (Excel の共有文字列が複数のセルで使われる場合)、
/// いずれかでマスクされていればそれを採る。
pub(crate) fn node_outputs(text: &str, result: &MaskResult, segs: &[(usize, usize, usize)], originals: &[String]) -> Vec<Option<String>> {
    let ranges: Vec<(usize, usize)> = segs.iter().map(|&(s, e, _)| (s, e)).collect();
    let pieces = split_output(text, result, &ranges);
    let mut out: Vec<Option<String>> = vec![None; originals.len()];
    for (&(_, _, node), piece) in segs.iter().zip(pieces) {
        if piece != originals[node] && out[node].is_none() {
            out[node] = Some(piece);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn split_across_runs() {
        // 「山田太郎様」が「山」「田太」「郎様」の 3 つに分かれている
        let text = "山田太郎様";
        let e = Engine::new(&Config::default());
        let r = e.mask(text, &mut MaskSession::default());
        assert_eq!(r.output, "<NAME_1>様");
        let a = "山".len();
        let b = a + "田太".len();
        let out = split_output(text, &r, &[(0, a), (a, b), (b, text.len())]);
        assert_eq!(out, vec!["<NAME_1>", "", "様"]);
    }

    #[test]
    fn kinds() {
        assert_eq!(DocKind::from_path(Path::new("a.DOCX")), DocKind::Docx);
        assert_eq!(DocKind::from_path(Path::new("a.md")), DocKind::Text);
        assert!(DocKind::Pdf.writes_text());
        assert_eq!(PropertiesMode::parse("Clear"), Some(PropertiesMode::Clear));
    }
}
