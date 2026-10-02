//! 「墨と和紙」の配色とスタイル。
//! ライトは生成りの和紙の地に墨の文字、ダークは墨の地に和紙色の文字。強調色は Windows のアクセントカラー (手動指定も可)。

use eframe::egui::{
    self, style::Selection, Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Theme, Visuals,
};

use crate::fonts::SEMIBOLD;

/// Windows のアクセントカラーが取れないときの強調色。
pub const FALLBACK_ACCENT: u32 = 0x0078D4;

/// テーマごとの色トークン。
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub layer: Color32,
    pub card: Color32,
    pub card_stroke: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub control_pressed: Color32,
    pub control_stroke: Color32,
    pub editor_bg: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub accent_hover: Color32,
    /// 選択中の項目などに敷く薄い強調色
    pub accent_soft: Color32,
    pub success: Color32,
    pub caution: Color32,
    pub critical: Color32,
    pub gutter: Color32,
    /// 検索の一致箇所 (背景)
    pub find: Color32,
    /// 検索の現在の一致箇所 (背景)
    pub find_current: Color32,
}

pub fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// WCAG の相対輝度 (0..1)。
pub fn relative_luminance(c: Color32) -> f32 {
    let f = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
}

/// WCAG のコントラスト比 (1..21)。
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// 地の色 bg の上で、文字として 4.5:1 以上になるまで c を暗く (ダークでは明るく) する。
fn legible_on(c: Color32, bg: Color32, dark: bool) -> Color32 {
    let target = if dark { Color32::WHITE } else { Color32::BLACK };
    let mut out = c;
    for i in 1..=20 {
        if contrast(out, bg) >= 4.5 {
            break;
        }
        out = mix(c, target, i as f32 * 0.05);
    }
    out
}

impl Palette {
    /// accent: 手動指定か Windows のアクセントカラー。None なら既定の青。
    pub fn new(dark: bool, accent: Option<Color32>) -> Self {
        let base = accent.unwrap_or(hex(FALLBACK_ACCENT));
        let (bg, text) = if dark { (hex(0x171513), hex(0xECE7DD)) } else { (hex(0xF4F1EA), hex(0x1E1C1A)) };
        // 強調色は文字にも使うので、地の上で読める明るさにそろえる
        let accent = legible_on(if dark { mix(base, Color32::WHITE, 0.25) } else { base }, bg, dark);
        let accent_text = if contrast(accent, Color32::WHITE) >= contrast(accent, hex(0x1A1512)) { Color32::WHITE } else { hex(0x1A1512) };
        let accent_hover = if dark { mix(accent, Color32::WHITE, 0.12) } else { mix(accent, Color32::BLACK, 0.12) };
        let accent_soft = accent.gamma_multiply(if dark { 0.22 } else { 0.12 });
        if dark {
            Self {
                dark,
                bg,
                layer: hex(0x1D1B18),
                card: hex(0x22201C),
                card_stroke: hex(0x34302A),
                control: hex(0x26231F),
                control_hover: hex(0x2E2B26),
                control_pressed: hex(0x211E1A),
                control_stroke: hex(0x46413A),
                editor_bg: hex(0x12100E),
                text,
                text_secondary: hex(0xB3AB9D),
                accent,
                accent_text,
                accent_hover,
                accent_soft,
                success: hex(0x8CC47A),
                caution: hex(0xE2B44E),
                critical: hex(0xF08C8C),
                gutter: hex(0x7D766B),
                find: Color32::from_rgba_unmultiplied(0xE0, 0xB0, 0x28, 70),
                find_current: Color32::from_rgba_unmultiplied(0xF0, 0xBE, 0x28, 150),
            }
        } else {
            Self {
                dark,
                bg,
                layer: hex(0xEFEBE2),
                card: hex(0xFBF9F5),
                card_stroke: hex(0xE0D9CC),
                control: hex(0xFFFDF9),
                control_hover: hex(0xF1EDE5),
                control_pressed: hex(0xE9E4D9),
                control_stroke: hex(0xCFC7B8),
                editor_bg: hex(0xFFFEFB),
                text,
                text_secondary: hex(0x5E5850),
                accent,
                accent_text,
                accent_hover,
                accent_soft,
                success: hex(0x3C7233),
                caution: hex(0x8A5A00),
                critical: hex(0xA3262B),
                gutter: hex(0x9C9588),
                find: Color32::from_rgba_unmultiplied(0xF2, 0xC9, 0x4C, 120),
                find_current: Color32::from_rgba_unmultiplied(0xEE, 0xA8, 0x1E, 190),
            }
        }
    }

    pub fn current(ctx: &egui::Context) -> Self {
        ctx.data(|d| d.get_temp::<Palette>(egui::Id::new("sumiveil-palette-").with(ctx.theme() == Theme::Dark)))
            .unwrap_or_else(|| Palette::new(ctx.theme() == Theme::Dark, None))
    }

