//! `template.rs` の単体テスト (本体から分離。`template.rs` の子モジュール `tests` として組み込まれる)

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
