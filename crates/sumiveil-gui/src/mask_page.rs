//! マスク画面: ツールバー・左右比較エディタ・検出一覧・ステータスバー。

use std::sync::Arc;

use eframe::egui::{
    self, text::LayoutJob, text_selection::CCursorRange, Color32, CornerRadius, FontId, Frame, Galley, Margin, Pos2, RichText,
    ScrollArea, Sense, Stroke, TextBuffer, TextEdit, TextFormat, TextureHandle, Vec2,
};
use sumiveil_core::catalog;
use sumiveil_core::formats::{self, Document, Output, PropertiesMode};
use sumiveil_core::regex::Regex;
use sumiveil_core::report::{build_report, ReportOptions};
use sumiveil_core::search::{self, Query};
use sumiveil_core::{MaskSession, Replacement};

use crate::app::{App, InputInfo, Page, ToastKind};
use crate::icons::Icon;
use crate::shortcuts::Action;
use crate::theme::Palette;
use crate::widgets;

/// ハイライト表示を行うテキストの上限 (これを超えると通常表示)。
const HIGHLIGHT_LIMIT: usize = 4 * 1024 * 1024;

#[derive(Default)]
pub struct MaskUi {
    pub selected: Option<usize>,
    pub filter_cat: Option<String>,
    pub search: String,
    pub include_original_in_report: bool,
    sync: SyncState,
    jump: Option<usize>,
    icon_tex: Option<TextureHandle>,
    /// 保存の前に、文書のプロパティの扱いを選ぶダイアログを表示中
    props_dialog: bool,
    props_remember: bool,
    pub find: FindState,
    /// 狭いウィンドウでも検出一覧を表示する (「⋯」メニューで開いた)
    panel_forced: bool,
    /// 検索の一致箇所へ移動する要求 (右側なら true, 開始, 終了)
    find_jump: Option<(bool, usize, usize)>,
}

/// 検索バーの状態。
#[derive(Default)]
pub struct FindState {
    pub open: bool,
    pub query: Query,
    /// 次のフレームで入力欄にフォーカスする
    focus: bool,
    /// 現在の一致 (0 始まり)
    current: usize,
    /// 検出一覧と連動しない
    unlinked: bool,
    /// (検索条件, 入力の世代, 結果の世代): 変わったときだけ一致を計算し直す
    key: Option<(Query, u64, u64)>,
    re: Option<Regex>,
    error: Option<String>,
    left: Vec<(usize, usize)>,
    right: Vec<(usize, usize)>,
    truncated: bool,
    /// 開いた直後に移動する一致の位置 (一括処理の横断検索から開いたとき)
    pending_start: Option<usize>,
}

impl FindState {
    pub fn open_and_focus(&mut self) {
        self.open = true;
        self.focus = true;
    }

    /// 移動の対象 (元のテキストに一致がなければマスク後)。
    fn primary(&self) -> (bool, &[(usize, usize)]) {
        if self.left.is_empty() && !self.right.is_empty() {
            (true, &self.right)
        } else {
            (false, &self.left)
        }
    }

    /// 検出一覧の絞り込みに使う条件 (連動中で、検索語があるとき)。
    fn list_filter(&self) -> Option<&Regex> {
        (self.open && !self.unlinked).then_some(self.re.as_ref()).flatten()
    }
}

#[derive(Default)]
struct SyncState {
    prev_left: Vec2,
    prev_right: Vec2,
    force_left: Option<Vec2>,
    force_right: Option<Vec2>,
    left_tops: Vec<f32>,
    right_tops: Vec<f32>,
    left_view_h: f32,
    left_view_w: f32,
    right_view_w: f32,
    right_view_h: f32,
}

impl MaskUi {
    pub fn icon_texture(&mut self, ctx: &egui::Context) -> TextureHandle {
        self.icon_tex
            .get_or_insert_with(|| {
                let img = egui::ColorImage::from_rgba_unmultiplied([128, 128], &crate::icon::render(128));
                ctx.load_texture("sumiveil-icon", img, egui::TextureOptions::LINEAR)
            })
            .clone()
    }
}

// ───────────────────────── 操作 ─────────────────────────

pub fn open_dialog(app: &mut App) {
    // 既定の対象 (batch.include の拡張子) + メール・Office 文書・PDF
    let mut exts: Vec<String> = sumiveil_core::config::DEFAULT_BATCH_INCLUDE.iter().filter_map(|p| p.strip_prefix("*.")).map(str::to_string).collect();
    for e in formats::DOCUMENT_EXTENSIONS {
        if !exts.iter().any(|x| x == e) {
            exts.push(e.to_string());
        }
    }
    let mut dlg = rfd::FileDialog::new().set_title(app.t("ファイルを開く", "Open file"));
    dlg = dlg.add_filter(app.t("対応しているファイル", "Supported files"), &exts);
    dlg = dlg.add_filter(app.t("Office 文書 (Word / Excel / PowerPoint)", "Office documents"), &["docx", "docm", "xlsx", "xlsm", "pptx", "pptm"]);
    dlg = dlg.add_filter(app.t("メール (.eml / .msg)", "Email (.eml / .msg)"), &["eml", "msg"]);
    dlg = dlg.add_filter("PDF", &["pdf"]);
    dlg = dlg.add_filter(app.t("すべてのファイル", "All files"), &["*"]);
    if let Some(p) = dlg.pick_file() {
        app.open_file(&p);
    }
}

pub fn paste_replace(app: &mut App) {
    match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
        Ok(t) => app.set_input(t, InputInfo { encoding_name: "UTF-8".into(), ..Default::default() }),
        Err(_) => {
            let m = app.t("クリップボードにテキストがありません", "No text in clipboard").to_string();
            app.toast(ToastKind::Error, m);
        }
    }
}

pub fn copy_output(app: &mut App) {
    let Some(d) = &app.done else { return };
    let text = d.result.output.clone();
    app.ctx.copy_text(text);
    let m = app.t("マスク結果をコピーしました", "Copied the masked text").to_string();
    app.toast(ToastKind::Success, m);
}

pub fn save_output(app: &mut App) {
    let Some(d) = &app.done else { return };
    if let Some(doc) = app.input_info.doc.clone() {
        // Office 文書のプロパティの扱いが決まっていなければ、先に選んでもらう
        if doc.kind.has_properties() && !doc.properties.is_empty() && app.properties_mode().is_none() {
            app.mask.props_dialog = true;
            return;
        }
        return save_document(app, &doc);
    }
    let output = d.result.output.clone();
    let default_name = match &app.input_info.path {
        Some(p) => {
            let stem = p.file_stem().unwrap_or_default().to_string_lossy();
            match p.extension() {
                Some(e) => format!("{stem}{}.{}", app.cfg().batch.suffix, e.to_string_lossy()),
                None => format!("{stem}{}", app.cfg().batch.suffix),
            }
        }
        None => "masked.txt".to_string(),
    };
    let mut dlg = rfd::FileDialog::new().set_title(app.t("マスク結果を保存", "Save masked text")).set_file_name(&default_name);
    if let Some(dir) = app.input_info.path.as_ref().and_then(|p| p.parent()) {
        dlg = dlg.set_directory(dir);
    }
    if let Some(path) = dlg.save_file() {
        let enc = app.input_info.encoding.unwrap_or(sumiveil_core::encoding_rs::UTF_8);
        let bytes = sumiveil_core::encoding::encode(&output, enc, app.input_info.bom);
        match std::fs::write(&path, bytes) {
            Ok(()) => {
                let m = format!("{}: {} ({})", app.t("保存しました", "Saved"), path.display(), sumiveil_core::encoding::display_name(enc, app.input_info.bom));
                app.toast(ToastKind::Success, m);
            }
            Err(e) => app.toast(ToastKind::Error, format!("{}: {e}", path.display())),
        }
    }
}

fn save_report(app: &mut App) {
    let Some(d) = &app.done else { return };
    let rep = build_report(
        &d.text,
        &d.result,
        &ReportOptions {
            source: app.input_info.path.as_ref().map(|p| p.display().to_string()),
            encoding: Some(app.input_info.encoding_name.clone()),
            profile: &app.loaded.active_profile,
            include_original: app.mask.include_original_in_report,
            include_masked: true,
            japanese_names: app.lang.is_ja(),
        },
    );
    let Ok(json) = serde_json::to_string_pretty(&rep) else { return };
    if let Some(path) = rfd::FileDialog::new().set_file_name("sumiveil-report.json").add_filter("JSON", &["json"]).save_file() {
        match std::fs::write(&path, json + "\n") {
            Ok(()) => {
                let m = format!("{}: {}", app.t("レポートを保存しました", "Saved report"), path.display());
                app.toast(ToastKind::Success, m);
            }
            Err(e) => app.toast(ToastKind::Error, e.to_string()),
        }
    }
}

