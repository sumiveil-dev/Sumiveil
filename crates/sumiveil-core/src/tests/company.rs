//! `company.rs` の単体テスト (本体から分離。`company.rs` の子モジュール `tests` として組み込まれる)

use super::*;

/// (会社名, 人名) を文字列で返す。
fn run(text: &str) -> (Vec<&str>, Vec<&str>) {
    let d = NameDict::get();
    let (c, n) = analyze(text, Some(&d));
    (c.iter().map(|m| &text[m.start..m.end]).collect(), n.iter().map(|m| &text[m.start..m.end]).collect())
}

#[test]
fn suffix_and_prefix_forms() {
    assert_eq!(run("アオバ商事株式会社"), (vec!["アオバ商事株式会社"], vec![]));
    assert_eq!(run("株式会社アオバ商事"), (vec!["株式会社アオバ商事"], vec![]));
    assert_eq!(run("アオバ商事（株）"), (vec!["アオバ商事（株）"], vec![]));
    assert_eq!(run("㈱アオバ"), (vec!["㈱アオバ"], vec![]));
}

#[test]
fn stops_before_address_terms_titles_and_departments() {
    assert_eq!(run("株式会社アオバ商事御中").0, vec!["株式会社アオバ商事"]);
    assert_eq!(run("株式会社アオバ商事営業部").0, vec!["株式会社アオバ商事"]);
    assert_eq!(run("株式会社アオバ商事代表取締役").0, vec!["株式会社アオバ商事"]);
    assert_eq!(run("アオバ商事株式会社東京支店長").0, vec!["アオバ商事株式会社"]);
}

#[test]
fn names_after_the_company_are_returned_separately() {
    assert_eq!(run("アオバ商事株式会社田中様"), (vec!["アオバ商事株式会社"], vec!["田中"]));
    assert_eq!(run("株式会社アオバ商事田中太郎"), (vec!["株式会社アオバ商事"], vec!["田中太郎"]));
    assert_eq!(run("株式会社アオバ商事営業部田中様"), (vec!["株式会社アオバ商事"], vec!["田中"]));
    assert_eq!(run("株式会社アオバ商事代表取締役田中太郎"), (vec!["株式会社アオバ商事"], vec!["田中太郎"]));
}

#[test]
fn surname_inside_a_company_name_is_kept() {
    // 「田中製作所」「山田商店」は社名の一部 (敬称が続くときだけ人名として切る)
    assert_eq!(run("株式会社田中製作所").0, vec!["株式会社田中製作所"]);
    assert_eq!(run("株式会社山田商店田中様"), (vec!["株式会社山田商店"], vec!["田中"]));
    assert_eq!(run("株式会社サンプル山田").0, vec!["株式会社サンプル山田"]);
}

#[test]
fn spaces_and_symbols_inside_company_names() {
    assert_eq!(run("sample test株式会社").0, vec!["sample test株式会社"]);
    assert_eq!(run("sample test 株式会社").0, vec!["sample test 株式会社"]);
    assert_eq!(run("株式会社ABC Systems Japan").0, vec!["株式会社ABC Systems Japan"]);
    assert_eq!(run("アオバ　商事株式会社").0, vec!["アオバ　商事株式会社"]);
    assert_eq!(run("Aoba-Tech株式会社").0, vec!["Aoba-Tech株式会社"]);
    assert_eq!(run("Aoba.Tech株式会社").0, vec!["Aoba.Tech株式会社"]);
}

#[test]
fn words_before_the_company_are_not_taken_in() {
    assert_eq!(run("contract with Sample Test株式会社").0, vec!["Sample Test株式会社"]);
    assert_eq!(run("担当 アオバ商事株式会社").0, vec!["アオバ商事株式会社"]);
    assert_eq!(run("弊社アオバ商事株式会社では").0, vec!["アオバ商事株式会社"]);
    assert_eq!(run("2024年度アオバ商事株式会社").0, vec!["アオバ商事株式会社"]);
    assert_eq!(run("田中様アオバ商事株式会社").0, vec!["アオバ商事株式会社"]);
}

#[test]
fn legal_form_alone_is_not_a_company() {
    for text in ["当社は株式会社です", "株式会社とは何か", "株式会社化する", "株式会社等", "株式会社の設立手続き"] {
        assert!(run(text).0.is_empty(), "{text}");
    }
}

#[test]
fn both_sides_are_masked_when_unclear() {
    // 前後のどちらも社名らしいときは、露出させないよう両方を含める
    assert_eq!(run("コールセンター株式会社ツバサシステムズ").0, vec!["コールセンター株式会社ツバサシステムズ"]);
    assert_eq!(run("アオバ商事株式会社株式会社ミナト").0, vec!["アオバ商事株式会社", "株式会社ミナト"]);
}
