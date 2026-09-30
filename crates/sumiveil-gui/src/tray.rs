//! タスクトレイアイコンとグローバルホットキー。
//! イベントはチャネル経由でアプリに渡し、`request_repaint` で `App::logic` を起こす (ウィンドウ非表示中も動く)。

use std::sync::mpsc::Sender;

use eframe::egui;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use sumiveil_core::lang::Lang;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMsg {
    Show,
    MaskClipboard,
    Quit,
}

pub struct Tray {
    icon: TrayIcon,
    show_id: MenuId,
    mask_id: MenuId,
    quit_id: MenuId,
}

impl Tray {
    pub fn new(lang: Lang, hotkey_label: &str, tx: Sender<TrayMsg>, ctx: egui::Context) -> Result<Self, String> {
        let menu = Menu::new();
        let show = MenuItem::new(lang.t("Sumiveil を表示", "Show Sumiveil"), true, None);
        let mask_label = if hotkey_label.is_empty() {
            lang.t("クリップボードをマスク", "Mask clipboard").to_string()
        } else {
            format!("{}\t{hotkey_label}", lang.t("クリップボードをマスク", "Mask clipboard"))
        };
        let mask = MenuItem::new(mask_label, true, None);
        let quit = MenuItem::new(lang.t("終了", "Quit"), true, None);
        menu.append_items(&[&show, &mask, &PredefinedMenuItem::separator(), &quit]).map_err(|e| e.to_string())?;
        let rgba = crate::icon::render(32);
        let icon = Icon::from_rgba(rgba, 32, 32).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Sumiveil")
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;
        let (show_id, mask_id, quit_id) = (show.id().clone(), mask.id().clone(), quit.id().clone());

        {
            let tx = tx.clone();
            let ctx = ctx.clone();
            TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
                let open = match e {
                    TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } => true,
                    TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => true,
                    _ => false,
                };
                if open {
                    let _ = tx.send(TrayMsg::Show);
                    ctx.request_repaint();
                }
            }));
        }
        {
            let (s, m, q) = (show_id.clone(), mask_id.clone(), quit_id.clone());
            let tx = tx.clone();
            let ctx = ctx.clone();
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                let msg = if e.id == s {
                    Some(TrayMsg::Show)
                } else if e.id == m {
                    Some(TrayMsg::MaskClipboard)
                } else if e.id == q {
                    Some(TrayMsg::Quit)
                } else {
                    None
                };
                if let Some(msg) = msg {
                    let _ = tx.send(msg);
                    ctx.request_repaint();
                }
            }));
        }
        Ok(Self { icon: tray, show_id, mask_id, quit_id })
    }

    pub fn set_tooltip(&self, text: &str) {
        let _ = self.icon.set_tooltip(Some(text));
    }

    #[allow(dead_code)]
    pub fn ids(&self) -> (&MenuId, &MenuId, &MenuId) {
        (&self.show_id, &self.mask_id, &self.quit_id)
    }
}

/// グローバルホットキーの登録状態。
pub struct Hotkey {
    manager: GlobalHotKeyManager,
    current: Option<HotKey>,
}

impl Hotkey {
    pub fn new(tx: Sender<TrayMsg>, ctx: egui::Context) -> Result<Self, String> {
        let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
        GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
            if e.state == HotKeyState::Pressed {
                let _ = tx.send(TrayMsg::MaskClipboard);
                ctx.request_repaint();
            }
        }));
        Ok(Self { manager, current: None })
    }

    /// ホットキーを (再) 登録する。空文字なら解除のみ。
    pub fn set(&mut self, spec: &str) -> Result<(), String> {
        if let Some(old) = self.current.take() {
            let _ = self.manager.unregister(old);
        }
        let spec = spec.trim();
        if spec.is_empty() {
            return Ok(());
        }
        let hk: HotKey = parse(spec)?;
        self.manager.register(hk).map_err(|e| e.to_string())?;
        self.current = Some(hk);
        Ok(())
    }
}

pub fn parse(spec: &str) -> Result<HotKey, String> {
    spec.parse::<HotKey>().map_err(|e| e.to_string())
}