/// メール・Office 文書を元の形式で、PDF / .msg をテキストで保存する。
fn save_document(app: &mut App, doc: &Arc<Document>) {
    let suffix = app.cfg().batch.suffix.clone();
    let default_name = match &app.input_info.path {
        Some(p) => {
            let stem = p.file_stem().unwrap_or_default().to_string_lossy();
            let name = match p.extension() {
                Some(e) => format!("{stem}{suffix}.{}", e.to_string_lossy()),
                None => format!("{stem}{suffix}"),
            };
            if doc.kind.writes_text() {
                formats::text_output_name(&name)
            } else {
                name
            }
        }
        None => "masked.txt".to_string(),
    };
    let mut dlg = rfd::FileDialog::new().set_title(app.t("マスク結果を保存", "Save masked file")).set_file_name(&default_name);
    if let Some(dir) = app.input_info.path.as_ref().and_then(|p| p.parent()) {
        dlg = dlg.set_directory(dir);
    }
    let Some(path) = dlg.save_file() else { return };
    if app.input_info.path.as_ref().and_then(|p| p.canonicalize().ok()) == path.canonicalize().ok() {
        let m = app.t("元のファイルには上書きできません。別の名前で保存してください", "Cannot overwrite the original file. Choose another name").to_string();
        return app.toast(ToastKind::Error, m);
    }
    // 画面と同じ条件 (一時的な除外も含む) でマスクし直し、同じ連番のままプロパティも処理する
    let engine = app.engine.clone();
    let mut session = MaskSession::new();
    let dets = engine.detect_excluding(&doc.text, Some(&app.excluded));
    let result = engine.apply(&doc.text, &dets, &mut session);
    let properties = app.properties_mode().unwrap_or(PropertiesMode::Clear);
    let mut opts = formats::WriteOptions { engine: &engine, session: &mut session, properties, text_encoding: None };
    let written = formats::write(doc, &result, &mut opts).and_then(|(out, rep)| {
        let bytes = match out {
            Output::Bytes(b) => b,
            Output::Text(t) => t.into_bytes(),
        };
        std::fs::write(&path, bytes).map_err(|e| e.to_string()).map(|_| rep)
    });
    match written {
        Ok(rep) => {
            let props = match rep.properties {
                Some("cleared") => app.t(" (プロパティは消去)", " (properties cleared)"),
                Some("masked") => app.t(" (プロパティはマスク)", " (properties masked)"),
                Some("kept") => app.t(" (プロパティはそのまま)", " (properties kept)"),
                _ => "",
            };
            let m = format!("{}: {}{props}", app.t("保存しました", "Saved"), path.display());
            app.toast(ToastKind::Success, m);
            for w in rep.warnings {
                app.toast(ToastKind::Info, w);
            }
        }
        Err(e) => app.toast(ToastKind::Error, format!("{}: {e}", path.display())),
    }
}

/// 文書を開いているときの状態バー (1 行)。形式・読み取り専用・注意・プロパティの扱い。
/// 詳しい内容は、押すと開くポップアップで見せる (縦に積み重ねない)。
fn state_bar(app: &mut App, ui: &mut egui::Ui) {
    let Some(doc) = app.input_info.doc.clone() else { return };
    let p = Palette::current(ui.ctx());
    let ja = app.lang.is_ja();
    let small = |s: &str, c: Color32| RichText::new(s).size(13.0).color(c);
    Frame::new().fill(p.accent.gamma_multiply(0.10)).corner_radius(CornerRadius::same(4)).inner_margin(Margin::symmetric(10, 2)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let how = if doc.kind.writes_text() {
                app.t("保存するとマスクしたテキスト (.txt) になります", "Saved as masked text (.txt)")
            } else {
                app.t("保存すると同じ形式で書き出します", "Saved in the same format")
            };
            let head = if ja { format!("{} · 読み取り専用", doc.kind.label(true)) } else { format!("{} · read-only", doc.kind.label(false)) };
            widgets::icon_text(ui, Icon::Document, &head, p.text).on_hover_text(format!(
                "{}\n{how}",
                app.t("本文を取り出して表示しています (編集はできません)", "Showing the extracted text (read-only)")
            ));
            if !doc.warnings.is_empty() {
                ui.label(small("·", p.text_secondary));
                let label = if ja { format!("注意 {} 件", doc.warnings.len()) } else { format!("{} notes", doc.warnings.len()) };
                widgets::flyout_button(ui, widgets::icon_label(ui, Icon::Warning, &format!("{label} ▾"), p.caution), 420.0, |ui| {
                    for w in &doc.warnings {
                        widgets::icon_text(ui, Icon::Warning, w, p.text);
                        ui.add_space(2.0);
                    }
                });
            }
            if doc.kind.has_properties() && !doc.properties.is_empty() {
                ui.label(small("·", p.text_secondary));
                let setting = PropertiesMode::parse(&app.cfg().files.properties);
                let state = match setting.or(app.input_info.props_choice) {
                    Some(m) => mode_label(m, ja),
                    None => app.t("保存時に確認", "ask when saving"),
                };
                let label = format!("{}: {state} ▾", app.t("プロパティ", "Properties"));
                let color = if setting.or(app.input_info.props_choice).is_none() { p.accent } else { p.text };
                let mut chosen = None;
                let mut remember = app.mask.props_remember;
                widgets::flyout_button(ui, widgets::icon_label(ui, Icon::Tag, &label, color), 440.0, |ui| {
                    ui.label(RichText::new(app.t("文書のプロパティ (作成者など)", "Document properties (author, etc.)")).strong());
                    crate::props_prompt::list(ui, &doc.properties);
                    ui.add_space(6.0);
                    if setting.is_none() {
                        widgets::secondary(ui, app.t("保存するときの扱いを選んでください", "Choose what to do when saving"));
                        chosen = crate::props_prompt::buttons(ui, ja, &mut remember);
                    } else {
                        widgets::secondary(ui, app.t("扱いは 設定 →「ファイル」で変更できます", "Change this in Settings → Files"));
                    }
                });
                app.mask.props_remember = remember;
                if let Some(m) = chosen {
                    apply_props_choice(app, m);
                }
            }
        });
    });
    ui.add_space(6.0);
}

fn mode_label(m: PropertiesMode, ja: bool) -> &'static str {
    match (m, ja) {
        (PropertiesMode::Mask, true) => "マスクする",
        (PropertiesMode::Clear, true) => "消す",
        (PropertiesMode::Keep, true) => "そのまま残す",
        (PropertiesMode::Mask, false) => "mask",
        (PropertiesMode::Clear, false) => "clear",
        (PropertiesMode::Keep, false) => "keep",
    }
}

/// プロパティの扱いを決める (「今後もこの方法にする」なら設定に保存)。
fn apply_props_choice(app: &mut App, m: PropertiesMode) {
    app.input_info.props_choice = Some(m);
    if app.mask.props_remember {
        app.mask.props_remember = false;
        app.set_base("files.properties", m.as_str());
    }
}

/// 設定 (`gui.shortcuts`) のショートカットを実行する。キーの登録中は何もしない。
pub fn global_shortcuts(app: &mut App, ctx: &egui::Context) {
    if app.shortcut_recording.is_some() {
        return;
    }
    let pressed: Vec<Action> = app.shortcuts.iter().filter(|(_, sc)| ctx.input_mut(|i| i.consume_shortcut(sc))).map(|(a, _)| *a).collect();
    for a in pressed {
        match a {
            Action::Open => open_dialog(app),
            Action::Paste => paste_replace(app),
            Action::Copy => copy_output(app),
            Action::Save => save_output(app),
            Action::Run => app.request_mask(),
            Action::Settings => app.page = Page::Settings,
            Action::Find => {
                app.page = Page::Mask;
                app.mask.find.open_and_focus();
            }
        }
    }
}

/// ツールチップ用: 操作に割り当てたキー (なしなら空)。
fn key_hint(app: &App, a: Action) -> String {
    crate::shortcuts::display(a.spec(&app.loaded.config.gui.shortcuts))
}

trait HoverKey {
    /// キーが割り当てられていればツールチップに出す (「なし」なら出さない)。
    fn on_hover_key(self, key: String) -> Self;
}

impl HoverKey for egui::Response {
    fn on_hover_key(self, key: String) -> Self {
        if key.is_empty() {
            self
        } else {
            self.on_hover_text(key)
        }
    }
}

pub fn handle_dropped_files(app: &mut App, ctx: &egui::Context) {
    let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
    if let Some(p) = dropped.first() {
        if p.is_dir() {
            app.batch.input_dir = p.display().to_string();
            app.page = Page::Batch;
        } else {
            app.open_file(p);
        }
    }
    // ドラッグ中の表示
    if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
        let p = Palette::current(ctx);
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop")));
        let rect = ctx.content_rect();
        painter.rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(90));
        painter.rect_stroke(rect.shrink(12.0), CornerRadius::same(12), Stroke::new(2.0, p.accent), egui::StrokeKind::Inside);
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, app.t("ドロップして開く", "Drop to open"), FontId::proportional(24.0), Color32::WHITE);
    }
}

