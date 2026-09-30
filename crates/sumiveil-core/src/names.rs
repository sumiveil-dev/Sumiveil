//! 人名検出 (多層方式)。
//!
//! 1. ルール層: 敬称・項目名・英語の敬称/挨拶など文脈から抽出
//! 2. 辞書層: 内蔵の姓・名辞書 (`dict` モジュール) で敬称なしの人名も抽出
//! 3. スコア統合: 文脈・辞書・文字種から信頼度を算出
//!
//! 文中の同一人名への伝播はエンジン側 (`engine`) で行う。

use std::sync::{Arc, OnceLock};

use regex::Regex;

use crate::detector::{Detect, RawMatch};
use crate::dict::NameDict;
use crate::morph::{Morph, NounKind};
use crate::text::char_after;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RuleKind {
    JpHonorific,
    JpLabel,
    /// 自己紹介 (「〜と申します」「担当の〜です」)
    JpSelfIntro,
    En,
    /// ローマ字の敬称 (「Tanaka-san」「Sato san」)
    Romaji,
}

struct Rule {
    re: Regex,
    conf: f32,
    kind: RuleKind,
}

const JP_NAME_CHARS: &str = r"\p{Han}\p{Katakana}\p{Hiragana}ー々\x{FF66}-\x{FF9F}";

fn rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let jp = JP_NAME_CHARS;
        let defs: Vec<(String, f32, RuleKind)> = vec![
            (
                // 姓の途中のヶ・ケ・ノ (「一番ケ瀬」「千ノ木」) は漢字にはさまれたときだけ。
                // 長い姓名 (「左衛門三郎花子様」= 5 + 2 文字) も先頭が欠けないよう、最大 8 文字まで
                r"(?P<v>\p{Han}(?:[\p{Han}々]|[ヶケノ]\p{Han}){0,7}(?:[ 　]\p{Han}[\p{Han}々]{0,3}|[ 　]\p{Hiragana}{2,4})?|\p{Katakana}[\p{Katakana}ー]{1,9}(?:[ 　・=＝][\p{Katakana}ー]{2,10})?)[ 　]?(?P<h>様|さま|さん|くん|君|ちゃん|殿|氏|先生|先輩)".to_string(),
                0.75,
                RuleKind::JpHonorific,
            ),
            (
                format!(r"(?:氏名|名前|お名前|担当者名|担当者|担当|宛先|宛名|差出人|送信者|作成者|記入者|承認者|申請者|依頼者|契約者|代表者|受取人|名義人|口座名義|フルネーム|ご芳名|芳名|患者名|利用者名|顧客名|会員名|署名)[ 　]*[:：][ 　]*(?P<v>[{jp}]{{1,10}}(?:[ 　][{jp}]{{1,10}})?)"),
                0.85,
                RuleKind::JpLabel,
            ),
            (
                r"(?P<v>\p{Han}[\p{Han}々]{0,3}(?:[ 　]\p{Han}[\p{Han}々]{0,3})?)(?:と申します|と申す者|でございます)".to_string(),
                0.8,
                RuleKind::JpSelfIntro,
            ),
            (
                r"(?:担当の|担当者の|営業の|窓口の|私、|私は|わたくし、|わたくしは)(?P<v>\p{Han}[\p{Han}々]{1,3})(?:です|が担当|が対応|より|から)".to_string(),
                0.75,
                RuleKind::JpSelfIntro,
            ),
            (
                r"(?:代表取締役(?:社長)?|取締役|代表者|理事長)[ 　]+(?P<v>\p{Han}[\p{Han}々]{0,3}[ 　]?\p{Han}[\p{Han}々]{0,3})".to_string(),
                0.8,
                RuleKind::JpLabel,
            ),
            (
                r"(?:Mr|Mrs|Ms|Miss|Mx|Dr|Prof)\.?[ ](?P<v>[A-Z][a-zA-Z'\-]+(?:[ ][A-Z][a-zA-Z'\-]+)?)".to_string(),
                0.85,
                RuleKind::En,
            ),
            (
                r"(?m)(?:^|[^A-Za-z])(?i:name|full name|contact person|contact|attn|attention|author|reviewer|assignee|reporter|owner|requester|approver|customer|client|employee|patient|signed by|from|to|cc)[ \t]*:[ \t]*(?P<v>[A-Z][a-zA-Z'\-]+(?:[ ][A-Z]\.)?(?:[ ][A-Z][a-zA-Z'\-]+){0,2})".to_string(),
                0.75,
                RuleKind::En,
            ),
            (
                r"(?:Dear|Hi|Hello|Hey)[ ,]+(?P<v>[A-Z][a-z'\-]+(?:[ ][A-Z][a-z'\-]+)?)[ ]*[,!.:\r\n]".to_string(),
                0.7,
                RuleKind::En,
            ),
            (
                r"(?:Regards|Best regards|Kind regards|Sincerely|Best|Cheers|Thanks|Thank you),?[ \t]*\r?\n[ \t]*(?P<v>[A-Z][a-z'\-]+(?:[ ][A-Z][a-z'\-]+)?)[ \t]*(?:\r?\n|$)".to_string(),
                0.7,
                RuleKind::En,
            ),
            (
                // 同じ行の結び (「Best regards, Hanako Suzuki」)。1 語だと「Thanks, Bye」等と区別できないので 2 語のみ
                r"(?m)(?:Regards|Best regards|Kind regards|Sincerely|Cheers|Thanks|Thank you),[ \t]*(?P<v>[A-Z][a-z'\-]+[ ][A-Z][a-z'\-]+)[ \t]*[.!]?[ \t]*$".to_string(),
                0.7,
                RuleKind::En,
            ),
            (
                r"(?-u:\b)(?P<v>[A-Z][a-z]{1,15}(?:[ ][A-Z][a-z]{1,15})?)(?:-|[ ])(?P<h>san|sama|sensei|kun|chan|dono)(?-u:\b)".to_string(),
                0.8,
                RuleKind::Romaji,
            ),
        ];
        defs.into_iter()
            .map(|(p, conf, kind)| Rule { re: Regex::new(&p).expect("name rule regex"), conf, kind })
            .collect()
    })
}

