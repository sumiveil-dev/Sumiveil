//! 設定画面。変更は即座に設定ファイルへ保存される (コメントは保持)。

use eframe::egui::{self, collapsing_header::CollapsingState, FontId, Frame, Id, Margin, RichText, ScrollArea, TextEdit, Ui};
use sumiveil_core::catalog::{self, CATALOG, CATEGORIES};
use sumiveil_core::config::{self, CustomRule, KeywordGroup, ThemeMode, DEFAULT_TOML};
use sumiveil_core::template::{RenderCtx, Template, PRESETS};

use crate::app::{App, ToastKind};
use crate::icons::Icon;
use crate::shortcuts::{Action, Recorded};
use crate::theme::Palette;
use crate::widgets::{self, InfoKind};
use crate::win;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Section {
    #[default]
    General,
    /// 検出対象 (感度・人名と地名を含む)
    Detectors,
    Display,
    /// 辞書とルール (キーワード辞書・許可リスト・カスタムルール)
    Rules,
    Profiles,
    /// ファイル (Office 文書のプロパティ・設定ファイル)
    Files,
    /// キー操作とトレイ
    Keys,
    About,
}

/// 「辞書とルール」の上部の切り替え。
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum RulesTab {
    #[default]
    Keywords,
    Allowlist,
    Custom,
}

impl Section {
    /// `--page settings:<name>` の名前から。旧版の名前 (names / custom / tray など) も受け付ける。
    pub fn from_name(name: &str) -> (Section, Option<RulesTab>) {
        match name {
            "detectors" | "names" | "sensitivity" => (Section::Detectors, None),
            "display" => (Section::Display, None),
            "rules" | "keywords" => (Section::Rules, Some(RulesTab::Keywords)),
            "allowlist" => (Section::Rules, Some(RulesTab::Allowlist)),
            "custom" => (Section::Rules, Some(RulesTab::Custom)),
            "profiles" => (Section::Profiles, None),
            "files" | "file" => (Section::Files, None),
            "keys" | "tray" | "shortcuts" => (Section::Keys, None),
            "about" => (Section::About, None),
            _ => (Section::General, None),
        }
    }
}

#[derive(Default)]
pub struct SettingsUi {
    pub section: Section,
    pub rules_tab: RulesTab,
    search: String,
    custom_rules: Option<Vec<CustomRule>>,
    keywords: Option<Vec<KeywordGroup>>,
    keyword_texts: Vec<String>,
    allow: Option<[String; 3]>,
    test_text: String,
    new_profile: (String, String),
    confirm_reset: bool,
    confirm_delete: Option<String>,
}

/// フォーカス中だけ編集用バッファを持ち、確定時 (Enter / フォーカス喪失) に値を返すテキスト欄。
fn commit_field(ui: &mut Ui, id: Id, current: &str, hint: &str, width: f32, mono: bool) -> Option<String> {
    let focused = ui.memory(|m| m.has_focus(id));
    let mut buf: String = if focused { ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| current.to_string()) } else { current.to_string() };
    let mut te = TextEdit::singleline(&mut buf).id(id).hint_text(hint).desired_width(width);
    if mono {
        te = te.font(egui::TextStyle::Monospace);
    }
    let r = ui.add(te);
    ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    if r.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
        if buf != current {
            return Some(buf);
        }
    }
    None
}

/// スライダー付き設定行。ユーザーが操作を終え、値が実際に変わったときだけ Some(新しい値) を返す。
/// - 押している間 (ドラッグ中) の値は一時メモリに持ち、つまみと数値はそれで描く。保存はボタンを離したときに 1 回だけ。
/// - f32 の設定値を f64 に広げると刻み幅への丸めで「変更」扱いになり、保存 → 再読込が毎フレーム繰り返されるため、丸めてから比べる
fn slider_row(ui: &mut Ui, label: &str, desc: &str, value: f64, range: std::ops::RangeInclusive<f64>, step: f64) -> Option<f64> {
    let snap = |x: f64| (x / step).round() * step;
    let current = snap(value);
    let id = ui.id().with(("slider_row", label));
    let held: Option<f64> = widgets::pending(ui.ctx(), id);
    let mut v = held.unwrap_or(current);
    let decimals = if step >= 1.0 { 0 } else { 2 };
    let r = widgets::setting_row(ui, label, desc, |ui| {
        // 右から左に並ぶため、値の表示 → スライダーの順に追加する
        let p = Palette::current(ui.ctx());
        let (vrect, _) = ui.allocate_exact_size(egui::vec2(56.0, 28.0), egui::Sense::hover());
        let r = widgets::slider(ui, &mut v, range, step, 220.0);
        ui.painter().rect(vrect, 4.0, p.control, egui::Stroke::new(1.0, p.control_stroke), egui::StrokeKind::Inside);
        ui.painter().text(vrect.center(), egui::Align2::CENTER_CENTER, format!("{:.*}", decimals, snap(v)), FontId::monospace(13.0), p.text);
        r
    });
    let v = snap(v);
    let pressing = r.is_pointer_button_down_on() || r.dragged();
    let finished = if pressing {
        widgets::set_pending(ui.ctx(), id, Some(v));
        false
    } else if held.is_some() {
        // ボタンを離した最初のフレーム: 保持していた値を確定する
        widgets::set_pending::<f64>(ui.ctx(), id, None);
        true
    } else {
        // キー操作 (矢印 / Home / End) はその場で確定
        r.changed()
    };
    (finished && (v - current).abs() > step / 4.0).then_some(v)
}

/// テンプレート欄 + プリセットメニュー。確定したら Some(新しい値 or 空=既定に戻す)。
fn template_field(ui: &mut Ui, id: Id, current: &str, hint: &str, width: f32, ja: bool) -> Option<String> {
    let mut out = commit_field(ui, id, current, hint, width, true);
    let chevron = widgets::icon_label(ui, Icon::ChevronDown, "", ui.visuals().text_color());
    ui.menu_button(chevron, |ui| {
        for (tpl, en, jp) in PRESETS {
            if ui.button(format!("{}   {}", if ja { jp } else { en }, tpl)).clicked() {
                out = Some(tpl.to_string());
                ui.close();
            }
        }
        ui.separator();
        if ui.button(if ja { "既定に戻す" } else { "Reset to default" }).clicked() {
            out = Some(String::new());
            ui.close();
        }
    });
    out
}

fn sections(ja: bool) -> Vec<(Section, Icon, &'static str)> {
    let t = |a: &'static str, b: &'static str| if ja { a } else { b };
    vec![
        (Section::General, Icon::Settings, t("全般", "General")),
        (Section::Detectors, Icon::Veil, t("検出対象", "Detectors")),
        (Section::Display, Icon::Edit, t("マスクの表示", "Replacement")),
        (Section::Rules, Icon::List, t("辞書とルール", "Dictionaries & rules")),
        (Section::Profiles, Icon::Tag, t("プロファイル", "Profiles")),
        (Section::Files, Icon::Document, t("ファイル", "Files")),
        (Section::Keys, Icon::Keyboard, t("キー操作とトレイ", "Keys & tray")),
        (Section::About, Icon::Info, t("情報", "About")),
    ]
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let p = Palette::current(ui.ctx());
    let ja = app.lang.is_ja();
    egui::Panel::left("settings-nav")
        .exact_size(200.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 16, right: 8, top: 16, bottom: 8 }))
        .show(ui, |ui| {
            widgets::title(ui, app.t("設定", "Settings"));
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            for (s, g, label) in sections(ja) {
                if widgets::nav_item(ui, g, label, app.settings.section == s, true).clicked() {
                    app.settings.section = s;
                }
            }
        });
    egui::CentralPanel::no_frame().show(ui, |ui| {
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            Frame::new().inner_margin(Margin { left: 16, right: 24, top: 20, bottom: 24 }).show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(900.0));
                if app.loaded.active_profile != "default" && matches!(app.settings.section, Section::Detectors | Section::Display) {
                    let msg = if ja {
                        format!("プロファイル「{}」を編集中です。ここでの変更はこのプロファイルにのみ保存されます。", app.loaded.active_profile)
                    } else {
                        format!("Editing profile \"{}\". Changes here are saved to this profile only.", app.loaded.active_profile)
                    };
                    widgets::info_bar(ui, InfoKind::Info, &msg);
                    ui.add_space(8.0);
                }
                match app.settings.section {
                    Section::General => general(app, ui),
                    Section::Detectors => detection_section(app, ui),
                    Section::Display => display(app, ui),
                    Section::Rules => rules_section(app, ui),
                    Section::Profiles => profiles(app, ui),
                    Section::Files => files_section(app, ui),
                    Section::Keys => keys_section(app, ui),
                    Section::About => app.about(ui),
                }
            });
        });
    });
}

