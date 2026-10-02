//! `text.rs` の単体テスト (本体から分離。`text.rs` の子モジュール `tests` として組み込まれる)

use super::*;

#[test]
fn digits() {
    assert_eq!(digits_only("０９０-1234－５６７８"), "09012345678");
}

#[test]
fn line_col() {
    let t = "abc\nあいう\nx";
    let idx = LineIndex::new(t);
    assert_eq!(idx.line_col(t, 0), (1, 1));
    let p = t.find('い').unwrap();
    assert_eq!(idx.line_col(t, p), (2, 2));
    assert_eq!(idx.line_col(t, t.len() - 1), (3, 1));
}

#[test]
fn back_forward() {
    let t = "あいうえお";
    assert_eq!(back_chars(t, t.len(), 2), "あいう".len());
    assert_eq!(forward_chars(t, 0, 2), "あい".len());
    assert_eq!(forward_chars(t, 0, 99), t.len());
}
