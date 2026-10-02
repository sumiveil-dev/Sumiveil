//! `detector.rs` の単体テスト (本体から分離。`detector.rs` の子モジュール `tests` として組み込まれる)

use super::*;

#[test]
fn all_catalog_regexes_compile() {
    warm_up();
}

#[test]
fn catalog_examples_are_detected() {
    for (i, d) in CATALOG.iter().enumerate() {
        if let Some(det) = RegexDetector::from_catalog(i) {
            let mut out = vec![];
            det.detect(d.example, &mut out);
            assert!(!out.is_empty(), "example for {} not detected: {:?}", d.id, d.example);
        }
    }
}

fn run(id: &str, text: &str) -> Vec<String> {
    let i = CATALOG.iter().position(|d| d.id == id).unwrap();
    let det = RegexDetector::from_catalog(i).unwrap();
    let mut out = vec![];
    det.detect(text, &mut out);
    out.iter().map(|m| text[m.start..m.end].to_string()).collect()
}

#[test]
fn phone_variants() {
    assert_eq!(run("phone_jp", "電話 03(1234)5678 まで"), vec!["03(1234)5678"]);
    assert_eq!(run("phone_jp", "携帯０９０－１２３４－５６７８です"), vec!["０９０－１２３４－５６７８"]);
    assert!(run("phone_jp", "注文番号 0901234567890123").is_empty());
    assert_eq!(run("phone_jp", "+81 90-1234-5678"), vec!["+81 90-1234-5678"]);
}

#[test]
fn ip_boundaries() {
    assert_eq!(run("ipv4", "host 10.0.0.1."), vec!["10.0.0.1"]);
    assert!(run("ipv4", "ver 1.2.3.4.5").is_empty());
    assert_eq!(run("ipv4", "net 10.1.0.0/16 ok"), vec!["10.1.0.0/16"]);
}

#[test]
fn password_kv_values() {
    assert_eq!(run("password_kv", "DB_PASSWORD=hunter2"), vec!["hunter2"]);
    assert_eq!(run("password_kv", "\"password\": \"s3cr3t\""), vec!["s3cr3t"]);
    assert!(run("password_kv", "password=${DB_PASS}").is_empty());
    assert!(run("password_kv", "bypass=1").is_empty());
}

#[test]
fn context_required() {
    assert!(run("drivers_license_jp", "注文 301234567890").is_empty());
    assert_eq!(run("drivers_license_jp", "免許証番号: 301234567890"), vec!["301234567890"]);
}

#[test]
fn hostname_not_filenames() {
    assert!(run("hostname", "open report.docx and main.rs").is_empty());
    assert!(run("hostname", "photo.jpg").is_empty());
    assert_eq!(run("hostname", "see api.corp.example.co.jp/x"), vec!["api.corp.example.co.jp"]);
}

#[test]
fn windows_path_user() {
    assert_eq!(run("windows_user_path", r"C:\Users\taro\Desktop"), vec!["taro"]);
    assert!(run("windows_user_path", r"C:\Users\Public\x").is_empty());
}
