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
use crate::lexicon as lx;
use crate::morph::{Morph, NounKind};
use crate::text::{char_before, is_digit_like, is_han};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RuleKind {
    JpHonorific,
    JpLabel,
    /// 自己紹介 (「〜と申します」「担当の〜です」)
    JpSelfIntro,
    En,
    /// ローマ字の敬称 (「Tanaka-san」「Sato san」)
    Romaji,
    /// 姓 + 役職 (「田中部長」)。辞書の姓のときだけ採る
    JpTitleSuffix,
    /// 役職・部署 + 姓 (「課長の田中」「営業部 田中」)。辞書の姓のときだけ採る
    JpAfterTitle,
    /// 姓 + 助詞 + 人の行動 (「田中が担当」「山本に連絡」)。辞書の姓のときだけ採る
    JpSurnameAction,
}

struct Rule {
    re: Regex,
    conf: f32,
    kind: RuleKind,
}

const JP_NAME_CHARS: &str = r"\p{Han}\p{Katakana}\p{Hiragana}ー々\x{FF66}-\x{FF9F}";

/// 人名に使う漢字 (敬称の字「様・殿・氏」を除く)。正規表現の文字クラス。
const HAN_NAME: &str = r"[\p{Han}々&&[^様殿氏]]";

/// 語の一覧を正規表現の選択肢にする (長いものを先に)。
fn alternation(words: &[&str]) -> String {
    let mut w: Vec<&str> = words.to_vec();
    w.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
    w.iter().map(|s| regex::escape(s)).collect::<Vec<_>>().join("|")
}

/// 漢字の敬称の後ろに続くと熟語になる字の組 (「様式」「氏名」「殿堂」)。この場合は敬称ではない。
const HONORIFIC_COMPOUNDS: &[&str] = &[
    "様式", "様子", "様相", "様態", "様変", "氏名", "氏族", "氏神", "殿下", "殿堂", "殿方", "殿様", "君主", "君臨", "君子", "先生方", "先輩方",
];

fn rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let jp = JP_NAME_CHARS;
        let defs: Vec<(String, f32, RuleKind)> = vec![
            (
                // 姓の途中のヶ・ケ・ノ (「一番ケ瀬」「千ノ木」) は漢字にはさまれたときだけ。
                // 長い姓名 (「左衛門三郎花子様」= 5 + 2 文字) も先頭が欠けないよう、最大 8 文字まで
                // 名前の漢字に敬称の字 (様・殿・氏) は含めない (「田中様佐藤様」を 2 人に分ける)
                format!(r"(?P<v>{h}(?:{h}|[ヶケノ]{h}){{0,7}}(?:[ 　]{h}{h}{{0,3}}|[ 　]\p{{Hiragana}}{{2,4}})?|\p{{Katakana}}[\p{{Katakana}}ー]{{1,9}}(?:[ 　・=＝][\p{{Katakana}}ー]{{2,10}})?)[ 　]?(?P<h>様|さま|さん|くん|君|ちゃん|殿|氏|先生|先輩)", h = HAN_NAME),
                0.75,
                RuleKind::JpHonorific,
            ),
            (
                format!(r"(?:氏名|名前|お名前|担当者名|担当者|担当|宛先|宛名|差出人|送信者|作成者|記入者|承認者|申請者|依頼者|契約者|代表者|受取人|名義人|口座名義|フルネーム|ご芳名|芳名|患者名|利用者名|顧客名|会員名|署名)[ 　]*[:：][ 　]*(?P<v>[{jp}]{{1,20}}(?:[ 　][{jp}]{{1,10}})?)"),
                0.85,
                RuleKind::JpLabel,
            ),
            (
                r"(?P<v>\p{Han}[\p{Han}々]{0,3}(?:[ 　]\p{Han}[\p{Han}々]{0,3})?)(?:と申します|と申す者|でございます)".to_string(),
                0.8,
                RuleKind::JpSelfIntro,
            ),
            (
                r"(?:担当の|担当者の|営業の|窓口の|担当は|担当者は|窓口は|私、|私は|わたくし、|わたくしは)(?P<v>\p{Han}[\p{Han}々]{1,3})(?:です|が担当|が対応|より|から)".to_string(),
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
            (
                format!(r"(?P<v>{h}{{2,8}})(?P<h>{t})", h = HAN_NAME, t = alternation(lx::TITLE_SUFFIXES)),
                0.75,
                RuleKind::JpTitleSuffix,
            ),
            (
                // 役職の直後は区切りが無くてもよい (「担当田中」「部長田中」)。部署の字は区切りがあるときだけ (「室内田園」を拾わない)
                format!(r"(?:(?:{t})(?:の|[ 　])?|(?:[部課室係]|チーム|グループ)(?:の|[ 　]))(?P<v>{h}{{2,6}})", h = HAN_NAME, t = alternation(lx::TITLES)),
                0.7,
                RuleKind::JpAfterTitle,
            ),
            (
                // 手紙の結び「田中より」(行末) も同じ扱い
                format!(r"(?m)(?P<v>{h}{{2,8}})(?:(?:が|は|に|と|から|より|へ|も)(?:{a})|より[ 　]*$)", h = HAN_NAME, a = alternation(lx::PERSON_ACTIONS)),
                0.7,
                RuleKind::JpSurnameAction,
            ),
        ];
        defs.into_iter()
            .map(|(p, conf, kind)| Rule { re: Regex::new(&p).expect("name rule regex"), conf, kind })
            .collect()
    })
}


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

