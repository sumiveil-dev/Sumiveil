//! `search.rs` の単体テスト (本体から分離。`search.rs` の子モジュール `tests` として組み込まれる)

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
