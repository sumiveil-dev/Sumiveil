//! `validators.rs` の単体テスト (本体から分離。`validators.rs` の子モジュール `tests` として組み込まれる)

use super::*;

#[test]
fn luhn_cards() {
    assert_eq!(credit_card("4111 1111 1111 1111"), Verdict::Accept);
    assert_eq!(credit_card("5555-5555-5555-4444"), Verdict::Accept);
    assert_eq!(credit_card("3530111333300000"), Verdict::Accept); // JCB テスト番号
    assert_eq!(credit_card("378282246310005"), Verdict::Accept); // Amex テスト番号
    assert_eq!(credit_card("4111 1111 1111 1112"), Verdict::Reject);
    assert_eq!(credit_card("1234567812345670"), Verdict::Reject); // IIN 不明
}

#[test]
fn my_number_cd() {
    // チェックディジット計算で作成したダミー番号
    assert_eq!(my_number("1234 5678 9018"), Verdict::Accept);
    assert_eq!(my_number("123456789012"), Verdict::Reject);
    assert_eq!(my_number("111111111111"), Verdict::Reject);
}

#[test]
fn corporate_cd() {
    // 架空の法人番号 (チェックディジットは正しい)
    assert_eq!(corporate_number("7123456789012"), Verdict::Accept);
    assert_eq!(corporate_number("T7123456789012"), Verdict::Strong);
    assert_eq!(corporate_number("1000012050002"), Verdict::Reject);
}

#[test]
fn iban_mod97() {
    assert_eq!(iban("GB82 WEST 1234 5698 7654 32"), Verdict::Strong);
    assert_eq!(iban("GB82 WEST 1234 5698 7654 33"), Verdict::Reject);
}

#[test]
fn phones() {
    assert_eq!(phone_jp("090-1234-5678"), Verdict::Strong);
    assert_eq!(phone_jp("03-1234-5678"), Verdict::Strong);
    assert_eq!(phone_jp("+81 90 1234 5678"), Verdict::Strong);
    assert_eq!(phone_jp("0312345678"), Verdict::Accept);
    assert_eq!(phone_jp("031234567"), Verdict::Reject);
    assert_eq!(phone_jp("03-123-45678-9"), Verdict::Reject);
}

#[test]
fn ips() {
    assert_eq!(ipv4("192.168.1.10"), Verdict::Accept);
    assert_eq!(ipv4("127.0.0.1"), Verdict::Reject);
    assert_eq!(ipv4("1.02.3.4"), Verdict::Reject);
    assert_eq!(ipv6("2001:db8::1"), Verdict::Accept);
    assert_eq!(ipv6("::1"), Verdict::Reject);
    assert_eq!(ipv6("std::io"), Verdict::Reject);
}

#[test]
fn ssn() {
    assert_eq!(us_ssn("123-45-6789"), Verdict::Accept);
    assert_eq!(us_ssn("666-45-6789"), Verdict::Reject);
}