// ───────────────────────── 画面 ─────────────────────────

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let p = Palette::current(ui.ctx());
    update_find(app);
    apply_pending_find(app);
    egui::Panel::top("mask-commandbar")
        .frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 8, right: 12, top: 8, bottom: 8 }))
        .show_separator_line(false)
        .show(ui, |ui| command_bar(app, ui));
    egui::Panel::bottom("mask-status")
        .frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 12, right: 12, top: 4, bottom: 6 }))
        .show_separator_line(false)
        .show(ui, |ui| status_bar(app, ui));
    // 狭いウィンドウでは検出一覧を自動で閉じる (コマンドバーの「⋯」から開ける)
    let narrow = ui.available_width() < 960.0;
    let show_panel = app.cfg().gui.show_detection_panel && (!narrow || app.mask.panel_forced);
    if show_panel {
        egui::Panel::right("mask-detections")
            .default_size(320.0)
            .size_range(240.0..=560.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 8, right: 12, top: 0, bottom: 4 }))
            .show(ui, |ui| detection_panel(app, ui));
    }
    egui::CentralPanel::no_frame().show(ui, |ui| {
        Frame::new().inner_margin(Margin { left: 8, right: 8, top: 0, bottom: 4 }).show(ui, |ui| {
            state_bar(app, ui);
            find_bar(app, ui);
            editors(app, ui);
        });
    });
    // 保存の前に、文書のプロパティの扱いを選ぶ
    if app.mask.props_dialog {
        let ja = app.lang.is_ja();
        let props = app.input_info.doc.as_ref().map(|d| d.properties.clone()).unwrap_or_default();
        let mut remember = app.mask.props_remember;
        let r = crate::props_prompt::dialog(ui.ctx(), "save-props", ja, &props, &mut remember);
        app.mask.props_remember = remember;
        if let Some(choice) = r {
            app.mask.props_dialog = false;
            if let Some(m) = choice {
                apply_props_choice(app, m);
                save_output(app);
            }
        }
    }
}

/// 上部の操作列。左によく使う操作、右にプロファイルと主操作 (結果をコピー ▾)。
fn command_bar(app: &mut App, ui: &mut egui::Ui) {
    let compact = ui.available_width() < 900.0;
    let ja = app.lang.is_ja();
    let has = app.done.is_some();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if widgets::command_button(ui, Icon::Open, app.t("開く", "Open"), true, compact).on_hover_key(key_hint(app, Action::Open)).clicked() {
            open_dialog(app);
        }
        if widgets::command_button(ui, Icon::Paste, app.t("貼り付け", "Paste"), true, compact).on_hover_key(key_hint(app, Action::Paste)).clicked() {
            paste_replace(app);
        }
        if widgets::command_toggle(ui, Icon::Find, app.t("検索", "Find"), app.mask.find.open, compact).on_hover_key(key_hint(app, Action::Find)).clicked() {
            if app.mask.find.open {
                app.mask.find.open = false;
            } else {
                app.mask.find.open_and_focus();
            }
        }
        // 自動マスクがオフのときだけ「マスク実行」を出す (更新待ちならアクセント色)
        if !app.cfg().gui.auto_mask {
            let label = app.t("マスク実行", "Run");
            let r = if app.is_stale() && !app.input.is_empty() { widgets::accent_button(ui, Icon::Play, label) } else { widgets::command_button(ui, Icon::Play, label, true, compact) };
            if r.on_hover_key(key_hint(app, Action::Run)).clicked() {
                app.request_mask();
            }
        }
        // その他 (⋯)
        let more = egui::Button::new(widgets::icon_label(ui, Icon::More, "", ui.visuals().text_color())).frame_when_inactive(false).min_size(Vec2::new(32.0, 32.0));
        egui::containers::menu::MenuButton::from_button(more).ui(ui, |ui| {
            ui.set_min_width(240.0);
            let color = ui.visuals().text_color();
            if ui.add_enabled(!app.input.is_empty(), egui::Button::new(widgets::icon_label(ui, Icon::Clear, app.t("クリア", "Clear"), color))).clicked() {
                app.set_input(String::new(), InputInfo { encoding_name: "UTF-8".into(), ..Default::default() });
                app.done = None;
                ui.close();
            }
            if ui.button(widgets::icon_label(ui, Icon::Refresh, app.t("マスクを再実行", "Run masking again"), color)).clicked() {
                app.request_mask();
                ui.close();
            }
            ui.separator();
            let mut show = app.cfg().gui.show_detection_panel && (ui.ctx().content_rect().width() >= 960.0 || app.mask.panel_forced);
            if ui.checkbox(&mut show, app.t("検出一覧を表示", "Show detection list")).changed() {
                app.mask.panel_forced = show;
                if show != app.cfg().gui.show_detection_panel {
                    app.set_base("gui.show_detection_panel", show);
                }
            }
        })
        .0
        .on_hover_text(app.t("その他の操作", "More"));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let copy = widgets::split_button(ui, Icon::Copy, app.t("結果をコピー", "Copy result"), has, compact, |ui| {
                let color = ui.visuals().text_color();
                let save = format!("{}   {}", app.t("ファイルに保存…", "Save to file…"), key_hint(app, Action::Save));
                if ui.add_enabled(has, egui::Button::new(widgets::icon_label(ui, Icon::Save, &save, color))).clicked() {
                    save_output(app);
                    ui.close();
                }
                if ui.add_enabled(has, egui::Button::new(widgets::icon_label(ui, Icon::Report, app.t("検出レポートを保存 (JSON)…", "Save detection report (JSON)…"), color))).clicked() {
                    save_report(app);
                    ui.close();
                }
                let label = app.t("レポートに元の値を含める", "Include original values in report");
                ui.checkbox(&mut app.mask.include_original_in_report, label);
            });
            if copy.on_hover_key(key_hint(app, Action::Copy)).clicked() {
                copy_output(app);
            }
            ui.add_space(8.0);
            profile_picker(app, ui, compact, ja);
        });
    });
}

/// プロファイルの選択 (コマンドバーの右)。
fn profile_picker(app: &mut App, ui: &mut egui::Ui, compact: bool, ja: bool) {
    let current = app.loaded.active_profile.clone();
    let profiles = app.loaded.profiles.clone();
    let name = if current == "default" { app.t("既定", "Default").to_string() } else { current.clone() };
    let label = if compact { name } else if ja { format!("プロファイル: {name}") } else { format!("Profile: {name}") };
    let mut chosen: Option<String> = None;
    egui::ComboBox::from_id_salt("profile")
        .selected_text(label)
        .width(if compact { 110.0 } else { 200.0 })
        .show_ui(ui, |ui| {
            for pr in &profiles {
                let n = if pr.name == "default" { app.t("既定", "Default").to_string() } else { pr.name.clone() };
                let r = ui.selectable_label(pr.name == current, n);
                let r = if pr.description.is_empty() { r } else { r.on_hover_text(&pr.description) };
                if r.clicked() {
                    chosen = Some(pr.name.clone());
                }
            }
        })
        .response
        .on_hover_text(app.t("プロファイル (用途ごとの設定のセット)", "Profile (a set of settings for a purpose)"));
    if let Some(c) = chosen.filter(|c| *c != current) {
        app.set_base("general.active_profile", c.as_str());
    }
}

/// ステータスバー: 左に形式・行数・文字数・ファイル名 (フルパスはホバー)、右に処理時間と注意。
fn status_bar(app: &mut App, ui: &mut egui::Ui) {
    let p = Palette::current(ui.ctx());
    let ja = app.lang.is_ja();
    ui.horizontal(|ui| {
        let small = |s: String| RichText::new(s).size(12.0).color(p.text_secondary);
        let lines = app.input.matches('\n').count() + usize::from(!app.input.is_empty());
        let chars = app.input.chars().count();
        // 文書は形式名 (表示言語に合わせる)、テキストは文字コード
        let format = match &app.input_info.doc {
            Some(d) => d.kind.label(ja).to_string(),
            None => app.input_info.encoding_name.clone(),
        };
        ui.label(small(format));
        ui.label(small("·".into()));
        ui.label(small(if ja { format!("{lines} 行 · {chars} 文字") } else { format!("{lines} lines · {chars} chars") }));
        let path = app.input_info.path.clone();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(d) = &app.done {
                ui.label(small(format!("{:.1} ms", d.elapsed.as_secs_f64() * 1000.0)));
            }
            if app.is_stale() && !app.input.is_empty() {
                ui.label(small("·".into()));
                ui.label(RichText::new(app.t("更新待ち", "Pending")).size(12.0).color(p.caution));
            }
            if !app.manual_terms.is_empty() {
                ui.label(small("·".into()));
                let n = app.manual_terms.len();
                let list: Vec<&str> = app.manual_terms.iter().map(|t| t.text.as_str()).collect();
                ui.label(RichText::new(if ja { format!("手動で指定 {n}") } else { format!("{n} manual") }).size(12.0).color(p.accent)).on_hover_text(list.join("\n"));
            }
            if !app.engine.warnings.is_empty() {
                ui.label(small("·".into()));
                let w = app.engine.warnings.join("\n");
                ui.label(RichText::new(format!("⚠ {}", app.engine.warnings.len())).size(12.0).color(p.caution)).on_hover_text(w);
            }
            // ファイル名 (残りの幅で省略表示。フルパスはホバーで)
            if let Some(path) = path {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.label(small("·".into()));
                    ui.add(egui::Label::new(small(name)).truncate()).on_hover_text(path.display().to_string());
                });
            }
        });
    });
}

