//! 検出器の実行部。カタログの正規表現定義をコンパイル・キャッシュし、境界・文脈・検証を適用する。

use std::sync::{Arc, OnceLock};

use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
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
    /// 文脈語が必須の検出器で使う、文脈語の検索 (本文全体ではなく文脈語の近くだけを調べるため)
    keywords: Option<Arc<AhoCorasick>>,
}

impl RegexDetector {
    pub fn from_catalog(index: usize) -> Option<Self> {
        match &CATALOG[index].kind {
            SpecKind::Regex(spec) => Some(Self { regexes: compiled(index, spec), spec: *spec, keywords: keyword_matcher(index, spec) }),
            _ => None,
        }
    }

    fn scan(&self, text: &str, from: usize, to: usize, out: &mut Vec<RawMatch>) {
        let part = &text[from..to];
        for (re, base, has_v) in self.regexes.iter() {
            if *has_v {
                for caps in re.captures_iter(part) {
                    let m = caps.name("v").or_else(|| caps.get(0)).unwrap();
                    consider(&self.spec, text, from + m.start(), from + m.end(), *base, out);
                }
            } else {
                for m in re.find_iter(part) {
                    consider(&self.spec, text, from + m.start(), from + m.end(), *base, out);
                }
            }
        }
    }
}

/// 文脈語が必須で、検証で文脈を省けない (validator が無い) 検出器だけ、文脈語の検索を用意する。
fn keyword_matcher(index: usize, spec: &RegexSpec) -> Option<Arc<AhoCorasick>> {
    static CACHE: OnceLock<Vec<OnceLock<Option<Arc<AhoCorasick>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| CATALOG.iter().map(|_| OnceLock::new()).collect());
    cache[index]
        .get_or_init(|| {
            let ctx = spec.context.filter(|c| c.required && spec.validator.is_none())?;
            AhoCorasickBuilder::new().ascii_case_insensitive(true).build(ctx.keywords).ok().map(Arc::new)
        })
        .clone()
}

/// 一致 1 件の長さの上限の目安 (文字数)。文脈語の近くを調べる範囲に足す。
const MAX_MATCH_CHARS: usize = 80;

/// 文脈語の前後で、一致が入りうる範囲 (重なりはまとめる)。範囲の端は空白・改行まで広げ、一致の途中で切らないようにする。
fn context_regions(text: &str, ac: &AhoCorasick, ctx: &ContextSpec) -> Vec<(usize, usize)> {
    let widen_back = |i: usize| {
        let p = back_chars(text, i, ctx.after + MAX_MATCH_CHARS);
        text[..p].rfind(char::is_whitespace).map_or(0, |w| w)
    };
    let widen_fwd = |i: usize| {
        let p = forward_chars(text, i, ctx.before + MAX_MATCH_CHARS);
        text[p..].find(char::is_whitespace).map_or(text.len(), |w| p + w)
    };
    let mut regions: Vec<(usize, usize)> = vec![];
    for m in ac.find_iter(text) {
        let (from, to) = (widen_back(m.start()), widen_fwd(m.end()));
        match regions.last_mut() {
            Some(last) if from <= last.1 => last.1 = last.1.max(to),
            _ => regions.push((from, to)),
        }
    }
    regions
}

impl Detect for RegexDetector {
    fn detect(&self, text: &str, out: &mut Vec<RawMatch>) {
        match (&self.keywords, &self.spec.context) {
            (Some(ac), Some(ctx)) => {
                for (from, to) in context_regions(text, ac, ctx) {
                    self.scan(text, from, to, out);
                }
            }
            _ => self.scan(text, 0, text.len(), out),
        }
    }
}

