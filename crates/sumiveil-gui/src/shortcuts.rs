//! アプリ内のショートカットキー (設定 `gui.shortcuts`) の解釈・表示・キー登録。

use eframe::egui::{self, Event, Key, KeyboardShortcut, Modifiers};
use sumiveil_core::config::ShortcutsConfig;

/// ショートカットで実行できる操作。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Open,
    Paste,
    Copy,
    Save,
    Run,
    Settings,
    Find,
}

impl Action {
    pub const ALL: [Action; 7] = [Action::Open, Action::Paste, Action::Copy, Action::Save, Action::Run, Action::Settings, Action::Find];

    /// 設定キー (`gui.shortcuts.<id>`)。
    pub fn id(self) -> &'static str {
        match self {
            Action::Open => "open",
            Action::Paste => "paste",
            Action::Copy => "copy",
            Action::Save => "save",
            Action::Run => "run",
            Action::Settings => "settings",
            Action::Find => "find",
        }
    }

    pub fn label(self, ja: bool) -> &'static str {
        match (self, ja) {
            (Action::Open, true) => "ファイルを開く",
            (Action::Open, false) => "Open file",
            (Action::Paste, true) => "クリップボードから貼り付け (置き換え)",
            (Action::Paste, false) => "Paste from clipboard (replace)",
            (Action::Copy, true) => "マスク結果をコピー",
            (Action::Copy, false) => "Copy masked result",
            (Action::Save, true) => "マスク結果を保存",
            (Action::Save, false) => "Save masked result",
            (Action::Run, true) => "マスクを再実行",
            (Action::Run, false) => "Run masking again",
            (Action::Settings, true) => "設定を開く",
            (Action::Settings, false) => "Open settings",
            (Action::Find, true) => "検索",
            (Action::Find, false) => "Find",
        }
    }

    /// 現在の設定値 (空なら無効)。
    pub fn spec(self, s: &ShortcutsConfig) -> &str {
        s.get(self.id()).unwrap_or("")
    }

    pub fn default_spec(self) -> String {
        self.spec(&ShortcutsConfig::default()).to_string()
    }
}

/// 設定から有効なショートカットの一覧を作る。修飾キーの多いものから並べる
/// (egui の照合では Ctrl+O が Ctrl+Shift+O にも一致するため、具体的なものを先に調べる)。
pub fn compile(s: &ShortcutsConfig) -> Vec<(Action, KeyboardShortcut)> {
    let mut v: Vec<(Action, KeyboardShortcut)> = Action::ALL.iter().filter_map(|a| parse(a.spec(s)).map(|k| (*a, k))).collect();
    v.sort_by_key(|(_, k)| std::cmp::Reverse(k.modifiers.ctrl as u8 + k.modifiers.shift as u8 + k.modifiers.alt as u8));
    v
}

fn modifiers(ctrl: bool, shift: bool, alt: bool) -> Modifiers {
    Modifiers { alt, ctrl, shift, mac_cmd: false, command: ctrl }
}

/// "Ctrl+Shift+C" / "F5" / "Ctrl+," を解釈する。空・不正なら None。
pub fn parse(spec: &str) -> Option<KeyboardShortcut> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    let (mods, key) = if let Some(m) = spec.strip_suffix("++") {
        (m, "+")
    } else {
        spec.rsplit_once('+').unwrap_or(("", spec))
    };
    let (mut ctrl, mut shift, mut alt) = (false, false, false);
    for t in mods.split('+').map(str::trim).filter(|t| !t.is_empty()) {
        match t.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            _ => return None,
        }
    }
    let key = key.trim();
    let capitalized = {
        let mut c = key.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_ascii_lowercase()).unwrap_or_default()
    };
    let key = Key::from_name(key).or_else(|| Key::from_name(&key.to_ascii_uppercase())).or_else(|| Key::from_name(&capitalized))?;
    Some(KeyboardShortcut::new(modifiers(ctrl, shift, alt), key))
}

/// 設定ファイルに書く形 ("Ctrl+Alt+Shift+C")。記号キーは ASCII の記号で書く (Ctrl+,)。
pub fn format(sc: &KeyboardShortcut) -> String {
    let mut s = String::new();
    for (on, name) in [(sc.modifiers.ctrl || sc.modifiers.command, "Ctrl+"), (sc.modifiers.alt, "Alt+"), (sc.modifiers.shift, "Shift+")] {
        if on {
            s.push_str(name);
        }
    }
    let sym = sc.logical_key.symbol_or_name();
    s.push_str(if sym.is_ascii() { sym } else { sc.logical_key.name() });
    s
}

/// グローバルホットキー用の表記 (global-hotkey クレートが読める名前: Ctrl+Alt+Comma)。
pub fn format_for_hotkey(sc: &KeyboardShortcut) -> String {
    let mut s = format(sc);
    let sym = sc.logical_key.symbol_or_name();
    if sym.len() == 1 && !sym.chars().all(|c| c.is_ascii_alphanumeric()) {
        s.truncate(s.len() - sym.len());
        s.push_str(sc.logical_key.name());
    }
    s
}

/// 画面に出す形 ("Ctrl + Shift + C")。
pub fn display(spec: &str) -> String {
    match parse(spec) {
        Some(sc) => format(&sc).replace('+', " + ").replace(" +  + ", " + +"),
        None => spec.to_string(),
    }
}

/// キー登録中に押されたキー。
pub enum Recorded {
    Key(KeyboardShortcut),
    /// Backspace / Delete: 「なし」にする
    Clear,
    /// Esc: 取り消し
    Cancel,
}

/// Ctrl / Shift / Alt / Windows キーそのもの (egui 0.36 では単独のキーイベントとしても届く)。
fn is_modifier_key(k: Key) -> bool {
    matches!(
        k,
        Key::ShiftLeft | Key::ShiftRight | Key::ControlLeft | Key::ControlRight | Key::AltLeft | Key::AltRight | Key::SuperLeft | Key::SuperRight
    )
}

/// 登録中に押されたキーを取り出す。ほかの部品 (ボタンの Enter / Space 操作や入力欄) やショートカットに渡らないよう、
/// キー入力はすべて取り除く。修飾キーだけが押された間は None (組み合わせるキーを待つ)。
pub fn take_recorded(ctx: &egui::Context) -> Option<Recorded> {
    ctx.input_mut(|i| {
        let pos = i.events.iter().position(|e| matches!(e, Event::Key { key, pressed: true, repeat: false, .. } if !is_modifier_key(*key)));
        let ev = pos.map(|p| i.events.remove(p));
        // Ctrl+C / X / V はキーではなくコピー・切り取り・貼り付けのイベントとして届く (予約キーとして知らせるために拾う)
        let clip = i.events.iter().find_map(|e| match e {
            Event::Copy => Some(Key::C),
            Event::Cut => Some(Key::X),
            Event::Paste(_) => Some(Key::V),
            _ => None,
        });
        i.events.retain(|e| !matches!(e, Event::Key { .. } | Event::Text(_) | Event::Copy | Event::Cut | Event::Paste(_)));
        let (key, m) = match ev {
            Some(Event::Key { key, modifiers, .. }) => (key, modifiers),
            _ => (clip?, Modifiers::COMMAND),
        };
        let plain = !m.ctrl && !m.alt && !m.shift;
        Some(match key {
            Key::Escape if plain => Recorded::Cancel,
            Key::Backspace | Key::Delete if plain => Recorded::Clear,
            _ => Recorded::Key(KeyboardShortcut::new(modifiers(m.ctrl || m.command, m.shift, m.alt), key)),
        })
    })
}

#[cfg(test)]
mod tests {
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
}
