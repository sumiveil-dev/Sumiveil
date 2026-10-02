//! `custom.rs` の単体テスト (本体から分離。`custom.rs` の子モジュール `tests` として組み込まれる)

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
