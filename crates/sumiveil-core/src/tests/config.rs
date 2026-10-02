//! `config.rs` の単体テスト (本体から分離。`config.rs` の子モジュール `tests` として組み込まれる)

use super::*;

#[test]
fn default_toml_loads_cleanly() {
    let l = load_str(DEFAULT_TOML, None).unwrap();
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    assert!(l.profiles.iter().any(|p| p.name == "llm"));
    assert_eq!(l.config.masking.template, "<{label}_{n}>");
}

#[test]
fn empty_is_default() {
    let l = load_str("", None).unwrap();
    assert_eq!(l.config.masking, MaskingConfig::default());
    assert!(l.config.detector_enabled("email"));
    assert!(!l.config.detector_enabled("uuid"));
}

#[test]
fn profiles_overlay() {
    let s = r#"
[detectors.email]
enabled = true
[allowlist]
values = ["a"]
[profiles.strict]
description = "厳しめ"
[profiles.strict.detectors.email]
enabled = false
[profiles.strict.allowlist]
values = ["b"]
"#;
    let l = load_str(s, Some("strict")).unwrap();
    assert!(!l.config.detector_enabled("email"));
    assert_eq!(l.config.allowlist.values, vec!["a", "b"]);
    assert_eq!(l.config.general.active_profile, "strict");
    let l = load_str(s, None).unwrap();
    assert!(l.config.detector_enabled("email"));
}

#[test]
fn category_disable() {
    let l = load_str("[categories.network]\nenabled = false", None).unwrap();
    assert!(!l.config.detector_enabled("ipv4"));
    assert!(l.config.detector_enabled("email"));
}

#[test]
fn includes_merge() {
    let dir = std::env::temp_dir().join(format!("sumiveil-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("team.toml"), "[[keywords]]\nlabel = \"CLIENT\"\nwords = [\"Acme\"]\n[masking]\ntemplate = \"[{label}]\"").unwrap();
    std::fs::write(dir.join("me.toml"), "include = [\"team.toml\"]\n[[keywords]]\nlabel = \"MINE\"\nwords = [\"Foo\"]").unwrap();
    let l = load(&dir.join("me.toml"), None).unwrap();
    assert_eq!(l.config.keywords.len(), 2);
    assert_eq!(l.config.masking.template, "[{label}]");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn edit_preserves_comments() {
    let src = "# 先頭コメント\n[detectors.email]\nenabled = true # 行末コメント\n";
    let mut d = ConfigDoc::from_str(Path::new("x.toml"), src).unwrap();
    d.set("detectors.email.enabled", false);
    d.set("detectors.ipv4.template", "[IP]");
    d.set_raw("masking.min_confidence", "0.7");
    let out = d.to_toml_string();
    assert!(out.contains("# 先頭コメント"));
    assert!(out.contains("enabled = false # 行末コメント"));
    let l = load_str(&out, None).unwrap();
    assert!(!l.config.detector_enabled("email"));
    assert_eq!(l.config.detectors["ipv4"].template.as_deref(), Some("[IP]"));
    assert!((l.config.masking.min_confidence - 0.7).abs() < 1e-6);
    assert_eq!(d.get("detectors.email.enabled").as_deref(), Some("false"));
}

#[test]
fn accent_color_and_obsolete_backdrop() {
    // 開発中の旧設定 (背景効果があった頃) の gui.backdrop は、未知のキーとして読み飛ばす
    let l = load_str("[gui]\nbackdrop = true\n", None).unwrap();
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!(l.config.gui.accent_color, "auto");
    assert_eq!(l.config.gui.manual_accent(), None);
    let l = load_str("[gui]\naccent_color = \"#E81123\"\n", None).unwrap();
    assert_eq!(l.config.gui.manual_accent(), Some([0xE8, 0x11, 0x23]));
    let l = load_str("[gui]\naccent_color = \"red\"\n", None).unwrap();
    assert_eq!(l.warnings.len(), 1);
    assert_eq!(parse_hex_color("0078d4"), Some([0x00, 0x78, 0xD4]));
    assert_eq!(parse_hex_color("#12345"), None);
}

#[test]
fn shortcuts() {
    let l = load_str("", None).unwrap();
    assert_eq!(l.config.gui.shortcuts.run, "F5");
    // 一部だけ変更。残りは既定値
    let l = load_str("[gui.shortcuts]\nrun = \"F6\"\nsave = \"\"\n", None).unwrap();
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!((l.config.gui.shortcuts.run.as_str(), l.config.gui.shortcuts.save.as_str(), l.config.gui.shortcuts.open.as_str()), ("F6", "", "Ctrl+O"));

    assert_eq!(normalize_shortcut(" shift + ctrl + c "), Ok(Some("ctrl+shift+c".into())));
    assert_eq!(normalize_shortcut("Ctrl+,"), normalize_shortcut("Control+Comma"));
    assert_eq!(normalize_shortcut("Ctrl++"), Ok(Some("ctrl+plus".into())));
    assert_eq!(normalize_shortcut(""), Ok(None));
    assert_eq!(normalize_shortcut("Win+C"), Err(ShortcutError::Syntax));
    assert_eq!(normalize_shortcut("Ctrl+Shift"), Err(ShortcutError::Syntax));
    assert_eq!(normalize_shortcut("Ctrl+ControlLeft"), Err(ShortcutError::Syntax));
    assert_eq!(normalize_shortcut("Ctrl+C"), Err(ShortcutError::Reserved));
    assert_eq!(normalize_shortcut("A"), Err(ShortcutError::Reserved));
    assert_eq!(normalize_shortcut("Shift+Tab"), Err(ShortcutError::Reserved));
    assert!(normalize_shortcut("Ctrl+Shift+V").is_ok());

    let g = GuiConfig::default();
    assert_eq!(shortcut_conflict(&g, "run", "Ctrl+S"), Some("save"));
    assert_eq!(shortcut_conflict(&g, "save", "Ctrl+S"), None);
    assert_eq!(shortcut_conflict(&g, "run", "ctrl+alt+m"), Some("hotkey"));

    let l = load_str("[gui]\nhotkey = \"Ctrl+Alt+M\"\n[gui.shortcuts]\nrun = \"Ctrl+S\"\ncopy = \"Ctrl+C\"\nopen = \"Ctrl+Alt+M\"\npaste = \"Hyper+V\"\n", None).unwrap();
    assert_eq!(l.warnings.len(), 4, "{:?}", l.warnings);
}

#[test]
fn edit_serialized_lists() {
    let mut d = ConfigDoc::from_str(Path::new("x.toml"), "").unwrap();
    let rules = vec![CustomRule { id: "prj".into(), label: "PROJECT".into(), pattern: "PRJ-\\d{4}".into(), ..Default::default() }];
    d.set_serialized("custom_rules", &rules).unwrap();
    d.set_serialized("allowlist.values", &vec!["a".to_string()]).unwrap();
    let l = load_str(&d.to_toml_string(), None).unwrap();
    assert_eq!(l.config.custom_rules, rules);
    assert!(l.config.allowlist.values.contains(&"a".to_string()));
}