// ───────────────────────── 全般 ─────────────────────────

/// 手動指定用の色見本 (Windows 11 の「色」設定と同じ系統)。
const ACCENT_SWATCHES: [u32; 16] = [
    0xFFB900, 0xF7630C, 0xDA3B01, 0xE81123, 0xEA005E, 0xE3008C, 0xB146C2, 0x744DA9, //
    0x0078D4, 0x0063B1, 0x0099BC, 0x00B7C3, 0x00B294, 0x107C10, 0x567C73, 0x7A7574,
];

fn rgb_hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// アクセントカラー: 自動 (Windows の設定) / 手動 (色見本・カラーピッカー・#RRGGBB)。
fn accent_setting(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    let manual = app.cfg().gui.manual_accent();
    let mut is_manual = manual.is_some();
    // 行の右端から並ぶため、表示したい順 (自動 → 手動) の逆に書く
    let opts = [(true, Icon::Edit, if ja { "手動" } else { "Manual" }), (false, Icon::Auto, if ja { "自動 (Windows)" } else { "Auto (Windows)" })];
    let changed = widgets::setting_row(
        ui,
        app.t("アクセントカラー", "Accent color"),
        app.t("ボタンや選択中の項目などに使う色です。「自動」は Windows の設定に従います", "Used for buttons and selections. \"Auto\" follows the Windows setting"),
        |ui| widgets::segmented(ui, &mut is_manual, &opts),
    );
    if changed {
        if is_manual {
            // 見た目が急に変わらないよう、今の色から始める
            let c = app.system_accent.unwrap_or(egui::Color32::from_rgb(0x00, 0x78, 0xD4));
            app.set_base("gui.accent_color", rgb_hex([c.r(), c.g(), c.b()]));
        } else {
            app.set_base("gui.accent_color", "auto");
        }
        return;
    }
    let Some(cur) = manual else { return };

    let mut picked: Option<[u8; 3]> = None;
    ui.add_space(2.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        for rgb in ACCENT_SWATCHES {
            let c = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
            let color = egui::Color32::from_rgb(c[0], c[1], c[2]);
            if widgets::color_swatch(ui, color, c == cur).on_hover_text(rgb_hex(c)).clicked() {
                picked = Some(c);
            }
        }
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        // カラーピッカー: 操作中は画面だけに仮反映し、マウスを離したら 1 回だけ保存する
        let pid = ui.id().with("accent-picker");
        let held: Option<[u8; 3]> = widgets::pending(ui.ctx(), pid);
        let mut rgb = held.unwrap_or(cur);
        ui.label(app.t("カスタム", "Custom"));
        if egui::widgets::color_picker::color_edit_button_srgb(ui, &mut rgb).changed() {
            widgets::set_pending(ui.ctx(), pid, Some(rgb));
            app.accent_preview = Some(egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
            app.refresh_style();
        } else if held.is_some() && !ui.input(|i| i.pointer.any_down()) {
            widgets::set_pending::<[u8; 3]>(ui.ctx(), pid, None);
            app.accent_preview = None;
            picked = Some(rgb);
        }
        ui.add_space(8.0);
        if let Some(s) = commit_field(ui, Id::new("accent-hex"), &rgb_hex(cur), "#RRGGBB", 90.0, true) {
            match config::parse_hex_color(&s) {
                Some(c) => picked = Some(c),
                None => {
                    let m = app.t("色は #RRGGBB の形で入力してください (例: #0078D4)", "Enter the color as #RRGGBB (e.g. #0078D4)").to_string();
                    app.toast(ToastKind::Error, m);
                }
            }
        }
    });
    if let Some(c) = picked {
        app.accent_preview = None;
        if c != cur {
            app.set_base("gui.accent_color", rgb_hex(c));
        } else {
            app.refresh_style();
        }
    }
}

fn general(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    widgets::subtitle(ui, app.t("外観", "Appearance"));
    widgets::card(ui, |ui| {
        let cur = app.cfg().general.language.clone();
        let mut sel = cur.clone();
        widgets::setting_row(ui, app.t("表示言語", "Language"), app.t("「自動」は Windows の表示言語に従います", "\"Auto\" follows the Windows display language"), |ui| {
            egui::ComboBox::from_id_salt("lang")
                .selected_text(match sel.as_str() {
                    "ja" => "日本語",
                    "en" => "English",
                    _ => if ja { "自動" } else { "Auto" },
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut sel, "auto".to_string(), if ja { "自動" } else { "Auto" });
                    ui.selectable_value(&mut sel, "ja".to_string(), "日本語");
                    ui.selectable_value(&mut sel, "en".to_string(), "English");
                });
        });
        if sel != cur {
            app.set_base("general.language", sel.as_str());
        }
        ui.separator();
        let mut mode = app.cfg().general.theme;
        let opts = [
            (ThemeMode::Light, Icon::Sun, if ja { "ライト" } else { "Light" }),
            (ThemeMode::Dark, Icon::Moon, if ja { "ダーク" } else { "Dark" }),
            (ThemeMode::System, Icon::Auto, if ja { "システム" } else { "System" }),
        ];
        let changed = widgets::setting_row(ui, app.t("テーマ", "Theme"), app.t("「システム」は Windows のアプリモードに追従します", "\"System\" follows the Windows app mode"), |ui| widgets::segmented(ui, &mut mode, &opts));
        if changed {
            app.set_base("general.theme", match mode {
                ThemeMode::Light => "light",
                ThemeMode::Dark => "dark",
                ThemeMode::System => "system",
            });
        }
        ui.separator();
        let fs = app.cfg().gui.font_size as f64;
        if let Some(v) = slider_row(ui, app.t("エディタの文字サイズ", "Editor font size"), "", fs, 10.0..=24.0, 1.0) {
            app.set_base("gui.font_size", v);
        }
        ui.separator();
        accent_setting(app, ui);
        ui.separator();
        let cur = app.cfg().gui.renderer.clone();
        let mut sel = cur.clone();
        let label_of = |v: &str| -> &'static str {
            match v {
                "opengl" => "OpenGL",
                "directx" => "DirectX (GPU)",
                "software" => if ja { "ソフトウェア描画" } else { "Software" },
                _ => if ja { "自動 (推奨)" } else { "Auto (recommended)" },
            }
        };
        let desc = format!(
            "{}\n{}: {}",
            app.t("画面が真っ白・崩れる場合は「ソフトウェア描画」に。再起動後に反映されます", "Choose Software if the window is blank or garbled. Applies after restart"),
            app.t("現在", "Current"),
            app.renderer_info
        );
        widgets::setting_row(ui, app.t("描画方式", "Renderer"), &desc, |ui| {
            egui::ComboBox::from_id_salt("renderer").selected_text(label_of(&sel)).show_ui(ui, |ui| {
                for v in ["auto", "opengl", "directx", "software"] {
                    ui.selectable_value(&mut sel, v.to_string(), label_of(v));
                }
            });
        });
        if sel != cur {
            app.set_base("gui.renderer", sel.as_str());
            let m = app.t("描画方式は次回の起動から反映されます", "The renderer applies after restarting Sumiveil").to_string();
            app.toast(ToastKind::Info, m);
        }
    });
    ui.add_space(12.0);
    editor_card(app, ui);
}

