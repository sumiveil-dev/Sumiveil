//! 一括処理画面 (フォルダのマスクと横断検索)。処理は別スレッドで行い、進捗とキャンセルに対応する。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Instant;

use eframe::egui::{self, Frame, Margin, RichText, ScrollArea, TextEdit, Ui};
use sumiveil_core::batch::{self, BatchFile};
use sumiveil_core::formats::{self, PropertiesMode};
use sumiveil_core::report::{build_report, csv_field, JsonReport, ReportOptions};
use sumiveil_core::search::{self, Query};
use sumiveil_core::{Config, Engine, MaskSession};

use crate::app::{App, ToastKind};
use crate::icons::Icon;
use crate::theme::Palette;
use crate::widgets;
use crate::win;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum OutMode {
    #[default]
    Folder,
    Suffix,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ReportKind {
    #[default]
    None,
    Json,
    Csv,
}

enum Msg {
    Total(usize),
    File { rel: String, count: usize, error: Option<String>, note: Option<String> },
    Finished { report: Option<PathBuf>, cancelled: bool, error: Option<String> },
}

struct Running {
    cancel: Arc<AtomicBool>,
    rx: Receiver<Msg>,
    total: usize,
    done: usize,
    started: Instant,
}

pub struct LogLine {
    pub rel: String,
    pub count: usize,
    pub error: Option<String>,
    /// 警告 (取り除いた添付ファイル・変更履歴など)
    pub note: Option<String>,
}

/// 一括処理の種類 (画面上部の切り替え)。
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum BatchMode {
    #[default]
    Mask,
    Search,
}

/// 横断検索の 1 件。
pub struct SearchHit {
    start: usize,
    /// セル・ページなどの場所、テキストなら行番号
    place: String,
    snippet: String,
    range: (usize, usize),
}

/// 横断検索の結果 (ファイルごと)。
pub struct SearchFile {
    path: PathBuf,
    rel: String,
    hits: Vec<SearchHit>,
    /// 一致の総数 (表示は先頭の一部だけ)
    count: usize,
    error: Option<String>,
}

enum SearchMsg {
    Total(usize),
    File(SearchFile),
    Finished { cancelled: bool, error: Option<String> },
}

struct SearchRun {
    cancel: Arc<AtomicBool>,
    rx: Receiver<SearchMsg>,
    total: usize,
    done: usize,
    started: Instant,
}

#[derive(Default)]
pub struct SearchUi {
    query: Query,
    running: Option<SearchRun>,
    /// 一致があったファイル (と、読めなかったファイル)
    results: Vec<SearchFile>,
    scanned: usize,
    summary: Option<(usize, usize, f64, bool)>,
    error: Option<String>,
}

/// 1 ファイルで一覧に出す一致の上限
const HITS_PER_FILE: usize = 200;

#[derive(Default)]
pub struct BatchUi {
    pub mode: BatchMode,
    pub search: SearchUi,
    pub input_dir: String,
    recursive: bool,
    include: String,
    exclude: String,
    out_mode: OutMode,
    out_dir: String,
    suffix: String,
    encoding: String,
    report: ReportKind,
    shared_numbering: bool,
    running: Option<Running>,
    log: Vec<LogLine>,
    summary: Option<String>,
    last_out: Option<PathBuf>,
    initialized: bool,
    /// Office 文書のプロパティの扱いを選ぶダイアログを表示中
    props_prompt: bool,
    props_remember: bool,
}

impl BatchUi {
    pub fn init_from(&mut self, cfg: &Config) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        self.recursive = true;
        self.include = cfg.batch.include.join(";");
        self.exclude = cfg.batch.exclude.join(";");
        self.suffix = cfg.batch.suffix.clone();
        self.encoding = cfg.batch.output_encoding.clone();
        self.shared_numbering = true;
    }

    /// 裏で動いている処理の進捗を受け取る。
    pub fn poll(&mut self, ctx: &egui::Context) {
        self.poll_search(ctx);
        let Some(run) = &mut self.running else { return };
        let mut finished = None;
        while let Ok(m) = run.rx.try_recv() {
            match m {
                Msg::Total(n) => run.total = n,
                Msg::File { rel, count, error, note } => {
                    run.done += 1;
                    self.log.push(LogLine { rel, count, error, note });
                }
                Msg::Finished { report, cancelled, error } => finished = Some((report, cancelled, error, run.started.elapsed())),
            }
        }
        if let Some((report, cancelled, error, elapsed)) = finished {
            let files = self.log.len();
            let total: usize = self.log.iter().map(|l| l.count).sum();
            let errors = self.log.iter().filter(|l| l.error.is_some()).count();
            let mut s = format!("files={files} detections={total} errors={errors} time={:.1}s", elapsed.as_secs_f64());
            if cancelled {
                s.push_str(" (cancelled)");
            }
            if let Some(e) = error {
                s = e;
            }
            if let Some(r) = report {
                s.push_str(&format!("\nreport: {}", r.display()));
            }
            self.summary = Some(s);
            self.running = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn poll_search(&mut self, ctx: &egui::Context) {
        let s = &mut self.search;
        let Some(run) = &mut s.running else { return };
        let mut finished = None;
        while let Ok(m) = run.rx.try_recv() {
            match m {
                SearchMsg::Total(n) => run.total = n,
                SearchMsg::File(f) => {
                    run.done += 1;
                    if f.count > 0 || f.error.is_some() {
                        s.results.push(f);
                    }
                }
                SearchMsg::Finished { cancelled, error } => finished = Some((cancelled, error, run.started.elapsed(), run.done)),
            }
        }
        if let Some((cancelled, error, elapsed, scanned)) = finished {
            let hits = s.results.iter().map(|f| f.count).sum();
            let files = s.results.iter().filter(|f| f.count > 0).count();
            s.scanned = scanned;
            s.summary = Some((hits, files, elapsed.as_secs_f64(), cancelled));
            s.error = error;
            s.running = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

/// 起動時 (`--find`) の横断検索: 検索に切り替えて文字列を入れ、フォルダが決まっていればすぐ検索する。
pub fn search_on_start(app: &mut App, text: String) {
    app.batch.mode = BatchMode::Search;
    app.batch.search.query = Query { text, ..Default::default() };
    if !app.batch.input_dir.trim().is_empty() {
        start_search(app);
    }
}

/// フォルダ内のファイルを横断して検索する (元のファイルは読むだけ)。
fn start_search(app: &mut App) {
    let b = &mut app.batch;
    let root = PathBuf::from(b.input_dir.trim());
    if !root.is_dir() {
        let m = app.lang.t("フォルダが見つかりません", "Folder not found").to_string();
        return app.toast(ToastKind::Error, m);
    }
    let re = match b.search.query.compile() {
        Ok(Some(re)) => re,
        Ok(None) => return,
        Err(e) => {
            let m = format!("{}: {e}", app.lang.t("正規表現が正しくありません", "Invalid regular expression"));
            return app.toast(ToastKind::Error, m);
        }
    };
    let include = split(&b.include);
    let exclude = split(&b.exclude);
    let recursive = b.recursive;
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel2 = cancel.clone();
    let (tx, rx) = channel::<SearchMsg>();
    let ctx = app.ctx.clone();
    b.search.results.clear();
    b.search.summary = None;
    b.search.error = None;
    std::thread::spawn(move || {
        // 以前のマスク結果も検索の対象にする (接尾辞での除外はしない)
        let files = match batch::collect_files(&root, recursive, &include, &exclude, "", None) {
            Ok(f) => f,
            Err(e) => {
                let _ = tx.send(SearchMsg::Finished { cancelled: false, error: Some(e) });
                ctx.request_repaint();
                return;
            }
        };
        let _ = tx.send(SearchMsg::Total(files.len()));
        let mut cancelled = false;
        for f in &files {
            if cancel2.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }
            let rel = f.rel.display().to_string();
            let doc = std::fs::read(&f.path).map_err(|e| e.to_string()).and_then(|bytes| formats::open(&f.path, bytes, None));
            let file = match doc {
                Ok(doc) => {
                    let (all, _) = search::find_all(&re, &doc.text, usize::MAX);
                    let line_idx = sumiveil_core::text::LineIndex::new(&doc.text);
                    let hits = all
                        .iter()
                        .take(HITS_PER_FILE)
                        .map(|&(s, e)| {
                            let (snippet, range) = search::snippet(&doc.text, s, e, 30);
                            let place = match doc.location_at(s) {
                                Some(loc) => loc.to_string(),
                                None => format!("L{}", line_idx.line_col(&doc.text, s).0),
                            };
                            SearchHit { start: s, place, snippet, range }
                        })
                        .collect();
                    SearchFile { path: f.path.clone(), rel, hits, count: all.len(), error: None }
                }
                Err(e) => SearchFile { path: f.path.clone(), rel, hits: vec![], count: 0, error: Some(e) },
            };
            let _ = tx.send(SearchMsg::File(file));
            ctx.request_repaint();
        }
        let _ = tx.send(SearchMsg::Finished { cancelled, error: None });
        ctx.request_repaint();
    });
    b.search.running = Some(SearchRun { cancel, rx, total: 0, done: 0, started: Instant::now() });
}

fn split(s: &str) -> Vec<String> {
    s.split([';', ',', '\n']).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

/// `props`: Office 文書のプロパティ (作成者など) の扱い
fn start(app: &mut App, props: PropertiesMode) {
    let b = &mut app.batch;
    let root = PathBuf::from(b.input_dir.trim());
    if !root.is_dir() {
        let m = app.lang.t("入力フォルダが見つかりません", "Input folder not found").to_string();
        app.toast(ToastKind::Error, m);
        return;
    }
    let out_dir = (b.out_mode == OutMode::Folder).then(|| PathBuf::from(b.out_dir.trim()));
    if out_dir.as_ref().is_some_and(|d| d.as_os_str().is_empty()) {
        let m = app.lang.t("出力フォルダを指定してください", "Choose an output folder").to_string();
        app.toast(ToastKind::Error, m);
        return;
    }
    let include = split(&b.include);
    let exclude = split(&b.exclude);
    let suffix = b.suffix.clone();
    let encoding = if b.encoding.is_empty() { "same".to_string() } else { b.encoding.clone() };
    let report = b.report;
    let shared = b.shared_numbering;
    let recursive = b.recursive;
    let engine: Arc<Engine> = app.engine.clone();
    let profile = app.loaded.active_profile.clone();
    let ja = app.lang.is_ja();
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = channel::<Msg>();
    let ctx = app.ctx.clone();
    let cancel2 = cancel.clone();
    b.log.clear();
    b.summary = None;
    b.last_out = out_dir.clone().or_else(|| Some(root.clone()));
    std::thread::spawn(move || {
        let files: Vec<BatchFile> = match batch::collect_files(&root, recursive, &include, &exclude, &suffix, out_dir.as_deref()) {
            Ok(f) => f,
            Err(e) => {
                let _ = tx.send(Msg::Finished { report: None, cancelled: false, error: Some(e) });
                ctx.request_repaint();
                return;
            }
        };
        let _ = tx.send(Msg::Total(files.len()));
        let mut session = MaskSession::new();
        let mut reports: Vec<JsonReport> = vec![];
        let mut cancelled = false;
        for f in &files {
            if cancel2.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }
            if !shared {
                session.reset();
            }
            let out = batch::output_path(f, out_dir.as_deref(), &suffix);
            let msg = match batch::process_file(&engine, &mut session, f, &out, &encoding, props) {
                Ok(o) => {
                    if report != ReportKind::None {
                        let mut rep = build_report(
                            o.text(),
                            &o.result,
                            &ReportOptions { source: Some(f.path.display().to_string()), encoding: Some(o.encoding().to_string()), profile: &profile, include_original: false, include_masked: false, japanese_names: ja },
                        );
                        formats::annotate_report(&mut rep, &o.doc, &o.result, Some(&o.write));
                        reports.push(rep);
                    }
                    let notes: Vec<&str> = o.doc.warnings.iter().chain(&o.write.warnings).map(String::as_str).collect();
                    let note = (!notes.is_empty()).then(|| notes.join(" / "));
                    Msg::File { rel: f.rel.display().to_string(), count: o.result.replacements.len(), error: None, note }
                }
                Err(e) => Msg::File { rel: f.rel.display().to_string(), count: 0, error: Some(e), note: None },
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        }
        let report_path = match report {
            ReportKind::None => None,
            kind => {
                let dir = out_dir.clone().unwrap_or_else(|| root.clone());
                let path = dir.join(if kind == ReportKind::Json { "sumiveil-report.json" } else { "sumiveil-report.csv" });
                let body = if kind == ReportKind::Json {
                    serde_json::to_string_pretty(&reports).unwrap_or_default() + "\n"
                } else {
                    let mut s = String::from("\u{feff}file,line,column,id,category,name,confidence,replacement\r\n");
                    for r in &reports {
                        for d in &r.detections {
                            s.push_str(&format!(
                                "{},{},{},{},{},{},{:.2},{}\r\n",
                                csv_field(r.source.as_deref().unwrap_or("")),
                                d.line,
                                d.column,
                                csv_field(&d.id),
                                csv_field(&d.category),
                                csv_field(&d.name),
                                d.confidence,
                                csv_field(&d.replacement)
                            ));
                        }
                    }
                    s
                };
                let _ = std::fs::create_dir_all(&dir);
                std::fs::write(&path, body).ok().map(|_| path)
            }
        };
        let _ = tx.send(Msg::Finished { report: report_path, cancelled, error: None });
        ctx.request_repaint();
    });
    b.running = Some(Running { cancel, rx, total: 0, done: 0, started: Instant::now() });
}

fn folder_field(ui: &mut Ui, value: &mut String, hint: &str, pick_title: &str) {
    ui.horizontal(|ui| {
        let r = ui.add(TextEdit::singleline(value).hint_text(hint).desired_width(ui.available_width() - 110.0));
        crate::guide_mark!(ui.ctx(), format!("field:{hint}"), r.rect);
        if widgets::icon_button(ui, Icon::Folder, "…").clicked() {
            let mut d = rfd::FileDialog::new().set_title(pick_title);
            if !value.is_empty() {
                d = d.set_directory(value.as_str());
            }
            if let Some(p) = d.pick_folder() {
                *value = p.display().to_string();
            }
        }
    });
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        Frame::new().inner_margin(Margin { left: 24, right: 24, top: 16, bottom: 24 }).show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(900.0));
            widgets::title(ui, app.t("一括処理", "Batch"));
            let running = app.batch.running.is_some() || app.batch.search.running.is_some();
            ui.add_enabled_ui(!running, |ui| {
                let tabs = [(BatchMode::Mask, if ja { "マスク" } else { "Mask" }), (BatchMode::Search, if ja { "検索" } else { "Search" })];
                widgets::tabs(ui, &mut app.batch.mode, &tabs);
            });
            ui.add_space(4.0);
            let desc = match app.batch.mode {
                BatchMode::Mask => app.t(
                    "フォルダ内のテキスト・メール・Office 文書・PDF をまとめてマスクします (PDF と .msg はテキストで書き出します)。元のファイルは変更しません。",
                    "Mask text, email, Office and PDF files in a folder (PDF and .msg are written as text). Original files are never modified.",
                ),
                BatchMode::Search => app.t(
                    "フォルダ内のファイルから文字列を探します。結果を押すと、マスク画面でその位置を開きます。元のファイルは変更しません。",
                    "Find text across the files in a folder. Click a result to open it on the Mask page. Files are never modified.",
                ),
            };
            widgets::secondary(ui, desc);
            ui.add_space(12.0);
            ui.add_enabled_ui(!running, |ui| {
                widgets::card(ui, |ui| {
                    widgets::subtitle(ui, if ja { "対象" } else { "Target" });
                    folder_field(ui, &mut app.batch.input_dir, if ja { "フォルダ" } else { "Folder" }, if ja { "フォルダを選択" } else { "Choose folder" });
                    ui.checkbox(&mut app.batch.recursive, if ja { "サブフォルダも対象にする" } else { "Include subfolders" });
                });
                ui.add_space(8.0);
                let (title, desc) = if ja { ("対象のファイル", "対象パターン・除外パターン") } else { ("Files", "Include and exclude patterns") };
                widgets::expander(ui, "batch-patterns", title, desc, false, |ui| {
                    egui::Grid::new("batch-in").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                        ui.label(if ja { "対象パターン" } else { "Include" });
                        ui.add(TextEdit::singleline(&mut app.batch.include).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                        ui.end_row();
                        ui.label(if ja { "除外パターン" } else { "Exclude" });
                        ui.add(TextEdit::singleline(&mut app.batch.exclude).hint_text("*.bak;secret-*.txt").desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                        ui.end_row();
                    });
                });
            });
            ui.add_space(8.0);
            match app.batch.mode {
                BatchMode::Mask => mask_body(app, ui, ja),
                BatchMode::Search => search_body(app, ui, ja),
            }
        });
    });
    // Office 文書のプロパティの扱いを選ぶ (設定が「その都度確認」のとき、開始前に 1 回)
    if app.batch.props_prompt {
        let ja = app.lang.is_ja();
        let mut remember = app.batch.props_remember;
        let r = crate::props_prompt::dialog(ui.ctx(), "batch-props", ja, &[], &mut remember);
        app.batch.props_remember = remember;
        if let Some(choice) = r {
            app.batch.props_prompt = false;
            if let Some(m) = choice {
                if remember {
                    app.set_base("files.properties", m.as_str());
                }
                start(app, m);
            }
        }
    }
}

