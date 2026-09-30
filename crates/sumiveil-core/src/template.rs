//! マスク後の表示テンプレート。
//!
//! 変数:
//! - `{label}` `{label_ja}` `{category}` `{category_ja}` `{id}`
//! - `{n}` 同じ値には同じ連番
//! - `{len}` 元の文字数
//! - `{hash}` `{hash:N}` HMAC-SHA256 の先頭 N 桁 (既定 8)
//! - `{prefix:N}` `{suffix:N}` 元の値の先頭/末尾 N 文字
//! - `{fill}` `{fill:X}` `{fill:X:N}` 文字 X を元の長さ分 (または N 回) 繰り返す
//! - `{shape}` `{shape:X}` `{shape:X:K}` 英数字だけを X に置換し区切り文字は残す (末尾 K 文字は残す)
//! - `{fake}` 形式を保ったダミー値
//!
//! `{{` と `}}` はそれぞれ `{` と `}` を表す。

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

#[derive(Debug, Clone, PartialEq)]
enum Seg {
    Lit(String),
    Label,
    LabelJa,
    Category,
    CategoryJa,
    Id,
    N,
    Len,
    Hash(usize),
    Prefix(usize),
    Suffix(usize),
    Fill(String, Option<usize>),
    Shape(String, usize),
    Fake,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    segs: Vec<Seg>,
    source: String,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum TemplateError {
    #[error("閉じられていない '{{' があります (位置 {0})")]
    Unclosed(usize),
    #[error("不明な変数 '{{{0}}}'")]
    UnknownVar(String),
    #[error("変数 '{{{0}}}' の引数が不正です")]
    BadArg(String),
}

/// テンプレート描画時に必要な情報。
pub struct RenderCtx<'a> {
    pub original: &'a str,
    pub id: &'a str,
    pub label: &'a str,
    pub label_ja: &'a str,
    pub category: &'a str,
    pub category_ja: &'a str,
    pub n: usize,
    pub hash_key: &'a [u8],
}

