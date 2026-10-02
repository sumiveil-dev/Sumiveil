//! `diag.rs` の単体テスト (本体から分離。`diag.rs` の子モジュール `tests` として組み込まれる)

use super::*;

#[test]
fn redacts_quoted_text() {
    let msg = "byte index 5 is not a char boundary; it is inside 'あ' (bytes 3..6) of `山田太郎 090-1234-5678`";
    let r = redact(msg);
    assert!(!r.contains("山田") && r.contains("<省略>"), "{r}");
}

#[test]
fn dates() {
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(20_356), (2025, 9, 25));
    assert_eq!(civil_from_days(-1), (1969, 12, 31));
}

#[test]
fn report_has_no_registered_words() {
    let mut cfg = crate::Config::default();
    cfg.keywords.push(crate::config::KeywordGroup { label: "X".into(), name: "秘密".into(), words: vec!["プロジェクト黒猫".into()], ..Default::default() });
    cfg.allowlist.values.push("boss@corp.example".into());
    let loaded = LoadedConfig { config: cfg, path: None, active_profile: "default".into(), profiles: vec![], warnings: vec![] };
    let path = PathBuf::from(r"C:\tmp\sumiveil\config.toml");
    let info = Info { reason: Reason::Manual, program: "test", config_path: &path, loaded: Some(&loaded), config_error: None, extra: vec![], error: Some("panicked at `黒猫の件`".into()) };
    let r = build_report(&info);
    assert!(!r.contains("黒猫") && !r.contains("boss@") && !r.contains("秘密"), "{r}");
    assert!(r.contains("1 グループ / 1 語"), "{r}");
}