/// ファイル: Office 文書のプロパティの扱い + 設定ファイル。
fn files_section(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    widgets::subtitle(ui, app.t("Office 文書", "Office documents"));
    widgets::card(ui, |ui| {
        let cur = app.cfg().files.properties.trim().to_ascii_lowercase();
        let mut sel = cur.clone();
        let label_of = |v: &str| -> &'static str {
            match (v, ja) {
                ("mask", true) => "マスクする",
                ("clear", true) => "消す",
                ("keep", true) => "そのまま残す",
                (_, true) => "その都度確認する",
                ("mask", false) => "Mask",
                ("clear", false) => "Clear",
                ("keep", false) => "Keep",
                (_, false) => "Ask each time",
            }
        };
        widgets::setting_row(
            ui,
            app.t("Office 文書のプロパティ (作成者など)", "Office document properties (author, etc.)"),
            app.t(
                "Word / Excel / PowerPoint に記録された作成者・最終更新者・会社名・コメントの作成者などの扱い。コマンドラインでは「その都度確認」は「消す」になります",
                "Author, last editor, company and comment authors stored in Office files. On the command line, \"Ask\" means Clear",
            ),
            |ui| {
                egui::ComboBox::from_id_salt("files-properties").selected_text(label_of(&sel)).show_ui(ui, |ui| {
                    for v in ["ask", "clear", "mask", "keep"] {
                        ui.selectable_value(&mut sel, v.to_string(), label_of(v));
                    }
                });
            },
        );
        if sel != cur {
            app.set_base("files.properties", sel.as_str());
        }
    });
    ui.add_space(12.0);
    widgets::subtitle(ui, app.t("設定ファイル", "Settings file"));
    file(app, ui);
}

fn editor_card(app: &mut App, ui: &mut Ui) {
    widgets::subtitle(ui, app.t("エディタ", "Editor"));
    widgets::card(ui, |ui| {
        let g = app.cfg().gui.clone();
        // 「マスクを再実行」のキーは変更できるので、今の設定を表示する
        let run_key = crate::shortcuts::display(&g.shortcuts.run);
        let auto_desc = match (app.lang.is_ja(), run_key.is_empty()) {
            (true, false) => format!("オフにすると {run_key} / 「マスク実行」で実行します"),
            (true, true) => "オフにすると「マスク実行」で実行します".to_string(),
            (false, false) => format!("When off, run with {run_key} or the Run button"),
            (false, true) => "When off, run with the Run button".to_string(),
        };
        let rows: [(&str, &str, &str, bool); 5] = [
            ("gui.auto_mask", app.t("入力と同時にマスク", "Mask as you type"), &auto_desc, g.auto_mask),
            ("gui.word_wrap", app.t("折り返して表示", "Word wrap"), "", g.word_wrap),
            ("gui.show_line_numbers", app.t("行番号を表示", "Line numbers"), "", g.show_line_numbers),
            ("gui.sync_scroll", app.t("左右のスクロールを同期", "Sync scrolling"), "", g.sync_scroll),
            ("gui.show_detection_panel", app.t("検出一覧パネルを表示", "Show detection list"), "", g.show_detection_panel),
        ];
        for (i, (key, label, desc, val)) in rows.into_iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            let mut v = val;
            if widgets::toggle_row(ui, label, desc, &mut v) {
                app.set_base(key, v);
            }
        }
        ui.separator();
        if let Some(v) = slider_row(ui, app.t("自動マスクの待ち時間 (ミリ秒)", "Auto-mask delay (ms)"), "", g.debounce_ms as f64, 0.0..=2000.0, 50.0) {
            app.set_base("gui.debounce_ms", v as i64);
        }
    });
}

// ───────────────────────── 検出対象 ─────────────────────────

/// 検出対象: 感度、人名・地名 (折りたたみ)、検出器の一覧。
fn detection_section(app: &mut App, ui: &mut Ui) {
    widgets::card(ui, |ui| {
        let mc = app.cfg().masking.min_confidence as f64;
        let (label, desc) = (app.t("検出の感度 (最低信頼度)", "Sensitivity (minimum confidence)"), app.t("低くすると検出漏れが減り、誤検出が増えます", "Lower = fewer misses, more false positives"));
        if let Some(v) = slider_row(ui, label, desc, mc, 0.0..=1.0, 0.05) {
            app.set_scoped("masking.min_confidence", (v * 100.0).round() / 100.0);
        }
    });
    ui.add_space(8.0);
    let (title, desc) = (
        app.t("人名・地名", "Names & places"),
        app.t("敬称のない氏名・ローマ字の氏名・地名の検出方法", "How names without honorifics, romanized names and places are found"),
    );
    widgets::expander(ui, "names", title, desc, false, |ui| names(app, ui));
    ui.add_space(16.0);
    widgets::subtitle(ui, app.t("検出器", "Detectors"));
    detectors(app, ui);
}

// ───────────────────────── 辞書とルール ─────────────────────────

fn rules_section(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    let tabs = [
        (RulesTab::Keywords, if ja { "キーワード辞書" } else { "Keywords" }),
        (RulesTab::Allowlist, if ja { "許可リスト" } else { "Allowlist" }),
        (RulesTab::Custom, if ja { "カスタムルール" } else { "Custom rules" }),
    ];
    widgets::tabs(ui, &mut app.settings.rules_tab, &tabs);
    ui.add_space(8.0);
    match app.settings.rules_tab {
        RulesTab::Keywords => keywords(app, ui),
        RulesTab::Allowlist => allowlist(app, ui),
        RulesTab::Custom => custom_rules(app, ui),
    }
}

fn detectors(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    let p = Palette::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.add(TextEdit::singleline(&mut app.settings.search).hint_text(if ja { "検索 (名前・ID)" } else { "Search (name / id)" }).desired_width(260.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(ui, Icon::Refresh, if ja { "既定に戻す" } else { "Reset" }).on_hover_text(if ja { "検出器・カテゴリの個別設定をすべて削除" } else { "Remove all detector/category overrides" }).clicked() {
                app.edit_config(|d, prof| {
                    d.remove(&config::profile_key(prof, "detectors"));
                    d.remove(&config::profile_key(prof, "categories"));
                });
            }
            if widgets::icon_button(ui, Icon::Check, if ja { "すべて有効" } else { "Enable all" }).clicked() {
                app.edit_config(|d, prof| {
                    for c in CATEGORIES {
                        d.set(&config::profile_key(prof, &format!("categories.{}.enabled", c.id)), true);
                    }
                    for det in CATALOG {
                        d.set(&config::profile_key(prof, &format!("detectors.{}.enabled", det.id)), true);
                    }
                });
            }
        });
    });
    ui.add_space(8.0);
    let q = app.settings.search.to_lowercase();
    let cfg = app.cfg().clone();
    for c in CATEGORIES.iter().filter(|c| c.id != "custom") {
        let dets: Vec<_> = CATALOG
            .iter()
            .filter(|d| d.category == c.id)
            .filter(|d| q.is_empty() || d.id.contains(&q) || d.name_ja.to_lowercase().contains(&q) || d.name_en.to_lowercase().contains(&q))
            .collect();
        if dets.is_empty() {
            continue;
        }
        let enabled_n = dets.iter().filter(|d| cfg.detector_enabled(d.id)).count();
        widgets::card(ui, |ui| {
            let id = ui.make_persistent_id(("cat", c.id));
            let state = CollapsingState::load_with_default_open(ui.ctx(), id, !q.is_empty());
            state
                .show_header(ui, |ui| {
                    // 撮影用: 開閉ボタン (この時点の領域は左端の開閉ボタンだけ)
                    crate::guide_mark!(ui.ctx(), format!("cat:{}", c.id), ui.min_rect());
                    let mut on = cfg.category_enabled(c.id);
                    if widgets::toggle(ui, &mut on).changed() {
                        app.set_scoped(&format!("categories.{}.enabled", c.id), on);
                    }
                    ui.label(RichText::new(if ja { c.name_ja } else { c.name_en }).font(FontId::new(15.0, egui::FontFamily::Name(crate::fonts::SEMIBOLD.into()))));
                    widgets::secondary(ui, &format!("{enabled_n}/{}", dets.len()));
                })
                .body(|ui| {
                    ui.add_space(4.0);
                    let cat_tpl = cfg.categories.get(c.id).and_then(|x| x.template.clone()).unwrap_or_default();
                    ui.horizontal(|ui| {
                        widgets::secondary(ui, if ja { "カテゴリのマスク表示:" } else { "Category replacement:" });
                        if let Some(v) = template_field(ui, Id::new(("cat-tpl", c.id)), &cat_tpl, if ja { "(全体の既定)" } else { "(global default)" }, 220.0, ja) {
                            commit_template(app, &format!("categories.{}.template", c.id), v);
                        }
                    });
                    ui.add_space(4.0);
                    // 1 行 = [トグル][名前・ID・例 (残りの幅)][マスク表示の欄]。列幅を明示して折り返し崩れを防ぐ
                    for (i, d) in dets.iter().enumerate() {
                        if i > 0 {
                            ui.separator();
                        }
                        ui.horizontal(|ui| {
                            let mut on = cfg.detector_enabled(d.id);
                            let r = ui.add_enabled_ui(cfg.category_enabled(c.id), |ui| widgets::toggle(ui, &mut on)).inner;
                            if r.changed() {
                                app.set_scoped(&format!("detectors.{}.enabled", d.id), on);
                            }
                            ui.add_space(4.0);
                            let text_w = (ui.available_width() - 240.0).max(160.0);
                            ui.allocate_ui_with_layout(egui::vec2(text_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.set_width(text_w);
                                ui.label(if ja { d.name_ja } else { d.name_en });
                                ui.label(RichText::new(format!("{}  ·  {}", d.id, d.example.replace('\n', " ⏎ "))).size(11.5).color(p.text_secondary));
                            });
                            let tpl = cfg.detectors.get(d.id).and_then(|x| x.template.clone()).unwrap_or_default();
                            if let Some(v) = template_field(ui, Id::new(("det-tpl", d.id)), &tpl, if ja { "(既定)" } else { "(default)" }, 170.0, ja) {
                                commit_template(app, &format!("detectors.{}.template", d.id), v);
                            }
                        });
                    }
                });
        });
        ui.add_space(8.0);
    }
}

