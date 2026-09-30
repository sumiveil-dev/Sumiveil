//! 検出器の実行部。カタログの正規表現定義をコンパイル・キャッシュし、境界・文脈・検証を適用する。

use std::sync::{Arc, OnceLock};

use regex::Regex;

use crate::catalog::{Boundary, ContextSpec, RegexSpec, SpecKind, CATALOG, PREFECTURES};
use crate::text::{back_chars, char_after, char_before, forward_chars, is_digit_like, is_hyphen_like};
use crate::validators::Verdict;

/// 検出器が返す生の候補。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawMatch {
    pub start: usize,
    pub end: usize,
    pub confidence: f32,
}

pub trait Detect: Send + Sync {
    fn detect(&self, text: &str, out: &mut Vec<RawMatch>);
}

pub(crate) const DIGIT_CLASS: &str = "0-9０-９";
pub(crate) const HYPHEN_CLASS: &str = r"\-‐‑–—―−ー－ｰ";

pub(crate) fn expand_pattern(p: &str) -> String {
    p.replace("{DC}", DIGIT_CLASS)
        .replace("{HC}", HYPHEN_CLASS)
        .replace("{PREF}", PREFECTURES)
}

type Compiled = Arc<Vec<(Regex, f32, bool)>>;

fn cache() -> &'static Vec<OnceLock<Compiled>> {
    static CACHE: OnceLock<Vec<OnceLock<Compiled>>> = OnceLock::new();
    CACHE.get_or_init(|| CATALOG.iter().map(|_| OnceLock::new()).collect())
}

fn compiled(index: usize, spec: &RegexSpec) -> Compiled {
    cache()[index]
        .get_or_init(|| {
            Arc::new(
                spec.patterns
                    .iter()
                    .map(|(p, conf)| {
                        let re = Regex::new(&expand_pattern(p)).unwrap_or_else(|e| {
                            panic!("catalog regex for {} is invalid: {e}", CATALOG[index].id)
                        });
                        let has_v = re.capture_names().any(|n| n == Some("v"));
                        (re, *conf, has_v)
                    })
                    .collect(),
            )
        })
        .clone()
}

/// カタログの全正規表現を事前コンパイルする (起動直後にバックグラウンドで呼ぶと初回が速くなる)。
pub fn warm_up() {
    use rayon::prelude::*;
    CATALOG.par_iter().enumerate().for_each(|(i, d)| {
        if let SpecKind::Regex(spec) = &d.kind {
            let _ = compiled(i, spec);
        }
    });
}

pub struct RegexDetector {
    regexes: Compiled,
    spec: RegexSpec,
}

impl RegexDetector {
    pub fn from_catalog(index: usize) -> Option<Self> {
        match &CATALOG[index].kind {
            SpecKind::Regex(spec) => Some(Self { regexes: compiled(index, spec), spec: *spec }),
            _ => None,
        }
    }
}

impl Detect for RegexDetector {
    fn detect(&self, text: &str, out: &mut Vec<RawMatch>) {
        for (re, base, has_v) in self.regexes.iter() {
            if *has_v {
                for caps in re.captures_iter(text) {
                    let m = caps.name("v").or_else(|| caps.get(0)).unwrap();
                    consider(&self.spec, text, m.start(), m.end(), *base, out);
                }
            } else {
                for m in re.find_iter(text) {
                    consider(&self.spec, text, m.start(), m.end(), *base, out);
                }
            }
        }
    }
}

fn consider(spec: &RegexSpec, text: &str, s: usize, mut e: usize, base: f32, out: &mut Vec<RawMatch>) {
    if spec.trim_punct {
        while e > s {
            let c = text[..e].chars().next_back().unwrap();
            if ".,;:!?)]}'\"".contains(c) {
                e -= c.len_utf8();
            } else {
                break;
            }
        }
    }
    if s >= e || !boundary_ok(text, s, e, spec.boundary) {
        return;
    }
    let value = &text[s..e];
    let verdict = spec.validator.map(|f| f(value)).unwrap_or(Verdict::Accept);
    if verdict == Verdict::Reject {
        return;
    }
    let mut conf = base;
    if let Some(ctx) = &spec.context {
        if context_found(text, s, e, ctx) {
            conf = (conf + 0.15).min(1.0);
        } else if ctx.required && verdict != Verdict::Strong {
            return;
        }
    }
    if verdict == Verdict::Strong {
        conf = conf.max(0.9);
    }
    out.push(RawMatch { start: s, end: e, confidence: conf });
}

