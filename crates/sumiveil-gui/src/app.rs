//! アプリ本体: 状態・設定の読み書き・画面遷移。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, RichText, Stroke, ThemePreference};
use sumiveil_core::config::{self, ConfigDoc, LoadedConfig, ThemeMode};
use sumiveil_core::formats::{self, Document, PropertiesMode};
use sumiveil_core::lang::Lang;
use sumiveil_core::{encoding_rs, toml_edit};
use sumiveil_core::{Engine, MaskSession};

use crate::batch_page::BatchUi;
use crate::icons::Icon;
use crate::mask_page::MaskUi;
use crate::settings_page::SettingsUi;
use crate::theme::{self, Palette};
use crate::tray::{Hotkey, Tray, TrayMsg};
use crate::widgets::{self, InfoKind};
use crate::win;
use crate::worker::{Done, Job, Worker};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Mask,
    Batch,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

pub struct Toast {
    pub at: Instant,
    pub text: String,
    pub kind: ToastKind,
}

/// 入力テキストの由来。
#[derive(Clone, Default)]
pub struct InputInfo {
    pub path: Option<PathBuf>,
    pub encoding: Option<&'static encoding_rs::Encoding>,
    pub bom: bool,
    pub encoding_name: String,
    /// メール・Office 文書・PDF の読み込み結果。あるときは左側を編集不可にし、保存は元の形式 (PDF / .msg はテキスト) で書き出す
    pub doc: Option<Arc<Document>>,
    /// このファイルのプロパティ (作成者など) の扱い。設定が "ask" のときに利用者が選んだもの
    pub props_choice: Option<PropertiesMode>,
}

pub struct Startup {
    pub file: Option<PathBuf>,
    pub start_in_tray: bool,
    /// 起動時に開く画面 (mask / batch[:search] / settings[:section] / about)
    pub page: Option<String>,
    /// 起動時に検索する文字列 (`--find`)
    pub find: Option<String>,
}

pub struct App {
    pub ctx: egui::Context,
    pub hwnd: isize,
    pub lang: Lang,
    /// Windows のアクセントカラー (自動のとき使う。フォーカスを得るたびに読み直す)
    pub system_accent: Option<Color32>,
    /// 設定画面でカラーピッカーを操作している間だけ使う仮の色 (保存前のプレビュー)
    pub accent_preview: Option<Color32>,
    pub renderer_info: String,
    pub cfg_path: PathBuf,
    pub loaded: LoadedConfig,
    pub engine: Arc<Engine>,
    pub config_error: Option<String>,
    last_file_text: Option<String>,
    _watcher: Option<notify::RecommendedWatcher>,
    watch_rx: Receiver<()>,
    pub page: Page,
    pub nav_expanded: bool,
    /// 狭いウィンドウでナビゲーションを一時的に展開しているか
    nav_peek: bool,
    /// アプリ内のショートカット (設定を読み込んだときに解釈しておく)
    pub shortcuts: Vec<(crate::shortcuts::Action, egui::KeyboardShortcut)>,
    /// キーを登録中の項目 (操作の ID、グローバルホットキーは "hotkey")。登録中はショートカットを実行しない
    pub shortcut_recording: Option<&'static str>,

    // マスク画面
    pub input: String,
    pub input_info: InputInfo,
    pub edited_at: Option<Instant>,
    force_mask: bool,
    pub gen: u64,
    pub submitted: u64,
    pub done: Option<Done>,
    pub excluded: HashSet<String>,
    pub excluded_display: Vec<String>,
    /// 検索バーから「今だけマスク」した語 (設定ファイルには保存しない)
    pub manual_terms: Vec<sumiveil_core::search::Query>,
    pub mask: MaskUi,
    worker: Worker,

    pub settings: SettingsUi,
    pub batch: BatchUi,
    pub toasts: Vec<Toast>,

    tray: Option<Tray>,
    hotkey: Option<Hotkey>,
    hotkey_spec: String,
    tray_tx: Sender<TrayMsg>,
    tray_rx: Receiver<TrayMsg>,
    quitting: bool,
    caption_dark: Option<bool>,
    applied_font_size: f32,
    applied_accent: Option<Color32>,
    was_focused: bool,
    first_frame: bool,
    start_hidden: bool,
    offscreen: bool,
}