fn commit_template(app: &mut App, key: &str, v: String) {
    if v.trim().is_empty() {
        app.remove_scoped(key);
    } else {
        match Template::parse(&v) {
            Ok(_) => app.set_scoped(key, v.as_str()),
            Err(e) => app.toast(ToastKind::Error, e.to_string()),
        }
    }
}

// ───────────────────────── マスクの表示 ─────────────────────────

fn display(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    let p = Palette::current(ui.ctx());
    widgets::subtitle(ui, app.t("既定のマスク表示", "Default replacement"));
    widgets::card(ui, |ui| {
        widgets::secondary(ui, app.t(
            "テンプレートで自由に指定できます。{label} {label_ja} {category} {n}=同じ値には同じ連番 {len} {hash:8} {prefix:3} {suffix:4} {fill:●} {fill:*:8} {shape:*} {shape:*:4} {fake}。固定の語や文字 (例: [個人情報] や ■) も使えます。",
            "Use a template: {label} {label_ja} {category} {n}=same value same number {len} {hash:8} {prefix:3} {suffix:4} {fill:●} {fill:*:8} {shape:*} {shape:*:4} {fake}. Fixed words or characters (e.g. [REDACTED], ■) also work.",
        ));
        ui.add_space(6.0);
        let cur = app.cfg().masking.template.clone();
        ui.horizontal(|ui| {
            if let Some(v) = template_field(ui, Id::new("global-tpl"), &cur, "<{label}_{n}>", 320.0, ja) {
                let v = if v.is_empty() { "<{label}_{n}>".to_string() } else { v };
                match Template::parse(&v) {
                    Ok(_) => app.set_scoped("masking.template", v.as_str()),
                    Err(e) => app.toast(ToastKind::Error, e.to_string()),
                }
            }
        });
        ui.add_space(8.0);
        // プレビュー
        let preview_src = ui.data_mut(|d| d.get_temp::<String>(Id::new("global-tpl"))).filter(|s| !s.is_empty()).unwrap_or(cur);
        let salt = app.cfg().masking.hash_salt.clone();
        match Template::parse(&preview_src) {
            Ok(t) => {
                egui::Grid::new("tpl-preview").num_columns(3).spacing([16.0, 4.0]).show(ui, |ui| {
                    let samples = [
                        ("email", "EMAIL", "メールアドレス", "contact", "taro.yamada@corp.example.co.jp"),
                        ("phone_jp", "PHONE", "電話番号", "contact", "090-1234-5678"),
                        ("person_name", "NAME", "氏名", "personal", "山田太郎"),
                        ("credit_card", "CREDIT_CARD", "カード番号", "finance", "4111-1111-1111-1111"),
                        ("ipv4", "IPV4", "IPアドレス", "network", "10.20.30.40"),
                    ];
                    for (id, label, label_ja, cat, orig) in samples {
                        let out = t.render(&RenderCtx {
                            original: orig,
                            id,
                            label,
                            label_ja,
                            category: cat,
                            category_ja: catalog::category(cat).map(|c| c.name_ja).unwrap_or(cat),
                            n: 1,
                            hash_key: salt.as_bytes(),
                        });
                        ui.label(RichText::new(orig).monospace().color(p.text_secondary));
                        ui.label("→");
                        ui.label(RichText::new(out).monospace());
                        ui.end_row();
                    }
                });
            }
            Err(e) => {
                ui.colored_label(p.critical, e.to_string());
            }
        }
    });
    ui.add_space(12.0);
    widgets::subtitle(ui, app.t("カテゴリごとのマスク表示", "Per-category replacement"));
    widgets::card(ui, |ui| {
        let cfg = app.cfg().clone();
        egui::Grid::new("cat-tpls").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
            for c in CATEGORIES {
                ui.label(if ja { c.name_ja } else { c.name_en });
                let cur = cfg.categories.get(c.id).and_then(|x| x.template.clone()).unwrap_or_default();
                ui.horizontal(|ui| {
                    if let Some(v) = template_field(ui, Id::new(("cat-tpl2", c.id)), &cur, if ja { "(既定)" } else { "(default)" }, 260.0, ja) {
                        commit_template(app, &format!("categories.{}.template", c.id), v);
                    }
                });
                ui.end_row();
            }
        });
        widgets::secondary(ui, app.t("検出器ごとの指定は「検出対象」で行えます。", "Per-detector templates are in \"Detectors\"."));
    });
    ui.add_space(12.0);
    widgets::subtitle(ui, app.t("ハッシュ・ダミー値", "Hash & fake values"));
    widgets::card(ui, |ui| {
        let cur = app.cfg().masking.hash_salt.clone();
        widgets::setting_row(ui, app.t("ソルト", "Salt"), app.t("{hash} と {fake} の元になる秘密の文字列。チームで揃えると同じ値が同じ結果になります。", "Secret used by {hash} and {fake}. Share it across a team to get consistent results."), |ui| {
            if let Some(v) = commit_field(ui, Id::new("salt"), &cur, "", 220.0, true) {
                app.set_scoped("masking.hash_salt", v.as_str());
            }
        });
    });
}

// ───────────────────────── 人名・地名 ─────────────────────────

