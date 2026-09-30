//! Office 文書のプロパティ (作成者・会社名など) をどう扱うかの選択 (設定 `files.properties = "ask"` のとき)。

use eframe::egui::{self, RichText, Ui};
use sumiveil_core::formats::{PropertiesMode, Property};

use crate::widgets;

/// 選択ボタンの行 ([マスクする] [消す] [そのまま残す] と「今後もこの方法にする」)。選ばれたら Some。
pub fn buttons(ui: &mut Ui, ja: bool, remember: &mut bool) -> Option<PropertiesMode> {
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        let t = |a: &'static str, b: &'static str| if ja { a } else { b };
        if widgets::accent_button(ui, crate::icons::Icon::Clear, t("消す", "Clear")).on_hover_text(t("値を空にします (確実)", "Empties the values (safest)")).clicked() {
            chosen = Some(PropertiesMode::Clear);
        }
        if ui.button(t("マスクする", "Mask")).on_hover_text(t("本文と同じ方法でマスクします (敬称のない名前などは見逃すことがあります)", "Masks like the body text (may miss plain names)")).clicked() {
            chosen = Some(PropertiesMode::Mask);
        }
        if ui.button(t("そのまま残す", "Keep")).clicked() {
            chosen = Some(PropertiesMode::Keep);
        }
        ui.add_space(8.0);
        ui.checkbox(remember, t("今後もこの方法にする", "Always do this"));
    });
    chosen
}

/// プロパティの一覧 (「作成者: 山田太郎」)。
pub fn list(ui: &mut Ui, props: &[Property]) {
    for p in props.iter().take(8) {
        ui.label(RichText::new(format!("{}: {}", p.label, p.value)).size(12.5));
    }
    if props.len() > 8 {
        widgets::secondary(ui, &format!("… (+{})", props.len() - 8));
    }
}

/// 選択ダイアログ。Some(Some(mode)) = 選んだ、Some(None) = 取り消し、None = 表示中。
pub fn dialog(ctx: &egui::Context, id: &str, ja: bool, props: &[Property], remember: &mut bool) -> Option<Option<PropertiesMode>> {
    let mut out = None;
    let r = egui::Modal::new(egui::Id::new(id)).show(ctx, |ui| {
        ui.set_max_width(520.0);
        widgets::subtitle(ui, if ja { "文書のプロパティをどうしますか?" } else { "What should happen to document properties?" });
        ui.add_space(4.0);
        widgets::secondary(
            ui,
            if ja {
                "Office 文書には、本文とは別に作成者・最終更新者・会社名・コメントの作成者などが記録されています。"
            } else {
                "Office documents store the author, last editor, company and comment authors separately from the body."
            },
        );
        ui.add_space(6.0);
        if props.is_empty() {
            widgets::secondary(ui, if ja { "(対象のファイルに含まれていれば、選んだ方法で処理します)" } else { "(Applied to files that contain them)" });
        } else {
            list(ui, props);
        }
        ui.add_space(10.0);
        if let Some(m) = buttons(ui, ja, remember) {
            out = Some(Some(m));
        }
        ui.add_space(4.0);
        if ui.button(if ja { "キャンセル" } else { "Cancel" }).clicked() {
            out = Some(None);
        }
    });
    if out.is_none() && r.should_close() {
        out = Some(None);
    }
    out
}
