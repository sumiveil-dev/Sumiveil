//! `fake.rs` の単体テスト (本体から分離。`fake.rs` の子モジュール `tests` として組み込まれる)

use super::*;

fn ctx<'a>(id: &'a str, orig: &'a str, n: usize) -> RenderCtx<'a> {
    RenderCtx { original: orig, id, label: "X", label_ja: "X", category: "c", category_ja: "c", n, hash_key: b"k" }
}

#[test]
fn fakes() {
    assert_eq!(fake(&ctx("email", "a@example1.co.jp", 2)), "user2@example.com");
    assert_eq!(fake(&ctx("ipv4", "10.0.0.1", 1)), "192.0.2.1");
    assert_eq!(fake(&ctx("person_name", "山田", 1)), "甲野一郎");
    assert_eq!(fake(&ctx("person_name", "Bob", 7)), "John Doe2");
    assert_eq!(fake(&ctx("credit_card", "5555-5555-5555-4444", 1)), "4111-1111-1111-1111");
    let f = fake(&ctx("aws_access_key", "AKIAIOSFODNN7EXAMPLE", 1));
    assert_eq!(f.len(), 20);
    assert!(f.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    assert_eq!(f, fake(&ctx("aws_access_key", "AKIAIOSFODNN7EXAMPLE", 1)));
}