/// 一括マスク: 出力先・詳細 (折りたたみ)・開始と進捗・結果。
fn mask_body(app: &mut App, ui: &mut Ui, ja: bool) {
    let p = Palette::current(ui.ctx());
    let running = app.batch.running.is_some();
    ui.add_enabled_ui(!running, |ui| {
        widgets::card(ui, |ui| {
            widgets::subtitle(ui, if ja { "出力先" } else { "Output" });
            ui.horizontal_wrapped(|ui| {
                ui.radio_value(&mut app.batch.out_mode, OutMode::Folder, if ja { "別のフォルダ (フォルダ構成を維持)" } else { "Another folder (keep structure)" });
                ui.radio_value(&mut app.batch.out_mode, OutMode::Suffix, if ja { "元のファイルの横に接尾辞付きで保存" } else { "Next to originals with a suffix" });
            });
            match app.batch.out_mode {
                OutMode::Folder => folder_field(ui, &mut app.batch.out_dir, if ja { "出力フォルダ" } else { "Output folder" }, if ja { "出力フォルダを選択" } else { "Choose output folder" }),
                OutMode::Suffix => {
                    ui.horizontal(|ui| {
                        ui.label(if ja { "接尾辞" } else { "Suffix" });
                        ui.add(TextEdit::singleline(&mut app.batch.suffix).desired_width(120.0));
                        widgets::secondary(ui, &format!("report.txt → report{}.txt", app.batch.suffix));
                    });
                }
            }
        });
        ui.add_space(8.0);
        let (title, desc) = if ja { ("詳細", "文字コード・レポート・連番") } else { ("Details", "Encoding, report and numbering") };
        widgets::expander(ui, "batch-details", title, desc, false, |ui| {
            egui::Grid::new("batch-out").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label(if ja { "文字コード" } else { "Encoding" });
                egui::ComboBox::from_id_salt("batch-enc")
                    .selected_text(match app.batch.encoding.as_str() {
                        "same" | "" => if ja { "入力と同じ" } else { "Same as input" },
                        other => other,
                    })
                    .show_ui(ui, |ui| {
                        for (v, l) in [("same", if ja { "入力と同じ" } else { "Same as input" }), ("utf-8", "UTF-8"), ("utf-8-bom", "UTF-8 (BOM)"), ("shift_jis", "Shift_JIS"), ("utf-16le", "UTF-16LE")] {
                            ui.selectable_value(&mut app.batch.encoding, v.to_string(), l);
                        }
                    });
                ui.end_row();
                ui.label(if ja { "レポート" } else { "Report" });
                ui.horizontal(|ui| {
                    ui.radio_value(&mut app.batch.report, ReportKind::None, if ja { "なし" } else { "None" });
                    ui.radio_value(&mut app.batch.report, ReportKind::Csv, "CSV");
                    ui.radio_value(&mut app.batch.report, ReportKind::Json, "JSON");
                });
                ui.end_row();
            });
            ui.checkbox(&mut app.batch.shared_numbering, if ja { "ファイル間で連番 {n} を共通にする (同じ値は全ファイルで同じ番号)" } else { "Share {n} numbering across files" });
        });
    });
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if let Some(run) = &app.batch.running {
            let frac = if run.total > 0 { run.done as f32 / run.total as f32 } else { 0.0 };
            ui.add(egui::ProgressBar::new(frac).text(format!("{}/{}", run.done, run.total)).desired_width(ui.available_width() - 140.0));
            if widgets::icon_button(ui, Icon::Stop, if ja { "中止" } else { "Cancel" }).clicked() {
                run.cancel.store(true, Ordering::Relaxed);
            }
        } else {
            if widgets::accent_button(ui, Icon::Play, if ja { "開始" } else { "Start" }).clicked() {
                // プロパティの扱いが「その都度確認」なら、開始前に 1 回だけ選んでもらう
                match PropertiesMode::parse(&app.cfg().files.properties) {
                    Some(m) => start(app, m),
                    None => app.batch.props_prompt = true,
                }
            }
            if let Some(out) = app.batch.last_out.clone() {
                if app.batch.summary.is_some() && widgets::icon_button(ui, Icon::Folder, if ja { "出力先を開く" } else { "Open output" }).clicked() {
                    win::open_in_explorer(&out);
                }
            }
            let prof = if app.loaded.active_profile == "default" { app.t("既定", "Default").to_string() } else { app.loaded.active_profile.clone() };
            widgets::secondary(ui, &format!("{}: {prof}", if ja { "プロファイル" } else { "Profile" }));
        }
    });
    if let Some(s) = &app.batch.summary {
        ui.add_space(8.0);
        widgets::info_bar(ui, widgets::InfoKind::Success, s);
    }
    if !app.batch.log.is_empty() {
        ui.add_space(8.0);
        widgets::card(ui, |ui| {
            ScrollArea::vertical().id_salt("batch-log").max_height(320.0).stick_to_bottom(true).show(ui, |ui| {
                for l in &app.batch.log {
                    ui.horizontal(|ui| {
                        match &l.error {
                            None => widgets::icon_text(ui, Icon::Check, "", p.success),
                            Some(_) => widgets::icon_text(ui, Icon::Warning, "", p.critical),
                        };
                        ui.label(RichText::new(&l.rel).monospace().size(12.5));
                        match &l.error {
                            None => ui.label(RichText::new(format!("{}", l.count)).color(p.text_secondary)),
                            Some(e) => ui.label(RichText::new(e).color(p.critical).size(12.0)),
                        };
                        if let Some(n) = &l.note {
                            ui.label(RichText::new(n).color(p.caution).size(12.0));
                        }
                    });
                }
            });
        });
    }
}

