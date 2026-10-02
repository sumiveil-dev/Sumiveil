//! `encoding.rs` の単体テスト (本体から分離。`encoding.rs` の子モジュール `tests` として組み込まれる)

use super::*;

#[test]
fn roundtrip_sjis() {
    let s = "山田太郎 090-1234-5678 ｱｲｳ";
    let b = encode(s, encoding_rs::SHIFT_JIS, false);
    let d = decode(&b, None);
    assert_eq!(d.text, s);
    assert_eq!(d.encoding, encoding_rs::SHIFT_JIS);
}

#[test]
fn utf8_bom_and_utf16() {
    let s = "テスト test";
    let d = decode(&encode(s, UTF_8, true), None);
    assert_eq!(d.text, s);
    assert!(d.bom);
    assert_eq!(d.display_name(), "UTF-8 (BOM)");
    let d = decode(&encode(s, UTF_16LE, true), None);
    assert_eq!((d.text.as_str(), d.encoding), (s, UTF_16LE));
    let d = decode(&encode("hello world log", UTF_16LE, false), None);
    assert_eq!((d.text.as_str(), d.encoding), ("hello world log", UTF_16LE));
}

#[test]
fn eucjp() {
    let s = "これはEUC-JPの日本語テキストです。住所は東京都です。";
    let b = encode(s, encoding_rs::EUC_JP, false);
    assert_eq!(decode(&b, None).text, s);
    assert_eq!(decode(&b, Some("euc-jp")).text, s);
}

#[test]
fn labels() {
    assert_eq!(encoding_for_label("sjis").unwrap().0, encoding_rs::SHIFT_JIS);
    assert_eq!(encoding_for_label("UTF-8-BOM").unwrap(), (UTF_8, true));
    assert!(encoding_for_label("nope").is_none());
}