pub(crate) fn context_found(text: &str, s: usize, e: usize, ctx: &ContextSpec) -> bool {
    let from = back_chars(text, s, ctx.before);
    let to = forward_chars(text, e, ctx.after);
    let window = text[from..to].to_lowercase();
    ctx.keywords.iter().any(|k| window.contains(k))
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

pub(crate) fn boundary_ok(text: &str, s: usize, e: usize, b: Boundary) -> bool {
    let before = char_before(text, s);
    let after = char_after(text, e);
    match b {
        Boundary::None => true,
        Boundary::Alnum => !before.is_some_and(is_word) && !after.is_some_and(is_word),
        Boundary::Digit => {
            let bad = |c: char| is_digit_like(c) || c.is_ascii_alphabetic() || is_hyphen_like(c) || c == '_';
            !before.is_some_and(bad) && !after.is_some_and(bad)
        }
        Boundary::Ip => {
            if before.is_some_and(|c| is_word(c) || c == '-') || after.is_some_and(|c| is_word(c) || c == '-') {
                return false;
            }
            if before == Some('.') {
                if char_before(text, s - 1).is_some_and(|c| c.is_ascii_alphanumeric()) {
                    return false;
                }
            }
            if after == Some('.') {
                if char_after(text, e + 1).is_some_and(|c| c.is_ascii_alphanumeric()) {
                    return false;
                }
            }
            true
        }
        Boundary::Hex => {
            let bad = |c: char| is_word(c) || c == ':';
            !before.is_some_and(bad) && !after.is_some_and(bad)
        }
        Boundary::Path => {
            !before.is_some_and(|c| is_word(c) || matches!(c, '\\' | '/' | ':' | '.'))
                && !after.is_some_and(is_word)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_catalog_regexes_compile() {
        warm_up();
    }

    #[test]
    fn catalog_examples_are_detected() {
        for (i, d) in CATALOG.iter().enumerate() {
            if let Some(det) = RegexDetector::from_catalog(i) {
                let mut out = vec![];
                det.detect(d.example, &mut out);
                assert!(!out.is_empty(), "example for {} not detected: {:?}", d.id, d.example);
            }
        }
    }

    fn run(id: &str, text: &str) -> Vec<String> {
        let i = CATALOG.iter().position(|d| d.id == id).unwrap();
        let det = RegexDetector::from_catalog(i).unwrap();
        let mut out = vec![];
        det.detect(text, &mut out);
        out.iter().map(|m| text[m.start..m.end].to_string()).collect()
    }

    #[test]
    fn phone_variants() {
        assert_eq!(run("phone_jp", "電話 03(1234)5678 まで"), vec!["03(1234)5678"]);
        assert_eq!(run("phone_jp", "携帯０９０－１２３４－５６７８です"), vec!["０９０－１２３４－５６７８"]);
        assert!(run("phone_jp", "注文番号 0901234567890123").is_empty());
        assert_eq!(run("phone_jp", "+81 90-1234-5678"), vec!["+81 90-1234-5678"]);
    }

    #[test]
    fn ip_boundaries() {
        assert_eq!(run("ipv4", "host 10.0.0.1."), vec!["10.0.0.1"]);
        assert!(run("ipv4", "ver 1.2.3.4.5").is_empty());
        assert_eq!(run("ipv4", "net 10.1.0.0/16 ok"), vec!["10.1.0.0/16"]);
    }

    #[test]
    fn password_kv_values() {
        assert_eq!(run("password_kv", "DB_PASSWORD=hunter2"), vec!["hunter2"]);
        assert_eq!(run("password_kv", "\"password\": \"s3cr3t\""), vec!["s3cr3t"]);
        assert!(run("password_kv", "password=${DB_PASS}").is_empty());
        assert!(run("password_kv", "bypass=1").is_empty());
    }

    #[test]
    fn context_required() {
        assert!(run("drivers_license_jp", "注文 301234567890").is_empty());
        assert_eq!(run("drivers_license_jp", "免許証番号: 301234567890"), vec!["301234567890"]);
    }

    #[test]
    fn hostname_not_filenames() {
        assert!(run("hostname", "open report.docx and main.rs").is_empty());
        assert!(run("hostname", "photo.jpg").is_empty());
        assert_eq!(run("hostname", "see api.corp.example.co.jp/x"), vec!["api.corp.example.co.jp"]);
    }

    #[test]
    fn windows_path_user() {
        assert_eq!(run("windows_user_path", r"C:\Users\taro\Desktop"), vec!["taro"]);
        assert!(run("windows_user_path", r"C:\Users\Public\x").is_empty());
    }
}