// ───────────────────────── 検索 ─────────────────────────

/// 検索条件・本文・結果が変わったときだけ、一致箇所を計算し直す。
fn update_find(app: &mut App) {
    let f = &mut app.mask.find;
    if !f.open || f.query.is_empty() {
        f.re = None;
        f.error = None;
        f.left.clear();
        f.right.clear();
        f.key = None;
        return;
    }
    let done_gen = app.done.as_ref().map_or(0, |d| d.gen);
    let key = (f.query.clone(), app.gen, done_gen);
    if f.key.as_ref() == Some(&key) {
        return;
    }
    f.key = Some(key);
    match f.query.compile() {
        Ok(re) => {
            f.error = None;
            f.re = re;
        }
        Err(e) => {
            f.error = Some(e);
            f.re = None;
        }
    }
    const LIMIT: usize = 10_000;
    let (left, lt) = f.re.as_ref().map(|re| search::find_all(re, &app.input, LIMIT)).unwrap_or_default();
    let (right, rt) = match (&f.re, &app.done) {
        (Some(re), Some(d)) => search::find_all(re, &d.result.output, LIMIT),
        _ => (vec![], false),
    };
    f.left = left;
    f.right = right;
    f.truncated = lt || rt;
    let n = f.primary().1.len();
    if f.current >= n {
        f.current = 0;
    }
}

/// ファイルを開き、検索バーに `query` を入れて、`start` の位置の一致へ移動する (横断検索の結果から)。
pub fn open_at(app: &mut App, path: &std::path::Path, query: Query, start: usize) {
    app.open_file(path);
    if app.input_info.path.as_deref() != Some(path) {
        return; // 開けなかった (エラーはトースト済み)
    }
    let f = &mut app.mask.find;
    f.query = query;
    f.open = true;
    f.current = 0;
    f.pending_start = Some(start);
}

/// 起動時 (`--find`) の検索: 検索バーを開いて文字列を入れ、最初の一致へ移動する。
pub fn find_on_start(app: &mut App, text: String) {
    let f = &mut app.mask.find;
    f.query = Query { text, ..Default::default() };
    f.open = true;
    f.current = 0;
    f.pending_start = Some(0);
}

/// 開いた直後の移動: 新しい本文の行位置 (エディタの配置) が分かってから移動する。
fn apply_pending_find(app: &mut App) {
    let Some(start) = app.mask.find.pending_start else { return };
    // 左右とも新しい本文の配置が済むまで待つ (右側はマスクの完了後。右側も対応する行へ合わせるため)
    let lines = app.input.matches('\n').count() + 1;
    let out_lines = app.done.as_ref().filter(|d| *d.text == app.input).map(|d| d.result.output.matches('\n').count() + 1);
    let right_ok = match out_lines {
        Some(n) => app.mask.sync.right_tops.len() == n,
        // 自動マスクがオフなら結果は来ないので、左側だけで移動する
        None => !app.cfg().gui.auto_mask,
    };
    let ready = app.mask.sync.left_tops.len() == lines && right_ok;
    if !ready {
        app.ctx.request_repaint();
        return;
    }
    let f = &mut app.mask.find;
    f.pending_start = None;
    if let Some(i) = f.left.iter().position(|h| h.0 >= start) {
        f.current = i;
        let (s, e) = f.left[i];
        app.mask.find_jump = Some((false, s, e));
    }
}

/// 一致箇所へ移動する (dir: +1 次 / -1 前 / 0 現在)。
fn find_step(app: &mut App, dir: i32) {
    let f = &mut app.mask.find;
    let (right, hits) = f.primary();
    if hits.is_empty() {
        return;
    }
    let n = hits.len() as i32;
    let cur = (f.current as i32 + dir).rem_euclid(n) as usize;
    let (s, e) = hits[cur];
    f.current = cur;
    app.mask.find_jump = Some((right, s, e));
}

/// 検索バー (開いているときだけ、1 行。狭いときは折り返す)。
fn find_bar(app: &mut App, ui: &mut egui::Ui) {
    if !app.mask.find.open {
        return;
    }
    let p = Palette::current(ui.ctx());
    let ja = app.lang.is_ja();
    let input_id = egui::Id::new("find-input");
    let mut step: Option<i32> = None;
    let mut close = false;
    Frame::new().fill(p.card).stroke(Stroke::new(1.0, p.card_stroke)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let compact = ui.available_width() < 640.0;
            ui.spacing_mut().item_spacing.x = 6.0;
            widgets::icon_text(ui, Icon::Find, "", p.text_secondary);
            let f = &mut app.mask.find;
            let stroke = if f.error.is_some() { Stroke::new(1.0, p.critical) } else { Stroke::NONE };
            let r = Frame::new().stroke(stroke).corner_radius(CornerRadius::same(4)).show(ui, |ui| {
                ui.add(egui::TextEdit::singleline(&mut f.query.text).id(input_id).hint_text(if ja { "検索する文字列" } else { "Find" }).desired_width(if compact { 150.0 } else { 220.0 }))
            });
            let te = r.inner;
            if f.focus {
                te.request_focus();
                f.focus = false;
            }
            if te.changed() {
                f.current = 0;
                step = Some(0);
            }
            // Enter: 次へ / Shift+Enter: 前へ (入力欄にフォーカスがあるとき)
            if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                step = Some(if ui.input(|i| i.modifiers.shift) { -1 } else { 1 });
                te.request_focus();
            }
            let tg = |ui: &mut egui::Ui, on: &mut bool, text: &str, tip: &str| {
                let b = egui::Button::new(RichText::new(text).monospace().size(13.0)).min_size(Vec2::new(30.0, 28.0));
                let b = if *on { b.fill(p.accent.gamma_multiply(0.25)).stroke(Stroke::new(1.0, p.accent)) } else { b.frame_when_inactive(false) };
                let r = ui.add(b).on_hover_text(tip);
                if r.clicked() {
                    *on = !*on;
                }
                r.clicked()
            };
            let c1 = tg(ui, &mut f.query.case_sensitive, "Aa", if ja { "大文字と小文字を区別する" } else { "Match case" });
            let c2 = tg(ui, &mut f.query.regex, ".*", if ja { "正規表現を使う" } else { "Use regular expression" });
            if c1 || c2 {
                f.current = 0;
                step = Some(0);
            }
            // 件数
            let (count, color) = match (&f.error, f.primary().1.len()) {
                (Some(_), _) => (app_t(ja, "正規表現が正しくありません", "Invalid regular expression"), p.critical),
                (None, _) if f.query.is_empty() => (String::new(), p.text_secondary),
                (None, 0) => (app_t(ja, "見つかりません", "No results"), p.text_secondary),
                (None, n) => {
                    let total = if f.truncated { format!("{n}+") } else { n.to_string() };
                    (if ja { format!("{} / {total} 件", f.current + 1) } else { format!("{} of {total}", f.current + 1) }, p.text_secondary)
                }
            };
            let lbl = ui.label(RichText::new(count).size(12.5).color(color));
            if let Some(e) = &f.error {
                lbl.on_hover_text(e);
            }
            let has = !f.primary().1.is_empty();
            let color = p.text;
            if ui.add_enabled(has, egui::Button::new(widgets::icon_label(ui, Icon::Up, "", color)).frame_when_inactive(false)).on_hover_text(if ja { "前へ (Shift+Enter / Shift+F3)" } else { "Previous (Shift+Enter / Shift+F3)" }).clicked() {
                step = Some(-1);
            }
            if ui.add_enabled(has, egui::Button::new(widgets::icon_label(ui, Icon::Down, "", color)).frame_when_inactive(false)).on_hover_text(if ja { "次へ (Enter / F3)" } else { "Next (Enter / F3)" }).clicked() {
                step = Some(1);
            }
            ui.add_space(6.0);
            // 見つけた文字列をマスク対象にする
            let can_mask = f.re.is_some() && f.error.is_none();
            let query = f.query.clone();
            let linked = !f.unlinked;
            let mask_label = app_t(ja, "マスク対象にする", "Mask this");
            let mask_text = if compact { "▾".to_string() } else { format!("{mask_label} ▾") };
            let mask_btn = egui::Button::new(widgets::icon_label(ui, Icon::Veil, &mask_text, color));
            let n_manual = app.manual_terms.len();
            let mut action: Option<u8> = None;
            {
                let (r, _) = egui::containers::menu::MenuButton::from_button(mask_btn).ui(ui, |ui| {
                    ui.set_min_width(300.0);
                    if ui.add_enabled(can_mask, egui::Button::new(app_t(ja, "今だけマスクする (このアプリを閉じるまで)", "Mask for now (until Sumiveil closes)"))).clicked() {
                        action = Some(1);
                        ui.close();
                    }
                    if ui.add_enabled(can_mask, egui::Button::new(app_t(ja, "キーワード辞書に登録する (次回以降も)", "Add to keywords (always)"))).clicked() {
                        action = Some(2);
                        ui.close();
                    }
                    ui.separator();
                    let clear = if ja { format!("手動の指定をすべて解除 ({n_manual})") } else { format!("Clear manual terms ({n_manual})") };
                    if ui.add_enabled(n_manual > 0, egui::Button::new(clear)).clicked() {
                        action = Some(3);
                        ui.close();
                    }
                });
                r.on_hover_text(app_t(ja, "見つけた文字列をマスク対象にする", "Mask the found text"));
            }
            let link_label = app_t(ja, "検出一覧も絞り込む", "Filter detection list");
            // 連動の切り替え (アイコン)。連動中は検出一覧の上に「検索と連動中」と表示される
            if widgets::command_toggle(ui, Icon::Link, &link_label, linked, true).clicked() {
                app.mask.find.unlinked = linked;
            }
            match action {
                Some(1) => {
                    if !app.manual_terms.contains(&query) {
                        let mut terms = app.manual_terms.clone();
                        terms.push(query.clone());
                        app.set_manual_terms(terms);
                    }
                    let m = if ja { format!("「{}」を今だけマスクします", query.text) } else { format!("Masking \"{}\" for now", query.text) };
                    app.toast(ToastKind::Success, m);
                }
                Some(2) => {
                    let (groups, rules) = search::register_term(app.cfg(), &query);
                    app.edit_config(|d, _| {
                        let _ = d.set_serialized("keywords", &groups);
                        let _ = d.set_serialized("custom_rules", &rules);
                    });
                    let m = if ja { format!("「{}」をキーワード辞書に登録しました", query.text) } else { format!("Added \"{}\" to keywords", query.text) };
                    app.toast(ToastKind::Success, m);
                }
                Some(3) => app.set_manual_terms(vec![]),
                _ => {}
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(widgets::icon_label(ui, Icon::Close, "", color)).frame_when_inactive(false)).on_hover_text(app_t(ja, "閉じる (Esc)", "Close (Esc)")).clicked() {
                    close = true;
                }
            });
        });
    });
    ui.add_space(6.0);
    // F3 / Shift+F3、Esc
    let (f3, shift, esc) = ui.input(|i| (i.key_pressed(egui::Key::F3), i.modifiers.shift, i.key_pressed(egui::Key::Escape)));
    if f3 {
        step = Some(if shift { -1 } else { 1 });
    }
    if esc && !egui::Popup::is_any_open(ui.ctx()) {
        close = true;
    }
    if close {
        app.mask.find.open = false;
        return;
    }
    if let Some(d) = step {
        update_find(app);
        find_step(app, d);
    }
}