fn is_hiragana(c: char) -> bool {
    ('\u{3041}'..='\u{309F}').contains(&c)
}

/// 敬称として扱えるか。お客様・ご担当者様 (前が「お・ご・御」) と、熟語 (「様式」「氏名」) の一部は除く。
fn honorific_applies(text: &str, name_start: usize, honorific_end: usize) -> bool {
    if char_before(text, name_start).is_some_and(|c| "おご御".contains(c)) {
        return false;
    }
    let (before, after) = (&text[..honorific_end], &text[honorific_end..]);
    // 敬称の終わりから熟語が続く (「様式」= 様 + 式、「先生方」= 先生 + 方)
    !HONORIFIC_COMPOUNDS.iter().any(|w| {
        w.char_indices().skip(1).any(|(k, _)| before.ends_with(&w[..k]) && after.starts_with(&w[k..]))
    })
}

/// 「課長の」「営業部 」のように、人を指す役職・部署の後ろか。
/// 部署の字 (部・課など) は前が部署名のときだけ (「平野部の森林」の「部」は部署ではない)。
fn after_person_marker(text: &str, s: usize) -> bool {
    let head = text[..s].trim_end_matches([' ', '　']);
    let head = head.strip_suffix('の').unwrap_or(head);
    if lx::ends_with_any(head, lx::TITLES).is_some() {
        return true;
    }
    let unit = ["部", "課", "室", "係", "チーム", "グループ"].iter().find_map(|u| head.strip_suffix(u));
    unit.is_some_and(|before| lx::ends_with_any(before, lx::DEPARTMENT_WORDS).is_some())
}

/// [s, e) の末尾にある辞書の人名 (姓 + 名、または 2 文字以上の姓) の開始位置。長いものを優先。無ければ None。
fn known_name_suffix(d: &NameDict, text: &str, s: usize, e: usize) -> Option<usize> {
    let starts: Vec<usize> = text[s..e].char_indices().map(|(i, _)| s + i).collect();
    // 名が 2 文字以上の「姓 + 名」→ 姓 → 名が 1 文字の「姓 + 名」の順 (「本日田中」を「日田 + 中」と読まない)
    let full = |min_given: usize| starts.iter().copied().find(|&i| d.split_full_name(&text[i..e]).is_some_and(|g| g >= min_given));
    full(2)
        .or_else(|| starts.iter().copied().find(|&i| (2..=4).contains(&text[i..e].chars().count()) && d.is_jp_surname(&text[i..e])))
        .or_else(|| full(1))
}

