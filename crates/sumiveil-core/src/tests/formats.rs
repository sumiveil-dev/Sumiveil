//! `formats/mod.rs` の単体テスト (本体から分離。`formats/mod.rs` の子モジュール `tests` として組み込まれる)

use super::*;
use crate::config::Config;

#[test]
fn split_across_runs() {
    // 「山田太郎様」が「山」「田太」「郎様」の 3 つに分かれている
    let text = "山田太郎様";
    let e = Engine::new(&Config::default());
    let r = e.mask(text, &mut MaskSession::default());
    assert_eq!(r.output, "<NAME_1>様");
    let a = "山".len();
    let b = a + "田太".len();
    let out = split_output(text, &r, &[(0, a), (a, b), (b, text.len())]);
    assert_eq!(out, vec!["<NAME_1>", "", "様"]);
}

#[test]
fn kinds() {
    assert_eq!(DocKind::from_path(Path::new("a.DOCX")), DocKind::Docx);
    assert_eq!(DocKind::from_path(Path::new("a.md")), DocKind::Text);
    assert!(DocKind::Pdf.writes_text());
    assert_eq!(PropertiesMode::parse("Clear"), Some(PropertiesMode::Clear));
}
