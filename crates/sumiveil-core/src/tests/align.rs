//! `align.rs` の単体テスト (本体から分離。`align.rs` の子モジュール `tests` として組み込まれる)

use super::*;
use crate::config::Config;
use crate::engine::mask_text;

#[test]
fn same_line_count() {
    let t = "a@example1.co.jp\nhello\n10.1.2.3";
    let r = mask_text(&Config::default(), t);
    let rows = align(t, &r);
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().enumerate().all(|(i, row)| row.left == Some(i) && row.right == Some(i)));
}

#[test]
fn multiline_collapse() {
    let t = "before\n-----BEGIN PRIVATE KEY-----\nAAAA\nBBBB\n-----END PRIVATE KEY-----\nafter a@example1.co.jp";
    let r = mask_text(&Config::default(), t);
    assert_eq!(r.output, "before\n<PRIVATE_KEY_1>\nafter <EMAIL_1>");
    let rows = align(t, &r);
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0], Row { left: Some(0), right: Some(0) });
    assert_eq!(rows[1], Row { left: Some(1), right: Some(1) });
    assert_eq!(rows[2], Row { left: Some(2), right: None });
    assert_eq!(rows[4], Row { left: Some(4), right: None });
    assert_eq!(rows[5], Row { left: Some(5), right: Some(2) });
}
