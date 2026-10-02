//! `formats/xml.rs` の単体テスト (本体から分離。`formats/xml.rs` の子モジュール `tests` として組み込まれる)

use super::*;

#[test]
fn scan_and_edit() {
    let x = r#"<?xml version="1.0"?><w:p><w:r><w:t xml:space="preserve">A &amp; B</w:t></w:r><!-- c --><w:t/><x a="1>2" b='q'/></w:p>"#;
    let toks = scan(x);
    let text: Vec<&str> = toks.iter().filter_map(|t| if let Tok::Text { span } = t { Some(&x[span.0..span.1]) } else { None }).collect();
    assert_eq!(text, vec!["A &amp; B"]);
    assert!(toks.iter().any(|t| matches!(t, Tok::Start { name: "w:t", self_closing: true, .. })));
    let tag = toks.iter().find_map(|t| if let Tok::Start { name: "x", tag, .. } = t { Some(*tag) } else { None }).unwrap();
    assert_eq!(attr_value(x, tag, "a").as_deref(), Some("1>2"));
    assert_eq!(attr_value(x, tag, "b").as_deref(), Some("q"));
    assert_eq!(unescape("A &amp; B &#x3042;&#12354;"), "A & B ああ");
    let (s, _) = x.find("A &amp; B").map(|p| (p, 0)).unwrap();
    let edited = apply_edits(x, vec![((s, s + "A &amp; B".len()), escape_text("<X> & Y"))]);
    assert!(edited.contains(">&lt;X&gt; &amp; Y</w:t>"));
}
