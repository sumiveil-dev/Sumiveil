//! 文字列検索 (GUI の検索バーとフォルダの横断検索で共有)。

use regex::{Regex, RegexBuilder};

use crate::config::{Config, CustomRule, KeywordGroup};

/// 検索条件。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    pub text: String,
    /// 大文字小文字を区別する
    pub case_sensitive: bool,
    /// 正規表現として解釈する
    pub regex: bool,
}

impl Query {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// 検索用の正規表現を作る。空なら None、不正な正規表現なら Err(理由)。
    pub fn compile(&self) -> Result<Option<Regex>, String> {
        if self.text.is_empty() {
            return Ok(None);
        }
        let pat = if self.regex { self.text.clone() } else { regex::escape(&self.text) };
        RegexBuilder::new(&pat)
            .case_insensitive(!self.case_sensitive)
            .multi_line(true)
            .size_limit(8 * 1024 * 1024)
            .build()
            .map(Some)
            .map_err(|e| match e {
                regex::Error::Syntax(s) => s.lines().last().unwrap_or("").trim().to_string(),
                other => other.to_string(),
            })
    }

    /// 検出一覧の絞り込み用: `s` が条件に一致するか (空なら常に一致)。
    pub fn matches(&self, re: Option<&Regex>, s: &str) -> bool {
        re.is_none_or(|r| r.is_match(s))
    }
}

/// 手動でマスク対象にした語のラベル。
pub const MANUAL_LABEL: &str = "MANUAL";

/// 「今だけマスク」用: 検索語を一時的に加えた設定 (元の設定は変えない)。
/// 文字列はキーワードグループ、正規表現はカスタムルールとして加える。
pub fn config_with_terms(cfg: &Config, terms: &[Query]) -> Config {
    let mut c = cfg.clone();
    if terms.is_empty() {
        return c;
    }
    for cs in [false, true] {
        let words: Vec<String> = terms.iter().filter(|t| !t.regex && t.case_sensitive == cs && !t.text.is_empty()).map(|t| t.text.clone()).collect();
        if !words.is_empty() {
            c.keywords.push(KeywordGroup { label: MANUAL_LABEL.into(), name: "手動で指定".into(), words, case_sensitive: cs, ..Default::default() });
        }
    }
    for (i, t) in terms.iter().filter(|t| t.regex && !t.text.is_empty()).enumerate() {
        c.custom_rules.push(CustomRule {
            id: format!("manual_tmp_{i}"),
            name: "手動で指定".into(),
            label: MANUAL_LABEL.into(),
            pattern: t.text.clone(),
            case_insensitive: !t.case_sensitive,
            ..Default::default()
        });
    }
    // 利用者が「カスタム」カテゴリを無効にしていても、手動の指定は効かせる
    c.categories.entry("custom".into()).or_default().enabled = Some(true);
    c
}

/// 「キーワード辞書に登録」用: 文字列はグループ「手動で追加」に語を足し、正規表現はカスタムルールとして足す。
/// 変更後の (キーワード一覧, カスタムルール一覧)。すでにあれば変更なし。
pub fn register_term(cfg: &Config, term: &Query) -> (Vec<KeywordGroup>, Vec<CustomRule>) {
    let mut groups = cfg.keywords.clone();
    let mut rules = cfg.custom_rules.clone();
    if term.regex {
        if !rules.iter().any(|r| r.pattern == term.text) {
            let n = (1..).find(|n| !rules.iter().any(|r| r.id == format!("manual_{n}"))).unwrap_or(1);
            rules.push(CustomRule {
                id: format!("manual_{n}"),
                name: "手動で追加".into(),
                label: MANUAL_LABEL.into(),
                pattern: term.text.clone(),
                case_insensitive: !term.case_sensitive,
                ..Default::default()
            });
        }
    } else {
        let pos = groups.iter().position(|g| g.label == MANUAL_LABEL && g.case_sensitive == term.case_sensitive);
        let g = match pos {
            Some(i) => &mut groups[i],
            None => {
                groups.push(KeywordGroup { label: MANUAL_LABEL.into(), name: "手動で追加".into(), case_sensitive: term.case_sensitive, ..Default::default() });
                groups.last_mut().unwrap()
            }
        };
        if !g.words.contains(&term.text) {
            g.words.push(term.text.clone());
        }
    }
    (groups, rules)
}

/// 一致箇所 (バイト位置) を最大 `limit` 件まで返す。空の一致は除く。2 つ目は上限で打ち切ったか。
pub fn find_all(re: &Regex, text: &str, limit: usize) -> (Vec<(usize, usize)>, bool) {
    let mut out = vec![];
    for m in re.find_iter(text) {
        if m.start() == m.end() {
            continue;
        }
        if out.len() >= limit {
            return (out, true);
        }
        out.push((m.start(), m.end()));
    }
    (out, false)
}

/// 一致箇所の前後を含む 1 行の抜粋。(抜粋, 抜粋の中の一致範囲)
pub fn snippet(text: &str, start: usize, end: usize, context_chars: usize) -> (String, (usize, usize)) {
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i);
    let before: String = {
        let b: Vec<char> = text[line_start..start].chars().collect();
        let skip = b.len().saturating_sub(context_chars);
        let s: String = b[skip..].iter().collect();
        if skip > 0 { format!("…{s}") } else { s }
    };
    let hit = text[start..end].replace(['\r', '\n'], " ");
    let after: String = {
        let a: Vec<char> = text[end..line_end].chars().collect();
        let s: String = a.iter().take(context_chars).collect();
        if a.len() > context_chars { format!("{s}…") } else { s }
    };
    let before = before.replace('\t', " ").trim_start().to_string();
    let after = after.replace(['\t', '\r'], " ");
    let a = before.len();
    (format!("{before}{hit}{after}"), (a, a + hit.len()))
}

#[cfg(test)]
#[path = "tests/search.rs"]
mod tests;