/// 人名・地名の設定 (「検出対象」の折りたたみの中身)。
fn names(app: &mut App, ui: &mut Ui) {
    let n = app.cfg().names.clone();
    {
        widgets::secondary(ui, app.t(
            "人名は 3 段階で検出します: (1) 敬称・項目名などの文脈ルール、(2) 内蔵の姓名辞書 (約 3 万語。ローマ字表記の日本人名にも対応) による敬称なしの氏名。辞書にない珍しい姓も、辞書の名と並んでいれば姓の字の特徴から見分けます、(3) 文字種・辞書・文脈のスコア統合。",
            "Names are detected in layers: (1) context rules (honorifics, labels), (2) built-in name dictionary (~30k entries, including romanized Japanese names) for names without honorifics; rare surnames not in the dictionary are recognized when followed by a known given name, (3) combined scoring.",
        ));
        ui.add_space(6.0);
        let mut v = n.propagate;
        if widgets::toggle_row(ui, app.t("同じ名前を文中すべてでマスク", "Mask every occurrence of a found name"), app.t("「山田様」を見つけたら本文中の「山田」もマスクします", "If \"Mr. Yamada\" is found, other \"Yamada\" occurrences are masked too"), &mut v) {
            app.set_scoped("names.propagate", v);
        }
        ui.separator();
        let mut v = n.use_dictionary;
        if widgets::toggle_row(ui, app.t("内蔵の姓名辞書を使う", "Use built-in name dictionary"), app.t("敬称がない「山田太郎」「Robert Johnson」なども検出します", "Also detects names without honorifics"), &mut v) {
            app.set_scoped("names.use_dictionary", v);
        }
        ui.separator();
        let (label, desc) = (app.t("辞書検出のしきい値", "Dictionary threshold"), app.t("低いほど多く検出 (誤検出も増加)", "Lower = more detections"));
        if let Some(v) = slider_row(ui, label, desc, n.dictionary_threshold as f64, 0.3..=0.9, 0.05) {
            app.set_scoped("names.dictionary_threshold", (v * 100.0).round() / 100.0);
        }
        ui.separator();
        let found = sumiveil_core::morph::find_dictionary(&n.morphology_dict);
        let status = match &found {
            Some(p) => format!("{}: {}", app.t("辞書", "Dictionary"), p.display()),
            None => app.t("辞書が見つかりません。インストーラーで「形態素解析辞書」を選ぶと使えます。", "Dictionary not found. Select \"Morphology dictionary\" in the installer to enable this.").to_string(),
        };
        let mut v = n.use_morphology;
        let label = app.t("形態素解析を使う (追加の辞書が必要)", "Use morphological analysis (extra dictionary)");
        let desc = format!("{}  {}", app.t("文脈から敬称のない人名・地名も見つけます。処理は 1MB あたり約 0.1 秒増えます。", "Finds names without honorifics from context. Adds ~0.1 s per MB."), status);
        if widgets::toggle_row(ui, label, &desc, &mut v) {
            app.set_scoped("names.use_morphology", v);
        }
        ui.separator();
        let mut v = app.cfg().detector_enabled("place_name");
        if widgets::toggle_row(ui, app.t("地名を検出 (辞書)", "Detect place names (dictionary)"), app.t("約 6.6 万件の地名辞書で「〜市」「〜駅」「〜在住」などを検出します", "Detects \"...city\", \"...station\", \"lives in ...\" using ~66k place names"), &mut v) {
            app.set_scoped("detectors.place_name.enabled", v);
        }
    }
}

// ───────────────────────── カスタムルール ─────────────────────────

fn custom_rules(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    let p = Palette::current(ui.ctx());
    if app.settings.custom_rules.is_none() {
        app.settings.custom_rules = Some(app.cfg().custom_rules.clone());
    }
    widgets::secondary(ui, app.t(
        "正規表現で独自の検出ルールを追加できます。名前付きグループ (?P<v>...) があればその部分だけをマスクします。先読み・後読みも使えます。カスタムルールは全プロファイル共通です。",
        "Add your own regex rules. If a named group (?P<v>...) exists, only that part is masked. Lookaround is supported. Custom rules are shared by all profiles.",
    ));
    ui.add_space(8.0);
    let mut rules = app.settings.custom_rules.take().unwrap_or_default();
    let dirty = rules != app.cfg().custom_rules;
    let mut remove = None;
    for (i, r) in rules.iter_mut().enumerate() {
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                widgets::toggle(ui, &mut r.enabled);
                ui.label("ID");
                ui.add(TextEdit::singleline(&mut r.id).desired_width(110.0));
                ui.label(if ja { "名前" } else { "Name" });
                ui.add(TextEdit::singleline(&mut r.name).desired_width(140.0));
                ui.label(if ja { "ラベル" } else { "Label" });
                ui.add(TextEdit::singleline(&mut r.label).desired_width(100.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::bar_button(ui, Icon::Delete, "", true).on_hover_text(if ja { "削除" } else { "Delete" }).clicked() {
                        remove = Some(i);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label(if ja { "パターン" } else { "Pattern" });
                ui.add(TextEdit::singleline(&mut r.pattern).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY));
            });
            ui.horizontal(|ui| {
                ui.label(if ja { "表示" } else { "Template" });
                let mut t = r.template.clone().unwrap_or_default();
                if ui.add(TextEdit::singleline(&mut t).font(egui::TextStyle::Monospace).hint_text(if ja { "(既定)" } else { "(default)" }).desired_width(160.0)).changed() {
                    r.template = if t.is_empty() { None } else { Some(t) };
                }
                ui.label(if ja { "優先度" } else { "Priority" });
                ui.add(egui::DragValue::new(&mut r.priority).range(0..=200));
                ui.checkbox(&mut r.case_insensitive, if ja { "大文字小文字を区別しない" } else { "Case-insensitive" });
            });
            if !r.pattern.is_empty() {
                match sumiveil_core::custom::compile_rule(r) {
                    Ok(_) => {
                        ui.label(RichText::new(if ja { "✓ 正規表現は有効です" } else { "✓ Valid pattern" }).size(12.0).color(p.success));
                    }
                    Err(e) => {
                        ui.label(RichText::new(format!("✗ {e}")).size(12.0).color(p.critical));
                    }
                }
            }
        });
        ui.add_space(6.0);
    }
    if let Some(i) = remove {
        rules.remove(i);
    }
    ui.horizontal(|ui| {
        if widgets::icon_button(ui, Icon::Add, if ja { "ルールを追加" } else { "Add rule" }).clicked() {
            let n = rules.len() + 1;
            rules.push(CustomRule { id: format!("rule{n}"), name: format!("Rule {n}"), label: "CUSTOM".into(), ..Default::default() });
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let save = ui.add_enabled_ui(dirty, |ui| widgets::accent_button(ui, Icon::Save, if ja { "保存" } else { "Save" })).inner;
            if save.clicked() {
                let bad = rules.iter().find(|r| r.id.trim().is_empty() || sumiveil_core::custom::compile_rule(r).is_err()).map(|r| r.id.clone());
                if let Some(id) = bad {
                    app.toast(ToastKind::Error, format!("{}: {id}", if ja { "ID が空か、正規表現が不正です" } else { "Empty ID or invalid regex" }));
                } else {
                    let rs = rules.clone();
                    app.edit_config(|d, _| {
                        let _ = d.set_serialized("custom_rules", &rs);
                    });
                    app.toast(ToastKind::Success, if ja { "カスタムルールを保存しました" } else { "Saved custom rules" });
                }
            }
            if ui.add_enabled(dirty, egui::Button::new(if ja { "元に戻す" } else { "Discard" })).clicked() {
                rules = app.cfg().custom_rules.clone();
            }
        });
    });
    // テスト
    ui.add_space(12.0);
    widgets::subtitle(ui, app.t("テスト", "Test"));
    widgets::card(ui, |ui| {
        ui.add(TextEdit::multiline(&mut app.settings.test_text).hint_text(if ja { "ここにテスト用の文章を入力" } else { "Type sample text here" }).desired_rows(4).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
        if !app.settings.test_text.is_empty() {
            for r in rules.iter().filter(|r| r.enabled && !r.pattern.is_empty()) {
                if let Ok(re) = sumiveil_core::custom::compile_rule(r) {
                    let spans = re.spans(&app.settings.test_text, r.group.as_deref());
                    let found: Vec<String> = spans.iter().take(20).map(|(s, e)| app.settings.test_text[*s..*e].to_string()).collect();
                    ui.label(RichText::new(format!("{}: {} {}  {}", r.id, spans.len(), if ja { "件" } else { "match(es)" }, found.join(" | "))).monospace().size(12.0));
                }
            }
        }
    });
    app.settings.custom_rules = Some(rules);
}

