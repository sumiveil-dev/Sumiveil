//! 日本の会社名・法人名の検出。法人格の語 (株式会社・(株) など) を起点に、前後の社名を読む。
//!
//! - 前に社名がある「アオバ商事株式会社」→ 社名 + 法人格 (後置形)
//! - 前に社名が無い「株式会社アオバ商事」→ 法人格 + 後ろの社名 (前置形)
//!
//! 社名は空白 (1 つ) をはさんで最大 4 語まで読む。敬称・宛名・役職・部署・人名・英語の機能語の手前で止める。
//! 後ろに続く人名 (「株式会社アオバ商事田中太郎」の「田中太郎」) は [`analyze`] が人名として返し、人名の検出器が使う。

use std::sync::{Arc, OnceLock};

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};

use crate::detector::{Detect, RawMatch};
use crate::dict::{NameDict, PlaceDict};
use crate::lexicon as lx;
use crate::morph::{Morph, NounKind};
use crate::text::{char_after, char_before, is_han};

/// 法人格の語。(株) などの略記は前にも後ろにも付く。
const LEGAL_FORMS: &[&str] = &[
    "株式会社", "有限会社", "合同会社", "合資会社", "合名会社", "一般社団法人", "一般財団法人", "公益社団法人", "公益財団法人",
    "特定非営利活動法人", "NPO法人", "ＮＰＯ法人", "医療法人社団", "医療法人財団", "医療法人", "学校法人", "社会福祉法人",
    "独立行政法人", "国立大学法人", "宗教法人", "社団法人", "財団法人", "(株)", "（株）", "㈱", "(有)", "（有）", "㈲", "(同)", "（同）",
];

/// 片側で読む語の数と文字数の上限。
const MAX_WORDS: usize = 4;
const MAX_CHARS: usize = 30;

/// 信頼度: 前置形 (法人格 + 社名) / 後置形 (社名 + 法人格) / 形態素解析の組織名だけ。
const CONF_PREFIX: f32 = 0.85;
const CONF_SUFFIX: f32 = 0.8;
const CONF_MORPH_ORG: f32 = 0.55;

fn legal_matcher() -> &'static AhoCorasick {
    static AC: OnceLock<AhoCorasick> = OnceLock::new();
    AC.get_or_init(|| AhoCorasickBuilder::new().match_kind(MatchKind::LeftmostLongest).build(LEGAL_FORMS).expect("legal forms"))
}

/// 社名に使われる文字 (漢字・カタカナ・英数字 (全角を含む)・&)。
fn is_word_char(c: char) -> bool {
    is_han(c)
        || ('\u{30A1}'..='\u{30FF}').contains(&c)
        || ('\u{FF66}'..='\u{FF9F}').contains(&c)
        || c.is_ascii_alphanumeric()
        || ('０'..='９').contains(&c)
        || ('Ａ'..='Ｚ').contains(&c)
        || ('ａ'..='ｚ').contains(&c)
        || matches!(c, '&' | '＆')
}

/// 語の途中にだけ現れる記号 (「Aoba-Tech」「Aoba.Tech」「O'Neil」)。
fn is_joiner(c: char) -> bool {
    matches!(c, '-' | '.' | '\u{27}' | '’' | '－' | '．')
}

fn is_space(c: char) -> bool {
    c == ' ' || c == '　'
}

fn is_any_digit(c: char) -> bool {
    c.is_ascii_digit() || ('０'..='９').contains(&c)
}

#[derive(Clone, Copy, Debug)]
struct Word {
    s: usize,
    e: usize,
}

/// `pos` で終わる語の先頭 (`floor` より前には戻らない)。
fn word_start(text: &str, pos: usize, floor: usize) -> usize {
    let mut s = pos;
    let mut n = 0;
    let mut it = text[floor..pos].char_indices().rev().peekable();
    while let Some((i, c)) = it.next() {
        let abs = floor + i;
        let joiner_inside = is_joiner(c) && s < pos && s == abs + c.len_utf8() && it.peek().is_some_and(|&(_, p)| is_word_char(p));
        if is_word_char(c) {
            s = abs;
            n += 1;
            if n >= MAX_CHARS {
                break;
            }
        } else if !joiner_inside {
            break;
        }
    }
    s
}

