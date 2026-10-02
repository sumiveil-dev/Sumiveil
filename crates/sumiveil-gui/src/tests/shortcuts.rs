//! `shortcuts.rs` の単体テスト (本体から分離。`shortcuts.rs` の子モジュール `tests` として組み込まれる)

use super::*;

#[test]
fn roundtrip() {
    for s in ["Ctrl+Shift+C", "F5", "Ctrl+,", "Ctrl+Alt+M", "Ctrl+Alt+Shift+F12", "Alt+1"] {
        assert_eq!(format(&parse(s).unwrap_or_else(|| panic!("{s}"))), s);
    }
    assert_eq!(format(&parse("ctrl+o").unwrap()), "Ctrl+O");
    assert_eq!(format(&parse("shift+ctrl+comma").unwrap()), "Ctrl+Shift+,");
    assert_eq!(parse("Ctrl+Comma"), parse("Ctrl+,"));
    assert!(parse("").is_none());
    assert!(parse("Win+C").is_none());
    assert!(parse("Ctrl+NoSuchKey").is_none());
    assert_eq!(format_for_hotkey(&parse("Ctrl+Alt+,").unwrap()), "Ctrl+Alt+Comma");
    assert_eq!(format_for_hotkey(&parse("Ctrl+Alt+M").unwrap()), "Ctrl+Alt+M");
    assert_eq!(display("Ctrl+Shift+C"), "Ctrl + Shift + C");
}

#[test]
fn modifier_keys_are_not_recorded() {
    let ctx = egui::Context::default();
    let press = |key: Key, m: Modifiers| Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: m };
    // Ctrl を押しただけでは確定せず、続く O で Ctrl+O になる
    let mut input = egui::RawInput::default();
    input.events = vec![press(Key::ControlLeft, Modifiers::COMMAND)];
    ctx.begin_pass(input);
    assert!(take_recorded(&ctx).is_none());
    let _ = ctx.end_pass();
    let mut input = egui::RawInput::default();
    input.events = vec![press(Key::ControlLeft, Modifiers::COMMAND), press(Key::O, Modifiers::COMMAND)];
    ctx.begin_pass(input);
    match take_recorded(&ctx) {
        Some(Recorded::Key(sc)) => assert_eq!(format(&sc), "Ctrl+O"),
        _ => panic!("Ctrl+O was not recorded"),
    }
    // 取り出したキーはショートカットの処理に渡らない
    assert!(!ctx.input_mut(|i| i.consume_shortcut(&parse("Ctrl+O").unwrap())));
    let _ = ctx.end_pass();
}

#[test]
fn compile_orders_specific_first() {
    let mut s = ShortcutsConfig::default();
    s.save = String::new();
    let c = compile(&s);
    assert_eq!(c.len(), 6);
    assert!(c[0].1.modifiers.shift, "Ctrl+Shift+… should come first");
    assert!(!c.iter().any(|(a, _)| *a == Action::Save));
}

#[test]
fn defaults_are_valid() {
    for a in Action::ALL {
        let spec = a.default_spec();
        assert!(parse(&spec).is_some(), "{spec}");
        assert!(sumiveil_core::config::normalize_shortcut(&spec).is_ok(), "{spec}");
    }
}