// ───────────────────────── キーワード辞書 ─────────────────────────

fn keywords(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    if app.settings.keywords.is_none() {
        let k = app.cfg().keywords.clone();
        app.settings.keyword_texts = k.iter().map(|g| g.words.join("\n")).collect();
        app.settings.keywords = Some(k);
    }
    widgets::secondary(ui, app.t(
        "必ずマスクしたい語 (社名・顧客名・製品のコードネームなど) を登録します。1 行に 1 語。全プロファイル共通です。",
        "Words that must always be masked (company, customer, codenames...). One per line. Shared by all profiles.",
    ));
    ui.add_space(8.0);
    let mut groups = app.settings.keywords.take().unwrap_or_default();
    let mut texts = std::mem::take(&mut app.settings.keyword_texts);
    texts.resize(groups.len(), String::new());
    for (g, t) in groups.iter_mut().zip(texts.iter()) {
        g.words = t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    }
    let dirty = groups != app.cfg().keywords;
    let mut remove = None;
    for (i, g) in groups.iter_mut().enumerate() {
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                widgets::toggle(ui, &mut g.enabled);
                ui.label(if ja { "ラベル" } else { "Label" });
                ui.add(TextEdit::singleline(&mut g.label).desired_width(120.0));
                ui.label(if ja { "名前" } else { "Name" });
                ui.add(TextEdit::singleline(&mut g.name).desired_width(160.0));
                ui.checkbox(&mut g.case_sensitive, if ja { "大文字小文字を区別" } else { "Case-sensitive" });
                ui.checkbox(&mut g.whole_word, if ja { "単語単位" } else { "Whole word" });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::bar_button(ui, Icon::Delete, "", true).clicked() {
                        remove = Some(i);
                    }
                });
            });
            ui.add(TextEdit::multiline(&mut texts[i]).desired_rows(4).desired_width(f32::INFINITY).hint_text(if ja { "1 行に 1 語" } else { "One word per line" }));
        });
        ui.add_space(6.0);
    }
    if let Some(i) = remove {
        groups.remove(i);
        texts.remove(i);
    }
    ui.horizontal(|ui| {
        if widgets::icon_button(ui, Icon::Add, if ja { "グループを追加" } else { "Add group" }).clicked() {
            groups.push(KeywordGroup { label: format!("KEYWORD{}", groups.len() + 1), ..Default::default() });
            texts.push(String::new());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let save = ui.add_enabled_ui(dirty, |ui| widgets::accent_button(ui, Icon::Save, if ja { "保存" } else { "Save" })).inner;
            if save.clicked() {
                let gs = groups.clone();
                app.edit_config(|d, _| {
                    let _ = d.set_serialized("keywords", &gs);
                });
                app.toast(ToastKind::Success, if ja { "キーワード辞書を保存しました" } else { "Saved keywords" });
            }
            if ui.add_enabled(dirty, egui::Button::new(if ja { "元に戻す" } else { "Discard" })).clicked() {
                groups = app.cfg().keywords.clone();
                texts = groups.iter().map(|g| g.words.join("\n")).collect();
            }
        });
    });
    app.settings.keywords = Some(groups);
    app.settings.keyword_texts = texts;
}

// ───────────────────────── 許可リスト ─────────────────────────

fn allowlist(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    let a = app.cfg().allowlist.clone();
    let saved = [a.values.join("\n"), a.domains.join("\n"), a.patterns.join("\n")];
    if app.settings.allow.is_none() {
        app.settings.allow = Some(saved.clone());
    }
    let mut bufs = app.settings.allow.take().unwrap();
    widgets::secondary(ui, app.t("ここに登録した値はマスクしません。全プロファイル共通です。", "Values listed here are never masked. Shared by all profiles."));
    ui.add_space(8.0);
    let labels = [
        (if ja { "値 (完全一致・大文字小文字無視)" } else { "Values (exact, case-insensitive)" }, if ja { "例: support@example.co.jp" } else { "e.g. support@example.com" }),
        (if ja { "ドメイン (メール・ホスト名・URL、サブドメイン含む)" } else { "Domains (email/host/URL incl. subdomains)" }, "example.com"),
        (if ja { "正規表現 (値全体に一致)" } else { "Regex (full match)" }, r"192\.168\..*"),
    ];
    for (i, (label, hint)) in labels.iter().enumerate() {
        widgets::card(ui, |ui| {
            ui.label(*label);
            ui.add(TextEdit::multiline(&mut bufs[i]).desired_rows(4).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace).hint_text(*hint));
        });
        ui.add_space(6.0);
    }
    let dirty = bufs != saved;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let save = ui.add_enabled_ui(dirty, |ui| widgets::accent_button(ui, Icon::Save, if ja { "保存" } else { "Save" })).inner;
            if save.clicked() {
                let split = |s: &str| -> Vec<String> { s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect() };
                let (v, d, p) = (split(&bufs[0]), split(&bufs[1]), split(&bufs[2]));
                if let Some(bad) = p.iter().find(|x| regex::Regex::new(x).is_err()) {
                    app.toast(ToastKind::Error, format!("{}: {bad}", if ja { "正規表現が不正です" } else { "Invalid regex" }));
                } else {
                    app.edit_config(|doc, _| {
                        let _ = doc.set_serialized("allowlist.values", &v);
                        let _ = doc.set_serialized("allowlist.domains", &d);
                        let _ = doc.set_serialized("allowlist.patterns", &p);
                    });
                    app.toast(ToastKind::Success, if ja { "許可リストを保存しました" } else { "Saved allowlist" });
                    bufs = [v.join("\n"), d.join("\n"), p.join("\n")];
                }
            }
            if ui.add_enabled(dirty, egui::Button::new(if ja { "元に戻す" } else { "Discard" })).clicked() {
                bufs = saved.clone();
            }
        });
    });
    app.settings.allow = Some(bufs);
}

// ───────────────────────── プロファイル ─────────────────────────

fn profiles(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    widgets::secondary(ui, app.t(
        "プロファイルは基本設定の上に重ねる差分です。用途 (LLM 送信用・ログ共有用など) ごとに切り替えられます。検出対象・マスク表示・人名の設定は、選択中のプロファイルに保存されます。",
        "Profiles are overlays on top of the base settings. Detector, replacement and name settings are saved to the active profile.",
    ));
    ui.add_space(8.0);
    let active = app.loaded.active_profile.clone();
    let list = app.loaded.profiles.clone();
    let mut use_p: Option<String> = None;
    let mut del: Option<String> = None;
    widgets::card(ui, |ui| {
        for (i, pr) in list.iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            ui.horizontal(|ui| {
                let name = if pr.name == "default" { if ja { "既定 (default)" } else { "Default" }.to_string() } else { pr.name.clone() };
                ui.label(RichText::new(name).strong());
                widgets::secondary(ui, &pr.description);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if pr.name != "default" {
                        if app.settings.confirm_delete.as_deref() == Some(&pr.name) {
                            if ui.button(if ja { "本当に削除" } else { "Confirm delete" }).clicked() {
                                del = Some(pr.name.clone());
                            }
                        } else if widgets::bar_button(ui, Icon::Delete, "", true).on_hover_text(if ja { "削除" } else { "Delete" }).clicked() {
                            app.settings.confirm_delete = Some(pr.name.clone());
                        }
                    }
                    if pr.name == active {
                        let p = Palette::current(ui.ctx());
                        widgets::badge(ui, if ja { "使用中" } else { "Active" }, p.accent.gamma_multiply(0.2), p.accent);
                    } else if ui.button(if ja { "使用する" } else { "Use" }).clicked() {
                        use_p = Some(pr.name.clone());
                    }
                });
            });
        }
    });
    if let Some(p) = use_p {
        app.set_base("general.active_profile", p.as_str());
    }
    if let Some(p) = del {
        app.settings.confirm_delete = None;
        let was_active = p == active;
        app.edit_config(|d, _| {
            d.remove(&format!("profiles.{p}"));
            if was_active {
                d.set("general.active_profile", "default");
            }
        });
    }
    ui.add_space(12.0);
    widgets::subtitle(ui, app.t("新しいプロファイル", "New profile"));
    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(if ja { "名前" } else { "Name" });
            ui.add(TextEdit::singleline(&mut app.settings.new_profile.0).hint_text("my_profile").desired_width(160.0));
            ui.label(if ja { "説明" } else { "Description" });
            ui.add(TextEdit::singleline(&mut app.settings.new_profile.1).desired_width(260.0));
            let name = app.settings.new_profile.0.trim().to_string();
            let valid = !name.is_empty() && name != "default" && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') && !list.iter().any(|p| p.name == name);
            if ui.add_enabled(valid, egui::Button::new(if ja { "作成して使用" } else { "Create & use" })).clicked() {
                let desc = app.settings.new_profile.1.clone();
                app.edit_config(|d, _| {
                    d.set(&format!("profiles.{name}.description"), desc.as_str());
                    d.set("general.active_profile", name.as_str());
                });
                app.settings.new_profile = Default::default();
            }
        });
        widgets::secondary(ui, app.t("名前は半角英数字・_・- のみ", "Letters, digits, _ and - only"));
    });
}