/// 名前の直前にあったら取り除く文字 (組織・役職の末尾など)。
const TRIM_BEFORE: &str = "部課係室社局店所班科会団省庁当者先宛各御貴弊";

/// 1 文字の姓として許容するもの。
const SINGLE_CHAR_SURNAMES: &str = "林森関東西南北堀辻泉岡原谷沢菊楠桜梅滝藤門丸角城島坂岩峰岸宮星牧杉柳桐榎柴畑浜嶋舘乾巽楓";

const JP_STOP: &[&str] = &[
    "皆", "各位", "客", "関係者", "担当者", "担当", "利用者", "管理者", "先方", "貴社", "御社", "弊社", "当社", "同社", "他社", "読者",
    "奥", "神", "王", "仏", "母", "父", "兄", "姉", "弟", "妹", "息子", "娘", "子", "孫", "夫", "妻", "嫁", "旦那", "彼", "彼女",
    "相手", "上司", "部下", "同僚", "後輩", "先輩", "学生", "生徒", "患者", "医者", "業者", "店員", "社員", "職員", "会員", "苦労",
    "世話", "馳走", "愁傷", "陰", "嬢", "主人", "家族", "殿", "様", "貴殿", "貴方", "貴女", "本人", "当人", "同氏", "自分", "私",
    "僕", "俺", "我", "人", "方", "大家", "家主", "住人", "隣人", "友人", "恋人", "先生", "教授", "顧客", "顧問", "役員", "代表",
    "取引先", "講師", "作者", "著者", "筆者", "選手", "監督", "知事", "大臣", "総理", "議員", "天皇", "陛下", "殿下", "王子", "姫",
    "菩薩", "如来", "地蔵", "看護師", "運転手", "医師", "弁護士", "税理士", "会計士", "技師", "職人", "配達員", "警察", "駅員",
    "営業", "管理人", "未定", "不明", "同上", "無し", "なし", "空欄", "省略", "様式", "仕様", "同様", "模様", "多様", "異様",
    "有様", "一様", "お客", "お疲れ", "ご苦労", "ユーザー", "ゲスト", "オーナー", "メンバー", "スタッフ", "ドクター", "マスター",
    "クライアント", "カスタマー", "パパ", "ママ", "ボス", "プロ", "ファン", "サポート", "チーム", "ベンダー", "パートナー",
];