/// [s, e) の先頭にある辞書の人名 (姓 + 名、または姓) の範囲。無ければ None。
fn known_name_prefix(d: &NameDict, text: &str, s: usize, e: usize) -> Option<(usize, usize)> {
    let idx: Vec<usize> = text[s..e].char_indices().map(|(i, _)| s + i).chain(std::iter::once(e)).collect();
    // 長いものから: 姓 + 名 → 姓 (2 文字以上)
    (3..idx.len()).rev().find(|&k| d.is_full_name(&text[s..idx[k]])).map(|k| (s, idx[k])).or_else(|| {
        (2..idx.len().min(5)).rev().find(|&k| d.is_jp_surname(&text[s..idx[k]])).map(|k| (s, idx[k]))
    })
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

    /// 候補の前に付いた組織名・役職・日付を取り除いた開始位置。
    /// 切る位置は先頭か、組織の字 (部・課など) ・役職 (部長など) の直後だけ (「小鳥遊」を「遊」で切らない)。
    /// 辞書の姓で始まる切り位置があればそこ (「営業部長田中様」→「田中」、「矢部様」は「部」で切らない)、無ければ最後の切り位置。
    fn trim_leading_org(&self, text: &str, s: usize, e: usize) -> usize {
        let mut s = s;
        // 「1日田中様」の「日」: 直前が数字なら日付の字を除く
        if char_before(text, s).is_some_and(is_digit_like) {
            if let Some(c) = text[s..e].chars().next().filter(|c| lx::DATE_UNIT_CHARS.contains(*c)) {
                s += c.len_utf8();
            }
        }
        let cand = &text[s..e];
        let head = &cand[..cand.find([' ', '　']).unwrap_or(cand.len())];
        let mut cuts: Vec<usize> = head.char_indices().filter(|(_, c)| lx::ORG_TAIL_CHARS.contains(*c)).map(|(i, c)| i + c.len_utf8()).collect();
        for (i, _) in head.char_indices() {
            if let Some(t) = lx::starts_with_any(&head[i..], lx::TITLES) {
                cuts.push(i + t.len());
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        let known = self.dict.as_ref().and_then(|d| {
            std::iter::once(0).chain(cuts.iter().copied()).find(|&i| i < head.len() && d.is_jp_surname(&head[i..]) && (i == 0 || head[i..].chars().count() >= 2))
        });
        s + known.or_else(|| cuts.last().copied().filter(|&i| i < cand.len())).unwrap_or(0)
    }

    /// 「商事 田中様」のように空白の前が組織名なら、空白の後ろからの開始位置。
    fn drop_org_before_space(&self, text: &str, s: usize, e: usize) -> usize {
        let Some(sp) = text[s..e].find([' ', '　']) else { return s };
        let first = &text[s..s + sp];
        let sp_len = text[s + sp..].chars().next().map_or(1, |c| c.len_utf8());
        let second = &text[s + sp + sp_len..e];
        let katakana = |w: &str| !w.is_empty() && w.chars().all(|c| ('\u{30A1}'..='\u{30FF}').contains(&c));
        let keep_full = katakana(first) && katakana(second)
            || match &self.dict {
                // 後ろが辞書の名 (2 文字以上) なら、前は辞書にない珍しい姓 (「五百旗頭 太郎さん」) として残す。組織・役職の字を含むものは除く
                Some(d) => {
                    d.is_jp_surname(first)
                        || (d.is_jp_given(second) && (d.starts_with_surname(first) || (second.chars().count() >= 2 && d.could_be_surname(first))))
                }
                None => !first.chars().any(|c| "事社所業部課会店局団院校園".contains(c)),
            };
        if keep_full {
            s
        } else {
            s + sp + sp_len
        }
    }

    /// ルール層の候補を整形して信頼度を返す。不採用なら None。
    fn refine(&self, text: &str, s: usize, e: usize, kind: RuleKind, base: f32, honorific_end: usize) -> Option<(usize, usize, f32)> {
        let mut s = s;
        let mut e = e;
        let mut conf = base;
        match kind {
            RuleKind::JpHonorific => {
                if !honorific_applies(text, s, honorific_end) {
                    return None;
                }
                s = self.trim_leading_org(text, s, e);
                if s >= e {
                    return None;
                }
                s = self.drop_org_before_space(text, s, e);
                let cand = text[s..e].trim();
                let first = cand.chars().next()?;
                if is_han(first) {
                    let han_len = cand.chars().filter(|c| is_han(*c)).count();
                    if han_len == 1 && !lx::SINGLE_CHAR_SURNAMES.contains(first) {
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
                // 項目名の後ろの会社名・部署名を取り除く (「担当: アオバ商事株式会社営業部 田中」→「田中」)
                s = self.trim_leading_org(text, s, e);
                if s >= e {
                    return None;
                }
                s = self.drop_org_before_space(text, s, e);
            }
            RuleKind::JpSurnameAction => {
                // 前に付いた語 (「本日田中が」の「本日」) を除き、末尾の辞書の姓 (または姓 + 名) だけを採る
                let d = self.dict.as_ref()?;
                let ns = known_name_suffix(d, text, s, e)?;
                if crate::dict::is_major_place(&text[ns..e]) {
                    return None;
                }
                s = ns;
            }
            RuleKind::JpTitleSuffix | RuleKind::JpAfterTitle => {
                // 役職は人名でなくても付くので、辞書の姓 (または姓 + 名) のときだけ採る
                let d = self.dict.as_ref()?;
                if kind == RuleKind::JpAfterTitle && !after_person_marker(text, s) {
                    return None;
                }
                if kind == RuleKind::JpTitleSuffix {
                    s = self.trim_leading_org(text, s, e);
                }
                let (ns, ne) = known_name_prefix(d, text, s, e)?;
                if kind == RuleKind::JpTitleSuffix && ne != e {
                    return None;
                }
                s = ns;
                e = ne;
            }
            RuleKind::JpSelfIntro => {
                let cand = &text[s..e];
                if let Some((i, c)) = cand.char_indices().filter(|(_, c)| lx::ORG_TAIL_CHARS.contains(*c)).last() {
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
            _ => JP_STOP.contains(&cand) || lx::SHOP_WORDS.contains(&cand),
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

impl NameDetector {
    fn rule_candidates(&self, text: &str, rule: &Rule) -> Vec<RawMatch> {
        rule.re
            .captures_iter(text)
            .filter_map(|caps| {
                let v = caps.name("v")?;
                let h_end = caps.name("h").map_or(v.end(), |h| h.end());
                let (s, e, confidence) = self.refine(text, v.start(), v.end(), rule.kind, rule.conf, h_end)?;
                Some(RawMatch { start: s, end: e, confidence })
            })
            .collect()
    }

    fn dictionary_candidates(&self, text: &str) -> Vec<RawMatch> {
        let Some(d) = &self.dict else { return vec![] };
        // 会社名のすぐ後ろの人名 (「株式会社アオバ商事田中太郎」)。会社名の検出器と同じ解析を使う
        let mut v = crate::company::analyze(text, Some(d)).1;
        v.extend(d.scan_names(text).into_iter().filter(|c| c.2 >= self.dict_threshold).map(|(start, end, confidence)| RawMatch { start, end, confidence }));
        v
    }
}

impl Detect for NameDetector {
    /// 規則ごと・辞書の層は互いに独立なので並行して調べる (人名は検出器の中でいちばん時間がかかるため)。
    fn detect(&self, text: &str, out: &mut Vec<RawMatch>) {
        use rayon::prelude::*;
        let (by_rules, by_dict) = rayon::join(
            || rules().par_iter().flat_map_iter(|rule| self.rule_candidates(text, rule)).collect::<Vec<_>>(),
            || self.dictionary_candidates(text),
        );
        out.extend(by_rules);
        out.extend(by_dict);
        self.morph_candidates(text, out);
    }
}

#[cfg(test)]
#[path = "tests/names.rs"]
mod tests;