fn app_t(ja: bool, a: &str, b: &str) -> String {
    if ja { a.to_string() } else { b.to_string() }
}

// ───────────────────────── エディタ ─────────────────────────

struct Span {
    start: usize,
    end: usize,
    rep: usize,
}

struct EditorOut {
    offset: Vec2,
    view_h: f32,
    view_w: f32,
    tops: Vec<f32>,
    hovered_rep: Option<usize>,
    clicked_rep: Option<usize>,
    changed: bool,
    rect: egui::Rect,
}

/// 検索の一致箇所 (エディタの強調表示用)。
#[derive(Clone, Copy, Default)]
struct Hits<'a> {
    all: &'a [(usize, usize)],
    current: Option<(usize, usize)>,
}

#[allow(clippy::too_many_arguments)]
fn build_job(text: &str, spans: Option<&[Span]>, hits: Hits, reps: &[Replacement], selected: Option<usize>, font: &FontId, pal: &Palette, wrap_width: f32, wrap: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = if wrap { wrap_width } else { f32::INFINITY };
    let base = TextFormat { font_id: font.clone(), color: pal.text, ..Default::default() };
    let ok = |s: usize, e: usize| s < e && e <= text.len() && text.is_char_boundary(s) && text.is_char_boundary(e);
    // 重ならない検出箇所だけを使う
    let mut spans_ok: Vec<&Span> = vec![];
    for s in spans.unwrap_or(&[]) {
        if ok(s.start, s.end) && spans_ok.last().is_none_or(|p| p.end <= s.start) {
            spans_ok.push(s);
        }
    }
    let hits_ok: Vec<(usize, usize)> = hits.all.iter().copied().filter(|&(s, e)| ok(s, e)).collect();
    // 区切り位置 (検出箇所と一致箇所の両端) で分け、区間ごとに書式を決める
    let mut cuts: Vec<usize> = Vec::with_capacity(2 + 2 * (spans_ok.len() + hits_ok.len()));
    cuts.push(0);
    cuts.push(text.len());
    for s in &spans_ok {
        cuts.extend([s.start, s.end]);
    }
    for &(s, e) in &hits_ok {
        cuts.extend([s, e]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let (mut si, mut hi) = (0, 0);
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        while si < spans_ok.len() && spans_ok[si].end <= a {
            si += 1;
        }
        while hi < hits_ok.len() && hits_ok[hi].1 <= a {
            hi += 1;
        }
        let mut f = base.clone();
        if let Some(s) = spans_ok.get(si).filter(|s| s.start <= a) {
            let (bg, fg) = pal.category_colors(&reps[s.rep].meta.category);
            f.color = fg;
            f.background = bg;
            // 墨の帯: 地の色に加えて、下に同じ系統の細い線を引く (色だけに頼らず見分けられるように)
            f.underline = Stroke::new(1.0, fg);
            if selected == Some(s.rep) {
                f.background = pal.accent.gamma_multiply(if pal.dark { 0.55 } else { 0.35 });
                f.color = pal.text;
                f.underline = Stroke::new(1.5, pal.accent);
            }
        }
        if let Some(&h) = hits_ok.get(hi).filter(|h| h.0 <= a) {
            let cur = hits.current == Some(h);
            f.background = if cur { pal.find_current } else { pal.find };
            f.color = pal.text;
            if cur {
                f.underline = Stroke::new(2.0, pal.accent);
            }
        }
        job.append(&text[a..b], 0.0, f);
    }
    job
}

fn char_to_byte(text: &str, ci: usize) -> usize {
    text.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(text.len())
}

#[allow(clippy::too_many_arguments)]
fn editor(
    ui: &mut egui::Ui,
    id: &str,
    text: &mut dyn TextBuffer,
    spans: Option<&[Span]>,
    hits: Hits,
    reps: &[Replacement],
    selected: Option<usize>,
    force: Option<Vec2>,
    hint: &str,
    font_size: f32,
    wrap: bool,
    line_numbers: bool,
) -> EditorOut {
    let pal = Palette::current(ui.ctx());
    let font = FontId::monospace(font_size);
    let row_h = ui.fonts_mut(|f| f.row_height(&font));
    let char_w = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
    let line_count = text.as_str().matches('\n').count() + 1;
    let digits = line_count.to_string().len().max(3) as f32;
    let gutter_w = if line_numbers { digits * char_w + 20.0 } else { 6.0 };
    let te_id = egui::Id::new(id).with("te");

    let mut sa = ScrollArea::both().id_salt(id).auto_shrink([false, false]);
    if let Some(o) = force {
        sa = sa.scroll_offset(o);
    }
    let mut out = EditorOut { offset: Vec2::ZERO, view_h: 0.0, view_w: 0.0, tops: vec![], hovered_rep: None, clicked_rep: None, changed: false, rect: ui.max_rect() };
    let sa_out = sa.show_viewport(ui, |ui, viewport| {
        out.view_h = viewport.height();
        out.view_w = viewport.width();
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(gutter_w);
            let mut layouter = |ui: &egui::Ui, buf: &dyn TextBuffer, wrap_width: f32| -> Arc<Galley> {
                let job = build_job(buf.as_str(), spans, hits, reps, selected, &font, &pal, wrap_width, wrap);
                ui.fonts_mut(|f| f.layout_job(job))
            };
            let rows = ((viewport.height() / row_h).floor() as usize).max(1);
            let te = TextEdit::multiline(text)
                .id(te_id)
                .font(font.clone())
                .frame(Frame::NONE)
                .margin(Margin::symmetric(4, 4))
                .desired_width(if wrap { ui.available_width() } else { f32::INFINITY })
                .desired_rows(rows)
                .lock_focus(true)
                .hint_text(hint)
                .layouter(&mut layouter);
            let o = te.show(ui);
            let resp = &o.response.response;
            out.changed = resp.changed();

            // 行番号 (左端に固定) と論理行の位置
            let clip = ui.clip_rect();
            let painter = ui.painter();
            if line_numbers {
                let gutter = egui::Rect::from_min_max(clip.left_top(), Pos2::new(clip.left() + gutter_w - 6.0, clip.bottom()));
                painter.rect_filled(gutter, CornerRadius::ZERO, pal.editor_bg);
            }
            let mut new_line = true;
            let mut line_no = 0usize;
            for row in &o.galley.rows {
                if new_line {
                    line_no += 1;
                    out.tops.push(row.pos.y);
                    let y = o.galley_pos.y + row.pos.y;
                    if line_numbers && y + row_h >= clip.top() && y <= clip.bottom() {
                        painter.text(
                            Pos2::new(clip.left() + gutter_w - 12.0, y),
                            egui::Align2::RIGHT_TOP,
                            line_no.to_string(),
                            font.clone(),
                            pal.gutter,
                        );
                    }
                }
                new_line = row.ends_with_newline;
            }

            // 撮影用: 最初の検出箇所の位置
            #[cfg(feature = "guide-capture")]
            if let Some(s) = spans.and_then(|s| s.first()) {
                let ci = text.as_str().get(..s.start).map(|p| p.chars().count()).unwrap_or(0);
                let r = o.galley.pos_from_cursor(egui::text::CCursor::new(ci));
                crate::guide_mark!(ui.ctx(), format!("{id}:span0"), r.translate(o.galley_pos.to_vec2()).expand2(Vec2::new(6.0, 0.0)));
            }
            // ホバー・クリックされた検出箇所
            if let (Some(spans), Some(pos)) = (spans, resp.hover_pos()) {
                let cc = o.galley.cursor_from_pos(pos - o.galley_pos);
                let b = char_to_byte(text.as_str(), cc.index.0);
                let i = spans.partition_point(|s| s.start <= b);
                if i > 0 {
                    let s = &spans[i - 1];
                    if b < s.end {
                        out.hovered_rep = Some(s.rep);
                        if resp.clicked() {
                            out.clicked_rep = Some(s.rep);
                        }
                    }
                }
            }
            if let Some(ri) = out.hovered_rep {
                let r = &reps[ri];
                let lang_ja = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("lang-ja"))).unwrap_or(true);
                resp.clone().on_hover_ui_at_pointer(|ui| {
                    let (bg, fg) = pal.category_colors(&r.meta.category);
                    ui.horizontal(|ui| {
                        widgets::badge(ui, &r.meta.label, bg, fg);
                        ui.label(RichText::new(if lang_ja { &r.meta.name_ja } else { &r.meta.name_en }).strong());
                    });
                    ui.label(RichText::new(format!("{}  →  {}", truncate(&r.original, 60), r.replacement)).monospace());
                    ui.label(RichText::new(format!("{}: {:.0}%", if lang_ja { "信頼度" } else { "Confidence" }, r.confidence * 100.0)).small().color(pal.text_secondary));
                });
            }
            out.rect = resp.rect;
        });
    });
    out.offset = sa_out.state.offset;
    out
}

