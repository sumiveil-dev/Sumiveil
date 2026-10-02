//! 内蔵辞書 (人名・地名)。
//!
//! 出典: mecab-ipadic (人名・地域・人名の読みから作ったローマ字, IPADIC ライセンス), US Census 1990/2010 (パブリックドメイン)。
//! `scripts/gen-dict.ps1` で生成した UTF-8 の単語リストを埋め込み、初回使用時にハッシュ集合へ展開する。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use crate::detector::{Detect, RawMatch};

static JP_SURNAMES: &str = include_str!("../data/jp_surnames.txt");
static JP_GIVEN: &str = include_str!("../data/jp_given.txt");
static JP_PLACES: &str = include_str!("../data/jp_places.txt");
static EN_GIVEN: &str = include_str!("../data/en_given.txt");
static EN_SURNAMES: &str = include_str!("../data/en_surnames.txt");
/// 日本人の姓・名のローマ字 (IPADIC の読みからヘボン式と表記ゆれを生成。小文字)
static JP_ROMAJI_SURNAMES: &str = include_str!("../data/jp_romaji_surnames.txt");
static JP_ROMAJI_GIVEN: &str = include_str!("../data/jp_romaji_given.txt");

/// 英語の名のうち、一般的な単語でもあるもの (辞書層では使わない)。
const EN_GIVEN_EXCLUDE: &[&str] = &[
    "Will", "May", "June", "April", "August", "Mark", "Bill", "Grant", "Chase", "Hope", "Faith", "Joy", "Grace", "Page",
    "Guy", "Art", "Don", "Ray", "Frank", "Rich", "Sky", "Star", "Dawn", "Major", "Royal", "Price", "Case", "Long", "Young",
    "Else", "Van", "Del", "Son", "Man", "An", "In", "Le", "La", "Lu", "Ok", "Sun", "Page", "Sterling", "Hunter", "Miles",
    "Dean", "Lane", "Rose", "Summer", "Winter", "Autumn", "Crystal", "Amber", "Destiny", "Harmony", "Trinity", "Unique",
    "Precious", "Justice", "King", "Duke", "Earl", "Baby", "Sunny", "Honey", "Angel", "Mercy", "Patience", "Constance",
    "Christian", "Norman", "Wade", "Drew", "Gene", "Carol", "Ben", "Tom", "Jean", "Lou", "Ed", "Al", "Mac", "Max", "Rob",
];

const EN_SECOND_STOP: &[&str] = &[
    "The", "And", "For", "With", "From", "This", "That", "Street", "Avenue", "Road", "Inc", "Corp", "Ltd", "Company",
    "University", "College", "School", "Hospital", "Center", "Centre", "Group", "Team", "Department", "Office",
];

/// 地名とみなす直後の語。
const PLACE_SUFFIXES: &[&str] = &["在住", "出身", "市", "区", "町", "村", "郡", "駅", "県", "府"];

pub struct NameDict {
    jp_surnames: HashSet<&'static str>,
    /// 辞書の姓の文字の出現数 (先頭・末尾・すべての位置)。辞書にない珍しい姓を「姓らしさ」で見分けるのに使う
    sur_first: HashMap<char, u32>,
    sur_last: HashMap<char, u32>,
    sur_any: HashMap<char, u32>,
    jp_given: HashSet<&'static str>,
    en_given: HashSet<&'static str>,
    en_surnames: HashSet<&'static str>,
    romaji_surnames: HashSet<&'static str>,
    romaji_given: HashSet<&'static str>,
}