    /// カテゴリ別のハイライト色 (背景, 文字)。日本の伝統色を参考に、互いに見分けやすい色相を選んだ。
    pub fn category_colors(&self, category: &str) -> (Color32, Color32) {
        let base = match category {
            "contact" => hex(0x2F5D8A),      // 藍
            "personal" => hex(0xB33E5C),     // 紅
            "jp_id" => hex(0xC0682A),        // 柿
            "intl_id" => hex(0xA67C00),      // 山吹
            "finance" => hex(0x4F7F2F),      // 萌黄
            "network" => hex(0x2A8088),      // 浅葱
            "secret" => hex(0x6A4C93),       // 江戸紫
            "location" => hex(0x9A4F7A),     // 牡丹
            "organization" => hex(0x3E7F5E), // 若竹
            _ => hex(0x6E6A64),              // 鼠
        };
        if self.dark {
            (base.gamma_multiply(0.40), legible_on(mix(base, Color32::WHITE, 0.55), self.editor_bg, true))
        } else {
            (base.gamma_multiply(0.16), legible_on(mix(base, Color32::BLACK, 0.30), self.editor_bg, false))
        }
    }

    fn visuals(&self) -> Visuals {
        let mut v = if self.dark { Visuals::dark() } else { Visuals::light() };
        let r3 = CornerRadius::same(3);
        v.panel_fill = self.bg;
        v.window_fill = self.card;
        v.window_stroke = Stroke::new(1.0, self.card_stroke);
        v.window_corner_radius = CornerRadius::same(4);
        v.menu_corner_radius = CornerRadius::same(4);
        v.window_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(if self.dark { 100 } else { 30 }) };
        v.popup_shadow = Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(if self.dark { 90 } else { 26 }) };
        v.extreme_bg_color = self.editor_bg;
        v.text_edit_bg_color = Some(self.editor_bg);
        v.faint_bg_color = self.layer;
        v.code_bg_color = self.layer;
        v.hyperlink_color = self.accent;
        v.warn_fg_color = self.caution;
        v.error_fg_color = self.critical;
        v.override_text_color = None;
        v.selection = Selection { bg_fill: self.accent.gamma_multiply(if self.dark { 0.42 } else { 0.24 }), stroke: Stroke::new(1.0, self.accent) };
        v.text_cursor.stroke = Stroke::new(1.5, self.accent);

        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.card;
        w.noninteractive.weak_bg_fill = self.card;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.card_stroke);
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.text);
        w.noninteractive.corner_radius = r3;

        w.inactive.bg_fill = self.control;
        w.inactive.weak_bg_fill = self.control;
        w.inactive.bg_stroke = Stroke::new(1.0, self.control_stroke);
        w.inactive.fg_stroke = Stroke::new(1.0, self.text);
        w.inactive.corner_radius = r3;
        w.inactive.expansion = 0.0;

        w.hovered.bg_fill = self.control_hover;
        w.hovered.weak_bg_fill = self.control_hover;
        w.hovered.bg_stroke = Stroke::new(1.0, self.text_secondary);
        w.hovered.fg_stroke = Stroke::new(1.0, self.text);
        w.hovered.corner_radius = r3;
        w.hovered.expansion = 0.0;

        w.active.bg_fill = self.control_pressed;
        w.active.weak_bg_fill = self.control_pressed;
        w.active.bg_stroke = Stroke::new(1.0, self.accent);
        w.active.fg_stroke = Stroke::new(1.0, self.text_secondary);
        w.active.corner_radius = r3;
        w.active.expansion = 0.0;

        w.open.bg_fill = self.control_pressed;
        w.open.weak_bg_fill = self.control_pressed;
        w.open.bg_stroke = Stroke::new(1.0, self.control_stroke);
        w.open.fg_stroke = Stroke::new(1.0, self.text);
        w.open.corner_radius = r3;
        v
    }
}

/// テーマ・余白・文字サイズを設定する。accent は手動指定か Windows のアクセントカラー。
pub fn apply(ctx: &egui::Context, accent: Option<Color32>, mono_size: f32) {
    for theme in [Theme::Light, Theme::Dark] {
        let p = Palette::new(theme == Theme::Dark, accent);
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("sumiveil-palette-").with(theme == Theme::Dark), p));
        ctx.set_visuals_of(theme, p.visuals());
        ctx.style_mut_of(theme, |s| {
            s.spacing.item_spacing = egui::vec2(8.0, 6.0);
            s.spacing.button_padding = egui::vec2(11.0, 5.0);
            s.spacing.interact_size = egui::vec2(32.0, 30.0);
            s.spacing.icon_spacing = 7.0;
            s.spacing.window_margin = Margin::same(16);
            s.spacing.menu_margin = Margin::same(6);
            s.spacing.combo_height = 320.0;
            s.spacing.scroll = egui::style::ScrollStyle::thin();
            s.interaction.tooltip_delay = 0.35;
            s.text_styles = [
                (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
                (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
                (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
                (TextStyle::Heading, FontId::new(20.0, FontFamily::Name(SEMIBOLD.into()))),
                (TextStyle::Monospace, FontId::new(mono_size, FontFamily::Monospace)),
            ]
            .into();
        });
    }
}

#[cfg(test)]
#[path = "tests/theme.rs"]
mod tests;