fn consider(spec: &RegexSpec, text: &str, s: usize, e: usize, base: f32, out: &mut Vec<RawMatch>) {
    let (s, mut e) = match spec.refine {
        Some(f) => match f(text, s, e) {
            Some(r) => r,
            None => return,
        },
        None => (s, e),
    };
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
    let window = text[from..to].as_bytes();
    // 文脈語は小文字で書く決まり。英字だけ大文字・小文字を区別せずに比べる (範囲を小文字にした文字列は作らない)
    ctx.keywords.iter().any(|k| {
        let k = k.as_bytes();
        k.len() <= window.len() && window.windows(k.len()).any(|w| w.eq_ignore_ascii_case(k))
    })
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_katakana(c: char) -> bool {
    ('\u{30A1}'..='\u{30FA}').contains(&c) || ('\u{FF66}'..='\u{FF9D}').contains(&c)
}

/// 英字の直後でも区切りとみなす項目名 (小文字)。「TEL03-1234-5678」「IP192.168.0.1」
const DIGIT_LABELS: &[&str] = &["tel", "fax", "phone", "mobile", "mob", "cell"];
const IP_LABELS: &[&str] = &["ip", "ipv4", "ipaddr", "addr"];

/// 位置 `s` の直前が「項目名の英字だけの語」か (「TEL」の後ろの数字など)。
fn after_label(text: &str, s: usize, labels: &[&str]) -> bool {
    let head = &text[..s];
    let word_start = head.rfind(|c: char| !c.is_ascii_alphabetic()).map_or(0, |i| i + head[i..].chars().next().map_or(1, char::len_utf8));
    let word = &head[word_start..];
    !word.is_empty() && labels.iter().any(|l| word.eq_ignore_ascii_case(l))
}

/// 位置 `e` の直後が「項目名の英字だけの語」か (「…5678FAX03-…」の「FAX」)。
fn before_label(text: &str, e: usize, labels: &[&str]) -> bool {
    let tail = &text[e..];
    let end = tail.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(tail.len());
    end > 0 && labels.iter().any(|l| tail[..end].eq_ignore_ascii_case(l))
}

/// 長音「ー」がカタカナの語の一部か (「センター」「ナンバー」の後ろの数字は区切られているとみなす)。
fn long_vowel_of_word(text: &str, at: usize, before: bool) -> bool {
    let neighbor = if before { char_before(text, at) } else { char_after(text, at) };
    let Some(c) = neighbor.filter(|c| matches!(c, 'ー' | 'ｰ')) else { return false };
    let beyond = if before { char_before(text, at - c.len_utf8()) } else { char_after(text, at + c.len_utf8()) };
    beyond.is_some_and(is_katakana)
}

/// 前後の境界の判定。前・後の文字がそれぞれ「隣に来てはいけない文字」でないこと。
/// 例外 (カタカナ語の長音、英字の項目名) は表のように並べて足す。
pub(crate) fn boundary_ok(text: &str, s: usize, e: usize, b: Boundary) -> bool {
    let before = char_before(text, s);
    let after = char_after(text, e);
    match b {
        Boundary::None => true,
        Boundary::Alnum => !before.is_some_and(is_word) && !after.is_some_and(is_word),
        Boundary::Digit => {
            let bad = |c: char| is_digit_like(c) || c.is_ascii_alphabetic() || is_hyphen_like(c) || c == '_';
            let before_ok = !before.is_some_and(bad) || long_vowel_of_word(text, s, true) || after_label(text, s, DIGIT_LABELS);
            let after_ok = !after.is_some_and(bad) || long_vowel_of_word(text, e, false) || before_label(text, e, DIGIT_LABELS);
            before_ok && after_ok
        }
        Boundary::Ip => {
            let bad = |c: char| is_word(c) || c == '-';
            if before.is_some_and(bad) && !after_label(text, s, IP_LABELS) || after.is_some_and(bad) {
                return false;
            }
            // 「1.2.3.4.5」のように数字の並びの途中なら不採用 (「No.192.168.0.1」の「No.」は区切り)
            let dotted_before = char_before(text, s - before.map_or(0, char::len_utf8)).is_some_and(|c| c.is_ascii_digit());
            let dotted_after = char_after(text, e + after.map_or(0, char::len_utf8)).is_some_and(|c| c.is_ascii_alphanumeric());
            !(before == Some('.') && dotted_before || after == Some('.') && dotted_after)
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
#[path = "tests/detector.rs"]
mod tests;
