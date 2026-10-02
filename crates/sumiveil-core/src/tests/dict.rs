//! `dict.rs` の単体テスト (本体から分離。`dict.rs` の子モジュール `tests` として組み込まれる)

use super::*;

fn names(text: &str) -> Vec<(String, f32)> {
    let d = NameDict::get();
    let mut v: Vec<(String, f32)> = d.scan_names(text).into_iter().map(|(s, e, c)| (text[s..e].to_string(), c)).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

#[test]
fn jp_full_names() {
    let v = names("本日は山田太郎が出席、佐藤 花子も参加");
    let got: Vec<&str> = v.iter().filter(|x| x.1 >= 0.6).map(|x| x.0.as_str()).collect();
    assert!(got.contains(&"山田太郎"), "{v:?}");
    assert!(got.contains(&"佐藤 花子"), "{v:?}");
}

#[test]
fn jp_common_words_not_names() {
    let v = names("本日の会議室は情報管理部で東京都庁の予定");
    assert!(v.iter().all(|x| x.1 < 0.6), "{v:?}");
}

#[test]
fn rare_surnames_with_known_given_names() {
    let d = NameDict::get();
    // 辞書にない珍しい姓 (テストの前提を確認)
    for s in ["月見里", "五百旗頭"] {
        assert!(!d.is_jp_surname(s) && d.is_surname_like(s), "{s}");
    }
    let v = names("月見里花子が来社、五百旗頭 太郎さんから電話");
    let got: Vec<&str> = v.iter().filter(|x| x.1 >= 0.6).map(|x| x.0.as_str()).collect();
    assert!(got.contains(&"月見里花子") && got.contains(&"五百旗頭 太郎"), "{v:?}");
    // 一般語 + 名に見える語 (「新規」「予定」など) は姓らしいとみなさない
    for w in ["新規", "予定", "会議室", "事業部", "第一", "各位"] {
        assert!(!d.is_surname_like(w), "{w}");
    }
    // 地名・日付 + 名、一般語でもある名 (未来・勝利・昭和) は文脈がなければ人名にしない
    let v = names("北海道大地を走る。日本未来会議。本日 未来について。大阪純一郎ビル。生年月日 昭和");
    assert!(v.iter().all(|x| x.1 < 0.6), "{v:?}");
}

#[test]
fn en_names() {
    let v = names("Meeting with Robert Johnson and Will Power tomorrow");
    let got: Vec<&str> = v.iter().filter(|x| x.1 >= 0.6).map(|x| x.0.as_str()).collect();
    assert_eq!(got, vec!["Robert Johnson"]);
}

#[test]
fn places() {
    let d = PlaceDetector::new();
    let t = "横浜市在住で、新宿駅を利用";
    let mut out = vec![];
    d.detect(t, &mut out);
    let got: Vec<&str> = out.iter().map(|m| &t[m.start..m.end]).collect();
    assert!(got.contains(&"新宿駅"), "{got:?}");
}
