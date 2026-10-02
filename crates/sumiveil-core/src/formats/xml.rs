//! Office 文書 (OOXML) 用の最小限の XML 走査。
//!
//! 書き戻しで元のバイト列 (書式・名前空間・属性の並び) をそのまま残すため、汎用の XML ライブラリは使わず、
//! タグと文字の位置だけを取り出す。対象は機械が生成した整形式の XML。

/// 走査結果の 1 要素。位置はすべて元の文字列のバイト位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok<'a> {
    /// 開始タグ。`tag` は `<` から `>` まで。`self_closing` は `<a/>`
    Start { name: &'a str, tag: (usize, usize), self_closing: bool },
    End { name: &'a str },
    /// タグの間の文字 (エスケープされたまま)
    Text { span: (usize, usize) },
}

pub fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// XML を走査する。コメント・CDATA・処理命令・DOCTYPE は読み飛ばす。
pub fn scan(xml: &str) -> Vec<Tok<'_>> {
    let b = xml.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        let lt = match memchr(b'<', &b[i..]) {
            Some(p) => i + p,
            None => {
                out.push(Tok::Text { span: (i, b.len()) });
                break;
            }
        };
        if lt > i {
            out.push(Tok::Text { span: (i, lt) });
        }
        let rest = &xml[lt..];
        let skip_to = |pat: &str| rest.find(pat).map(|p| lt + p + pat.len()).unwrap_or(b.len());
        if rest.starts_with("<!--") {
            i = skip_to("-->");
        } else if rest.starts_with("<![CDATA[") {
            i = skip_to("]]>");
        } else if rest.starts_with("<?") {
            i = skip_to("?>");
        } else if rest.starts_with("<!") {
            i = skip_to(">");
        } else if rest.starts_with("</") {
            let end = skip_to(">");
            let name = xml[lt + 2..end.saturating_sub(1).max(lt + 2)].trim();
            out.push(Tok::End { name });
            i = end;
        } else {
            // 開始タグ: 属性値の中の > に注意して終わりを探す
            let mut j = lt + 1;
            let mut quote: Option<u8> = None;
            while j < b.len() {
                match (quote, b[j]) {
                    (None, b'"') | (None, b'\'') => quote = Some(b[j]),
                    (Some(q), c) if c == q => quote = None,
                    (None, b'>') => break,
                    _ => {}
                }
                j += 1;
            }
            let end = (j + 1).min(b.len());
            let self_closing = j > 0 && b.get(j - 1) == Some(&b'/');
            let name_end = xml[lt + 1..j].find(|c: char| c.is_whitespace() || c == '/' || c == '>').map(|p| lt + 1 + p).unwrap_or(j);
            out.push(Tok::Start { name: &xml[lt + 1..name_end], tag: (lt, end), self_closing });
            i = end;
        }
    }
    out
}

fn memchr(needle: u8, hay: &[u8]) -> Option<usize> {
    hay.iter().position(|&c| c == needle)
}

/// タグ内の属性 (名前, 値の範囲 (引用符の内側))。
pub fn attrs(xml: &str, tag: (usize, usize)) -> Vec<(&str, (usize, usize))> {
    let b = xml.as_bytes();
    let (start, end) = tag;
    let mut out = vec![];
    // 要素名を飛ばす
    let mut i = start + 1;
    while i < end && !b[i].is_ascii_whitespace() && b[i] != b'/' && b[i] != b'>' {
        i += 1;
    }
    loop {
        while i < end && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        if i >= end || b[i] == b'>' {
            break;
        }
        let name_start = i;
        while i < end && b[i] != b'=' && !b[i].is_ascii_whitespace() && b[i] != b'>' {
            i += 1;
        }
        let name = &xml[name_start..i];
        while i < end && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= end || b[i] != b'=' {
            continue;
        }
        i += 1;
        while i < end && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= end {
            break;
        }
        let q = b[i];
        if q != b'"' && q != b'\'' {
            break;
        }
        let v_start = i + 1;
        let v_end = xml[v_start..end].find(q as char).map(|p| v_start + p).unwrap_or(end);
        out.push((name, (v_start, v_end)));
        i = v_end + 1;
    }
    out
}

/// 属性の値 (エスケープを解除したもの)。
pub fn attr_value(xml: &str, tag: (usize, usize), name: &str) -> Option<String> {
    attrs(xml, tag).into_iter().find(|(n, _)| *n == name || local_name(n) == name).map(|(_, (s, e))| unescape(&xml[s..e]))
}

/// エスケープの解除 (定義済みの実体と文字参照)。
pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        let Some(semi) = rest.find(';').filter(|&i| i <= 12) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// 要素の文字としてエスケープする。
pub fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// 属性値としてエスケープする。
pub fn escape_attr(s: &str) -> String {
    escape_text(s).replace('"', "&quot;").replace('\'', "&apos;")
}

/// 置き換え (範囲, 新しい値 (エスケープ済み)) を適用した文字列を作る。範囲は重ならないこと。
pub fn apply_edits(xml: &str, mut edits: Vec<((usize, usize), String)>) -> String {
    edits.sort_by_key(|(r, _)| r.0);
    let mut out = String::with_capacity(xml.len());
    let mut pos = 0;
    for ((s, e), v) in edits {
        if s < pos {
            continue;
        }
        out.push_str(&xml[pos..s]);
        out.push_str(&v);
        pos = e;
    }
    out.push_str(&xml[pos..]);
    out
}

#[cfg(test)]
#[path = "../tests/formats_xml.rs"]
mod tests;
