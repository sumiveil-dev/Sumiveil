//! Sumiveil の小さな部品 (「墨と和紙」のデザイン)。
//! 角は小さめ (3〜4px)、線は細く、選択や強調は強調色の直線で示す。アイコンは icons.rs の自作アイコンを使う。

use eframe::egui::{
    self, Align, AtomLayout, Atoms, Color32, CornerRadius, FontFamily, FontId, Frame, Margin, Response, RichText, Sense, Stroke, TextWrapMode, Ui, Vec2,
};

use crate::fonts::SEMIBOLD;
use crate::icons::{self, Icon};
use crate::theme::Palette;

/// ボタンやラベルの中のアイコンの大きさ (論理ピクセル)
pub const ICON_SIZE: f32 = 16.0;

/// アイコン + ラベル (ボタンやラベルの中身)。ラベルが空ならアイコンだけ。
pub fn icon_label(ui: &Ui, icon: Icon, label: &str, color: Color32) -> Atoms<'static> {
    let img = icons::image(ui.ctx(), icon, ICON_SIZE, color);
    if label.is_empty() {
        Atoms::new(img)
    } else {
        Atoms::new((img, RichText::new(label.to_string()).color(color)))
    }
}

/// アイコン付きの文字 (表示用)。
pub fn icon_text(ui: &mut Ui, icon: Icon, label: &str, color: Color32) -> Response {
    let atoms = icon_label(ui, icon, label, color);
    ui.add(AtomLayout::new(atoms).wrap_mode(TextWrapMode::Wrap))
}

/// アイコン付きの文字。入りきらない部分は「…」で省く。
pub fn icon_text_truncated(ui: &mut Ui, icon: Icon, label: &str, color: Color32) -> Response {
    let atoms = icon_label(ui, icon, label, color);
    let w = ui.available_width();
    ui.add(AtomLayout::new(atoms).wrap_mode(TextWrapMode::Truncate).max_width(w))
}

pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    let color = ui.visuals().text_color();
    ui.add(egui::Button::new(icon_label(ui, icon, label, color)))
}

/// 枠なしのツールバー用ボタン。
pub fn bar_button(ui: &mut Ui, icon: Icon, label: &str, enabled: bool) -> Response {
    let color = ui.visuals().text_color();
    ui.add_enabled(enabled, egui::Button::new(icon_label(ui, icon, label, color)).frame_when_inactive(false))
}

/// 主操作のボタン (強調色の地)。
pub fn accent_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    let p = Palette::current(ui.ctx());
    let text = icon_label(ui, icon, label, p.accent_text);
    ui.add(egui::Button::new(text).fill(p.accent).stroke(Stroke::new(1.0, p.accent)))
}

/// 上部の操作列のボタン。`compact` のときは文字を隠し、ツールチップに出す。
pub fn command_button(ui: &mut Ui, icon: Icon, label: &str, enabled: bool, compact: bool) -> Response {
    let color = ui.visuals().text_color();
    let text = icon_label(ui, icon, if compact { "" } else { label }, color);
    let r = ui.add_enabled(enabled, egui::Button::new(text).frame_when_inactive(false).min_size(Vec2::new(32.0, 32.0)));
    if compact {
        r.on_hover_text(label)
    } else {
        r
    }
}

/// 切り替え状態を持つ操作列のボタン (検索バーの表示中など)。オンのときは薄い強調色の地と、下に強調色の線。
pub fn command_toggle(ui: &mut Ui, icon: Icon, label: &str, on: bool, compact: bool) -> Response {
    let p = Palette::current(ui.ctx());
    let text = icon_label(ui, icon, if compact { "" } else { label }, p.text);
    let mut b = egui::Button::new(text).min_size(Vec2::new(32.0, 32.0));
    b = if on { b.fill(p.accent_soft).stroke(Stroke::NONE) } else { b.frame_when_inactive(false) };
    let r = ui.add(b);
    if on {
        underline(ui, r.rect, 6.0, p.accent);
    }
    if compact {
        r.on_hover_text(label)
    } else {
        r
    }
}