/// フォルダの横断検索: 検索欄・進捗・ファイルごとの結果。
fn search_body(app: &mut App, ui: &mut Ui, ja: bool) {
    let p = Palette::current(ui.ctx());
    let running = app.batch.search.running.is_some();
    let mut go = false;
    widgets::card(ui, |ui| {
        widgets::subtitle(ui, if ja { "検索" } else { "Find" });
        ui.horizontal(|ui| {
            let q = &mut app.batch.search.query;
            let r = ui.add_enabled(!running, TextEdit::singleline(&mut q.text).hint_text(if ja { "検索する文字列" } else { "Text to find" }).desired_width((ui.available_width() - 220.0).max(160.0)));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                go = true;
            }
            let tg = |ui: &mut Ui, on: &mut bool, text: &str, tip: &str| {
                let b = egui::Button::new(RichText::new(text).monospace().size(13.0)).min_size(egui::vec2(30.0, 28.0));
                let b = if *on { b.fill(p.accent.gamma_multiply(0.25)).stroke(egui::Stroke::new(1.0, p.accent)) } else { b.frame_when_inactive(false) };
                if ui.add_enabled(!running, b).on_hover_text(tip).clicked() {
                    *on = !*on;
                }
            };
            tg(ui, &mut q.case_sensitive, "Aa", if ja { "大文字と小文字を区別する" } else { "Match case" });
            tg(ui, &mut q.regex, ".*", if ja { "正規表現を使う" } else { "Use regular expression" });
            if let Some(run) = &app.batch.search.running {
                if widgets::icon_button(ui, Icon::Stop, if ja { "中止" } else { "Cancel" }).clicked() {
                    run.cancel.store(true, Ordering::Relaxed);
                }
            } else if ui.add_enabled_ui(!app.batch.search.query.is_empty(), |ui| widgets::accent_button(ui, Icon::Find, if ja { "検索" } else { "Find" })).inner.clicked() {
                go = true;
            }
        });
    });
    if go && !running && !app.batch.search.query.is_empty() {
        start_search(app);
    }
    let s = &app.batch.search;
    ui.add_space(8.0);
    if let Some(run) = &s.running {
        let frac = if run.total > 0 { run.done as f32 / run.total as f32 } else { 0.0 };
        ui.add(egui::ProgressBar::new(frac).text(format!("{}/{}", run.done, run.total)));
    } else if let Some(e) = &s.error {
        widgets::info_bar(ui, widgets::InfoKind::Error, e);
    } else if let Some((hits, files, secs, cancelled)) = s.summary {
        let mut t = if ja {
            format!("{hits} 件 ({files} ファイル) · {} ファイルを検索 · {secs:.1} 秒", s.scanned)
        } else {
            format!("{hits} matches in {files} files · {} files searched · {secs:.1} s", s.scanned)
        };
        if cancelled {
            t.push_str(if ja { " (中止)" } else { " (cancelled)" });
        }
        widgets::secondary(ui, &t);
    }
    if s.results.is_empty() {
        return;
    }
    ui.add_space(4.0);
    let mut open: Option<(PathBuf, usize)> = None;
    for (fi, f) in s.results.iter().enumerate() {
        let id = ui.make_persistent_id(("search-file", &f.rel));
        let state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, fi < 10);
        state
            .show_header(ui, |ui| {
                let color = if f.error.is_some() { p.critical } else { p.text };
                widgets::icon_text_truncated(ui, Icon::Document, &f.rel, color);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if f.error.is_none() {
                        widgets::badge(ui, &f.count.to_string(), p.accent.gamma_multiply(0.2), p.accent);
                    }
                });
            })
            .body(|ui| {
                if let Some(e) = &f.error {
                    ui.label(RichText::new(e).size(12.0).color(p.critical));
                    return;
                }
                for h in &f.hits {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, 4.0, p.control_hover);
                    }
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(6.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
                    let (place_rect, _) = child.allocate_exact_size(egui::vec2(96.0, 20.0), egui::Sense::hover());
                    child.painter().text(place_rect.left_center(), egui::Align2::LEFT_CENTER, truncate_chars(&h.place, 14), egui::FontId::proportional(11.5), p.text_secondary);
                    let mut job = egui::text::LayoutJob::default();
                    let mono = egui::FontId::monospace(12.5);
                    let fmt = |c, bg| egui::TextFormat { font_id: mono.clone(), color: c, background: bg, ..Default::default() };
                    let (a, b) = h.range;
                    job.append(&h.snippet[..a], 0.0, fmt(p.text_secondary, egui::Color32::TRANSPARENT));
                    job.append(&h.snippet[a..b], 0.0, fmt(p.text, p.find));
                    job.append(&h.snippet[b..], 0.0, fmt(p.text_secondary, egui::Color32::TRANSPARENT));
                    child.add(egui::Label::new(job).truncate().selectable(false));
                    if resp.on_hover_text(if ja { "マスク画面で開く" } else { "Open on the Mask page" }).clicked() {
                        open = Some((f.path.clone(), h.start));
                    }
                }
                if f.count > f.hits.len() {
                    let more = f.count - f.hits.len();
                    widgets::secondary(ui, &if ja { format!("ほか {more} 件 (ファイルを開いて検索バーで確認できます)") } else { format!("{more} more (open the file to see them)") });
                }
            });
    }
    if let Some((path, start)) = open {
        let q = app.batch.search.query.clone();
        crate::mask_page::open_at(app, &path, q, start);
    }
}

fn truncate_chars(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
}
