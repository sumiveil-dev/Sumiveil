//! ユーザー定義の正規表現ルールとキーワード辞書。

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};

use crate::catalog::Boundary;
use crate::config::{CustomRule, KeywordGroup};
use crate::detector::{boundary_ok, Detect, RawMatch};

/// 通常は高速な `regex` を使い、先読み・後読み等が必要なときだけ `fancy-regex` を使う。
pub enum CompiledRe {
    Fast(regex::Regex),
    Fancy(fancy_regex::Regex),
}

pub fn compile_rule(r: &CustomRule) -> Result<CompiledRe, String> {
    compile_pattern(&r.pattern, r.case_insensitive)
}

pub fn compile_pattern(pattern: &str, case_insensitive: bool) -> Result<CompiledRe, String> {
    match regex::RegexBuilder::new(pattern).case_insensitive(case_insensitive).size_limit(50 << 20).build() {
        Ok(re) => Ok(CompiledRe::Fast(re)),
        Err(fast_err) => {
            let p = if case_insensitive { format!("(?i){pattern}") } else { pattern.to_string() };
            fancy_regex::RegexBuilder::new(&p)
                .backtrack_limit(200_000)
                .build()
                .map(CompiledRe::Fancy)
                .map_err(|e| format!("{e} ({fast_err})").lines().next().unwrap_or("").to_string())
        }
    }
}

impl CompiledRe {
    /// (開始, 終了) の一覧。group が指定されていればそのグループ、なければ `v`、なければ全体。
    pub fn spans(&self, text: &str, group: Option<&str>) -> Vec<(usize, usize)> {
        let mut out = vec![];
        match self {
            CompiledRe::Fast(re) => {
                let g = group.map(String::from).or_else(|| re.capture_names().flatten().find(|n| *n == "v").map(String::from));
                for caps in re.captures_iter(text) {
                    let m = g.as_deref().and_then(|g| name_or_index(&caps, g)).or_else(|| caps.get(0));
                    if let Some(m) = m {
                        if m.start() < m.end() {
                            out.push((m.start(), m.end()));
                        }
                    }
                }
            }
            CompiledRe::Fancy(re) => {
                let g = group.map(String::from).or_else(|| re.capture_names().flatten().find(|n| *n == "v").map(String::from));
                for caps in re.captures_iter(text) {
                    let Ok(caps) = caps else { break };
                    let m = match g.as_deref() {
                        Some(g) => match g.parse::<usize>() {
                            Ok(i) => caps.get(i),
                            Err(_) => caps.name(g),
                        },
                        None => None,
                    }
                    .or_else(|| caps.get(0));
                    if let Some(m) = m {
                        if m.start() < m.end() {
                            out.push((m.start(), m.end()));
                        }
                    }
                }
            }
        }
        out
    }
}

fn name_or_index<'t>(caps: &regex::Captures<'t>, g: &str) -> Option<regex::Match<'t>> {
    match g.parse::<usize>() {
        Ok(i) => caps.get(i),
        Err(_) => caps.name(g),
    }
}

pub struct CustomDetector {
    re: CompiledRe,
    group: Option<String>,
    confidence: f32,
}

impl CustomDetector {
    pub fn new(rule: &CustomRule) -> Result<Self, String> {
        Ok(Self { re: compile_rule(rule)?, group: rule.group.clone(), confidence: rule.confidence.clamp(0.0, 1.0) })
    }
}

impl Detect for CustomDetector {
    fn detect(&self, text: &str, out: &mut Vec<RawMatch>) {
        for (s, e) in self.re.spans(text, self.group.as_deref()) {
            out.push(RawMatch { start: s, end: e, confidence: self.confidence });
        }
    }
}

pub struct KeywordDetector {
    ac: AhoCorasick,
    whole_word: bool,
}

impl KeywordDetector {
    pub fn new(group: &KeywordGroup) -> Option<Self> {
        let words: Vec<&str> = group.words.iter().map(|w| w.trim()).filter(|w| !w.is_empty()).collect();
        if words.is_empty() {
            return None;
        }
        let ac = AhoCorasickBuilder::new()
            .ascii_case_insensitive(!group.case_sensitive)
            .match_kind(MatchKind::LeftmostLongest)
            .build(&words)
            .ok()?;
        Some(Self { ac, whole_word: group.whole_word })
    }
}

impl Detect for KeywordDetector {
    fn detect(&self, text: &str, out: &mut Vec<RawMatch>) {
        for m in self.ac.find_iter(text) {
            if self.whole_word && !boundary_ok(text, m.start(), m.end(), Boundary::Alnum) {
                continue;
            }
            out.push(RawMatch { start: m.start(), end: m.end(), confidence: 1.0 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_and_fancy() {
        let r = CustomRule { pattern: r"PRJ-\d{4}".into(), ..Default::default() };
        assert!(matches!(compile_rule(&r).unwrap(), CompiledRe::Fast(_)));
        let r = CustomRule { pattern: r"(?<=ID:)\d+".into(), ..Default::default() };
        let c = compile_rule(&r).unwrap();
        assert!(matches!(c, CompiledRe::Fancy(_)));
        assert_eq!(c.spans("ID:123 x", None), vec![(3, 6)]);
    }

    #[test]
    fn group_selection() {
        let r = CustomRule { pattern: r"code=(?P<v>\w+)".into(), ..Default::default() };
        let d = CustomDetector::new(&r).unwrap();
        let mut out = vec![];
        d.detect("a code=XYZ b", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(&"a code=XYZ b"[out[0].start..out[0].end], "XYZ");
    }

    #[test]
    fn keywords() {
        let g = KeywordGroup { words: vec!["Acme".into(), "プロジェクト黒猫".into()], whole_word: true, ..Default::default() };
        let d = KeywordDetector::new(&g).unwrap();
        let t = "ACME社とプロジェクト黒猫、acmeX";
        let mut out = vec![];
        d.detect(t, &mut out);
        let got: Vec<&str> = out.iter().map(|m| &t[m.start..m.end]).collect();
        assert_eq!(got, vec!["ACME", "プロジェクト黒猫"]);
    }
}