/// rect の下端に強調色の直線を引く (選択中の印)。
fn underline(ui: &Ui, rect: egui::Rect, inset: f32, color: Color32) {
    let line = egui::Rect::from_min_max(egui::pos2(rect.left() + inset, rect.bottom() - 2.0), egui::pos2(rect.right() - inset, rect.bottom()));
    ui.painter().rect_filled(line, CornerRadius::ZERO, color);
}

/// 分割ボタン: 左は主操作 (強調色の地)、右の ⌄ でメニューを開く。主操作の Response を返す。
pub fn split_button(ui: &mut Ui, icon: Icon, label: &str, enabled: bool, compact: bool, menu: impl FnOnce(&mut Ui)) -> Response {
    let p = Palette::current(ui.ctx());
    // 右寄せのレイアウトの中でも、主ボタンが左・⌄ が右になるようにする
    let rtl = ui.layout().prefer_right_to_left();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        let add_main = |ui: &mut Ui| {
            let text = icon_label(ui, icon, if compact { "" } else { label }, p.accent_text);
            let left = CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 };
            let main = ui.add_enabled(enabled, egui::Button::new(text).fill(p.accent).stroke(Stroke::new(1.0, p.accent)).corner_radius(left).min_size(Vec2::new(32.0, 32.0)));
            if compact { main.on_hover_text(label) } else { main }
        };
        let add_menu = |ui: &mut Ui| {
            let chevron = icon_label(ui, Icon::ChevronDown, "", p.accent_text);
            let right = CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 };
            let b = egui::Button::new(chevron).fill(p.accent).stroke(Stroke::new(1.0, p.accent)).corner_radius(right).min_size(Vec2::new(28.0, 32.0));
            egui::containers::menu::MenuButton::from_button(b).ui(ui, |ui| {
                ui.set_min_width(220.0);
                menu(ui)
            });
        };
        if rtl {
            add_menu(ui);
            add_main(ui)
        } else {
            let main = add_main(ui);
            add_menu(ui);
            main
        }
    })
    .inner
}

/// 押すとポップアップを開くボタン。中の操作でポップアップは閉じない (外側のクリックで閉じる)。
pub fn flyout_button(ui: &mut Ui, text: Atoms<'static>, width: f32, content: impl FnOnce(&mut Ui)) -> Response {
    let r = ui.add(egui::Button::new(text).frame_when_inactive(false));
    egui::Popup::from_toggle_button_response(&r)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .width(width)
        .show(|ui| {
            ui.set_min_width(width);
            content(ui)
        });
    r
}

/// 上部のタブ: 選択中の項目は太字にし、下に強調色の直線を引く。変更されたら true。
pub fn tabs<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let p = Palette::current(ui.ctx());
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (v, label) in options {
            let selected = *value == *v;
            let text = RichText::new(*label).size(14.0).color(if selected { p.text } else { p.text_secondary });
            let text = if selected { text.font(FontId::new(14.0, FontFamily::Name(SEMIBOLD.into()))) } else { text };
            let r = ui.add(egui::Button::new(text).frame_when_inactive(false).min_size(Vec2::new(0.0, 32.0)));
            if selected {
                underline(ui, r.rect, 8.0, p.accent);
            }
            if r.clicked() && !selected {
                *value = *v;
                changed = true;
            }
        }
    });
    changed
}

/// 折りたたみ: カードの見出し行を押すと中身を開閉する。
pub fn expander<R>(ui: &mut Ui, id_salt: &str, title: &str, desc: &str, default_open: bool, content: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let p = Palette::current(ui.ctx());
    let id = ui.make_persistent_id(("expander", id_salt));
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
    let mut out = None;
    Frame::new().fill(p.card).stroke(Stroke::new(1.0, p.card_stroke)).corner_radius(CornerRadius::same(4)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let open = state.is_open();
        let header = Frame::new().inner_margin(Margin::symmetric(16, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(title);
                    if !desc.is_empty() {
                        secondary(ui, desc);
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let icon = if open { Icon::ChevronUp } else { Icon::ChevronDown };
                    ui.add(icons::image(ui.ctx(), icon, ICON_SIZE, p.text_secondary));
                });
            });
        });
        let r = ui.interact(header.response.rect, id.with("header"), Sense::click());
        if r.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if r.clicked() {
            state.toggle(ui);
        }
        if state.is_open() {
            ui.separator();
            Frame::new().inner_margin(Margin { left: 16, right: 16, top: 4, bottom: 12 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                out = Some(content(ui));
            });
        }
    });
    state.store(ui.ctx());
    out
}

