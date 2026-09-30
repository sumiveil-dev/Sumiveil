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
mod tests {
    use super::*;

    fn q(text: &str, case: bool, regex: bool) -> Option<Regex> {
        Query { text: text.into(), case_sensitive: case, regex }.compile().unwrap()
    }

    #[test]
    fn literal_and_case() {
        let t = "Taro taro TARO 山田";
        assert_eq!(find_all(&q("taro", false, false).unwrap(), t, 100).0.len(), 3);
        assert_eq!(find_all(&q("taro", true, false).unwrap(), t, 100).0, vec![(5, 9)]);
        // 記号は文字どおり (正規表現として解釈しない)
        assert_eq!(find_all(&q("a.b", false, false).unwrap(), "a.b axb", 10).0, vec![(0, 3)]);
        assert_eq!(find_all(&q("山田", false, false).unwrap(), t, 10).0, vec![(15, 21)]);
    }

    #[test]
    fn regex_limit_and_errors() {
        let re = q(r"\d{3}-\d{4}", false, true).unwrap();
        assert_eq!(find_all(&re, "100-0001 / 530-0001", 10).0.len(), 2);
        let (v, cut) = find_all(&re, "100-0001 / 530-0001", 1);
        assert!(cut && v.len() == 1);
        assert!(Query { text: "(".into(), case_sensitive: false, regex: true }.compile().is_err());
        assert!(Query::default().compile().unwrap().is_none());
        // 空の一致 (^ など) は数えない
        assert!(find_all(&q("^", false, true).unwrap(), "a\nb", 10).0.is_empty());
    }

    #[test]
    fn manual_terms_are_masked() {
        let text = "プロジェクト黒猫の件、PRJ-0042 も確認";
        let base = Config::default();
        let before = crate::Engine::new(&base).mask(text, &mut crate::MaskSession::default());
        assert!(before.output.contains("黒猫") && before.output.contains("PRJ-0042"));
        let terms = [
            Query { text: "プロジェクト黒猫".into(), ..Default::default() },
            Query { text: r"PRJ-\d{4}".into(), regex: true, ..Default::default() },
        ];
        let c = config_with_terms(&base, &terms);
        let after = crate::Engine::new(&c).mask(text, &mut crate::MaskSession::default());
        assert!(!after.output.contains("黒猫") && !after.output.contains("PRJ-0042"), "{}", after.output);
        assert!(after.output.contains("<MANUAL_1>"));
        // 元の設定は変えない
        assert!(base.keywords.is_empty() && base.custom_rules.is_empty());
    }

    #[test]
    fn register_is_idempotent() {
        let mut c = Config::default();
        let t = Query { text: "アクメ商事".into(), ..Default::default() };
        (c.keywords, c.custom_rules) = register_term(&c, &t);
        (c.keywords, c.custom_rules) = register_term(&c, &t);
        assert_eq!(c.keywords.len(), 1);
        assert_eq!(c.keywords[0].words, vec!["アクメ商事"]);
        let r = Query { text: r"TCK-\d+".into(), regex: true, ..Default::default() };
        (c.keywords, c.custom_rules) = register_term(&c, &r);
        (c.keywords, c.custom_rules) = register_term(&c, &r);
        assert_eq!(c.custom_rules.len(), 1);
        assert_eq!(c.custom_rules[0].id, "manual_1");
    }

    #[test]
    fn snippets() {
        let t = "1行目\n担当: 山田太郎様 のご確認をお願いします\n3行目";
        let s = t.find("山田").unwrap();
        let (text, (a, b)) = snippet(t, s, s + "山田".len(), 4);
        assert_eq!(&text[a..b], "山田");
        // 前は 4 文字 (ちょうど行頭まで)、後ろは 4 文字で打ち切り
        assert_eq!(text, "担当: 山田太郎様 …");
        let (text, _) = snippet(t, s, s + "山田".len(), 2);
        assert_eq!(text, "…: 山田太郎…");
    }
}