const EN_STOP: &[&str] = &[
    "Team", "All", "Everyone", "There", "World", "Sir", "Madam", "Guys", "Folks", "Customer", "Support", "Admin", "Sales",
    "Friends", "Colleagues", "Hello", "Thanks", "Best", "Regards", "Hiring Manager", "Team Members", "Me", "You", "Us",
    "The", "This", "That", "It", "Unknown", "None", "Null", "Yes", "No", "Please", "Subject", "Re", "Fwd", "Date", "Sent",
    // 「Dear Mr. Yamada,」の「Mr」など (名前は敬称の規則で別に拾う)
    "Mr", "Mrs", "Ms", "Miss", "Mx", "Dr", "Prof",
    // ソースコードの型名 (「owner: String,」「to: Token」など)
    "String", "Str", "Option", "Some", "Vec", "Box", "Result", "Ok", "Err", "Self", "Token", "Value", "Object", "Integer",
    "Boolean", "Number", "Int", "Bool", "Char", "Byte", "Bytes", "Array", "List", "Map", "HashMap", "Dict", "Any", "Void",
];

/// ソースコードの宣言で始まる行 (この行の「項目名: 値」は人名として扱わない)。
const CODE_LINE_PREFIXES: &[&str] = &[
    "const ", "static ", "let ", "var ", "val ", "pub ", "fn ", "def ", "private ", "public ", "protected ", "readonly ", "type ",
];

/// 役職・職業などの語尾 (これで終わる候補は人名とみなさない)。
const JP_BAD_ENDINGS: &str = "師士員者人長家";

fn is_han(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' | '\u{F900}'..='\u{FAFF}' | '々')
}

fn is_hiragana(c: char) -> bool {
    ('\u{3041}'..='\u{309F}').contains(&c)
}

pub struct NameDetector {
    dict: Option<Arc<NameDict>>,
    dict_threshold: f32,
    morph: Option<Arc<Morph>>,
}

impl NameDetector {
    pub fn new(dict: Option<Arc<NameDict>>, dict_threshold: f32) -> Self {
        let _ = rules();
        Self { dict, dict_threshold, morph: None }
    }

    /// 形態素解析層を追加する。
    pub fn with_morphology(mut self, morph: Option<Arc<Morph>>) -> Self {
        self.morph = morph;
        self
    }

    /// 形態素解析の人名トークン (姓・名が並ぶものは結合) を候補にする。
    fn morph_candidates(&self, text: &str, out: &mut Vec<RawMatch>) {
        let Some(m) = &self.morph else { return };
        let nouns = m.proper_nouns(text);
        let is_person = |k: NounKind| matches!(k, NounKind::Surname | NounKind::GivenName | NounKind::PersonOther);
        let mut i = 0;
        while i < nouns.len() {
            if !is_person(nouns[i].kind) {
                i += 1;
                continue;
            }
            let start = nouns[i].start;
            let mut end = nouns[i].end;
            let mut kinds = vec![nouns[i].kind];
            let mut j = i + 1;
            while j < nouns.len() && is_person(nouns[j].kind) && matches!(&text[end..nouns[j].start], "" | " " | "　") {
                end = nouns[j].end;
                kinds.push(nouns[j].kind);
                j += 1;
            }
            let (s, g) = (kinds.contains(&NounKind::Surname), kinds.contains(&NounKind::GivenName));
            let mut conf: f32 = if s && g {
                0.8
            } else if s {
                0.6
            } else if kinds.contains(&NounKind::PersonOther) {
                0.65
            } else {
                0.5
            };
            let cand = &text[start..end];
            if JP_STOP.contains(&cand) || cand.chars().count() < 2 {
                conf = 0.0;
            }
            if conf > 0.0 {
                out.push(RawMatch { start, end, confidence: conf });
            }
            i = j;
        }
    }