// ───────────────────────── キー操作とトレイ ─────────────────────────

/// キー操作とトレイ: アプリ内のショートカット → グローバルホットキー → タスクトレイ。
fn keys_section(app: &mut App, ui: &mut Ui) {
    shortcuts_section(app, ui);
    ui.add_space(12.0);
    tray(app, ui);
}

fn tray(app: &mut App, ui: &mut Ui) {
    let g = app.cfg().gui.clone();
    widgets::subtitle(ui, app.t("グローバルホットキー", "Global hotkey"));
    hotkey_card(app, ui, &g);
    ui.add_space(12.0);
    widgets::subtitle(ui, app.t("タスクトレイ", "System tray"));
    widgets::card(ui, |ui| {
        let mut v = g.tray_enabled;
        if widgets::toggle_row(ui, app.t("タスクトレイにアイコンを表示", "Show tray icon"), "", &mut v) {
            app.set_base("gui.tray_enabled", v);
        }
        ui.separator();
        let mut v = g.close_to_tray;
        if widgets::toggle_row(ui, app.t("閉じるボタンでトレイに格納", "Close to tray"), app.t("ウィンドウを閉じても常駐し、ホットキーを使えます。終了はトレイメニューから。", "Keeps running so the hotkey works. Quit from the tray menu."), &mut v) {
            app.set_base("gui.close_to_tray", v);
        }
        ui.separator();
        let mut v = g.start_in_tray;
        if widgets::toggle_row(ui, app.t("起動時はトレイに格納", "Start in tray"), "", &mut v) {
            app.set_base("gui.start_in_tray", v);
        }
    });
}

fn hotkey_card(app: &mut App, ui: &mut Ui, g: &config::GuiConfig) {
    widgets::card(ui, |ui| {
        widgets::secondary(ui, app.t(
            "どのアプリを使っていても、ホットキーでクリップボードのテキストをその場でマスクします。コピー → ホットキー → 貼り付け で、マスク済みの文章を貼り付けられます。",
            "From any app, the hotkey masks the clipboard text in place: copy → hotkey → paste.",
        ));
        ui.add_space(4.0);
        let mut v = g.hotkey_enabled;
        if widgets::toggle_row(ui, app.t("ホットキーを使う", "Enable hotkey"), "", &mut v) {
            app.set_base("gui.hotkey_enabled", v);
        }
        ui.separator();
        let (label, desc) = (app.t("キーの組み合わせ", "Key combination"), app.t("ボタンを押してからキーを押します", "Click, then press the keys"));
        shortcut_row(app, ui, "hotkey", label, desc, &g.hotkey, HOTKEY_DEFAULT);
        ui.separator();
        let mut v = g.hotkey_notify;
        if widgets::toggle_row(ui, app.t("マスクしたら通知", "Notify after masking"), app.t("ウィンドウが隠れているときは Windows の通知で件数を表示", "Shows a Windows notification when the window is hidden"), &mut v) {
            app.set_base("gui.hotkey_notify", v);
        }
        ui.separator();
        if widgets::icon_button(ui, Icon::Play, app.t("今すぐクリップボードをマスク", "Mask clipboard now")).clicked() {
            app.mask_clipboard();
        }
    });
}

// ───────────────────────── ショートカットキー ─────────────────────────

const HOTKEY_DEFAULT: &str = "Ctrl+Alt+M";

fn shortcuts_section(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    widgets::subtitle(ui, app.t("アプリ内のショートカット", "Shortcuts in Sumiveil"));
    widgets::card(ui, |ui| {
        widgets::secondary(ui, app.t(
            "ボタンを押してから、割り当てたいキーを押してください。Esc で取り消し、Backspace で「なし」(無効) になります。Ctrl か Alt を含めてください (F1〜F24 は単独でも使えます)。",
            "Click a button, then press the keys to assign. Esc cancels, Backspace sets it to None. Include Ctrl or Alt (F1–F24 can be used alone).",
        ));
        ui.add_space(4.0);
        for (i, a) in Action::ALL.into_iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            let current = a.spec(&app.cfg().gui.shortcuts).to_string();
            shortcut_row(app, ui, a.id(), a.label(ja), "", &current, &a.default_spec());
        }
    });
    ui.add_space(6.0);
    if widgets::icon_button(ui, Icon::Refresh, app.t("キーをすべて既定に戻す", "Reset all keys to defaults")).clicked() {
        app.end_recording();
        app.edit_config(|d, _| {
            for a in Action::ALL {
                d.set(&format!("gui.shortcuts.{}", a.id()), a.default_spec());
            }
            d.set("gui.hotkey", HOTKEY_DEFAULT);
        });
    }
}

/// 操作 (またはグローバルホットキー = "hotkey") の表示名。
fn shortcut_owner_label(id: &str, ja: bool) -> &'static str {
    match Action::ALL.into_iter().find(|a| a.id() == id) {
        Some(a) => a.label(ja),
        None if ja => "グローバルホットキー",
        None => "Global hotkey",
    }
}

/// 割り当ててよいキーか。だめなら理由。
fn check_shortcut(app: &App, id: &str, spec: &str) -> Result<(), String> {
    let ja = app.lang.is_ja();
    let shown = crate::shortcuts::display(spec);
    match config::normalize_shortcut(spec) {
        Err(config::ShortcutError::Reserved) => {
            let has_mod = spec.contains("Ctrl") || spec.contains("Alt");
            return Err(match (ja, has_mod) {
                (true, true) => format!("{shown} は入力欄のコピー・貼り付けなどに使うため割り当てられません。別のキーを選んでください"),
                (true, false) => format!("{shown} は文字入力に使うため単独では割り当てられません。Ctrl か Alt と組み合わせてください"),
                (false, true) => format!("{shown} is used for copy/paste in text boxes. Choose another key"),
                (false, false) => format!("{shown} is used for typing. Combine it with Ctrl or Alt"),
            });
        }
        Err(config::ShortcutError::Syntax) => return Err(format!("{}: {shown}", app.t("このキーは使えません", "This key cannot be used"))),
        Ok(_) => {}
    }
    if let Some(other) = config::shortcut_conflict(&app.cfg().gui, id, spec) {
        let other = shortcut_owner_label(other, ja);
        return Err(if ja { format!("{shown} は「{other}」で使われています") } else { format!("{shown} is already used for \"{other}\"") });
    }
    if id == "hotkey" {
        if let Err(e) = crate::tray::parse(spec) {
            return Err(format!("{}: {shown} ({e})", app.t("ホットキーとして登録できないキーです", "Cannot be used as a global hotkey")));
        }
    }
    Ok(())
}