fn truncate(s: &str, n: usize) -> String {
    let s = s.replace('\n', "⏎");
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s
    }
}

/// 論理行位置の対応を使ってスクロール位置を変換する。
fn map_offset(y: f32, from: &[f32], to: &[f32], map: Option<&[usize]>) -> f32 {
    if from.is_empty() || to.is_empty() {
        return y;
    }
    let l = from.partition_point(|&t| t <= y).saturating_sub(1);
    let next = from.get(l + 1).copied().unwrap_or(from[l] + 20.0);
    let frac = ((y - from[l]) / (next - from[l]).max(1.0)).clamp(0.0, 1.0);
    let r = map.and_then(|m| m.get(l).copied()).unwrap_or(l).min(to.len() - 1);
    let rnext = to.get(r + 1).copied().unwrap_or(to[r] + (next - from[l]));
    to[r] + frac * (rnext - to[r])
}

fn pane_header(ui: &mut egui::Ui, title: &str, extra: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        ui.label(RichText::new(title).font(FontId::new(15.0, egui::FontFamily::Name(crate::fonts::SEMIBOLD.into()))));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), extra);
    });
}

fn editors(app: &mut App, ui: &mut egui::Ui) {
    let pal = Palette::current(ui.ctx());
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("lang-ja"), app.lang.is_ja()));
    let gui = app.cfg().gui.clone();

    // ジャンプ要求 (一覧のクリック) → スクロール位置と選択
    if let (Some(ri), Some(d)) = (app.mask.jump.take(), &app.done) {
        if let Some(r) = d.result.replacements.get(ri) {
            let valid = *d.text == app.input;
            let s = &mut app.mask.sync;
            let ll = d.text[..r.start].matches('\n').count();
            let rl = d.result.output[..r.out_start].matches('\n').count();
            if let Some(&t) = s.left_tops.get(ll) {
                s.force_left = Some(Vec2::new(0.0, (t - s.left_view_h * 0.35).max(0.0)));
                s.prev_left = s.force_left.unwrap();
            }
            if let Some(&t) = s.right_tops.get(rl) {
                s.force_right = Some(Vec2::new(0.0, (t - s.right_view_h * 0.35).max(0.0)));
                s.prev_right = s.force_right.unwrap();
            }
            if valid {
                let te_id = egui::Id::new("left-editor").with("te");
                if let Some(mut st) = egui::text_edit::TextEditState::load(ui.ctx(), te_id) {
                    let a = d.text[..r.start].chars().count();
                    let b = a + r.original.chars().count();
                    st.cursor.set_char_range(Some(CCursorRange::two(egui::text::CCursor::new(a), egui::text::CCursor::new(b))));
                    st.store(ui.ctx(), te_id);
                }
            }
        }
    }

    // 検索の一致箇所へ移動 → その側をスクロールして選択し、反対側も対応する行へ合わせる
    if let Some((right, s, e)) = app.mask.find_jump.take() {
        let text: &str = if right { app.done.as_ref().map_or("", |d| d.result.output.as_str()) } else { app.input.as_str() };
        if e <= text.len() && text.is_char_boundary(s) && text.is_char_boundary(e) {
            let line = text[..s].matches('\n').count();
            let font = FontId::monospace(gui.font_size);
            let (char_w, row_h) = ui.fonts_mut(|f| (f.glyph_width(&font, '0'), f.row_height(&font)));
            let maps = app.done.as_ref().filter(|d| *d.text == app.input);
            let sy = &mut app.mask.sync;
            let (tops, view_h, view_w, cur) = if right { (&sy.right_tops, sy.right_view_h, sy.right_view_w, sy.prev_right) } else { (&sy.left_tops, sy.left_view_h, sy.left_view_w, sy.prev_left) };
            if let Some(&t) = tops.get(line) {
                // すでに見えていればスクロールしない (見えていなければ、上から 35% の位置へ)
                let y = if t >= cur.y && t + row_h <= cur.y + view_h { cur.y } else { (t - view_h * 0.35).max(0.0) };
                // 折り返さない表示では、行頭からの幅を見積もって横にもスクロールする
                let x = if gui.word_wrap {
                    0.0
                } else {
                    let w = |s: &str| s.chars().map(|c| if c.is_ascii() { 1.0 } else { 2.0 }).sum::<f32>() * char_w;
                    let ls = text[..s].rfind('\n').map_or(0, |i| i + 1);
                    let (mx, mw) = (w(&text[ls..s]), w(&text[s..e]));
                    let usable = (view_w - 80.0).max(char_w * 8.0);
                    if mx >= cur.x && mx + mw <= cur.x + usable { cur.x } else { (mx - usable * 0.3).max(0.0) }
                };
                let other = if right {
                    map_offset(y, &sy.right_tops, &sy.left_tops, maps.map(|d| d.right_to_left.as_slice()))
                } else {
                    map_offset(y, &sy.left_tops, &sy.right_tops, maps.map(|d| d.left_to_right.as_slice()))
                };
                let other_x = if right { sy.prev_left.x } else { sy.prev_right.x };
                let (mine, theirs) = (Vec2::new(x, y), Vec2::new(other_x, other));
                let (fm, fo) = if right { (&mut sy.force_right, &mut sy.force_left) } else { (&mut sy.force_left, &mut sy.force_right) };
                *fm = Some(mine);
                if gui.sync_scroll {
                    *fo = Some(theirs);
                }
                if right {
                    sy.prev_right = mine;
                } else {
                    sy.prev_left = mine;
                }
            }
            let te_id = egui::Id::new(if right { "right-editor" } else { "left-editor" }).with("te");
            let mut st = egui::text_edit::TextEditState::load(ui.ctx(), te_id).unwrap_or_default();
            let a = text[..s].chars().count();
            let b = a + text[s..e].chars().count();
            st.cursor.set_char_range(Some(CCursorRange::two(egui::text::CCursor::new(a), egui::text::CCursor::new(b))));
            st.store(ui.ctx(), te_id);
        }
    }

    let done = app.done.as_ref();
    let valid = done.is_some_and(|d| *d.text == app.input) && app.input.len() <= HIGHLIGHT_LIMIT;
    let find = &app.mask.find;
    let (primary_right, primary) = find.primary();
    let cur_hit = primary.get(find.current).copied();
    let left_hits = Hits { all: &find.left, current: cur_hit.filter(|_| !primary_right) };
    let right_hits = Hits { all: &find.right, current: cur_hit.filter(|_| primary_right) };
    let reps: &[Replacement] = done.map(|d| d.result.replacements.as_slice()).unwrap_or(&[]);
    let left_spans: Option<Vec<Span>> = valid.then(|| reps.iter().enumerate().map(|(i, r)| Span { start: r.start, end: r.end, rep: i }).collect());
    let right_spans: Option<Vec<Span>> = done
        .filter(|d| d.result.output.len() <= HIGHLIGHT_LIMIT)
        .map(|_| reps.iter().enumerate().map(|(i, r)| Span { start: r.out_start, end: r.out_end, rep: i }).collect());

    let selected = app.mask.selected;
    let force_l = app.mask.sync.force_left.take();
    let force_r = app.mask.sync.force_right.take();
    let hint = app.t("ここにテキストを貼り付け (Ctrl+V)、またはファイルをドロップ", "Paste text here (Ctrl+V) or drop a file").to_string();
    let t_original = app.t("元のテキスト", "Original").to_string();
    let t_masked = app.t("マスク後", "Masked").to_string();
    let n = reps.len();
    let n_label = if app.lang.is_ja() { format!("{n} 件検出") } else { format!("{n} found") };
    let chars_label = if app.lang.is_ja() { format!("{} 文字", app.input.chars().count()) } else { format!("{} chars", app.input.chars().count()) };

    let mut left_out = None;
    let mut right_out = None;
    let mut input_changed = false;
    let empty = String::new();
    let output_text: &str = done.map(|d| d.result.output.as_str()).unwrap_or(&empty);
    // Office 文書・メール・PDF は書き戻しの対応がずれないよう、左側を編集不可にする (選択・コピーは可能)
    let read_only = app.input_info.doc.is_some();
    let input = &mut app.input;

    ui.columns(2, |cols| {
        // 左: 元のテキスト (編集可)
        pane_header(&mut cols[0], &t_original, |ui| {
            ui.label(RichText::new(&chars_label).size(12.0).color(pal.text_secondary));
        });
        Frame::new()
            .fill(pal.editor_bg)
            .stroke(Stroke::new(1.0, pal.card_stroke))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(1))
            .show(&mut cols[0], |ui| {
                let o = if read_only {
                    let mut ro: &str = input.as_str();
                    editor(ui, "left-editor", &mut ro, left_spans.as_deref(), left_hits, reps, selected, force_l, &hint, gui.font_size, gui.word_wrap, gui.show_line_numbers)
                } else {
                    editor(ui, "left-editor", input, left_spans.as_deref(), left_hits, reps, selected, force_l, &hint, gui.font_size, gui.word_wrap, gui.show_line_numbers)
                };
                input_changed = o.changed;
                left_out = Some(o);
            });
        // 右: マスク後 (読み取り専用・選択可)
        pane_header(&mut cols[1], &t_masked, |ui| {
            if done.is_some() {
                let (bg, fg) = (pal.accent.gamma_multiply(0.2), pal.accent);
                widgets::badge(ui, &n_label, bg, fg);
            }
        });
        Frame::new()
            .fill(pal.editor_bg)
            .stroke(Stroke::new(1.0, pal.card_stroke))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(1))
            .show(&mut cols[1], |ui| {
                let mut ro: &str = output_text;
                let o = editor(ui, "right-editor", &mut ro, right_spans.as_deref(), right_hits, reps, selected, force_r, "", gui.font_size, gui.word_wrap, gui.show_line_numbers);
                right_out = Some(o);
            });
    });

    let (Some(lo), Some(ro)) = (left_out, right_out) else { return };
    if input_changed {
        app.on_input_edited();
    }
    if let Some(r) = lo.clicked_rep.or(ro.clicked_rep) {
        app.mask.selected = Some(r);
        if ro.clicked_rep.is_some() {
            app.mask.jump = Some(r);
        }
    }

    // スクロール同期
    let s = &mut app.mask.sync;
    s.left_tops = lo.tops;
    s.right_tops = ro.tops;
    s.left_view_h = lo.view_h;
    s.right_view_h = ro.view_h;
    s.left_view_w = lo.view_w;
    s.right_view_w = ro.view_w;
    if gui.sync_scroll && force_l.is_none() && force_r.is_none() {
        let maps = app.done.as_ref().filter(|d| *d.text == app.input);
        if lo.offset != s.prev_left {
            let y = map_offset(lo.offset.y, &s.left_tops, &s.right_tops, maps.map(|d| d.left_to_right.as_slice()));
            let target = Vec2::new(lo.offset.x, y);
            s.force_right = Some(target);
            s.prev_right = target;
            ui.ctx().request_repaint();
        } else if ro.offset != s.prev_right {
            let y = map_offset(ro.offset.y, &s.right_tops, &s.left_tops, maps.map(|d| d.right_to_left.as_slice()));
            let target = Vec2::new(ro.offset.x, y);
            s.force_left = Some(target);
            s.prev_left = target;
            ui.ctx().request_repaint();
        }
    }
    if s.force_left.is_none() {
        s.prev_left = lo.offset;
    }
    if s.force_right.is_none() {
        s.prev_right = ro.offset;
    }
    let _ = (lo.rect, ro.rect, lo.hovered_rep, ro.hovered_rep);
}