/// `pos` から始まる語の末尾 (`ceil` より後には進まない)。
fn word_end(text: &str, pos: usize, ceil: usize) -> usize {
    let mut e = pos;
    let mut n = 0;
    let mut it = text[pos..ceil].char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let abs = pos + i;
        let joiner_inside = is_joiner(c) && e > pos && e == abs && it.peek().is_some_and(|&(_, p)| is_word_char(p));
        if is_word_char(c) {
            e = abs + c.len_utf8();
            n += 1;
            if n >= MAX_CHARS {
                break;
            }
        } else if !joiner_inside {
            break;
        }
    }
    e
}

/// 社名に含めない語 (英語の機能語・項目名など)。この語に当たったら、それより先は読まない。
fn is_stop_word(w: &str) -> bool {
    let lower = w.to_ascii_lowercase();
    lx::EN_FUNCTION_WORDS.contains(&lower.as_str()) || lx::COMPANY_LEADING_STOPS.contains(&w)
}

/// 法人格の前の語 (近いものから順)。空白 1 つをはさんで最大 MAX_WORDS 語。
fn read_left(text: &str, end: usize, floor: usize) -> Vec<Word> {
    let mut words = vec![];
    let mut pos = end;
    // 「アオバ商事 株式会社」のように法人格の直前に空白 1 つがあれば飛ばす
    if let Some(sp) = char_before(text, end).filter(|c| is_space(*c)) {
        if end - sp.len_utf8() > floor && char_before(text, end - sp.len_utf8()).is_some_and(is_word_char) {
            pos = end - sp.len_utf8();
        }
    }
    while words.len() < MAX_WORDS {
        let s = word_start(text, pos, floor);
        if s == pos || is_stop_word(&text[s..pos]) {
            break;
        }
        words.push(Word { s, e: pos });
        match char_before(text, s) {
            Some(sp) if is_space(sp) && s - sp.len_utf8() > floor && char_before(text, s - sp.len_utf8()).is_some_and(is_word_char) => {
                pos = s - sp.len_utf8();
            }
            _ => break,
        }
    }
    words
}

