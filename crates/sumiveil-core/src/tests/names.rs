//! `names.rs` の単体テスト (本体から分離。`names.rs` の子モジュール `tests` として組み込まれる)

use super::*;

fn run(text: &str) -> Vec<String> {
    let d = NameDetector::new(None, 0.6);
    let mut out = vec![];
    d.detect(text, &mut out);
    out.sort_by_key(|m| m.start);
    out.iter().map(|m| text[m.start..m.end].to_string()).collect()
}

#[test]
fn honorifics() {
    assert_eq!(run("山田様、お世話になっております。"), vec!["山田"]);
    assert_eq!(run("営業部山田太郎様"), vec!["山田太郎"]);
    assert_eq!(run("佐藤 花子さんへ"), vec!["佐藤 花子"]);
    assert_eq!(run("スミスさんと話した"), vec!["スミス"]);
    assert_eq!(run("林様"), vec!["林"]);
}

#[test]
fn rare_surname_before_given_name_is_kept() {
    // 辞書にない姓でも、後ろが辞書の名なら姓ごと検出する (名だけをマスクして姓が残らないように)
    let d = NameDetector::new(Some(NameDict::get()), 0.6);
    let text = "五百旗頭 太郎さんから電話。商事 太郎さんにも連絡。";
    let mut out = vec![];
    d.detect(text, &mut out);
    let got: Vec<&str> = out.iter().map(|m| &text[m.start..m.end]).collect();
    assert!(got.contains(&"五百旗頭 太郎"), "{got:?}");
    // 組織名の後ろの名は、これまでどおり名だけ
    assert!(!got.iter().any(|g| g.contains("商事")), "{got:?}");
}

#[test]
fn honorific_false_positives() {
    assert!(run("お客様各位").is_empty());
    assert!(run("皆様、ご担当者様").is_empty());
    assert!(run("申請様式を確認").is_empty());
    assert!(run("仕様です").is_empty());
    assert!(run("顧客氏名の欄").is_empty());
    assert!(run("看護師さんが来た").is_empty());
    assert!(run("お疲れ様です").is_empty());
}

#[test]
fn labels() {
    assert_eq!(run("氏名：鈴木 一郎です"), vec!["鈴木 一郎"]);
    assert_eq!(run("口座名義: ヤマダ タロウ"), vec!["ヤマダ タロウ"]);
    assert_eq!(run("代表取締役 田中 正"), vec!["田中 正"]);
    assert!(run("担当：未定").is_empty());
}

#[test]
fn self_introduction() {
    assert_eq!(run("営業部の山田と申します。"), vec!["山田"]);
    assert_eq!(run("担当の佐々木です。"), vec!["佐々木"]);
}

#[test]
fn english() {
    assert_eq!(run("Meeting with Mr. John Smith today"), vec!["John Smith"]);
    assert_eq!(run("Dear Alice,\nthanks"), vec!["Alice"]);
    assert_eq!(run("Best regards,\nBob Stone\n"), vec!["Bob Stone"]);
    assert_eq!(run("Assignee: Carol White"), vec!["Carol White"]);
    assert!(run("Hi team,").is_empty());
    assert!(run("Hello World!").is_empty());
    // 「Mr」を名前にしない
    assert_eq!(run("Dear Mr. Yamada,"), vec!["Yamada"]);
    assert_eq!(run("Best regards, Bob Stone"), vec!["Bob Stone"]);
    assert!(run("Thanks, Bye").is_empty());
    // ソースコードの定数・型注釈は人名にしない
    assert!(run("const CLIENT: Token = Token(1);").is_empty());
    assert!(run("    from: Option<String>,\n    owner: String,\n").is_empty());
    // 名前の後ろの括弧・メールアドレスはそのまま人名
    assert_eq!(run("Contact: John Smith (Sales)"), vec!["John Smith"]);
    assert_eq!(run("From: Jane Smith<jane@example.org>"), vec!["Jane Smith"]);
}

#[test]
fn romaji_honorifics() {
    // 辞書なし: ハイフンでつながる敬称だけ
    assert_eq!(run("Tanaka-san, thanks."), vec!["Tanaka"]);
    assert_eq!(run("Ask Kenji Sato-sensei"), vec!["Kenji Sato"]);
    assert!(run("Sato san will join").is_empty());
    let d = NameDetector::new(Some(NameDict::get()), 0.6);
    let mut out = vec![];
    let text = "Meeting with Sato san and Tanaka-kun tomorrow. Jose san? Puerto san";
    d.detect(text, &mut out);
    let mut got: Vec<&str> = out.iter().map(|m| &text[m.start..m.end]).collect();
    got.sort();
    assert_eq!(got, vec!["Sato", "Tanaka"]);
}