/// カード (和紙の地 + 細い枠 + 角丸 4)。
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = Palette::current(ui.ctx());
    Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.card_stroke))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// ページの見出し。左に強調色の短い縦線を添える。
pub fn title(ui: &mut Ui, text: &str) {
    let p = Palette::current(ui.ctx());
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(4.0, 24.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::ZERO, p.accent);
        ui.add_space(4.0);
        ui.label(RichText::new(text).font(FontId::new(26.0, FontFamily::Name(SEMIBOLD.into()))));
    });
}

pub fn subtitle(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(FontId::new(16.0, FontFamily::Name(SEMIBOLD.into()))));
}

pub fn secondary(ui: &mut Ui, text: &str) -> Response {
    let p = Palette::current(ui.ctx());
    ui.label(RichText::new(text).color(p.text_secondary).size(12.5))
}

/// ドラッグ中など、操作途中の値を一時メモリから取り出す。
/// 画面は操作に追従させつつ、設定ファイルへの保存は操作の終わりに 1 回だけにするために使う。
pub fn pending<T: Clone + Send + Sync + 'static>(ctx: &egui::Context, id: egui::Id) -> Option<T> {
    ctx.data(|d| d.get_temp(id))
}

/// 操作途中の値を保持する (`None` で破棄)。
pub fn set_pending<T: Clone + Send + Sync + 'static>(ctx: &egui::Context, id: egui::Id, value: Option<T>) {
    ctx.data_mut(|d| match value {
        Some(v) => {
            d.insert_temp(id, v);
        }
        None => d.remove::<T>(id),
    });
}

/// キー登録ボタン。現在のキーを表示し、登録中は強調色の枠で強調する。
pub fn key_button(ui: &mut Ui, text: RichText, recording: bool) -> Response {
    let p = Palette::current(ui.ctx());
    let (fill, stroke) = if recording { (p.accent_soft, Stroke::new(1.5, p.accent)) } else { (p.control, Stroke::new(1.0, p.control_stroke)) };
    ui.add(egui::Button::new(text).fill(fill).stroke(stroke).min_size(Vec2::new(200.0, 30.0)))
}