/// 法人格の後ろの語 (近いものから順)。空白 1 つをはさんで最大 MAX_WORDS 語。
fn read_right(text: &str, start: usize, ceil: usize) -> Vec<Word> {
    let mut words = vec![];
    let mut pos = start;
    while words.len() < MAX_WORDS {
        let e = word_end(text, pos, ceil);
        if e == pos || is_stop_word(&text[pos..e]) {
            break;
        }
        words.push(Word { s: pos, e });
        match char_after(text, e) {
            Some(sp) if is_space(sp) && e + sp.len_utf8() < ceil && char_after(text, e + sp.len_utf8()).is_some_and(is_word_char) => {
                pos = e + sp.len_utf8();
            }
            _ => break,
        }
    }
    words
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StopKind {
    /// 様・御中・宛 など
    Address,
    /// 役職
    Title,
    /// 部署・支店など
    Unit,
}

#[derive(Clone, Copy, Debug)]
struct Stop {
    s: usize,
    e: usize,
    kind: StopKind,
}

/// 語の中の区切り (敬称・役職・部署)。部署は前の部署名・地名から始まる (「営業部」「東京支店」)。
fn find_stops(text: &str, w: Word, places: &PlaceDict) -> Vec<Stop> {
    let mut stops: Vec<Stop> = vec![];
    let mut p = w.s;
    while p < w.e {
        let rest = &text[p..w.e];
        let floor = stops.last().map_or(w.s, |s| s.e);
        let found = if let Some(t) = lx::starts_with_any(rest, lx::ADDRESS_TERMS) {
            Some(Stop { s: p, e: p + t.len(), kind: StopKind::Address })
        } else if let Some(t) = lx::starts_with_any(rest, lx::TITLES) {
            // 「東京支店長」「営業部長」のように前に地名・部署名があれば、そこから役職とみなす
            Some(Stop { s: unit_prefix(text, floor, p, places).unwrap_or(p), e: p + t.len(), kind: StopKind::Title })
        } else {
            let unit = lx::starts_with_any(rest, lx::ORG_UNITS)
                .or_else(|| rest.chars().next().filter(|c| lx::ORG_UNIT_CHARS.contains(*c)).map(|c| &rest[..c.len_utf8()]));
            unit.and_then(|u| unit_prefix(text, floor, p, places).map(|q| Stop { s: q, e: p + u.len(), kind: StopKind::Unit }))
        };
        match found {
            Some(st) => {
                p = st.e;
                stops.push(st);
            }
            None => p += rest.chars().next().map_or(1, char::len_utf8),
        }
    }
    stops
}

/// 部署の単位 (`p` から始まる「部」「支店」など) の前にある部署名・地名の先頭。無ければ None (単位とみなさない)。
fn unit_prefix(text: &str, floor: usize, p: usize, places: &PlaceDict) -> Option<usize> {
    let head = &text[floor..p];
    if let Some(d) = lx::ends_with_any(head, lx::DEPARTMENT_WORDS) {
        return Some(p - d.len());
    }
    // 地名 (2〜5 文字) で終わっていれば、その地名から (長いものを優先)
    let starts: Vec<usize> = head.char_indices().map(|(i, _)| floor + i).collect();
    let tail = &starts[starts.len().saturating_sub(5)..];
    tail.iter().find(|&&q| text[q..p].chars().count() >= 2 && places.contains(&text[q..p])).copied()
}

/// 区間 [s, e) の末尾の漢字の並びに人名があれば、その開始位置と信頼度。
/// - 姓 + 名 (辞書) が区間の終わりまでちょうど続く: 0.75
/// - 姓 (辞書、2 文字以上) の直後が敬称・役職: 0.85
/// - 区間全体が姓 (辞書、2 文字以上) で、直前が役職・部署: 0.7
fn name_in(text: &str, s: usize, e: usize, min_start: usize, prev: Option<StopKind>, next: Option<StopKind>, dict: &NameDict) -> Option<(usize, f32)> {
    let han_start = text[s..e].char_indices().rev().take_while(|&(_, c)| is_han(c)).last().map(|(i, _)| s + i)?;
    let from = han_start.max(min_start);
    if from >= e {
        return None;
    }
    for (i, _) in text[from..e].char_indices() {
        let i = from + i;
        let cand = &text[i..e];
        if dict.is_full_name(cand) {
            return Some((i, 0.75));
        }
        let surname = cand.chars().count() >= 2 && dict.is_jp_surname(cand);
        if surname && matches!(next, Some(StopKind::Address | StopKind::Title)) {
            return Some((i, 0.85));
        }
        if surname && i == s && matches!(prev, Some(StopKind::Title | StopKind::Unit)) {
            return Some((i, 0.7));
        }
    }
    None
}

/// 法人格の後ろの語を調べ、社名として読める部分の終わりと、続く人名を返す。
/// `company_first`: 前置形なら true (最初の語の先頭は社名。人名はその後ろからだけ探す)。
/// 後置形 (false) でも社名の部分が残れば返す (「コールセンター株式会社ツバサシステムズ」のように前後の両方が社名らしいとき)。
fn split_right(text: &str, words: &[Word], company_first: bool, dict: Option<&NameDict>, places: &PlaceDict) -> (Option<usize>, Vec<RawMatch>) {
    let mut names = vec![];
    let mut company_end: Option<usize> = None;
    let mut open = true;
    let mut last_end = words.first().map_or(0, |w| w.s);
    for (wi, &w) in words.iter().enumerate() {
        let stops = find_stops(text, w, places);
        let mut seg_s = w.s;
        let mut prev: Option<StopKind> = None;
        for k in 0..=stops.len() {
            let (seg_e, next) = match stops.get(k) {
                Some(st) => (st.s, Some(st.kind)),
                None => (w.e, None),
            };
            if seg_e > seg_s {
                // 前置形の最初の語は、少なくとも 1 文字は社名として残す
                let first_of_company = company_first && open && wi == 0 && seg_s == w.s;
                let min_start = if first_of_company { seg_s + text[seg_s..].chars().next().map_or(1, char::len_utf8) } else { seg_s };
                if let Some((ns, conf)) = dict.and_then(|d| name_in(text, seg_s, seg_e, min_start, prev, next, d)) {
                    if open {
                        company_end = Some(if ns > w.s { ns } else { last_end });
                        open = false;
                    }
                    names.push(RawMatch { start: ns, end: seg_e, confidence: conf });
                }
            }
            if let Some(st) = stops.get(k) {
                if open {
                    company_end = Some(if st.s > w.s { st.s } else { last_end });
                    open = false;
                }
                seg_s = st.e;
                prev = Some(st.kind);
            }
        }
        if open {
            last_end = w.e;
        }
    }
    if open {
        company_end = Some(last_end);
    }
    (company_end, names)
}

/// 後置形の社名の左端。最も遠い語から、前に付いた敬称・日付・項目名などを取り除く。社名が残らなければ None。
fn trim_left(text: &str, words: &[Word], places: &PlaceDict) -> Option<usize> {
    let far = *words.last()?;
    let near_end = words[0].e;
    let mut s = far.s;
    // 敬称・役職・部署の後ろから (「田中様アオバ商事株式会社」→「アオバ商事」)
    if let Some(st) = find_stops(text, far, places).last() {
        s = st.e;
    }
    // 「2024年度」「10月1日」などの日付の後ろから
    let mut prev_digit = false;
    let mut cut = s;
    for (i, c) in text[s..far.e].char_indices() {
        if prev_digit && "年月日度".contains(c) {
            cut = s + i + c.len_utf8();
        }
        prev_digit = is_any_digit(c) || (prev_digit && c == '年');
    }
    s = cut;
    // 項目名・主語 (弊社・取引先 など)
    while let Some(p) = lx::starts_with_any(&text[s..far.e], lx::COMPANY_LEADING_STOPS) {
        if s + p.len() >= far.e {
            break;
        }
        s += p.len();
    }
    // 空白の直後から始まるように (取り除いた結果が空白で始まる場合)
    while let Some(c) = char_after(text, s).filter(|c| is_space(*c)) {
        s += c.len_utf8();
    }
    let has_name = text[s..near_end].chars().any(|c| is_word_char(c) && !is_any_digit(c));
    (s < near_end && has_name).then_some(s)
}

/// 会社名 (開始, 終了, 信頼度) と、社名の後ろに続く人名を返す。
pub fn analyze(text: &str, dict: Option<&NameDict>) -> (Vec<RawMatch>, Vec<RawMatch>) {
    let places = PlaceDict::get();
    let hits: Vec<(usize, usize)> = legal_matcher().find_iter(text).map(|m| (m.start(), m.end())).collect();
    let mut companies = vec![];
    let mut names = vec![];
    for (k, &(ls, le)) in hits.iter().enumerate() {
        let floor = if k > 0 { hits[k - 1].1 } else { 0 };
        let ceil = hits.get(k + 1).map_or(text.len(), |h| h.0);
        let left = read_left(text, ls, floor);
        match trim_left(text, &left, &places) {
            Some(s) => {
                // 後ろに人名が続いていれば拾う (「アオバ商事株式会社田中太郎」)。
                // 後ろにも社名らしい語 (2 文字以上) が続けば、どちらが社名か決められないので両方を隠す (露出させない方を選ぶ)
                let right = read_right(text, le, ceil);
                let (r_end, tail) = split_right(text, &right, false, dict, &places);
                names.extend(tail);
                let end = r_end.filter(|&e| e > le && text[le..e].chars().filter(|c| is_word_char(*c)).count() >= 2).unwrap_or(le);
                companies.push(RawMatch { start: s, end, confidence: CONF_SUFFIX });
            }
            None => {
                // 「株式会社 アオバ商事」のように法人格の直後に空白 1 つがあれば飛ばす
                let r_start = match char_after(text, le) {
                    Some(c) if is_space(c) => le + c.len_utf8(),
                    _ => le,
                };
                let right = read_right(text, r_start, ceil);
                let (end, tail) = split_right(text, &right, true, dict, &places);
                names.extend(tail);
                if let Some(end) = end.filter(|&e| e > r_start && text[r_start..e].chars().filter(|c| is_word_char(*c)).count() >= 2) {
                    companies.push(RawMatch { start: ls, end, confidence: CONF_PREFIX });
                }
            }
        }
    }
    (companies, names)
}

/// 会社名の検出器。形態素解析があれば、法人格の語が無い組織名も低い信頼度で候補にする。
pub struct CompanyDetector {
    dict: Option<Arc<NameDict>>,
    morph: Option<Arc<Morph>>,
}

impl CompanyDetector {
    pub fn new(dict: Option<Arc<NameDict>>, morph: Option<Arc<Morph>>) -> Self {
        Self { dict, morph }
    }
}

impl Detect for CompanyDetector {
    fn detect(&self, text: &str, out: &mut Vec<RawMatch>) {
        out.extend(analyze(text, self.dict.as_deref()).0);
        if let Some(m) = &self.morph {
            // 英字だけの語は IPADIC が組織名と誤って分類することがある (「token」「details」) ので除く
            for n in m.proper_nouns(text).into_iter().filter(|n| n.kind == NounKind::Organization) {
                let w = &text[n.start..n.end];
                if w.chars().count() >= 2 && !w.is_ascii() {
                    out.push(RawMatch { start: n.start, end: n.end, confidence: CONF_MORPH_ORG });
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/company.rs"]
mod tests;