fn lines(s: &'static str) -> impl Iterator<Item = &'static str> {
    s.lines().map(str::trim).filter(|l| !l.is_empty())
}

/// 漢字 (姓の途中のヶを含む)。
fn is_han(c: char) -> bool {
    crate::text::is_han(c) || c == 'ヶ'
}

/// 辞書にない姓の候補に含まれていたら姓とみなさない字 (組織・役職・文法的な語に多いもの)。
/// 辞書の名のうち一般語でもあるもの (文脈のない「珍しい姓 + 名」の判定では名とみなさない)。
const GIVEN_COMMON_WORDS: &[&str] = &[
    "未来", "大地", "真実", "勝利", "正義", "光明", "和平", "健康", "希望", "平和", "自由", "栄光", "青空", "太陽", "元気", "正直",
    "誠実", "昭和", "平成", "令和", "大正", "明治", "慶応", "文化", "文政", "天保", "安政", "一番", "大和", "日本", "永遠", "満天",
];

/// 文章によく出る大きな地名 (都道府県・主な都市・国)。文脈のない「珍しい姓 + 名」の判定では姓とみなさない。
/// 小さな地名は姓の由来になっていることが多いので、地名辞書全体では除外しない。
const MAJOR_PLACES: &[&str] = &[
    "日本", "北海道", "青森", "岩手", "宮城", "秋田", "山形", "福島", "茨城", "栃木", "群馬", "埼玉", "千葉", "東京", "神奈川", "新潟",
    "富山", "石川", "福井", "山梨", "長野", "岐阜", "静岡", "愛知", "三重", "滋賀", "京都", "大阪", "兵庫", "奈良", "和歌山", "鳥取",
    "島根", "岡山", "広島", "山口", "徳島", "香川", "愛媛", "高知", "福岡", "佐賀", "長崎", "熊本", "大分", "宮崎", "鹿児島", "沖縄",
    "札幌", "仙台", "横浜", "川崎", "名古屋", "神戸", "北九州", "那覇", "関東", "関西", "東北", "九州", "四国", "中部", "近畿", "北陸",
    "東海", "山陰", "山陽", "首都", "都内", "県内", "市内", "国内", "海外", "中国", "韓国", "米国", "英国", "台湾", "香港", "欧州",
];

/// 文章によく出る大きな地名か (姓と同じ表記でも、文脈のない姓としては扱わない)。
pub fn is_major_place(s: &str) -> bool {
    MAJOR_PLACES.contains(&s)
}

/// [s, e) の語が、人名を書くような位置にあるか (一般語の「入場して」「出口から」などを除くため)。
/// 行にその語だけ / 行末で、前が空白か「の」/ 後ろが「です」「でした」「と申します」「宛」。
fn name_like_position(text: &str, s: usize, e: usize) -> bool {
    let line_start = text[..s].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[e..].find(['\r', '\n']).map_or(text.len(), |i| e + i);
    let before = text[line_start..s].trim_end_matches([' ', '　']);
    let after = &text[e..line_end];
    let at_end = after.trim_start_matches([' ', '　']).trim_end_matches(['。', '.', '、', ',', ' ', '　']).is_empty();
    let spaced = text[line_start..s].ends_with([' ', '　', '\t']);
    before.is_empty() && at_end
        || at_end && (spaced || before.ends_with('の'))
        || ["です", "でした", "と申します", "宛"].iter().any(|w| after.starts_with(w))
}

/// (後半は「〜の件」「予定」「新規」などの一般語の字)
const NOT_SURNAME_CHARS: &str = "事業社所部課係室局店班科会団院校園省庁署員者人長師士様殿氏的性化型式用等及並又其此彼何毎各全諸第約計件予定済可否無非未再新旧最超";

impl NameDict {
    pub fn get() -> Arc<NameDict> {
        static D: OnceLock<Arc<NameDict>> = OnceLock::new();
        D.get_or_init(|| {
            let jp_surnames: HashSet<&'static str> = lines(JP_SURNAMES).collect();
            let (mut sur_first, mut sur_last, mut sur_any) = (HashMap::new(), HashMap::new(), HashMap::new());
            for s in &jp_surnames {
                let chars: Vec<char> = s.chars().collect();
                if chars.len() < 2 || !chars.iter().all(|c| is_han(*c)) {
                    continue;
                }
                *sur_first.entry(chars[0]).or_insert(0) += 1;
                *sur_last.entry(chars[chars.len() - 1]).or_insert(0) += 1;
                for c in chars {
                    *sur_any.entry(c).or_insert(0) += 1;
                }
            }
            Arc::new(NameDict {
                jp_surnames,
                sur_first,
                sur_last,
                sur_any,
                jp_given: lines(JP_GIVEN).collect(),
                en_given: lines(EN_GIVEN).filter(|n| !EN_GIVEN_EXCLUDE.contains(n)).collect(),
                en_surnames: lines(EN_SURNAMES).collect(),
                romaji_surnames: lines(JP_ROMAJI_SURNAMES).collect(),
                romaji_given: lines(JP_ROMAJI_GIVEN).collect(),
            })
        })
        .clone()
    }

    pub fn is_jp_surname(&self, s: &str) -> bool {
        self.jp_surnames.contains(s)
    }

    pub fn is_jp_given(&self, s: &str) -> bool {
        self.jp_given.contains(s)
    }

    /// 辞書にない珍しい姓 (「月見里」「五百旗頭」など) らしいか。
    /// 2〜4 文字の漢字で、先頭の字・末尾の字がそれぞれ辞書の姓の先頭・末尾に使われ、途中の字も姓に使われているもの。
    /// 組織・役職・一般語に多い字を含むものは除く。単独では使わず、名 (辞書) と並んだときの裏付けに使う。
    pub fn is_surname_like(&self, s: &str) -> bool {
        let chars: Vec<char> = s.chars().collect();
        if !(2..=4).contains(&chars.len()) || !chars.iter().all(|c| is_han(*c) && *c != '々' && *c != 'ヶ') {
            return false;
        }
        if chars.iter().any(|c| NOT_SURNAME_CHARS.contains(*c)) {
            return false;
        }
        let n = |m: &HashMap<char, u32>, c: &char| m.get(c).copied().unwrap_or(0);
        n(&self.sur_first, &chars[0]) >= 1 && n(&self.sur_last, &chars[chars.len() - 1]) >= 1 && chars.iter().all(|c| n(&self.sur_any, c) >= 1)
    }

    /// 文脈 (敬称・項目名) がないときの「辞書にない姓 + 辞書の名」の判定。
    /// 誤検出を避けるため、姓の側が地名 (「大阪」「北海道」) や日付の語 (「本日」) のもの、
    /// 名の側が一般語でもあるもの (「未来」「勝利」「昭和」) は採らない。
    fn rare_full_name(&self, sur: &str, giv: &str) -> bool {
        self.is_surname_like(sur)
            && !sur.ends_with(['日', '月', '年', '時'])
            && !MAJOR_PLACES.iter().any(|p| sur.starts_with(p))
            && !GIVEN_COMMON_WORDS.contains(&giv)
            && self.is_jp_given(giv)
    }

    /// 敬称などの文脈があるときの緩い判定: 1〜5 文字の漢字 (途中のヶ・ケ・ノを含む) で、組織・役職・一般語の字を含まない。
    pub fn could_be_surname(&self, s: &str) -> bool {
        let chars: Vec<char> = s.chars().collect();
        (1..=5).contains(&chars.len())
            && is_han(chars[0])
            && chars.iter().all(|c| is_han(*c) || "ケノ".contains(*c))
            && !chars.iter().any(|c| NOT_SURNAME_CHARS.contains(*c))
    }

    /// 辞書の姓か、姓らしい語か。
    pub fn is_surname_or_like(&self, s: &str) -> bool {
        self.is_jp_surname(s) || self.is_surname_like(s)
    }

    /// 「姓 + 名」がちょうど辞書の姓と名に分けられるか (姓・名とも 1〜4 文字、全体で 3 文字以上)。
    pub fn is_full_name(&self, s: &str) -> bool {
        self.split_full_name(s).is_some()
    }

    /// 「姓 + 名」に分けられれば、名の文字数 (名の長いほうの分け方を優先)。
    pub fn split_full_name(&self, s: &str) -> Option<usize> {
        let idx: Vec<usize> = s.char_indices().map(|x| x.0).collect();
        let n = idx.len();
        if !(3..=8).contains(&n) {
            return None;
        }
        (1..n).filter(|&k| k <= 4 && n - k <= 4).find(|&k| self.is_jp_surname(&s[..idx[k]]) && self.is_jp_given(&s[idx[k]..])).map(|k| n - k)
    }

    /// 先頭 1〜4 文字のいずれかが姓として辞書にあるか。
    pub fn starts_with_surname(&self, s: &str) -> bool {
        let chars: Vec<(usize, char)> = s.char_indices().collect();
        (1..=chars.len().min(4)).any(|n| {
            let end = chars.get(n).map(|x| x.0).unwrap_or(s.len());
            self.jp_surnames.contains(&s[..end])
        })
    }

    pub fn is_en_given_name(&self, s: &str) -> bool {
        self.en_given.contains(s)
    }

    /// ローマ字の日本人の姓・名か (大文字小文字は問わない)。
    pub fn is_romaji_name(&self, s: &str) -> bool {
        let l = s.to_ascii_lowercase();
        self.romaji_surnames.contains(l.as_str()) || self.romaji_given.contains(l.as_str())
    }

    /// 敬称なしの人名候補 (開始, 終了, 信頼度)。
    pub fn scan_names(&self, text: &str) -> Vec<(usize, usize, f32)> {
        // 日本語・英語・ローマ字の走査は独立なので並行して行う
        let (mut out, (en, romaji)) = rayon::join(
            || {
                let mut v = vec![];
                self.scan_jp(text, &mut v);
                v
            },
            || {
                rayon::join(
                    || {
                        let mut v = vec![];
                        self.scan_en(text, &mut v);
                        v
                    },
                    || {
                        let mut v = vec![];
                        self.scan_romaji(text, &mut v);
                        v
                    },
                )
            },
        );
        out.extend(en);
        out.extend(romaji);
        out
    }

    /// ローマ字の日本人名 (「Hanako Suzuki」「Suzuki Hanako」「YAMADA Taro」「Taro YAMADA」)。
    /// 1 語だけのものは誤検出が多いので、姓と名の両方が辞書にある 2 語の組だけを採る。
    fn scan_romaji(&self, text: &str, out: &mut Vec<(usize, usize, f32)>) {
        static RE: OnceLock<regex::Regex> = OnceLock::new();
        let re = RE.get_or_init(|| regex::Regex::new(r"(?-u:\b)([A-Z][a-z]{1,15}|[A-Z]{2,16})[ ]([A-Z][a-z]{1,15}|[A-Z]{2,16})(?-u:\b)").unwrap());
        let is_caps = |w: &str| w.len() >= 2 && w.bytes().all(|b| b.is_ascii_uppercase());
        let mut pos = 0;
        while let Some(c) = re.captures_at(text, pos) {
            let m = c.get(0).unwrap();
            let (a, b) = (c.get(1).unwrap().as_str(), c.get(2).unwrap().as_str());
            let (la, lb) = (a.to_ascii_lowercase(), b.to_ascii_lowercase());
            let sur = |w: &str| self.romaji_surnames.contains(w);
            let giv = |w: &str| self.romaji_given.contains(w);
            let conf = match (is_caps(a), is_caps(b)) {
                // 見出しなど全部大文字の 2 語は英語の可能性が高いので対象外
                (true, true) => None,
                (true, false) => (sur(&la) && giv(&lb)).then_some(0.78),
                (false, true) => (giv(&la) && sur(&lb)).then_some(0.78),
                (false, false) => {
                    if giv(&la) && sur(&lb) {
                        Some(0.72)
                    } else if sur(&la) && giv(&lb) {
                        Some(0.68)
                    } else {
                        None
                    }
                }
            };
            match conf {
                Some(conf) => {
                    out.push((m.start(), m.end(), conf));
                    pos = m.end();
                }
                // 2 語目から再試行 (「Meeting Hanako Suzuki」など)
                None => pos = c.get(2).unwrap().start(),
            }
        }
    }

    /// 敬称・項目名の無い姓。
    /// - 姓の並び (「田中・佐藤・鈴木」) で、前後に人を表す語 (「の 3 名」「出席者」など) がある: 0.75。語が無ければ 0.45 (使わない。「森林・平野・山地」のような一般語の並びがあるため)
    /// - 姓だけが、人名を書く位置にある (行に姓だけ・行末の空白の後・「〜です」・行末の「〜の田中」): 0.5
    ///
    /// 辞書の姓には「森林」「平野」「出口」のような一般語も多いので、文脈の無いものは低い信頼度にする。
    /// 既定のしきい値 (0.6) では使われず、strict プロファイル (0.5) で使われる。大きな地名 (「東京」など) は除く。
    fn scan_jp_lists_and_bare(&self, text: &str, runs: &[(usize, usize)], out: &mut Vec<(usize, usize, f32)>) {
        let is_item = |s: usize, e: usize| {
            let w = &text[s..e];
            let n = w.chars().count();
            (2..=4).contains(&n) && self.is_jp_surname(w) && !is_major_place(w) || self.is_full_name(w)
        };
        let separated = |e: usize, s2: usize| {
            let gap = text[e..s2].trim_matches([' ', '　']);
            crate::lexicon::LIST_SEPARATORS.contains(&gap)
        };
        let mut i = 0;
        while i < runs.len() {
            let mut j = i;
            while j < runs.len() && is_item(runs[j].0, runs[j].1) && (j == i || separated(runs[j - 1].1, runs[j].0)) {
                j += 1;
            }
            if j - i >= 2 {
                let (from, to) = (runs[i].0, runs[j - 1].1);
                let before = &text[crate::text::back_chars(text, from, 10)..from];
                let after = &text[to..crate::text::forward_chars(text, to, 10)];
                let person = crate::lexicon::PERSON_CONTEXT_WORDS.iter().any(|w| before.contains(w) || after.contains(w));
                for &(s, e) in &runs[i..j] {
                    out.push((s, e, if person { 0.75 } else { 0.45 }));
                }
                i = j;
                continue;
            }
            let (s, e) = runs[i];
            let w = &text[s..e];
            if (2..=4).contains(&w.chars().count()) && self.is_jp_surname(w) && !is_major_place(w) && name_like_position(text, s, e) {
                out.push((s, e, 0.5));
            }
            i += 1;
        }
    }

    fn scan_jp(&self, text: &str, out: &mut Vec<(usize, usize, f32)>) {
        // 漢字の連続区間を列挙
        let mut runs: Vec<(usize, usize)> = vec![];
        let mut start: Option<usize> = None;
        let mut prev: Option<char> = None;
        for (i, c) in text.char_indices() {
            // 「1日佐々木美咲」の「日」は日付の字なので人名の並びに含めない
            let date_unit = start.is_none() && prev.is_some_and(crate::text::is_digit_like) && crate::lexicon::DATE_UNIT_CHARS.contains(c);
            prev = Some(c);
            if date_unit {
                continue;
            }
            if is_han(c) && !crate::lexicon::is_name_split_char(c) {
                start.get_or_insert(i);
            } else if let Some(s) = start.take() {
                runs.push((s, i));
            }
        }
        if let Some(s) = start {
            runs.push((s, text.len()));
        }
        self.scan_jp_lists_and_bare(text, &runs, out);
        for (idx, &(s, e)) in runs.iter().enumerate() {
            let seg = &text[s..e];
            let n = seg.chars().count();
            // 「姓 名」(空白区切り)
            if let Some(&(s2, e2)) = runs.get(idx + 1) {
                let gap = &text[e..s2];
                if (gap == " " || gap == "　") && (1..=4).contains(&n) {
                    let given = &text[s2..e2];
                    let gn = given.chars().count();
                    if gn <= 4 && self.is_jp_surname(seg) && self.is_jp_given(given) {
                        let conf = if n >= 2 && gn >= 2 { 0.8 } else { 0.7 };
                        out.push((s, e2, conf));
                        continue;
                    }
                    // 辞書にない珍しい姓 + 辞書の名 (2 文字以上)。「五百旗頭 太郎」など
                    if (2..=4).contains(&gn) && self.rare_full_name(seg, given) {
                        out.push((s, e2, 0.66));
                        continue;
                    }
                }
            }
            // 「姓名」(連続)
            if (3..=7).contains(&n) {
                let idxs: Vec<usize> = seg.char_indices().map(|x| x.0).collect();
                let mut best: Option<f32> = None;
                for split in 1..n {
                    let sur = &seg[..idxs[split]];
                    let giv = &seg[idxs[split]..];
                    let gn = n - split;
                    if split > 4 || gn > 4 {
                        continue;
                    }
                    // 1 文字の姓 (林・森など) は、名が 2 文字以上の辞書の名のときだけ (「林太郎」「森陽子」)
                    if split == 1 && (gn < 2 || !crate::lexicon::SINGLE_CHAR_SURNAMES.contains(sur)) {
                        continue;
                    }
                    let conf = if self.is_jp_surname(sur) && self.is_jp_given(giv) {
                        if split == 1 {
                            0.6
                        } else if gn >= 2 {
                            0.68
                        } else {
                            0.55
                        }
                    } else if (2..=3).contains(&gn) && self.rare_full_name(sur, giv) {
                        // 辞書にない珍しい姓 + 辞書の名 (「月見里花子」など)。区切りがない分、少し低くする
                        0.6
                    } else {
                        continue;
                    };
                    best = Some(best.map_or(conf, |b: f32| b.max(conf)));
                }
                if let Some(conf) = best {
                    out.push((s, e, conf));
                }
            }
        }
    }

    fn scan_en(&self, text: &str, out: &mut Vec<(usize, usize, f32)>) {
        static RE: OnceLock<regex::Regex> = OnceLock::new();
        let re = RE.get_or_init(|| regex::Regex::new(r"(?-u:\b)([A-Z][a-z]{1,15})[ ]([A-Z][a-z]{1,20})(?-u:\b)").unwrap());
        let mut pos = 0;
        while let Some(c) = re.captures_at(text, pos) {
            let m = c.get(0).unwrap();
            let first = c.get(1).unwrap().as_str();
            let second = c.get(2).unwrap().as_str();
            if self.en_given.contains(first) && !EN_SECOND_STOP.contains(&second) {
                let conf = if self.en_surnames.contains(second) { 0.75 } else { 0.58 };
                out.push((m.start(), m.end(), conf));
                pos = m.end();
            } else {
                // 2 語目から再試行 (「Meeting John Smith」など)
                pos = c.get(2).unwrap().start();
            }
        }
    }
}

// ───────────────────────── 地名 ─────────────────────────

pub struct PlaceDict {
    places: HashSet<&'static str>,
}

impl PlaceDict {
    pub fn get() -> Arc<PlaceDict> {
        static D: OnceLock<Arc<PlaceDict>> = OnceLock::new();
        D.get_or_init(|| Arc::new(PlaceDict { places: lines(JP_PLACES).collect() })).clone()
    }

    pub fn contains(&self, s: &str) -> bool {
        self.places.contains(s)
    }
}

/// 地名 + 「市/区/町/村/駅/在住/出身」等を検出する。形態素解析があれば地域名トークンも使う。
pub struct PlaceDetector {
    dict: Arc<PlaceDict>,
    morph: Option<Arc<crate::morph::Morph>>,
}

impl PlaceDetector {
    pub fn new() -> Self {
        Self { dict: PlaceDict::get(), morph: None }
    }

    pub fn with_morphology(mut self, morph: Option<Arc<crate::morph::Morph>>) -> Self {
        self.morph = morph;
        self
    }
}

impl Default for PlaceDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detect for PlaceDetector {
    fn detect(&self, text: &str, out: &mut Vec<RawMatch>) {
        for suffix in PLACE_SUFFIXES {
            for (pos, _) in text.match_indices(suffix) {
                // 直前の漢字・カタカナ列 (最大 8 文字)
                let before: Vec<(usize, char)> = text[..pos]
                    .char_indices()
                    .rev()
                    .take_while(|(_, c)| is_han(*c) || ('\u{30A1}'..='\u{30FA}').contains(c) || *c == 'ー')
                    .take(8)
                    .collect();
                // 長い候補から順に辞書照合
                for k in (1..=before.len()).rev() {
                    let s = before[k - 1].0;
                    let cand = &text[s..pos];
                    if cand.chars().count() >= 2 && self.dict.contains(cand) {
                        out.push(RawMatch { start: s, end: pos + suffix.len(), confidence: 0.75 });
                        break;
                    }
                }
            }
        }
        if let Some(m) = &self.morph {
            for n in m.proper_nouns(text) {
                if n.kind == crate::morph::NounKind::Place && text[n.start..n.end].chars().count() >= 2 {
                    out.push(RawMatch { start: n.start, end: n.end, confidence: 0.65 });
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/dict.rs"]
mod tests;