/// 色見本 (角の小さい四角)。選択中は外側に墨の枠を付けてチェックを出す。
pub fn color_swatch(ui: &mut Ui, color: Color32, selected: bool) -> Response {
    let p = Palette::current(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
    let inner = rect.shrink(if selected { 4.0 } else if resp.hovered() { 1.0 } else { 2.0 });
    let painter = ui.painter();
    if selected {
        painter.rect_stroke(rect.shrink(1.0), CornerRadius::same(4), Stroke::new(2.0, p.text), egui::StrokeKind::Inside);
    }
    painter.rect_filled(inner, CornerRadius::same(2), color);
    if selected {
        let lum = 0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32;
        let mark = if lum > 150.0 { Color32::BLACK } else { Color32::WHITE };
        icons::paint(painter, Icon::Check, inner.center(), 18.0, mark);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// オン/オフの切り替え。角の小さい長方形の溝と、四角いつまみ。
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let p = Palette::current(ui.ctx());
    let size = Vec2::new(40.0, 20.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let hovered = resp.hovered();
    let (fill, stroke, knob) = if *on {
        (if hovered { p.accent_hover } else { p.accent }, p.accent, p.accent_text)
    } else {
        (if hovered { p.control_hover } else { p.control }, p.text_secondary, p.text_secondary)
    };
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(3), fill, Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
    let k = if hovered { 13.0 } else { 12.0 };
    let x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), t);
    painter.rect_filled(egui::Rect::from_center_size(egui::pos2(x, rect.center().y), Vec2::splat(k)), CornerRadius::same(2), knob);
    resp
}

/// スライダー。細い溝 (選択済みの側は強調色) と、縦長の四角いつまみ。
/// クリックした位置へ移動、ドラッグ、フォーカス中は矢印キー / Home / End で操作できる。
pub fn slider(ui: &mut Ui, value: &mut f64, range: std::ops::RangeInclusive<f64>, step: f64, width: f32) -> Response {
    let p = Palette::current(ui.ctx());
    let (min, max) = (*range.start(), *range.end());
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::click_and_drag());
    let track_l = rect.left() + 8.0;
    let track_r = rect.right() - 8.0;
    let snap = |v: f64| ((v / step).round() * step).clamp(min, max);
    if resp.clicked() || resp.drag_started() {
        resp.request_focus();
    }
    let mut target: Option<f64> = None;
    if resp.is_pointer_button_down_on() || resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let t = ((pos.x - track_l) / (track_r - track_l)).clamp(0.0, 1.0) as f64;
            target = Some(min + t * (max - min));
        }
    }
    if resp.has_focus() {
        let (dec, inc, home, end) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
            )
        });
        let cur = target.unwrap_or(*value);
        if dec {
            target = Some(cur - step);
        } else if inc {
            target = Some(cur + step);
        } else if home {
            target = Some(min);
        } else if end {
            target = Some(max);
        }
    }
    if let Some(v) = target.map(snap) {
        if (v - *value).abs() > step / 4.0 {
            *value = v;
            resp.mark_changed();
        }
    }

    let t = if max > min { ((*value - min) / (max - min)).clamp(0.0, 1.0) as f32 } else { 0.0 };
    let cy = rect.center().y;
    let x = egui::lerp(track_l..=track_r, t);
    let painter = ui.painter();
    // 溝
    painter.rect_filled(egui::Rect::from_min_max(egui::pos2(track_l, cy - 1.5), egui::pos2(track_r, cy + 1.5)), CornerRadius::ZERO, p.control_stroke);
    painter.rect_filled(egui::Rect::from_min_max(egui::pos2(track_l, cy - 1.5), egui::pos2(x, cy + 1.5)), CornerRadius::ZERO, p.accent);
    // つまみ (縦長の四角)
    let pressed = resp.is_pointer_button_down_on() || resp.dragged();
    let h = if pressed { 16.0 } else if resp.hovered() || resp.has_focus() { 20.0 } else { 18.0 };
    let knob = egui::Rect::from_center_size(egui::pos2(x, cy), Vec2::new(8.0, h));
    painter.rect(knob, CornerRadius::same(2), if pressed { p.accent_hover } else { p.accent }, Stroke::new(1.0, p.card), egui::StrokeKind::Outside);
    // キーボードフォーカスの枠
    if resp.has_focus() && !pressed {
        painter.rect_stroke(rect.shrink2(Vec2::new(0.0, 4.0)), CornerRadius::same(3), Stroke::new(1.5, p.text), egui::StrokeKind::Outside);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// 設定行: 左にラベルと説明、右に部品。
pub fn setting_row<R>(ui: &mut Ui, label: &str, desc: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    // 説明文が右側の操作部品に重ならないよう、前フレームで測った部品の幅を除いた幅で折り返す。
    // それでも文字の幅が足りないときは、部品を説明文の下に回す
    let id = ui.id().with(("setting_row", label));
    let control_w: f32 = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(160.0);
    let text_w = ui.available_width() - control_w - 24.0;
    let text = |ui: &mut Ui| {
        ui.label(label);
        if !desc.is_empty() {
            secondary(ui, desc);
        }
    };
    let mut out = None;
    let mut measured = control_w;
    let control = |ui: &mut Ui| {
        out = Some(add(ui));
        measured = ui.min_rect().width();
    };
    if text_w >= 180.0 {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_max_width(text_w);
                text(ui);
            });
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| control(ui));
        });
    } else {
        ui.vertical(|ui| {
            text(ui);
            ui.add_space(4.0);
            let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
            ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(Align::Center), |ui| control(ui));
        });
    }
    if (measured - control_w).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(id, measured));
        ui.ctx().request_repaint();
    }
    out.unwrap()
}

