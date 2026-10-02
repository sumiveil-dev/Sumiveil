//! `theme.rs` の単体テスト (本体から分離。`theme.rs` の子モジュール `tests` として組み込まれる)

use super::*;

/// 文字として使う色が、地の色に対して WCAG AA (4.5:1) 以上あること。
#[test]
fn text_colors_meet_wcag_aa() {
    for dark in [false, true] {
        for accent in [None, Some(hex(0xFFD400)), Some(hex(0x0000FF)), Some(hex(0x808080))] {
            let p = Palette::new(dark, accent);
            for (name, fg, bg) in [
                ("text/bg", p.text, p.bg),
                ("text/card", p.text, p.card),
                ("text/editor", p.text, p.editor_bg),
                ("secondary/bg", p.text_secondary, p.bg),
                ("secondary/card", p.text_secondary, p.card),
                ("accent/bg", p.accent, p.bg),
                ("accent_text/accent", p.accent_text, p.accent),
                ("critical/bg", p.critical, p.bg),
                ("caution/bg", p.caution, p.bg),
                ("success/bg", p.success, p.bg),
            ] {
                let c = contrast(fg, bg);
                assert!(c >= 4.5, "dark={dark} accent={accent:?} {name}: {c:.2}");
            }
            for cat in ["contact", "personal", "jp_id", "intl_id", "finance", "network", "secret", "location", "organization", "custom"] {
                let (_, fg) = p.category_colors(cat);
                let c = contrast(fg, p.editor_bg);
                assert!(c >= 4.5, "dark={dark} {cat}: {c:.2}");
            }
        }
    }
}
