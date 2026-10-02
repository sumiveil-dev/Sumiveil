//! `formats/eml.rs` の単体テスト (本体から分離。`formats/eml.rs` の子モジュール `tests` として組み込まれる)

use super::*;

#[test]
fn base64_encoding() {
    assert_eq!(base64(b"Man"), "TWFu");
    assert_eq!(base64(b"Ma"), "TWE=");
    assert_eq!(base64(b"M"), "TQ==");
    assert_eq!(base64("あ".as_bytes()), "44GC");
}

#[test]
fn header_words() {
    assert_eq!(encode_word("Hello", 9), "Hello");
    assert!(encode_word("<NAME_1>", 6).starts_with("=?UTF-8?B?"));
    let long = "件名".repeat(30);
    assert!(encode_word(&long, 9).lines().all(|l| l.trim_start().len() <= 76));
}