/// 使用中の描画方式と GPU (問い合わせ・不具合調査用に「情報」画面へ表示する)。
fn renderer_info(cc: &eframe::CreationContext<'_>) -> String {
    if let Some(gl) = &cc.gl {
        use eframe::glow::HasContext;
        // Safety: 作成直後の有効な GL コンテキストに対する読み取り専用の問い合わせ
        let (renderer, version) = unsafe { (gl.get_parameter_string(eframe::glow::RENDERER), gl.get_parameter_string(eframe::glow::VERSION)) };
        return format!("OpenGL / {renderer} / {version}");
    }
    if let Some(rs) = &cc.wgpu_render_state {
        let info = rs.adapter.get_info();
        return format!("{:?} / {} ({:?})", info.backend, info.name, info.device_type);
    }
    "unknown".into()
}

/// 表示言語。日本語を表示できるフォントが無い PC では、文字化けを避けるため英語にする。
fn ui_lang(setting: &str) -> Lang {
    let lang = Lang::resolve(setting);
    if lang.is_ja() && !crate::fonts::has_japanese() {
        Lang::En
    } else {
        lang
    }
}

fn theme_pref(mode: ThemeMode) -> ThemePreference {
    match mode {
        ThemeMode::Light => ThemePreference::Light,
        ThemeMode::Dark => ThemePreference::Dark,
        ThemeMode::System => ThemePreference::System,
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: Startup) -> Self {
        let ctx = cc.egui_ctx.clone();
        let hwnd = {
            use eframe::wgpu::rwh::{HasWindowHandle, RawWindowHandle};
            match cc.window_handle().map(|h| h.as_raw()) {
                Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
                _ => 0,
            }
        };
        win::limit_window_size(hwnd);
        crate::fonts::install(&ctx);
        let system_accent = win::accent_color();
        let renderer_info = renderer_info(cc);
        crate::diag::set_renderer(&renderer_info);
        crate::diag::debug_log(&format!("renderer: {renderer_info}\njapanese font: {}", crate::fonts::has_japanese()));

        let cfg_path = config::resolve_config_path(None);
        let (loaded, config_error) = match config::load(&cfg_path, None) {
            Ok(l) => (l, None),
            Err(e) => (config::load_str("", None).expect("default config"), Some(e.to_string())),
        };
        let last_file_text = std::fs::read_to_string(&cfg_path).ok();
        let lang = ui_lang(&loaded.config.general.language);
        ctx.set_theme(theme_pref(loaded.config.general.theme));
        let engine = Arc::new(Engine::new(&loaded.config));

        // 設定ファイルの監視
        let (wtx, watch_rx) = channel::<()>();
        let watcher = {
            use notify::{RecursiveMode, Watcher};
            let target = cfg_path.clone();
            let ctx2 = ctx.clone();
            let w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(ev) = res {
                    if ev.paths.iter().any(|p| p.file_name() == target.file_name()) {
                        let _ = wtx.send(());
                        ctx2.request_repaint_after(Duration::from_millis(150));
                    }
                }
            });
            match w {
                Ok(mut w) => {
                    if let Some(dir) = cfg_path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                        let _ = w.watch(dir, RecursiveMode::NonRecursive);
                    }
                    Some(w)
                }
                Err(_) => None,
            }
        };

        let (tray_tx, tray_rx) = channel::<TrayMsg>();
        let mut app = Self {
            worker: Worker::new(ctx.clone()),
            ctx: ctx.clone(),
            hwnd,
            lang,
            system_accent,
            accent_preview: None,
            renderer_info,
            cfg_path,
            loaded,
            engine,
            config_error,
            last_file_text,
            _watcher: watcher,
            watch_rx,
            page: Page::Mask,
            nav_expanded: true,
            nav_peek: false,
            shortcuts: vec![],
            shortcut_recording: None,
            input: String::new(),
            input_info: InputInfo { encoding_name: "UTF-8".into(), ..Default::default() },
            edited_at: None,
            force_mask: false,
            gen: 0,
            submitted: 0,
            done: None,
            excluded: HashSet::new(),
            excluded_display: vec![],
            manual_terms: vec![],
            mask: MaskUi::default(),
            settings: SettingsUi::default(),
            batch: BatchUi::default(),
            toasts: vec![],
            tray: None,
            hotkey: None,
            hotkey_spec: String::new(),
            tray_tx,
            tray_rx,
            quitting: false,
            caption_dark: None,
            applied_font_size: 0.0,
            applied_accent: None,
            was_focused: false,
            first_frame: true,
            start_hidden: startup.start_in_tray,
            offscreen: startup.start_in_tray,
        };
        app.refresh_style();
        app.shortcuts = crate::shortcuts::compile(&app.loaded.config.gui.shortcuts);
        app.batch.init_from(&app.loaded.config);
        app.update_tray_and_hotkey();
        if let Some(f) = startup.file {
            // フォルダを渡されたら (ドロップと同じく) 一括処理の対象にする
            if f.is_dir() {
                app.batch.input_dir = f.display().to_string();
                app.page = Page::Batch;
            } else {
                app.open_file(&f);
            }
        }
        if let Some(p) = startup.page.as_deref() {
            use crate::settings_page::Section;
            let (page, section) = p.split_once(':').unwrap_or((p, ""));
            // 「情報」は設定の最後の項目になった (旧版の `--page about` も受け付ける)
            let section = if page == "about" { "about" } else { section };
            app.page = match page {
                "batch" => Page::Batch,
                "settings" | "about" => Page::Settings,
                _ => Page::Mask,
            };
            let (s, tab) = Section::from_name(section);
            app.settings.section = s;
            if let Some(t) = tab {
                app.settings.rules_tab = t;
            }
            if page == "batch" && section == "search" {
                app.batch.mode = crate::batch_page::BatchMode::Search;
            }
        }
        // --find: 一括処理の画面なら横断検索、それ以外は検索バー
        if let Some(q) = startup.find.filter(|q| !q.is_empty()) {
            if app.page == Page::Batch {
                crate::batch_page::search_on_start(&mut app, q);
            } else {
                app.page = Page::Mask;
                crate::mask_page::find_on_start(&mut app, q);
            }
        }
        // 正規表現の事前コンパイルと辞書の読み込みを裏で済ませる
        std::thread::spawn(|| {
            sumiveil_core::detector::warm_up();
            let _ = sumiveil_core::dict::NameDict::get();
        });
        app
    }

    pub fn cfg(&self) -> &sumiveil_core::Config {
        &self.loaded.config
    }

    pub fn t<'a>(&self, ja: &'a str, en: &'a str) -> &'a str {
        self.lang.t(ja, en)
    }

    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.toasts.push(Toast { at: Instant::now(), text: text.into(), kind });
        self.ctx.request_repaint();
    }

    // ───────────── 設定 ─────────────

    /// 設定ファイルを編集して保存し、読み込み直す。`f` にはプロファイル名が渡る。
    pub fn edit_config(&mut self, f: impl FnOnce(&mut ConfigDoc, &str)) {
        let mut doc = match ConfigDoc::open(&self.cfg_path) {
            Ok(d) => d,
            Err(e) => {
                self.toast(ToastKind::Error, e.to_string());
                return;
            }
        };
        let profile = self.loaded.active_profile.clone();
        f(&mut doc, &profile);
        let text = doc.to_toml_string();
        // 内容が変わらない書き込みはしない (無駄なディスク書き込みと再読込の連鎖を防ぐ)
        if self.cfg_path.exists() && self.last_file_text.as_deref() == Some(text.as_str()) {
            return;
        }
        if let Err(e) = config::load_str(&text, None) {
            self.toast(ToastKind::Error, format!("{}: {e}", self.t("設定を保存できません", "Cannot save settings")));
            return;
        }
        if let Err(e) = doc.save() {
            self.toast(ToastKind::Error, e.to_string());
            return;
        }
        self.last_file_text = Some(text);
        self.reload_config();
    }

    /// 基本設定 (プロファイルに依存しない項目) を編集する。
    pub fn set_base(&mut self, key: &str, value: impl Into<toml_edit::Value>) {
        let v = value.into();
        self.edit_config(|d, _| d.set(key, v));
    }

    /// 検出・表示に関する項目。有効なプロファイルがあればその中に書く。
    pub fn set_scoped(&mut self, key: &str, value: impl Into<toml_edit::Value>) {
        let v = value.into();
        self.edit_config(|d, p| d.set(&config::profile_key(p, key), v));
    }

    pub fn remove_scoped(&mut self, key: &str) {
        let key = key.to_string();
        self.edit_config(|d, p| {
            d.remove(&config::profile_key(p, &key));
        });
    }

    pub fn reload_config(&mut self) {
        match config::load(&self.cfg_path, None) {
            Ok(l) => {
                self.config_error = None;
                self.apply_loaded(l);
            }
            Err(e) => self.config_error = Some(e.to_string()),
        }
    }

    fn apply_loaded(&mut self, loaded: LoadedConfig) {
        let old_gui = self.loaded.config.gui.clone();
        self.loaded = loaded;
        let cfg = self.loaded.config.clone();
        self.lang = ui_lang(&cfg.general.language);
        self.ctx.set_theme(theme_pref(cfg.general.theme));
        self.refresh_style();
        self.shortcuts = crate::shortcuts::compile(&cfg.gui.shortcuts);
        self.rebuild_engine();
        self.caption_dark = None;
        if old_gui.tray_enabled != cfg.gui.tray_enabled || old_gui.hotkey != cfg.gui.hotkey || old_gui.hotkey_enabled != cfg.gui.hotkey_enabled || self.tray.is_none() {
            self.update_tray_and_hotkey();
        }
        self.request_mask();
    }

    fn check_config_file_changed(&mut self) {
        let mut changed = false;
        while self.watch_rx.try_recv().is_ok() {
            changed = true;
        }
        if !changed {
            return;
        }
        let now = std::fs::read_to_string(&self.cfg_path).ok();
        if now != self.last_file_text {
            self.last_file_text = now;
            self.reload_config();
            let msg = self.t("設定ファイルの変更を反映しました", "Reloaded settings from file").to_string();
            self.toast(ToastKind::Info, msg);
        }
    }

    // ───────────── トレイ・ホットキー ─────────────

    fn update_tray_and_hotkey(&mut self) {
        let gui = self.loaded.config.gui.clone();
        let hk_label = if gui.hotkey_enabled { gui.hotkey.clone() } else { String::new() };
        if gui.tray_enabled {
            // 言語やホットキー表示を反映するため作り直す
            self.tray = None;
            match Tray::new(self.lang, &hk_label, self.tray_tx.clone(), self.ctx.clone()) {
                Ok(t) => self.tray = Some(t),
                Err(e) => self.toast(ToastKind::Error, format!("tray: {e}")),
            }
        } else {
            self.tray = None;
        }
        self.sync_hotkey();
    }

    /// キーの登録を始める。登録中はグローバルホットキーを一時的に解除する
    /// (そのキーを押したときに Windows に横取りされ、クリップボードのマスクが動いてしまうため)。
    pub fn begin_recording(&mut self, id: &'static str) {
        self.shortcut_recording = Some(id);
        self.sync_hotkey();
    }

    /// キーの登録を終える (確定・取り消しとも)。
    pub fn end_recording(&mut self) {
        if self.shortcut_recording.take().is_some() {
            self.sync_hotkey();
        }
    }

    /// グローバルホットキーの登録を設定に合わせる (変わっていなければ何もしない)。
    pub fn sync_hotkey(&mut self) {
        let gui = self.loaded.config.gui.clone();
        if self.hotkey.is_none() {
            match Hotkey::new(self.tray_tx.clone(), self.ctx.clone()) {
                Ok(h) => self.hotkey = Some(h),
                Err(e) => self.toast(ToastKind::Error, format!("hotkey: {e}")),
            }
        }
        let spec = if gui.hotkey_enabled && self.shortcut_recording.is_none() { gui.hotkey.clone() } else { String::new() };
        if spec != self.hotkey_spec {
            if let Some(h) = &mut self.hotkey {
                match h.set(&spec) {
                    Ok(()) => self.hotkey_spec = spec,
                    Err(e) => {
                        self.hotkey_spec = String::new();
                        let msg = format!("{} ({spec}): {e}", self.t("ホットキーを登録できません", "Cannot register hotkey"));
                        self.toast(ToastKind::Error, msg);
                    }
                }
            }
        }
    }

    fn handle_tray(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.tray_rx.try_recv() {
            match msg {
                TrayMsg::Show => self.show_window(ctx),
                TrayMsg::MaskClipboard => self.mask_clipboard(),
                TrayMsg::Quit => {
                    self.quitting = true;
                    win::show_window(self.hwnd);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    pub fn show_window(&mut self, ctx: &egui::Context) {
        self.bring_onscreen(ctx);
        win::show_window(self.hwnd);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    /// トレイ起動で画面外に作ったウィンドウを画面中央へ移す (一度だけ)。
    fn bring_onscreen(&mut self, ctx: &egui::Context) {
        if self.offscreen {
            self.offscreen = false;
            let (monitor, size) = ctx.input(|i| (i.viewport().monitor_size, i.viewport().outer_rect.map(|r| r.size())));
            let size = size.unwrap_or(egui::vec2(1280.0, 800.0));
            let pos = match monitor {
                Some(m) => ((m - size) / 2.0).max(egui::Vec2::ZERO).to_pos2(),
                None => egui::pos2(80.0, 60.0),
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
        }
    }

    /// クリップボードの内容をその場でマスクする。
    pub fn mask_clipboard(&mut self) {
        let text = match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
            Ok(t) => t,
            Err(_) => {
                let msg = self.t("クリップボードにテキストがありません", "No text in clipboard").to_string();
                self.notify(ToastKind::Error, &msg);
                return;
            }
        };
        let r = self.engine.mask(&text, &mut MaskSession::new());
        let n = r.replacements.len();
        if n > 0 {
            if let Err(e) = arboard::Clipboard::new().and_then(|mut c| c.set_text(r.output.clone())) {
                self.notify(ToastKind::Error, &e.to_string());
                return;
            }
        }
        let msg = if self.lang.is_ja() {
            if n > 0 { format!("クリップボードの {n} 件をマスクしました") } else { "マスク対象は見つかりませんでした".to_string() }
        } else if n > 0 {
            format!("Masked {n} item(s) in the clipboard")
        } else {
            "Nothing to mask".to_string()
        };
        self.notify(if n > 0 { ToastKind::Success } else { ToastKind::Info }, &msg);
        if let Some(t) = &self.tray {
            t.set_tooltip(&format!("Sumiveil — {msg}"));
        }
    }

    /// ウィンドウが見えていればアプリ内トースト、隠れていれば Windows の通知。
    fn notify(&mut self, kind: ToastKind, msg: &str) {
        if win::is_visible(self.hwnd) {
            self.toast(kind, msg);
        } else if self.loaded.config.gui.hotkey_notify {
            let _ = win::toast("Sumiveil", msg);
        }
    }

    // ───────────── 入力 ─────────────

    pub fn set_input(&mut self, text: String, info: InputInfo) {
        self.input = text;
        self.input_info = info;
        self.excluded.clear();
        self.excluded_display.clear();
        self.mask.selected = None;
        self.request_mask();
    }

    /// ファイルを開く。拡張子で形式を判定する (テキスト・メール・Office 文書・PDF など)。
    pub fn open_file(&mut self, path: &Path) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => return self.toast(ToastKind::Error, format!("{}: {e}", path.display())),
        };
        match formats::open(path, bytes, None) {
            Ok(doc) => {
                let enc = doc.text_encoding();
                if !doc.kind.is_structured() && !doc.warnings.is_empty() {
                    let msg = self.t("一部の文字を変換できませんでした", "Some characters could not be decoded").to_string();
                    self.toast(ToastKind::Error, msg);
                }
                let text = doc.text.clone();
                let info = InputInfo {
                    path: Some(path.to_path_buf()),
                    encoding: enc.map(|e| e.0),
                    bom: enc.is_some_and(|e| e.1),
                    encoding_name: doc.encoding_name.clone(),
                    doc: doc.kind.is_structured().then(|| Arc::new(doc)),
                    props_choice: None,
                };
                self.set_input(text, info);
                self.page = Page::Mask;
            }
            Err(e) => self.toast(ToastKind::Error, format!("{}: {e}", path.display())),
        }
    }

    /// 検出エンジンを作り直す (設定 + 「今だけマスク」した語)。
    pub fn rebuild_engine(&mut self) {
        let cfg = sumiveil_core::search::config_with_terms(&self.loaded.config, &self.manual_terms);
        self.engine = Arc::new(Engine::new(&cfg));
    }

    /// 「今だけマスク」する語を追加・全解除する。
    pub fn set_manual_terms(&mut self, terms: Vec<sumiveil_core::search::Query>) {
        self.manual_terms = terms;
        self.rebuild_engine();
        self.request_mask();
    }

    /// 保存・一括処理で使うプロパティ (作成者など) の扱い。設定が "ask" で、まだ選んでいなければ None。
    pub fn properties_mode(&self) -> Option<PropertiesMode> {
        PropertiesMode::parse(&self.cfg().files.properties).or(self.input_info.props_choice)
    }

    /// すぐにマスクを実行する (自動マスクがオフでも実行)。
    pub fn request_mask(&mut self) {
        self.gen += 1;
        self.force_mask = true;
        self.ctx.request_repaint();
    }

    /// 入力が編集された。自動マスクが有効なら少し待ってから実行する。
    pub fn on_input_edited(&mut self) {
        self.gen += 1;
        self.edited_at = Some(Instant::now());
    }

    /// 表示中の結果が入力に追いついていない。
    pub fn is_stale(&self) -> bool {
        self.done.as_ref().is_some_and(|d| d.gen != self.gen) || (self.done.is_none() && !self.input.is_empty())
    }

    fn pump_worker(&mut self, ctx: &egui::Context) {
        if let Some(d) = self.worker.poll() {
            if self.done.as_ref().is_none_or(|old| d.gen >= old.gen) {
                self.done = Some(d);
            }
        }
        if self.submitted == self.gen {
            return;
        }
        let gui = &self.loaded.config.gui;
        let ready = if self.force_mask {
            true
        } else if gui.auto_mask {
            let wait = Duration::from_millis(gui.debounce_ms);
            let elapsed = self.edited_at.map(|t| t.elapsed()).unwrap_or(wait);
            if elapsed < wait {
                ctx.request_repaint_after(wait - elapsed);
                false
            } else {
                true
            }
        } else {
            false
        };
        if !ready {
            return;
        }
        self.force_mask = false;
        self.submitted = self.gen;
        self.worker.submit(Job {
            gen: self.gen,
            text: Arc::new(self.input.clone()),
            engine: self.engine.clone(),
            excluded: Arc::new(self.excluded.clone()),
        });
    }

    // ───────────── 描画 ─────────────

    fn update_caption(&mut self, ctx: &egui::Context) {
        let dark = ctx.theme() == egui::Theme::Dark;
        if self.caption_dark != Some(dark) {
            self.caption_dark = Some(dark);
            win::set_dark_caption(self.hwnd, dark);
        }
    }

    /// 実際に使うアクセントカラー (操作中のプレビュー → 手動指定 → Windows の設定)。
    pub fn effective_accent(&self) -> Option<Color32> {
        self.accent_preview
            .or_else(|| self.loaded.config.gui.manual_accent().map(|[r, g, b]| Color32::from_rgb(r, g, b)))
            .or(self.system_accent)
    }

    /// 文字サイズかアクセントカラーが変わっていたら、配色とスタイルを作り直す。
    pub fn refresh_style(&mut self) {
        let font = self.loaded.config.gui.font_size;
        let accent = self.effective_accent();
        if (font - self.applied_font_size).abs() > 0.01 || accent != self.applied_accent {
            theme::apply(&self.ctx, accent, font);
            self.applied_font_size = font;
            self.applied_accent = accent;
        }
    }

    /// ウィンドウがフォーカスを得たら Windows のアクセントカラーを読み直す (設定アプリで変えた場合に追従)。
    fn follow_system_accent(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| i.focused);
        if focused && !self.was_focused {
            let now = win::accent_color();
            if now != self.system_accent {
                self.system_accent = now;
                self.refresh_style();
            }
        }
        self.was_focused = focused;
    }

    fn nav(&mut self, ui: &mut egui::Ui) {
        let p = Palette::current(ui.ctx());
        // ウィンドウが狭いときはアイコンだけの表示にする。
        // このときハンバーガーボタンは一時的な展開だけを切り替え、ページを選ぶと閉じる
        let narrow = ui.ctx().content_rect().width() < 900.0;
        if !narrow {
            self.nav_peek = false;
        }
        let expanded = if narrow { self.nav_peek } else { self.nav_expanded };
        let width = if expanded { 196.0 } else { 52.0 };
        egui::Panel::left("nav")
            .exact_size(width)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 4, right: 4, top: 6, bottom: 8 }))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if widgets::nav_item(ui, Icon::Menu, "", false, false).clicked() {
                    if narrow {
                        self.nav_peek = !self.nav_peek;
                    } else {
                        self.nav_expanded = !self.nav_expanded;
                    }
                }
                ui.add_space(4.0);
                let items = [
                    (Page::Mask, Icon::Veil, self.t("マスク", "Mask")),
                    (Page::Batch, Icon::Folder, self.t("一括処理", "Batch")),
                ];
                for (page, g, label) in items {
                    if widgets::nav_item(ui, g, label, self.page == page, expanded).clicked() {
                        self.page = page;
                        self.nav_peek = false;
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    if widgets::nav_item(ui, Icon::Settings, self.t("設定", "Settings"), self.page == Page::Settings, expanded).clicked() {
                        self.page = Page::Settings;
                        self.nav_peek = false;
                    }
                });
            });
    }

    fn toasts_ui(&mut self, ctx: &egui::Context) {
        self.toasts.retain(|t| t.at.elapsed() < Duration::from_millis(3500));
        if self.toasts.is_empty() {
            return;
        }
        let p = Palette::current(ctx);
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -40.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                for t in &self.toasts {
                    let (accent, g) = match t.kind {
                        ToastKind::Info => (p.accent, Icon::Info),
                        ToastKind::Success => (p.success, Icon::Check),
                        ToastKind::Error => (p.critical, Icon::Warning),
                    };
                    Frame::new()
                        .fill(p.card)
                        .stroke(Stroke::new(1.0, p.card_stroke))
                        .corner_radius(CornerRadius::same(8))
                        .shadow(ui.visuals().popup_shadow)
                        .inner_margin(Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.set_max_width(420.0);
                            ui.horizontal(|ui| {
                                widgets::icon_text(ui, g, "", accent);
                                ui.label(RichText::new(&t.text).color(p.text));
                            });
                        });
                    ui.add_space(6.0);
                }
            });
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    /// 設定 →「情報」の中身 (スクロールは設定画面側で行う)。
    pub fn about(&mut self, ui: &mut egui::Ui) {
        {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let tex = self.mask.icon_texture(ui.ctx());
                ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(56.0, 56.0)));
                ui.vertical(|ui| {
                    widgets::title(ui, "Sumiveil");
                    widgets::secondary(ui, &format!("{} {}", self.t("バージョン", "Version"), sumiveil_core::VERSION));
                });
            });
            ui.add_space(12.0);
            widgets::card(ui, |ui| {
                widgets::subtitle(ui, self.t("このアプリについて", "About this app"));
                ui.label(self.t(
                    "テキスト中の個人情報・機密情報を検出してマスクします。すべての処理はこの PC 内で完結し、インターネットには一切接続しません (テレメトリもありません)。",
                    "Detects and masks personal and confidential information in text. Everything runs locally on this PC; it never connects to the Internet (no telemetry).",
                ));
                ui.add_space(6.0);
                let r = ui.label(format!("{}: {}", self.t("設定ファイル", "Settings file"), win::external_path(&self.cfg_path).display()));
                crate::guide_mark!(ui.ctx(), "about-cfg-path", r.rect);
                ui.label(format!("{}: sumiveil --help", self.t("コマンドライン", "Command line")));
                widgets::secondary(ui, &format!("{}: {}", self.t("描画", "Renderer"), self.renderer_info));
                // 同梱の利用ガイド (PDF) を既定のアプリで開く
                let guide = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("docs").join("Sumiveil-UserGuide-ja.pdf")));
                if let Some(guide) = guide.filter(|p| p.exists()) {
                    ui.add_space(4.0);
                    if widgets::icon_button(ui, Icon::Report, self.t("利用ガイド (PDF) を開く", "Open user guide (PDF, Japanese)")).clicked() {
                        let _ = std::process::Command::new("explorer.exe").arg(&guide).spawn();
                    }
                }
            });
            ui.add_space(8.0);
            widgets::card(ui, |ui| {
                widgets::subtitle(ui, self.t("問題が起きたとき", "Troubleshooting"));
                ui.label(self.t(
                    "診断レポート (バージョン・Windows・描画方式・設定のオン/オフや件数) をファイルに書き出します。自動では送信されません。内容を確認してから、問い合わせ (GitHub の Issues) にファイルを添付してください。入力した文章・ファイルの内容・辞書やルールに登録した語は含まれません。内部エラーが起きたときは自動で作成されます。",
                    "Writes a diagnostic report (version, Windows, renderer, settings switches and counts) to a file. It is never sent automatically; review it before attaching the file to a GitHub issue. It never includes your text, file contents or registered words. A report is also created automatically after an internal error.",
                ));
                ui.add_space(4.0);
                let mut create = false;
                let mut open_folder = false;
                ui.horizontal_wrapped(|ui| {
                    create = widgets::icon_button(ui, Icon::Report, self.t("診断レポートを作成", "Create diagnostic report")).clicked();
                    open_folder = widgets::icon_button(ui, Icon::Folder, self.t("保存先のフォルダを開く", "Open report folder")).clicked();
                });
                if create {
                    match crate::diag::create_manual(&self.cfg_path, &self.loaded, self.config_error.as_deref()) {
                        Ok(p) => {
                            win::reveal_in_explorer(&p);
                            let m = format!("{}: {}", self.t("診断レポートを作成しました", "Created diagnostic report"), p.display());
                            self.toast(ToastKind::Success, m);
                        }
                        Err(e) => self.toast(ToastKind::Error, e.to_string()),
                    }
                }
                if open_folder {
                    let dir = crate::diag::folder(&self.cfg_path);
                    let _ = std::fs::create_dir_all(&dir);
                    win::open_in_explorer(&dir);
                }
            });
            ui.add_space(8.0);
            widgets::card(ui, |ui| {
                widgets::subtitle(ui, self.t("ショートカット", "Keyboard shortcuts"));
                egui::Grid::new("shortcuts").num_columns(2).spacing([24.0, 4.0]).show(ui, |ui| {
                    // 設定で変更したキーを表示する (「設定」→「キー操作とトレイ」で変更できる)
                    let ja = self.lang.is_ja();
                    let none = self.t("なし", "None").to_string();
                    let key = |spec: &str| if spec.trim().is_empty() { none.clone() } else { crate::shortcuts::display(spec) };
                    let g = &self.loaded.config.gui;
                    let mut rows: Vec<(String, &str)> =
                        crate::shortcuts::Action::ALL.iter().map(|a| (key(a.spec(&g.shortcuts)), a.label(ja))).collect();
                    let hk = if g.hotkey_enabled { key(&g.hotkey) } else { none.clone() };
                    rows.push((hk, self.t("クリップボードをその場でマスク (どのアプリからでも)", "Mask clipboard in place (from any app)")));
                    for (k, v) in rows {
                        ui.label(RichText::new(k).monospace());
                        ui.label(v);
                        ui.end_row();
                    }
                });
            });
            ui.add_space(8.0);
            widgets::card(ui, |ui| {
                widgets::subtitle(ui, self.t("ライセンス・謝辞", "Licenses & acknowledgements"));
                ui.label(self.t(
                    "内蔵の人名・地名辞書は mecab-ipadic (IPADIC ライセンス) と米国国勢調査局の公開データ (パブリックドメイン) から作成しています。使用している Rust ライブラリの一覧はインストール先の THIRD-PARTY-NOTICES.txt を参照してください。",
                    "The built-in name/place dictionaries are derived from mecab-ipadic (IPADIC license) and US Census Bureau data (public domain). See THIRD-PARTY-NOTICES.txt in the install folder for Rust libraries.",
                ));
                egui::CollapsingHeader::new("IPADIC License").show(ui, |ui| {
                    ui.label(RichText::new(include_str!("../../sumiveil-core/data/IPADIC-LICENSE.txt")).monospace().size(11.0));
                });
                // インストール先を開きにくい環境 (パッケージとしてのインストールなど) でも、ここから開けるようにする
                let notices = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("THIRD-PARTY-NOTICES.txt")));
                if let Some(notices) = notices.filter(|p| p.exists()) {
                    if widgets::icon_button(ui, Icon::Report, self.t("THIRD-PARTY-NOTICES.txt を開く", "Open THIRD-PARTY-NOTICES.txt")).clicked() {
                        let _ = std::process::Command::new("notepad.exe").arg(&notices).spawn();
                    }
                }
            });
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.start_hidden {
            if self.tray.is_none() {
                // トレイが無いと操作できなくなるので表示する
                self.start_hidden = false;
                self.show_window(ctx);
            } else if !self.first_frame {
                // 最初の描画後 (eframe が表示した直後) に隠す
                self.start_hidden = false;
                win::hide_window(self.hwnd);
            } else {
                ctx.request_repaint();
            }
        }
        self.handle_tray(ctx);
        self.check_config_file_changed();
        self.batch.poll(ctx);
        // ウィンドウを閉じる → トレイに格納
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.loaded.config.gui.close_to_tray && self.tray.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            win::hide_window(self.hwnd);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.first_frame {
            self.first_frame = false;
            if self.start_hidden {
                ctx.request_repaint();
            }
        } else if self.offscreen && !self.start_hidden && win::is_visible(self.hwnd) {
            // 別の起動 (多重起動防止) などで表示された場合も画面内へ
            self.bring_onscreen(&ctx);
        }
        self.update_caption(&ctx);
        self.follow_system_accent(&ctx);
        if self.shortcut_recording.is_some() && self.page != Page::Settings {
            self.end_recording();
        }
        self.pump_worker(&ctx);
        crate::mask_page::global_shortcuts(self, &ctx);
        crate::mask_page::handle_dropped_files(self, &ctx);

        let p = Palette::current(&ctx);
        egui::CentralPanel::no_frame().show(ui, |ui| {
            ui.painter().rect_filled(ui.max_rect(), CornerRadius::ZERO, p.bg);
            self.nav(ui);
            egui::CentralPanel::no_frame().show(ui, |ui| {
                if let Some(err) = self.config_error.clone() {
                    ui.add_space(4.0);
                    widgets::info_bar(ui, InfoKind::Error, &format!("{}: {err}", self.t("設定ファイルにエラーがあるため既定値で動作しています", "Settings file has errors; running with defaults")));
                }
                match self.page {
                    Page::Mask => crate::mask_page::show(self, ui),
                    Page::Batch => crate::batch_page::show(self, ui),
                    Page::Settings => crate::settings_page::show(self, ui),
                }
            });
        });
        self.toasts_ui(&ctx);
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}
