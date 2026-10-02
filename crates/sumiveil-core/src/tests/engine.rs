//! `engine.rs` の単体テスト (本体から分離。`engine.rs` の子モジュール `tests` として組み込まれる)

use super::*;
use crate::config::{load_str, AllowlistConfig, CustomRule, KeywordGroup};

fn cfg(s: &str) -> Config {
    load_str(s, None).unwrap().config
}

#[test]
fn basic_masking() {
    let r = mask_text(&Config::default(), "連絡先: taro@example1.co.jp / 090-1234-5678");
    assert_eq!(r.output, "連絡先: <EMAIL_1> / <PHONE_1>");
    assert_eq!(r.replacements.len(), 2);
    assert_eq!(&r.output[r.replacements[0].out_start..r.replacements[0].out_end], "<EMAIL_1>");
}

#[test]
fn consistent_numbering() {
    let r = mask_text(&Config::default(), "a@example1.co.jp b@example1.co.jp A@EXAMPLE1.CO.JP 090-1111-2222 09011112222");
    assert_eq!(r.output, "<EMAIL_1> <EMAIL_2> <EMAIL_1> <PHONE_1> <PHONE_1>");
}

#[test]
fn templates_per_detector_and_category() {
    let c = cfg("[detectors.email]\ntemplate = \"[{label_ja}]\"\n[categories.contact]\ntemplate = \"{shape:*}\"");
    let r = mask_text(&c, "a@example1.co.jp 090-1234-5678");
    assert_eq!(r.output, "[メールアドレス] ***-****-****");
}

#[test]
fn allowlist() {
    let mut c = Config::default();
    c.allowlist = AllowlistConfig { values: vec!["10.0.0.1".into()], patterns: vec![r"192\.168\..*".into()], ..Default::default() };
    let r = mask_text(&c, "info@example.com admin@sub.example.com x@example1.co.jp 10.0.0.1 192.168.0.5 172.16.0.1");
    assert_eq!(r.output, "info@example.com admin@sub.example.com <EMAIL_1> 10.0.0.1 192.168.0.5 <IPV4_1>");
}

#[test]
fn overlap_priority() {
    // URL 認証情報 (優先度 88) がメール (60) より優先される
    let r = mask_text(&Config::default(), "postgres://admin:pa55@db.corp.local/app");
    assert!(r.output.contains("<URL_CREDENTIALS_1>"), "{}", r.output);
}

#[test]
fn custom_and_keywords() {
    let mut c = Config::default();
    c.custom_rules.push(CustomRule { id: "prj".into(), label: "PROJECT".into(), pattern: r"PRJ-\d{4}".into(), ..Default::default() });
    c.keywords.push(KeywordGroup { label: "CLIENT".into(), words: vec!["アクメ商事".into()], ..Default::default() });
    let r = mask_text(&c, "PRJ-1234 はアクメ商事向け");
    assert_eq!(r.output, "<PROJECT_1> は<CLIENT_1>向け");
}

#[test]
fn name_propagation() {
    let r = mask_text(&Config::default(), "山田様、お世話になっております。山田が伺います。");
    assert_eq!(r.output, "<NAME_1>様、お世話になっております。<NAME_1>が伺います。");
}

#[test]
fn disabled_category() {
    let c = cfg("[categories.contact]\nenabled = false");
    let r = mask_text(&c, "TEL 090-1234-5678 / 10.1.2.3");
    assert_eq!(r.output, "TEL 090-1234-5678 / <IPV4_1>");
}

#[test]
fn invalid_template_warns_but_works() {
    let c = cfg("[detectors.email]\ntemplate = \"{bogus}\"");
    let e = Engine::new(&c);
    assert!(!e.warnings.is_empty());
    let r = e.mask("a@example1.co.jp", &mut MaskSession::new());
    assert_eq!(r.output, "<EMAIL_1>");
}

#[test]
fn realistic_mixed_text() {
    let text = "\
From: 鈴木 一郎 <ichiro.suzuki@corp.example.co.jp>
To: 株式会社サンプル商事 田中様

お世話になっております。
サーバー db01.prod.internal (10.20.30.40) のパスワードは password=Hunter2! です。
カード番号 4111 1111 1111 1111、マイナンバー 1234 5678 9018。
住所: 東京都千代田区架空町1丁目2-3
C:\\Users\\ichiro\\Desktop\\memo.txt
AKIAIOSFODNN7EXAMPLE
";
    let r = mask_text(&Config::default(), text);
    for secret in ["ichiro.suzuki", "田中", "10.20.30.40", "db01.prod.internal", "Hunter2!", "4111", "1234 5678 9018", "架空町", "AKIAIOSFODNN7EXAMPLE", "サンプル商事"] {
        assert!(!r.output.contains(secret), "{secret} leaked:\n{}", r.output);
    }
    assert!(!r.output.contains("\\ichiro\\"), "{}", r.output);
}