// ───────────────────────── 検出一覧 ─────────────────────────

fn detection_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = Palette::current(ui.ctx());
    let ja = app.lang.is_ja();
    let n_all = app.done.as_ref().map_or(0, |d| d.result.replacements.len());

    // 見出し: 「検出 N」と種類のドロップダウン
    let mut new_filter: Option<Option<String>> = None;
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        let title = if ja { format!("検出 {n_all}") } else { format!("Detections {n_all}") };
        ui.label(RichText::new(title).font(FontId::new(15.0, egui::FontFamily::Name(crate::fonts::SEMIBOLD.into()))));
        let Some(done) = &app.done else { return };
        let reps = &done.result.replacements;
        let mut counts: Vec<(String, usize)> = vec![];
        for r in reps.iter() {
            match counts.iter_mut().find(|(c, _)| *c == r.meta.category) {
                Some(x) => x.1 += 1,
                None => counts.push((r.meta.category.clone(), 1)),
            }
        }
        let cat_name = |cat: &str| catalog::category(cat).map(|c| if ja { c.name_ja } else { c.name_en }).unwrap_or(cat).to_string();
        let current = match &app.mask.filter_cat {
            Some(c) => cat_name(c),
            None => (if ja { "すべて" } else { "All" }).to_string(),
        };
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::ComboBox::from_id_salt("det-cat")
                .selected_text(if ja { format!("種類: {current}") } else { format!("Type: {current}") })
                .width(150.0)
                .show_ui(ui, |ui| {
                    let all = if ja { format!("すべて ({})", reps.len()) } else { format!("All ({})", reps.len()) };
                    if ui.selectable_label(app.mask.filter_cat.is_none(), all).clicked() {
                        new_filter = Some(None);
                    }
                    for (cat, n) in &counts {
                        let (bg, fg) = pal.category_colors(cat);
                        let sel = app.mask.filter_cat.as_deref() == Some(cat.as_str());
                        let r = ui.horizontal(|ui| {
                            let (dot, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), Sense::hover());
                            ui.painter().rect(dot, CornerRadius::same(3), bg, Stroke::new(1.0, fg), egui::StrokeKind::Inside);
                            ui.selectable_label(sel, format!("{} ({n})", cat_name(cat)))
                        });
                        if r.inner.clicked() {
                            new_filter = Some(if sel { None } else { Some(cat.clone()) });
                        }
                    }
                });
        });
    });
    if let Some(f) = new_filter {
        app.mask.filter_cat = f;
    }

    let Some(done) = &app.done else {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            widgets::secondary(ui, app.t("テキストを入力すると、ここに検出結果が表示されます。", "Detections will appear here once you enter text."));
        });
        return;
    };
    let reps = &done.result.replacements;

    // 絞り込み欄 (検索バーと連動中は、検索語で絞り込む)
    let linked = app.mask.find.list_filter().cloned();
    if linked.is_some() {
        let mut unlink = false;
        Frame::new().fill(pal.accent.gamma_multiply(0.12)).corner_radius(CornerRadius::same(4)).inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let label = if ja { format!("検索と連動中: {}", app.mask.find.query.text) } else { format!("Linked to Find: {}", app.mask.find.query.text) };
                widgets::icon_text_truncated(ui, Icon::Link, &label, pal.text);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let b = egui::Button::new(widgets::icon_label(ui, Icon::Close, "", pal.text_secondary)).frame_when_inactive(false);
                    if ui.add(b).on_hover_text(if ja { "連動をやめる" } else { "Unlink" }).clicked() {
                        unlink = true;
                    }
                });
            });
        });
        if unlink {
            app.mask.find.unlinked = true;
        }
    } else {
        let hint = app.t("絞り込み (値・種類)", "Filter (value / type)").to_string();
        ui.add(egui::TextEdit::singleline(&mut app.mask.search).hint_text(hint).desired_width(f32::INFINITY));
    }
    ui.add_space(4.0);

    let q = app.mask.search.to_lowercase();
    let idx: Vec<usize> = reps
        .iter()
        .enumerate()
        .filter(|(_, r)| app.mask.filter_cat.as_deref().is_none_or(|c| r.meta.category == c))
        .filter(|(_, r)| match &linked {
            Some(re) => re.is_match(&r.original) || re.is_match(&r.replacement),
            None => q.is_empty() || r.original.to_lowercase().contains(&q) || r.meta.name_ja.contains(&q) || r.meta.name_en.to_lowercase().contains(&q) || r.meta.label.to_lowercase().contains(&q),
        })
        .map(|(i, _)| i)
        .collect();
    if idx.len() != reps.len() {
        let t = if ja { format!("{} / {} 件を表示", idx.len(), reps.len()) } else { format!("Showing {} of {}", idx.len(), reps.len()) };
        ui.label(RichText::new(t).size(11.5).color(pal.text_secondary));
    }

    let mut exclude: Option<String> = None;
    let mut select: Option<usize> = None;
    let row_h = 32.0;
    // 一覧の下の「除外中」の高さ (前のフレームで測った値。初回は閉じた状態の目安)
    let excluded_h = if app.excluded_display.is_empty() {
        0.0
    } else {
        ui.ctx().data(|d| d.get_temp::<f32>(egui::Id::new("det-excluded-h"))).unwrap_or(40.0)
    };
    let list_h = (ui.available_height() - excluded_h).max(80.0);
    let text_idx = sumiveil_core::text::LineIndex::new(&done.text);
    let doc_for_list = app.input_info.doc.clone().filter(|d| d.text == *done.text);
    ScrollArea::vertical().id_salt("det-list").max_height(list_h).auto_shrink([false, false]).show_rows(ui, row_h, idx.len(), |ui, range| {
        for &i in &idx[range] {
            let r = &reps[i];
            let (bg, fg) = pal.category_colors(&r.meta.category);
            let selected = app.mask.selected == Some(i);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h - 2.0), Sense::click());
            let fill = if selected { pal.accent.gamma_multiply(0.18) } else if resp.hovered() { pal.control_hover } else { Color32::TRANSPARENT };
            ui.painter().rect_filled(rect, CornerRadius::same(3), fill);
            if selected {
                // 選択中の印 (左端の縦線)
                ui.painter().rect_filled(egui::Rect::from_min_size(rect.min + Vec2::new(0.0, 8.0), Vec2::new(3.0, rect.height() - 16.0)), CornerRadius::ZERO, pal.accent);
            }
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(6.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
            let mut on = true;
            let tip = if ja { "チェックを外すとこの値をマスクしません (一時的)" } else { "Uncheck to keep this value (temporary)" };
            let cb = child.checkbox(&mut on, "").on_hover_text(tip);
            crate::guide_mark!(ui.ctx(), format!("det-check:{}", r.original), cb.rect);
            if cb.changed() && !on {
                exclude = Some(r.original.clone());
            }
            widgets::badge(&mut child, &r.meta.label, bg, fg);
            // 右端の場所を先に置き、値は残りの幅で省略表示する
            child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let place = match doc_for_list.as_ref().and_then(|d| d.location_at(r.start)) {
                    Some(loc) => truncate(loc, 18),
                    None => format!("L{}", text_idx.line_col(&done.text, r.start).0),
                };
                ui.label(RichText::new(place).size(11.5).color(pal.text_secondary));
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let mut job = LayoutJob::default();
                    job.append(&truncate(&r.original, 60), 0.0, TextFormat { font_id: FontId::monospace(12.5), color: pal.text, ..Default::default() });
                    if selected {
                        job.append(&format!("  → {}", truncate(&r.replacement, 30)), 0.0, TextFormat { font_id: FontId::proportional(12.0), color: pal.text_secondary, ..Default::default() });
                    }
                    ui.add(egui::Label::new(job).truncate().selectable(false));
                });
            });
            let resp = resp.on_hover_ui(|ui| {
                ui.label(RichText::new(if ja { &r.meta.name_ja } else { &r.meta.name_en }).strong());
                ui.label(RichText::new(format!("{}  →  {}", truncate(&r.original, 60), r.replacement)).monospace());
                ui.label(RichText::new(format!("{}: {:.0}%", if ja { "信頼度" } else { "Confidence" }, r.confidence * 100.0)).small().color(pal.text_secondary));
            });
            if resp.clicked() {
                select = Some(i);
            }
        }
    });
    if let Some(i) = select {
        app.mask.selected = Some(i);
        app.mask.jump = Some(i);
    }
    if let Some(v) = exclude {
        app.excluded.insert(v.to_lowercase());
        app.excluded_display.push(v);
        app.request_mask();
    }

    // 除外中の値 (実際の高さを測って、次のフレームから一覧の下にその分を空ける)
    if !app.excluded_display.is_empty() {
        let top = ui.cursor().top();
        ui.separator();
        let mut restore: Option<usize> = None;
        let mut allow: Option<usize> = None;
        let hdr = egui::CollapsingHeader::new(if ja { format!("除外中 ({})", app.excluded_display.len()) } else { format!("Kept ({})", app.excluded_display.len()) })
            .id_salt("excluded-hdr")
            .default_open(false)
            .show(ui, |ui| {
                ScrollArea::vertical().id_salt("excluded").max_height(26.0 * 6.0).show(ui, |ui| {
                    for (i, v) in app.excluded_display.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.small_button(if ja { "戻す" } else { "Undo" }).clicked() {
                                restore = Some(i);
                            }
                            if ui.small_button(if ja { "許可リストへ" } else { "Allowlist" }).on_hover_text(if ja { "設定の許可リストに追加して今後もマスクしない" } else { "Add to the allowlist in settings" }).clicked() {
                                allow = Some(i);
                            }
                            ui.label(RichText::new(truncate(v, 40)).monospace().size(12.0));
                        });
                    }
                });
            });
        crate::guide_mark!(ui.ctx(), "excluded", hdr.header_response.rect);
        let used = ui.cursor().top() - top + 4.0;
        let key = egui::Id::new("det-excluded-h");
        let prev: f32 = ui.ctx().data(|d| d.get_temp(key)).unwrap_or(0.0);
        if (used - prev).abs() > 0.5 {
            ui.ctx().data_mut(|d| d.insert_temp(key, used));
            ui.ctx().request_repaint();
        }
        if let Some(i) = restore {
            let v = app.excluded_display.remove(i);
            app.excluded.remove(&v.to_lowercase());
            app.request_mask();
        } else if let Some(i) = allow {
            let v = app.excluded_display.remove(i);
            let mut values = app.cfg().allowlist.values.clone();
            if !values.contains(&v) {
                values.push(v.clone());
            }
            app.edit_config(|d, _| {
                let _ = d.set_serialized("allowlist.values", &values);
            });
            app.excluded.remove(&v.to_lowercase());
            let m = app.t("許可リストに追加しました", "Added to allowlist").to_string();
            app.toast(ToastKind::Success, m);
        }
    }
}
