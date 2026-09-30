//! PDF。ページごとに文字を取り出し、マスクしたテキストとして書き出す (PDF 自体は書き換えない)。

use unicode_normalization::UnicodeNormalization;

use super::{DocKind, Document, Inner, TextBuilder};

/// PDF のフォントの対応表によっては、漢字が見た目の同じ部首や互換漢字 (「⼭⽥」U+2F2D など) として取り出される。
/// そのままでは辞書や規則に一致しないため、その範囲の文字だけを通常の漢字に直す (NFKC)。
fn fix_cjk_compat(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            let compat = matches!(c, '\u{2E80}'..='\u{2FDF}' | '\u{F900}'..='\u{FAFF}' | '\u{2F800}'..='\u{2FA1F}');
            let v: Vec<char> = if compat { c.nfkc().collect() } else { vec![c] };
            v
        })
        .collect()
}

pub(crate) fn open(bytes: &[u8]) -> Result<Document, String> {
    // 解析ライブラリは想定外の PDF で panic することがあるため、エラーとして扱う
    let pages = match std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem_by_pages(bytes)) {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            let msg = e.to_string();
            return Err(if msg.to_ascii_lowercase().contains("encrypt") || msg.contains("password") {
                "パスワードで保護された PDF は開けません".to_string()
            } else {
                format!("PDF を読み込めません: {msg}")
            });
        }
        Err(_) => return Err("この PDF は読み込めません (対応していない形式です)".to_string()),
    };
    let mut b = TextBuilder::default();
    for (i, page) in pages.iter().enumerate() {
        if i > 0 {
            b.newline();
            b.push(&format!("\n--- {} ページ ---\n", i + 1));
        }
        b.location(format!("{} ページ", i + 1));
        b.push(&fix_cjk_compat(page.trim_matches('\n')));
    }
    b.newline();
    let mut warnings = vec!["PDF はマスクしたテキスト (.txt) として書き出します。表やレイアウトは保たれません".to_string()];
    if b.text.trim().chars().filter(|c| !c.is_whitespace() && *c != '-').count() < 4 {
        warnings.push("文字を取り出せませんでした (スキャンした画像の PDF など)。この PDF はマスクできません".to_string());
    }
    Ok(Document {
        kind: DocKind::Pdf,
        text: b.text,
        encoding_name: DocKind::Pdf.label(true).to_string(),
        warnings,
        properties: vec![],
        locations: b.locations,
        inner: Inner::Plain,
    })
}