    /// ルール層の候補を整形して信頼度を返す。不採用なら None。
    fn refine(&self, text: &str, s: usize, e: usize, kind: RuleKind, base: f32, honorific_end: usize) -> Option<(usize, usize, f32)> {
        let mut s = s;
        let mut e = e;
        let mut conf = base;
        match kind {
            RuleKind::JpHonorific => {
                // 敬称の直後が漢字なら熟語の一部 (様式・氏名・殿堂など)
                if let Some(c) = char_after(text, honorific_end) {
                    if is_han(c) && !"宛方".contains(c) {
                        return None;
                    }
                }
                // お客様・ご担当者様 など
                if let Some(c) = text[..s].chars().next_back() {
                    if "おご御".contains(c) {
                        return None;
                    }
                }
                // 組織名などを前から取り除く。ただし辞書の姓で終わる部分があればそこから採る
                // (「矢部様」「長曽我部様」「御手洗様」の「部」「御」で切らない。「営業部矢部様」→「矢部」)
                let cand = &text[s..e];
                let head = &cand[..cand.find([' ', '　']).unwrap_or(cand.len())];
                // 切る位置は先頭か、組織などの字 (部・御など) の直後だけ (「小鳥遊」を「遊」で切らない)
                let cut_points = std::iter::once(0).chain(head.char_indices().filter(|(_, c)| TRIM_BEFORE.contains(*c)).map(|(i, c)| i + c.len_utf8()));
                let known_from = self.dict.as_ref().and_then(|d| {
                    let mut cps = cut_points;
                    cps.find(|&i| i < head.len() && d.is_jp_surname(&head[i..]) && (i == 0 || head[i..].chars().count() >= 2))
                });
                match known_from {
                    Some(i) => s += i,
                    None => {
                        if let Some((i, c)) = cand.char_indices().filter(|(_, c)| TRIM_BEFORE.contains(*c)).last() {
                            s += i + c.len_utf8();
                        }
                    }
                }
                if s >= e {
                    return None;
                }
                // 「商事 田中様」のように空白の前が組織名なら後ろだけを採る
                if let Some(sp) = text[s..e].find([' ', '　']) {
                    let first = &text[s..s + sp];
                    let sp_len = text[s + sp..].chars().next().map_or(1, |c| c.len_utf8());
                    let second = &text[s + sp + sp_len..e];
                    let keep_full = match &self.dict {
                        // 敬称の前で、後ろが辞書の名 (2 文字以上) なら、前は辞書にない珍しい姓
                        // (「五百旗頭 太郎さん」「躑躅森 花子様」) として残す。組織・役職の字を含むものは除く
                        Some(d) => {
                            d.is_jp_surname(first)
                                || (d.is_jp_given(second) && (d.starts_with_surname(first) || (second.chars().count() >= 2 && d.could_be_surname(first))))
                        }
                        None => !first.chars().any(|c| "事社所業部課会店局団院校園".contains(c)),
                    };
                    if !keep_full {
                        s += sp + sp_len;
                    }
                }
                let cand = text[s..e].trim();
                let first = cand.chars().next()?;
                if is_han(first) {
                    let han_len = cand.chars().filter(|c| is_han(*c)).count();
                    if han_len == 1 && !SINGLE_CHAR_SURNAMES.contains(first) {
                        return None;
                    }
                    if cand.chars().last().is_some_and(|c| JP_BAD_ENDINGS.contains(c)) {
                        return None;
                    }
                } else {
                    conf -= 0.1; // カタカナ
                }
            }
            RuleKind::JpLabel => {
                // 末尾のひらがな (「です」等) と敬称を除去
                let cand = &text[s..e];
                if cand.chars().next().is_some_and(|c| !is_hiragana(c)) {
                    let trimmed = cand.trim_end_matches(is_hiragana);
                    e = s + trimmed.len();
                }
                for suffix in ["様", "殿", "氏", "さん"] {
                    if text[s..e].ends_with(suffix) {
                        e -= suffix.len();
                    }
                }
                let t = text[s..e].trim_end_matches([' ', '　']);
                e = s + t.len();
            }
            RuleKind::JpSelfIntro => {
                let cand = &text[s..e];
                if let Some((i, c)) = cand.char_indices().filter(|(_, c)| TRIM_BEFORE.contains(*c)).last() {
                    s += i + c.len_utf8();
                }
                if s >= e {
                    return None;
                }
                // 辞書で姓と確認できない場合は信頼度を下げる (「担当の者です」等の誤検出対策)
                if let Some(d) = &self.dict {
                    if !d.starts_with_surname(&text[s..e]) {
                        conf -= 0.25;
                    }
                }
            }
            RuleKind::En => {
                // ソースコード中の「const CLIENT: Token = Token(1);」「from: Option<String>」のような
                // 型注釈・代入は、項目名 (Client: / From:) の後ろでも人名ではない
                // 「Token(1)」「Option<String>」のように空白なしで続く括弧は呼び出し・型引数
                // (「John Smith (Sales)」「Jane Smith <jane@example.org>」は人名なので対象外)
                let after = &text[e..];
                let generic = after.starts_with('<') && !after[..after.find('>').unwrap_or(after.len())].contains('@');
                if after.starts_with(['(', '[']) || generic {
                    return None;
                }
                let rest = after.trim_start_matches([' ', '\t']);
                if rest.starts_with(['=', '{', ';']) || rest.starts_with("::") || rest.starts_with("->") {
                    return None;
                }
                let line_start = text[..s].rfind('\n').map_or(0, |i| i + 1);
                let head = text[line_start..s].trim_start();
                if CODE_LINE_PREFIXES.iter().any(|k| head.starts_with(k)) {
                    return None;
                }
            }
            RuleKind::Romaji => {
                // 「-san」のようにハイフンでつながっていれば人名とみなす。
                // 空白区切り (「Sato san」) は、辞書にある日本人の姓・名のときだけ
                if text.as_bytes().get(e) != Some(&b'-') {
                    let known = self.dict.as_ref().is_some_and(|d| text[s..e].split(' ').any(|w| d.is_romaji_name(w)));
                    if !known {
                        return None;
                    }
                }
            }
        }
        if s >= e {
            return None;
        }
        let cand = &text[s..e];
        let stop = match kind {
            RuleKind::En | RuleKind::Romaji => EN_STOP.iter().any(|w| cand.eq_ignore_ascii_case(w) || cand.split(' ').next().is_some_and(|f| f.eq_ignore_ascii_case(w))),
            _ => JP_STOP.contains(&cand),
        };
        if stop {
            return None;
        }
        // 辞書で裏付けが取れれば信頼度を上げる
        if let Some(d) = &self.dict {
            if !matches!(kind, RuleKind::En | RuleKind::Romaji) && d.starts_with_surname(cand) {
                conf = (conf + 0.15).min(1.0);
            }
            if kind == RuleKind::En && d.is_en_given_name(cand.split(' ').next().unwrap_or("")) {
                conf = (conf + 0.1).min(1.0);
            }
        }
        Some((s, e, conf))
    }
}

impl Detect for NameDetector {
    fn detect(&self, text: &str, out: &mut Vec<RawMatch>) {
        for rule in rules() {
            for caps in rule.re.captures_iter(text) {
                let v = caps.name("v").unwrap();
                let h_end = caps.name("h").map(|h| h.end()).unwrap_or(v.end());
                if let Some((s, e, conf)) = self.refine(text, v.start(), v.end(), rule.kind, rule.conf, h_end) {
                    out.push(RawMatch { start: s, end: e, confidence: conf });
                }
            }
        }
        if let Some(d) = &self.dict {
            for (s, e, conf) in d.scan_names(text) {
                if conf >= self.dict_threshold {
                    out.push(RawMatch { start: s, end: e, confidence: conf });
                }
            }
        }
        self.morph_candidates(text, out);
    }
}

#[cfg(test)]
mod tests {
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
}