/// トグル付き設定行。変更されたら true。
pub fn toggle_row(ui: &mut Ui, label: &str, desc: &str, value: &mut bool) -> bool {
    setting_row(ui, label, desc, |ui| toggle(ui, value).changed())
}

/// セグメント切り替え。選択中の項目は墨の地に白抜きの文字。変更されたら true。
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, Icon, &str)]) -> bool {
    let p = Palette::current(ui.ctx());
    let mut changed = false;
    Frame::new()
        .fill(p.control)
        .stroke(Stroke::new(1.0, p.control_stroke))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(|ui| {
                for (v, icon, label) in options {
                    let selected = *value == *v;
                    let color = if selected { p.bg } else { p.text };
                    let mut b = egui::Button::new(icon_label(ui, *icon, label, color)).corner_radius(CornerRadius::same(2));
                    b = if selected { b.fill(p.text) } else { b.frame_when_inactive(false) };
                    if ui.add(b).clicked() && !selected {
                        *value = *v;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// ナビゲーション項目。選択中は左端に強調色の縦線を引く。
pub fn nav_item(ui: &mut Ui, icon: Icon, label: &str, selected: bool, expanded: bool) -> Response {
    let p = Palette::current(ui.ctx());
    let size = Vec2::new(ui.available_width(), 36.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let fill = if selected {
        p.accent_soft
    } else if resp.hovered() {
        p.control_hover
    } else {
        Color32::TRANSPARENT
    };
    let row = rect.shrink2(Vec2::new(4.0, 2.0));
    painter.rect_filled(row, CornerRadius::same(3), fill);
    if selected {
        let bar = egui::Rect::from_min_max(egui::pos2(row.left(), row.top() + 4.0), egui::pos2(row.left() + 3.0, row.bottom() - 4.0));
        painter.rect_filled(bar, CornerRadius::ZERO, p.accent);
    }
    icons::paint(painter, icon, egui::pos2(rect.left() + 26.0, rect.center().y), 18.0, p.text);
    if expanded {
        let font = if selected { FontId::new(14.0, FontFamily::Name(SEMIBOLD.into())) } else { FontId::proportional(14.0) };
        painter.text(egui::pos2(rect.left() + 48.0, rect.center().y), egui::Align2::LEFT_CENTER, label, font, p.text);
    }
    if expanded {
        resp
    } else {
        resp.on_hover_text(label)
    }
}

/// 色付きの小さな札。
pub fn badge(ui: &mut Ui, text: &str, bg: Color32, fg: Color32) -> Response {
    Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(3))
        .inner_margin(Margin::symmetric(7, 1))
        .show(ui, |ui| ui.label(RichText::new(text).color(fg).size(12.0)))
        .response
}

/// お知らせの帯。左端に種類の色の縦線、薄い地、アイコン。
pub fn info_bar(ui: &mut Ui, kind: InfoKind, text: &str) {
    let p = Palette::current(ui.ctx());
    let (color, icon) = match kind {
        InfoKind::Info => (p.accent, Icon::Info),
        InfoKind::Warning => (p.caution, Icon::Warning),
        InfoKind::Error => (p.critical, Icon::Warning),
        InfoKind::Success => (p.success, Icon::Check),
    };
    let bg = color.gamma_multiply(if p.dark { 0.16 } else { 0.10 });
    let r = Frame::new().fill(bg).corner_radius(CornerRadius::same(3)).inner_margin(Margin { left: 14, right: 12, top: 8, bottom: 8 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.add(icons::image(ui.ctx(), icon, ICON_SIZE, color));
            ui.label(RichText::new(text).color(p.text));
        });
    });
    let rect = r.response.rect;
    ui.painter().rect_filled(egui::Rect::from_min_max(rect.left_top(), egui::pos2(rect.left() + 3.0, rect.bottom())), CornerRadius::ZERO, color);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InfoKind {
    Info,
    Warning,
    Error,
    Success,
}
