//! 正規表現の検出結果の範囲を調整する関数 (`RegexSpec::refine`)。
//! 正規表現だけでは決められない前後のはみ出しを詰める。採用しない場合は None。

use crate::dict::PlaceDict;

/// (テキスト, 開始, 終了) → 調整後の (開始, 終了)。
pub type Refiner = fn(&str, usize, usize) -> Option<(usize, usize)>;

/// 国際電話番号の最後の数字の塊が、それまでと違う区切り (空白) で付いていれば外す。
/// 「+1 415-555-0132 2024」の「2024」(年など) を番号に含めない。区切りがそろっていれば (「+81 90 1234 5678」) そのまま。
pub fn phone_trailing_group(text: &str, s: usize, e: usize) -> Option<(usize, usize)> {
    let v = &text[s..e];
    let seps: Vec<(usize, char)> = v.char_indices().filter(|(_, c)| matches!(c, ' ' | '-' | '.')).collect();
    if let [.., (_, prev), (last_i, ' ')] = seps.as_slice() {
        if *prev != ' ' {
            return Some((s, s + last_i));
        }
    }
    Some((s, e))
}

/// 直前が数字のとき、先頭の日付の字 (年・月・日) を除く (「1日品川 300 あ 12-34」の「日」)。
pub fn drop_leading_date_unit(text: &str, s: usize, e: usize) -> Option<(usize, usize)> {
    let prev_digit = crate::text::char_before(text, s).is_some_and(crate::text::is_digit_like);
    match text[s..e].chars().next() {
        Some(c) if prev_digit && crate::lexicon::DATE_UNIT_CHARS.contains(c) => Some((s + c.len_utf8(), e)),
        _ => Some((s, e)),
    }
}

/// 市区町村から始まる住所の前に付いた語を除く。市区町村名の前の部分のうち、地名辞書にある最も長いものから始める。
/// 「担当田中横浜市…」→「横浜市…」。地名辞書に無ければ (架空の地名など) そのまま。
pub fn city_leading_place(text: &str, s: usize, e: usize) -> Option<(usize, usize)> {
    let v = &text[s..e];
    let marker = v.char_indices().find(|(_, c)| "市区町村郡".contains(*c)).map(|(i, _)| i)?;
    let places = PlaceDict::get();
    let start = v[..marker].char_indices().map(|(i, _)| i).find(|&i| places.contains(&v[i..marker]) && v[i..marker].chars().count() >= 2);
    Some((s + start.unwrap_or(0), e))
}