fn apply_shortcut(app: &mut App, id: &str, spec: &str) {
    let key = if id == "hotkey" { "gui.hotkey".to_string() } else { format!("gui.shortcuts.{id}") };
    app.set_base(&key, spec);
}

/// ショートカット 1 行: キー登録ボタンと「既定に戻す」ボタン。
fn shortcut_row(app: &mut App, ui: &mut Ui, id: &'static str, label: &str, desc: &str, current: &str, default: &str) {
    let recording = app.shortcut_recording == Some(id);
    // 押されたキーは、ボタンなどが Enter / Space として反応する前に取り出す
    if recording {
        match crate::shortcuts::take_recorded(ui.ctx()) {
            Some(Recorded::Cancel) => app.end_recording(),
            Some(Recorded::Clear) => {
                app.end_recording();
                apply_shortcut(app, id, "");
            }
            Some(Recorded::Key(sc)) => {
                let spec = if id == "hotkey" { crate::shortcuts::format_for_hotkey(&sc) } else { crate::shortcuts::format(&sc) };
                app.end_recording();
                match check_shortcut(app, id, &spec) {
                    Ok(()) => apply_shortcut(app, id, &spec),
                    Err(msg) => app.toast(ToastKind::Error, msg),
                }
            }
            None => {}
        }
    }
    // 上で登録を終えた場合も、行はこのフレームから新しい状態で描く
    let recording = app.shortcut_recording == Some(id);
    let current = match id {
        "hotkey" => app.cfg().gui.hotkey.clone(),
        _ => app.cfg().gui.shortcuts.get(id).unwrap_or(current).to_string(),
    };
    let current = current.as_str();
    let text = if recording {
        RichText::new(app.t("キーを押してください… (Esc で取り消し)", "Press keys… (Esc to cancel)"))
    } else if current.trim().is_empty() {
        RichText::new(app.t("なし", "None")).italics()
    } else {
        RichText::new(crate::shortcuts::display(current)).monospace()
    };
    let reset_tip = format!("{} ({})", app.t("既定に戻す", "Reset to default"), crate::shortcuts::display(default));
    let mut clicked = false;
    let mut reset = false;
    let mut button_rect = egui::Rect::NOTHING;
    widgets::setting_row(ui, label, desc, |ui| {
        // 行の右端から並ぶ: 既定に戻す → キー登録ボタン
        let color = ui.visuals().text_color();
        let r = ui.add_enabled(current != default, egui::Button::new(widgets::icon_label(ui, Icon::Refresh, "", color)).frame_when_inactive(false));
        reset = r.on_hover_text(reset_tip).clicked();
        let b = widgets::key_button(ui, text, recording);
        clicked = b.clicked();
        button_rect = b.rect;
        if clicked {
            // キーボードの Enter / Space でボタンが押し直されないようにする
            b.surrender_focus();
        }
    });
    if clicked {
        if recording {
            app.end_recording();
        } else {
            app.begin_recording(id);
        }
    } else if reset {
        app.end_recording();
        apply_shortcut(app, id, default);
    } else if recording && ui.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !button_rect.contains(p))) {
        // ほかの場所をクリックしたら取り消し
        app.end_recording();
    }
}

// ───────────────────────── 設定ファイル ─────────────────────────

fn file(app: &mut App, ui: &mut Ui) {
    let ja = app.lang.is_ja();
    let path = app.cfg_path.clone();
    widgets::card(ui, |ui| {
        let r = ui.label(RichText::new(path.display().to_string()).monospace());
        crate::guide_mark!(ui.ctx(), "files-cfg-path", r.rect);
        widgets::secondary(ui, if path.exists() {
            app.t("このファイルを直接編集しても自動で反映されます。チームでの共有には include やエクスポート/インポートを使えます。", "Edits to this file are applied automatically. Use include or export/import to share settings.")
        } else {
            app.t("(まだ作成されていません。設定を変更すると作成されます)", "(Not created yet. It is created when you change a setting.)")
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if widgets::icon_button(ui, Icon::Edit, if ja { "エディタで開く" } else { "Open in editor" }).clicked() {
                if !path.exists() {
                    if let Some(d) = path.parent() {
                        let _ = std::fs::create_dir_all(d);
                    }
                    let _ = std::fs::write(&path, DEFAULT_TOML);
                }
                let _ = std::process::Command::new("notepad.exe").arg(&path).spawn();
            }
            if widgets::icon_button(ui, Icon::Folder, if ja { "フォルダを開く" } else { "Open folder" }).clicked() {
                if path.exists() {
                    win::reveal_in_explorer(&path);
                } else if let Some(d) = path.parent() {
                    let _ = std::fs::create_dir_all(d);
                    win::open_in_explorer(d);
                }
            }
            if widgets::icon_button(ui, Icon::Refresh, if ja { "再読み込み" } else { "Reload" }).clicked() {
                app.reload_config();
                app.settings.custom_rules = None;
                app.settings.keywords = None;
                app.settings.allow = None;
            }
            if widgets::icon_button(ui, Icon::Save, if ja { "エクスポート…" } else { "Export…" }).clicked() {
                if let Some(to) = rfd::FileDialog::new().set_file_name("sumiveil-config.toml").add_filter("TOML", &["toml"]).save_file() {
                    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| DEFAULT_TOML.to_string());
                    match std::fs::write(&to, text) {
                        Ok(()) => app.toast(ToastKind::Success, format!("{}: {}", if ja { "エクスポートしました" } else { "Exported" }, to.display())),
                        Err(e) => app.toast(ToastKind::Error, e.to_string()),
                    }
                }
            }
            if widgets::icon_button(ui, Icon::Open, if ja { "インポート…" } else { "Import…" }).clicked() {
                if let Some(from) = rfd::FileDialog::new().add_filter("TOML", &["toml"]).pick_file() {
                    match config::load(&from, None) {
                        Ok(_) => {
                            if path.exists() {
                                let _ = std::fs::copy(&path, path.with_extension("toml.bak"));
                            }
                            if let Some(d) = path.parent() {
                                let _ = std::fs::create_dir_all(d);
                            }
                            match std::fs::copy(&from, &path) {
                                Ok(_) => {
                                    app.reload_config();
                                    app.settings.custom_rules = None;
                                    app.settings.keywords = None;
                                    app.settings.allow = None;
                                    app.toast(ToastKind::Success, if ja { "インポートしました (以前の設定は .bak に保存)" } else { "Imported (previous settings saved as .bak)" });
                                }
                                Err(e) => app.toast(ToastKind::Error, e.to_string()),
                            }
                        }
                        Err(e) => app.toast(ToastKind::Error, e.to_string()),
                    }
                }
            }
        });
    });
    ui.add_space(12.0);
    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(app.t("すべての設定を既定に戻す", "Reset all settings to defaults"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if app.settings.confirm_reset {
                    if ui.button(if ja { "本当に戻す" } else { "Confirm reset" }).clicked() {
                        if path.exists() {
                            let _ = std::fs::copy(&path, path.with_extension("toml.bak"));
                        }
                        if let Some(d) = path.parent() {
                            let _ = std::fs::create_dir_all(d);
                        }
                        let _ = std::fs::write(&path, DEFAULT_TOML);
                        app.settings.confirm_reset = false;
                        app.settings.custom_rules = None;
                        app.settings.keywords = None;
                        app.settings.allow = None;
                        app.reload_config();
                    }
                    if ui.button(if ja { "キャンセル" } else { "Cancel" }).clicked() {
                        app.settings.confirm_reset = false;
                    }
                } else if ui.button(if ja { "既定に戻す…" } else { "Reset…" }).clicked() {
                    app.settings.confirm_reset = true;
                }
            });
        });
    });
    let mut warnings = app.loaded.warnings.clone();
    warnings.extend(app.engine.warnings.iter().cloned());
    if !warnings.is_empty() {
        ui.add_space(12.0);
        widgets::subtitle(ui, app.t("警告", "Warnings"));
        for w in warnings {
            widgets::info_bar(ui, InfoKind::Warning, &w);
            ui.add_space(4.0);
        }
    }
}