impl Template {
    pub fn parse(src: &str) -> Result<Self, TemplateError> {
        let mut segs = Vec::new();
        let mut lit = String::new();
        let chars: Vec<(usize, char)> = src.char_indices().collect();
        let mut i = 0;
        while i < chars.len() {
            let (pos, c) = chars[i];
            match c {
                '{' if chars.get(i + 1).map(|x| x.1) == Some('{') => {
                    lit.push('{');
                    i += 2;
                }
                '}' if chars.get(i + 1).map(|x| x.1) == Some('}') => {
                    lit.push('}');
                    i += 2;
                }
                '{' => {
                    let close = chars[i + 1..].iter().position(|x| x.1 == '}').ok_or(TemplateError::Unclosed(pos))?;
                    let inner: String = chars[i + 1..i + 1 + close].iter().map(|x| x.1).collect();
                    if !lit.is_empty() {
                        segs.push(Seg::Lit(std::mem::take(&mut lit)));
                    }
                    segs.push(parse_var(&inner)?);
                    i += close + 2;
                }
                _ => {
                    lit.push(c);
                    i += 1;
                }
            }
        }
        if !lit.is_empty() {
            segs.push(Seg::Lit(lit));
        }
        Ok(Self { segs, source: src.to_string() })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// `{n}` を使っているか (連番の採番が必要か)
    pub fn uses_number(&self) -> bool {
        self.segs.iter().any(|s| matches!(s, Seg::N | Seg::Fake))
    }

    pub fn render(&self, ctx: &RenderCtx) -> String {
        let mut out = String::new();
        for seg in &self.segs {
            match seg {
                Seg::Lit(s) => out.push_str(s),
                Seg::Label => out.push_str(ctx.label),
                Seg::LabelJa => out.push_str(ctx.label_ja),
                Seg::Category => out.push_str(ctx.category),
                Seg::CategoryJa => out.push_str(ctx.category_ja),
                Seg::Id => out.push_str(ctx.id),
                Seg::N => out.push_str(&ctx.n.to_string()),
                Seg::Len => out.push_str(&ctx.original.chars().count().to_string()),
                Seg::Hash(n) => {
                    let h = hmac_hex(ctx.hash_key, ctx.label, ctx.original);
                    out.push_str(&h[..(*n).min(h.len())]);
                }
                Seg::Prefix(n) => out.extend(ctx.original.chars().take(*n)),
                Seg::Suffix(n) => {
                    let count = ctx.original.chars().count();
                    out.extend(ctx.original.chars().skip(count.saturating_sub(*n)));
                }
                Seg::Fill(x, n) => {
                    let count = n.unwrap_or_else(|| ctx.original.chars().count());
                    for _ in 0..count {
                        out.push_str(x);
                    }
                }
                Seg::Shape(x, keep) => out.push_str(&shape(ctx.original, x, *keep)),
                Seg::Fake => out.push_str(&crate::fake::fake(ctx)),
            }
        }
        out
    }
}

fn parse_var(inner: &str) -> Result<Seg, TemplateError> {
    let mut parts = inner.splitn(3, ':');
    let name = parts.next().unwrap_or("").trim();
    let a1 = parts.next();
    let a2 = parts.next();
    let num = |a: Option<&str>, default: usize| -> Result<usize, TemplateError> {
        match a {
            None => Ok(default),
            Some(s) => s.trim().parse().map_err(|_| TemplateError::BadArg(inner.to_string())),
        }
    };
    Ok(match name {
        "label" => Seg::Label,
        "label_ja" => Seg::LabelJa,
        "category" => Seg::Category,
        "category_ja" => Seg::CategoryJa,
        "id" => Seg::Id,
        "n" => Seg::N,
        "len" => Seg::Len,
        "hash" => Seg::Hash(num(a1, 8)?.clamp(1, 64)),
        "prefix" => Seg::Prefix(num(a1, 1)?),
        "suffix" => Seg::Suffix(num(a1, 4)?),
        "fill" => {
            let x = a1.filter(|s| !s.is_empty()).unwrap_or("*").to_string();
            let n = match a2 {
                Some(s) => Some(s.trim().parse().map_err(|_| TemplateError::BadArg(inner.to_string()))?),
                None => None,
            };
            Seg::Fill(x, n)
        }
        "shape" => {
            let x = a1.filter(|s| !s.is_empty()).unwrap_or("*").to_string();
            Seg::Shape(x, num(a2, 0)?)
        }
        "fake" => Seg::Fake,
        _ => return Err(TemplateError::UnknownVar(name.to_string())),
    })
}

/// 英数字 (全角含む)・かな漢字を x に置換し、区切り文字は残す。末尾 keep 文字の英数字は残す。
fn shape(original: &str, x: &str, keep: usize) -> String {
    let chars: Vec<char> = original.chars().collect();
    let total_alnum = chars.iter().filter(|c| c.is_alphanumeric()).count();
    let mut seen = 0;
    let mut out = String::new();
    for c in chars {
        if c.is_alphanumeric() {
            seen += 1;
            if seen > total_alnum.saturating_sub(keep) {
                out.push(c);
            } else {
                out.push_str(x);
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn hmac_hex(key: &[u8], label: &str, value: &str) -> String {
    let key = if key.is_empty() { b"sumiveil".as_slice() } else { key };
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(label.as_bytes());
    mac.update(b":");
    mac.update(value.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// よく使うテンプレートのプリセット (GUI のドロップダウン用)。
pub const PRESETS: &[(&str, &str, &str)] = &[
    ("<{label}_{n}>", "Numbered tag", "連番タグ  <EMAIL_1>"),
    ("[{label}]", "Tag", "タグ  [EMAIL]"),
    ("[{label_ja}]", "Japanese tag", "日本語タグ  [メールアドレス]"),
    ("{fill:*}", "Fill with *", "* で塗りつぶし (長さ維持)"),
    ("{fill:●}", "Fill with ●", "● で塗りつぶし (長さ維持)"),
    ("{fill:█}", "Black bar", "黒塗り (長さ維持)"),
    ("{fill:*:8}", "Fixed ********", "固定長 ********"),
    ("{shape:*}", "Keep separators", "区切り維持  ***-****-****"),
    ("{shape:*:4}", "Keep last 4", "末尾4文字を残す  ***-****-5678"),
    ("{label}_{hash:8}", "Hash", "ハッシュ  EMAIL_1a2b3c4d"),
    ("{fake}", "Fake value", "ダミー値に置換"),
    ("■", "Single mark", "■ 1文字"),
    ("[REDACTED]", "Fixed word", "固定の語  [REDACTED]"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(orig: &str) -> RenderCtx<'_> {
        RenderCtx {
            original: orig,
            id: "phone_jp",
            label: "PHONE",
            label_ja: "電話番号",
            category: "contact",
            category_ja: "連絡先",
            n: 3,
            hash_key: b"salt",
        }
    }

    fn r(t: &str, orig: &str) -> String {
        Template::parse(t).unwrap().render(&ctx(orig))
    }

    #[test]
    fn basics() {
        assert_eq!(r("<{label}_{n}>", "x"), "<PHONE_3>");
        assert_eq!(r("[{label_ja}]", "x"), "[電話番号]");
        assert_eq!(r("{{literal}}", "x"), "{literal}");
        assert_eq!(r("{fill:●}", "あいう"), "●●●");
        assert_eq!(r("{fill:*:5}", "ab"), "*****");
        assert_eq!(r("{shape:*}", "090-1234-5678"), "***-****-****");
        assert_eq!(r("{shape:*:4}", "090-1234-5678"), "***-****-5678");
        assert_eq!(r("{prefix:3}…{suffix:2}", "abcdefg"), "abc…fg");
        assert_eq!(r("{len}", "日本語"), "3");
        assert_eq!(r("{hash:6}", "a").len(), 6);
        assert_eq!(r("{hash}", "a"), r("{hash}", "a"));
        assert_ne!(r("{hash}", "a"), r("{hash}", "b"));
    }

    #[test]
    fn errors() {
        assert!(matches!(Template::parse("{nope}"), Err(TemplateError::UnknownVar(_))));
        assert!(matches!(Template::parse("abc {label"), Err(TemplateError::Unclosed(_))));
        assert!(matches!(Template::parse("{hash:x}"), Err(TemplateError::BadArg(_))));
    }

    #[test]
    fn presets_parse() {
        for (p, _, _) in PRESETS {
            Template::parse(p).unwrap();
        }
    }
}
