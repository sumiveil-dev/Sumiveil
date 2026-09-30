//! 検出器カタログ。カテゴリと各検出器のメタデータ・パターン定義。
//!
//! パターン中のプレースホルダ:
//! - `{DC}` 数字の文字クラス内容 (半角/全角)
//! - `{HC}` ハイフン類の文字クラス内容
//! - `{PREF}` 都道府県名の選択肢

use crate::validators::{self as v, Validator};

#[derive(Debug, Clone, Copy)]
pub struct CategoryInfo {
    pub id: &'static str,
    pub name_en: &'static str,
    pub name_ja: &'static str,
}

pub const CATEGORIES: &[CategoryInfo] = &[
    CategoryInfo { id: "contact", name_en: "Contact", name_ja: "連絡先" },
    CategoryInfo { id: "personal", name_en: "Personal", name_ja: "個人情報" },
    CategoryInfo { id: "jp_id", name_en: "Japanese IDs", name_ja: "日本の公的ID" },
    CategoryInfo { id: "intl_id", name_en: "International IDs", name_ja: "海外のID" },
    CategoryInfo { id: "finance", name_en: "Finance", name_ja: "金融" },
    CategoryInfo { id: "network", name_en: "Network / IT", name_ja: "ネットワーク/IT" },
    CategoryInfo { id: "secret", name_en: "Credentials / Secrets", name_ja: "認証情報・シークレット" },
    CategoryInfo { id: "location", name_en: "Location", name_ja: "位置情報" },
    CategoryInfo { id: "organization", name_en: "Organization", name_ja: "組織" },
    CategoryInfo { id: "custom", name_en: "Custom", name_ja: "カスタム" },
];

pub fn category(id: &str) -> Option<&'static CategoryInfo> {
    CATEGORIES.iter().find(|c| c.id == id)
}

/// マッチ前後の境界条件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    None,
    /// 前後が ASCII 英数字・`_` でないこと
    Alnum,
    /// 前後が数字 (全角含む)・ASCII 英字・ハイフン類でないこと
    Digit,
    /// IPv4 用: 前後が英数字でなく、`.` の後に数字が続かないこと
    Ip,
    /// 16 進 + コロン区切り (IPv6/MAC) 用
    Hex,
    /// Windows のドメイン\ユーザー用
    Path,
}

#[derive(Debug, Clone, Copy)]
pub struct ContextSpec {
    /// 小文字で記述する
    pub keywords: &'static [&'static str],
    /// true: キーワードが無ければ不採用 (validator が Strong を返した場合を除く)
    pub required: bool,
    /// マッチ位置の前方何文字を見るか
    pub before: usize,
    /// マッチ位置の後方何文字を見るか
    pub after: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct RegexSpec {
    /// (パターン, 基本信頼度)。名前付きグループ `v` があればその範囲をマスクする。
    pub patterns: &'static [(&'static str, f32)],
    pub boundary: Boundary,
    pub validator: Option<Validator>,
    pub context: Option<ContextSpec>,
    /// 末尾の句読点 (`.,;:!?)` 等) を取り除く
    pub trim_punct: bool,
}

const R: RegexSpec = RegexSpec {
    patterns: &[],
    boundary: Boundary::None,
    validator: None,
    context: None,
    trim_punct: false,
};

#[derive(Debug, Clone, Copy)]
pub enum SpecKind {
    Regex(RegexSpec),
    /// 人名 (ルール層 + 辞書層)
    PersonName,
    /// 地名辞書 + 「市/区/駅/在住」等
    PlaceName,
}

#[derive(Debug, Clone, Copy)]
pub struct DetectorSpec {
    pub id: &'static str,
    pub category: &'static str,
    /// テンプレートの `{label}`
    pub label: &'static str,
    /// テンプレートの `{label_ja}`
    pub label_ja: &'static str,
    pub name_en: &'static str,
    pub name_ja: &'static str,
    pub default_enabled: bool,
    pub priority: i32,
    pub kind: SpecKind,
    /// 検出されるべきサンプル (ダミーデータ)。テストと GUI の説明で使う。
    pub example: &'static str,
}

pub fn detector(id: &str) -> Option<&'static DetectorSpec> {
    CATALOG.iter().find(|d| d.id == id)
}

const fn ctx(keywords: &'static [&'static str], required: bool) -> Option<ContextSpec> {
    Some(ContextSpec { keywords, required, before: 24, after: 8 })
}

pub const PREFECTURES: &str = "北海道|青森県|岩手県|宮城県|秋田県|山形県|福島県|茨城県|栃木県|群馬県|埼玉県|千葉県|東京都|神奈川県|新潟県|富山県|石川県|福井県|山梨県|長野県|岐阜県|静岡県|愛知県|三重県|滋賀県|京都府|大阪府|兵庫県|奈良県|和歌山県|鳥取県|島根県|岡山県|広島県|山口県|徳島県|香川県|愛媛県|高知県|福岡県|佐賀県|長崎県|熊本県|大分県|宮崎県|鹿児島県|沖縄県";

const DATE_PATTERNS: &[(&str, f32)] = &[
    (r"[{DC}]{4}(?:[/\-.]|年)[ 　]?[{DC}]{1,2}(?:[/\-.]|月)[ 　]?[{DC}]{1,2}日?", 0.8),
    (r"(?:明治|大正|昭和|平成|令和)[ 　]?(?:[{DC}]{1,2}|元)年[ 　]?[{DC}]{1,2}月[ 　]?[{DC}]{1,2}日", 0.85),
    (r"[MTSHR]\.?[{DC}]{1,2}[/\-.][{DC}]{1,2}[/\-.][{DC}]{1,2}", 0.7),
    (r"[{DC}]{1,2}/[{DC}]{1,2}/[{DC}]{4}", 0.75),
    (r"(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)[a-z]*\.?\s[0-9]{1,2},?\s[0-9]{4}", 0.75),
    (r"[0-9]{1,2}\s(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)[a-z]*\.?\s[0-9]{4}", 0.75),
];

/// 関数呼び出し・コンストラクタの形 (「Token(0)」「os.getenv(」「getpass()」)。値ではなくコード。
fn is_code_call(s: &str) -> bool {
    let Some(i) = s.find('(') else { return false };
    let head = &s[..i];
    head.len() >= 2
        && head.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && head.chars().all(|c| c.is_ascii_alphanumeric() || "_.:".contains(c))
}

fn not_placeholder(s: &str) -> v::Verdict {
    let l = s.to_ascii_lowercase();
    // 「password: &str」「hash_password::<Sha256>(」のような型注釈・パスもコード
    let bad = s.starts_with(['$', '{', '<', '%', '(', '[', '&', ':'])
        || is_code_call(s)
        || s.chars().all(|c| c == '*' || c == 'x' || c == 'X' || c == '•' || c == '●')
        || ["null", "none", "true", "false", "nil", "undefined", "required", "optional", "string", "\"\"", "''"]
            .contains(&l.as_str());
    if bad {
        v::Verdict::Reject
    } else {
        v::Verdict::Accept
    }
}

fn not_common_user(s: &str) -> v::Verdict {
    let l = s.trim().to_ascii_lowercase();
    if ["public", "default", "default user", "all users", "defaultuser0", "%username%", "<user>", "username", "user", "name"]
        .contains(&l.as_str())
    {
        v::Verdict::Reject
    } else {
        v::Verdict::Accept
    }
}

fn hostname_ok(s: &str) -> v::Verdict {
    let l = s.to_ascii_lowercase();
    if ["asp.net", "vb.net", "ado.net", "e.co", "i.e", "e.g"].contains(&l.as_str()) {
        v::Verdict::Reject
    } else {
        v::Verdict::Accept
    }
}

fn domain_user(s: &str) -> v::Verdict {
    let dom = s.split('\\').next().unwrap_or("").to_ascii_uppercase();
    let reserved = ["HKLM", "HKCU", "HKCR", "HKU", "HKCC", "BUILTIN", "SYSTEM", "SOFTWARE", "HARDWARE", "SAM", "SECURITY", "NT", "WINDOWS", "PROGRA", "USERS", "CONTROL", "SRC", "LIB", "BIN", "DOCS", "APP", "APPS", "TEST", "TESTS", "DATA", "TEMP", "TMP", "OBJ", "BUILD", "DIST", "TARGET"];
    if dom.starts_with("HKEY_") || reserved.contains(&dom.as_str()) {
        v::Verdict::Reject
    } else {
        v::Verdict::Accept
    }
}

pub static CATALOG: &[DetectorSpec] = &[
    // ───────────── 連絡先 ─────────────
    DetectorSpec {
        id: "email", category: "contact", label: "EMAIL", label_ja: "メールアドレス",
        name_en: "Email address", name_ja: "メールアドレス",
        default_enabled: true, priority: 60,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9](?:[A-Za-z0-9\-]*[A-Za-z0-9])?(?:\.[A-Za-z0-9](?:[A-Za-z0-9\-]*[A-Za-z0-9])?)*\.[A-Za-z]{2,24}", 0.95)],
            ..R
        }),
        example: "連絡先: taro.yamada@corp.example.co.jp まで",
    },
    DetectorSpec {
        id: "phone_jp", category: "contact", label: "PHONE", label_ja: "電話番号",
        name_en: "Phone number (Japan)", name_ja: "電話番号 (日本)",
        default_enabled: true, priority: 65,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?:\+81[ 　{HC}]?(?:\(0\)|0)?|[(（]?[0０])[{DC}]{1,4}[)）]?[ 　{HC}]?[(（]?[{DC}]{1,4}[)）]?[ 　{HC}]?[{DC}]{3,4}", 0.7)],
            boundary: Boundary::Digit,
            validator: Some(v::phone_jp),
            context: ctx(&["tel", "電話", "携帯", "phone", "fax", "連絡先", "mobile"], false),
            ..R
        }),
        example: "TEL: 090-1234-5678",
    },
    DetectorSpec {
        id: "phone_intl", category: "contact", label: "PHONE", label_ja: "電話番号",
        name_en: "Phone number (international)", name_ja: "電話番号 (国際)",
        default_enabled: true, priority: 60,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"\+(?:[1-79]|8[02-9])[0-9]{0,2}[ \-.]?\(?[0-9]{1,4}\)?(?:[ \-.]?[0-9]{2,4}){2,4}", 0.85)],
            boundary: Boundary::Digit,
            validator: Some(v::phone_intl),
            ..R
        }),
        example: "Call +1 415-555-0132",
    },
    DetectorSpec {
        id: "postal_code_jp", category: "contact", label: "POSTAL", label_ja: "郵便番号",
        name_en: "Postal code (Japan)", name_ja: "郵便番号",
        default_enabled: true, priority: 55,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[
                (r"〒[ 　]?[{DC}]{3}[{HC}]?[{DC}]{4}", 0.95),
                (r"[{DC}]{3}[{HC}][{DC}]{4}", 0.6),
            ],
            boundary: Boundary::Digit,
            context: ctx(&["郵便", "〒", "住所", "zip", "postal"], false),
            ..R
        }),
        example: "〒100-0001 東京都千代田区",
    },
    // ───────────── 個人情報 ─────────────
    DetectorSpec {
        id: "person_name", category: "personal", label: "NAME", label_ja: "氏名",
        name_en: "Person name", name_ja: "人名",
        default_enabled: true, priority: 40,
        kind: SpecKind::PersonName,
        example: "山田太郎様、お世話になっております。",
    },
    DetectorSpec {
        id: "address_jp", category: "personal", label: "ADDRESS", label_ja: "住所",
        name_en: "Address (Japan, with prefecture)", name_ja: "住所 (都道府県から)",
        default_enabled: true, priority: 58,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?:{PREF})[^\s、。,，「」『』()（）<>]{1,25}?(?:[{DC}]{1,4}|[一二三四五六七八九十]{1,3})(?:(?:丁目|番地|番|号|[{HC}]|の)(?:[{DC}]{1,4}|[一二三四五六七八九十]{1,3})?){0,4}(?:号)?(?:[ 　]?[^\s、。,，]{1,20}?(?:ビル|マンション|ハイツ|コーポ|アパート|荘|タワー|レジデンス|ハウス|ヒルズ)(?:[ 　]?[{DC}]{1,4}(?:階|F|号室)?)?)?", 0.9)],
            validator: Some(v::has_digit),
            ..R
        }),
        example: "東京都千代田区架空町1丁目2-3 サンプルビル5F",
    },
    DetectorSpec {
        id: "address_jp_city", category: "personal", label: "ADDRESS", label_ja: "住所",
        name_en: "Address (Japan, city + block number)", name_ja: "住所 (市区町村から)",
        default_enabled: true, priority: 56,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[\p{Han}\p{Katakana}ヶケ]{1,5}[市区町村郡][\p{Han}\p{Katakana}\p{Hiragana}ヶケ々]{0,10}?(?:[{DC}]{1,4}|[一二三四五六七八九十]{1,3})(?:丁目|番地|[{HC}])(?:[{DC}]{1,4}|[一二三四五六七八九十]{1,3})(?:(?:番|号|[{HC}])(?:[{DC}]{1,4})?){0,2}(?:号)?", 0.75)],
            ..R
        }),
        example: "横浜市中区山下町1-2-3",
    },
    DetectorSpec {
        id: "address_en", category: "personal", label: "ADDRESS", label_ja: "住所",
        name_en: "Street address (English)", name_ja: "住所 (英語表記)",
        default_enabled: true, priority: 55,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[0-9]{1,5}\s(?:[A-Z][a-z]+\s){1,3}(?:Street|St\.?|Avenue|Ave\.?|Road|Rd\.?|Boulevard|Blvd\.?|Lane|Ln\.?|Drive|Dr\.?|Court|Ct\.?|Way|Place|Pl\.?)(?:,?\s(?:Apt|Suite|Unit)\.?\s?[A-Za-z0-9\-]+)?", 0.75)],
            boundary: Boundary::Digit,
            ..R
        }),
        example: "Ship to 1600 Example Avenue, Suite 12",
    },
    DetectorSpec {
        id: "birthdate", category: "personal", label: "BIRTHDATE", label_ja: "生年月日",
        name_en: "Date of birth", name_ja: "生年月日",
        default_enabled: true, priority: 57,
        kind: SpecKind::Regex(RegexSpec {
            patterns: DATE_PATTERNS,
            boundary: Boundary::Digit,
            context: ctx(&["生年月日", "誕生日", "生まれ", "birth", "dob", "born"], true),
            ..R
        }),
        example: "生年月日: 1985年4月1日",
    },
    DetectorSpec {
        id: "date", category: "personal", label: "DATE", label_ja: "日付",
        name_en: "Date (any)", name_ja: "日付 (すべて)",
        default_enabled: false, priority: 30,
        kind: SpecKind::Regex(RegexSpec { patterns: DATE_PATTERNS, boundary: Boundary::Digit, ..R }),
        example: "2024/03/15 に実施",
    },
    DetectorSpec {
        id: "age", category: "personal", label: "AGE", label_ja: "年齢",
        name_en: "Age", name_ja: "年齢",
        default_enabled: false, priority: 30,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[
                (r"[{DC}]{1,3}[ 　]?(?:歳|才)", 0.8),
                (r"(?i)(?:age|aged)\s?:?\s?(?P<v>[0-9]{1,3})", 0.7),
            ],
            boundary: Boundary::Digit,
            ..R
        }),
        example: "年齢 42歳",
    },
    DetectorSpec {
        id: "employee_id", category: "personal", label: "EMPLOYEE_ID", label_ja: "社員番号",
        name_en: "Employee ID", name_ja: "社員番号",
        default_enabled: true, priority: 62,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?i)(?:社員番号|社員ID|社員コード|従業員番号|職員番号|スタッフID|employee\s?(?:id|no\.?|number)|emp\s?id)[ 　]*[:：#＃]?[ 　]*(?P<v>[A-Za-z0-9０-９Ａ-Ｚ\-]{3,15})", 0.9)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "社員番号: A12345",
    },
    // ───────────── 日本の公的ID ─────────────
    DetectorSpec {
        id: "my_number", category: "jp_id", label: "MY_NUMBER", label_ja: "マイナンバー",
        name_en: "My Number (individual number)", name_ja: "マイナンバー (個人番号)",
        default_enabled: true, priority: 80,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[{DC}]{4}[ 　{HC}]?[{DC}]{4}[ 　{HC}]?[{DC}]{4}", 0.7)],
            boundary: Boundary::Digit,
            validator: Some(v::my_number),
            context: ctx(&["マイナンバー", "個人番号", "my number", "mynumber"], false),
            ..R
        }),
        example: "マイナンバー: 1234 5678 9018",
    },
    DetectorSpec {
        id: "corporate_number", category: "jp_id", label: "CORP_NUMBER", label_ja: "法人番号",
        name_en: "Corporate number / Invoice registration no.", name_ja: "法人番号・インボイス登録番号",
        default_enabled: true, priority: 78,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[TＴ]?[{DC}]{13}", 0.85)],
            boundary: Boundary::Digit,
            validator: Some(v::corporate_number),
            context: ctx(&["法人番号", "登録番号", "インボイス", "適格請求書", "corporate number", "invoice"], true),
            ..R
        }),
        example: "登録番号 T7000012050002",
    },
    DetectorSpec {
        id: "drivers_license_jp", category: "jp_id", label: "DRIVERS_LICENSE", label_ja: "運転免許証番号",
        name_en: "Driver's license number (Japan)", name_ja: "運転免許証番号",
        default_enabled: true, priority: 80,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[{DC}]{12}", 0.9)],
            boundary: Boundary::Digit,
            context: ctx(&["免許", "license", "licence"], true),
            ..R
        }),
        example: "運転免許証番号 301234567890",
    },
    DetectorSpec {
        id: "passport_jp", category: "jp_id", label: "PASSPORT", label_ja: "旅券番号",
        name_en: "Passport number (Japan)", name_ja: "パスポート番号",
        default_enabled: true, priority: 80,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[A-Z]{2}[{DC}]{7}", 0.9)],
            boundary: Boundary::Alnum,
            context: ctx(&["旅券", "パスポート", "passport"], true),
            ..R
        }),
        example: "パスポート番号: TK1234567",
    },
    DetectorSpec {
        id: "residence_card_jp", category: "jp_id", label: "RESIDENCE_CARD", label_ja: "在留カード番号",
        name_en: "Residence card number (Japan)", name_ja: "在留カード番号",
        default_enabled: true, priority: 80,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[A-Z]{2}[0-9]{8}[A-Z]{2}", 0.85)],
            boundary: Boundary::Alnum,
            context: ctx(&["在留", "residence"], false),
            ..R
        }),
        example: "在留カード AB12345678CD",
    },
    DetectorSpec {
        id: "pension_number_jp", category: "jp_id", label: "PENSION_NO", label_ja: "基礎年金番号",
        name_en: "Basic pension number (Japan)", name_ja: "基礎年金番号",
        default_enabled: true, priority: 80,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[{DC}]{4}[{HC}]?[{DC}]{6}", 0.9)],
            boundary: Boundary::Digit,
            context: ctx(&["年金", "pension"], true),
            ..R
        }),
        example: "基礎年金番号 1234-567890",
    },
    DetectorSpec {
        id: "health_insurance_jp", category: "jp_id", label: "HEALTH_INSURANCE", label_ja: "保険証番号",
        name_en: "Health insurance card number (Japan)", name_ja: "健康保険証の記号・番号",
        default_enabled: true, priority: 80,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[
                (r"記号[ 　]*[:：]?[ 　]*(?P<v>[0-9０-９A-Za-z]{1,8}[ 　・,、]*番号[ 　]*[:：]?[ 　]*[{DC}]{1,10})", 0.9),
                (r"(?:被保険者|保険者)番号[ 　]*[:：]?[ 　]*(?P<v>[{DC}]{6,8})", 0.9),
            ],
            ..R
        }),
        example: "保険証 記号 1234 番号 56",
    },
    DetectorSpec {
        id: "bank_account_jp", category: "jp_id", label: "BANK_ACCOUNT", label_ja: "口座番号",
        name_en: "Bank account (Japan)", name_ja: "銀行口座番号",
        default_enabled: true, priority: 80,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[
                (r"(?:口座番号|口座No\.?|口座)[ 　]*[:：]?[ 　]*(?:(?:普通|当座|貯蓄)(?:預金)?[ 　]*)?(?P<v>[{DC}]{7})", 0.9),
                (r"(?:普通|当座|貯蓄)(?:預金)?[ 　]*[:：]?[ 　]*(?:口座)?[ 　]*(?P<v>[{DC}]{7})", 0.85),
                (r"(?:支店番号|店番)[ 　]*[:：]?[ 　]*(?P<v>[{DC}]{3})", 0.8),
            ],
            boundary: Boundary::Digit,
            ..R
        }),
        example: "振込先: 普通 1234567",
    },
    // ───────────── 海外ID ─────────────
    DetectorSpec {
        id: "us_ssn", category: "intl_id", label: "SSN", label_ja: "社会保障番号",
        name_en: "US Social Security Number", name_ja: "米国社会保障番号 (SSN)",
        default_enabled: true, priority: 75,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[0-9]{3}-[0-9]{2}-[0-9]{4}", 0.75)],
            boundary: Boundary::Digit,
            validator: Some(v::us_ssn),
            context: ctx(&["ssn", "social security"], false),
            ..R
        }),
        example: "SSN: 123-45-6789",
    },
    DetectorSpec {
        id: "iban", category: "intl_id", label: "IBAN", label_ja: "IBAN",
        name_en: "IBAN", name_ja: "国際銀行口座番号 (IBAN)",
        default_enabled: true, priority: 80,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[A-Z]{2}[0-9]{2}(?: ?[A-Z0-9]{4}){2,7}(?: ?[A-Z0-9]{1,4})?", 0.95)],
            boundary: Boundary::Alnum,
            validator: Some(v::iban),
            ..R
        }),
        example: "IBAN GB82 WEST 1234 5698 7654 32",
    },
    DetectorSpec {
        id: "swift_bic", category: "intl_id", label: "SWIFT", label_ja: "SWIFTコード",
        name_en: "SWIFT / BIC code", name_ja: "SWIFT/BIC コード",
        default_enabled: true, priority: 70,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}(?:[A-Z0-9]{3})?", 0.85)],
            boundary: Boundary::Alnum,
            context: ctx(&["swift", "bic"], true),
            ..R
        }),
        example: "SWIFT: EXAMJPJT",
    },
    // ───────────── 金融 ─────────────
    DetectorSpec {
        id: "credit_card", category: "finance", label: "CREDIT_CARD", label_ja: "カード番号",
        name_en: "Credit card number", name_ja: "クレジットカード番号",
        default_enabled: true, priority: 85,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[{DC}]{4}[ 　{HC}]?[{DC}]{4}[ 　{HC}]?[{DC}]{4}[ 　{HC}]?[{DC}]{4}(?:[{DC}]{1,3})?|[{DC}]{4}[ 　{HC}]?[{DC}]{6}[ 　{HC}]?[{DC}]{4,5}", 0.9)],
            boundary: Boundary::Digit,
            validator: Some(v::credit_card),
            context: ctx(&["カード", "card", "クレジット", "visa", "master"], false),
            ..R
        }),
        example: "カード番号 4111-1111-1111-1111",
    },
    DetectorSpec {
        id: "card_expiry", category: "finance", label: "CARD_EXPIRY", label_ja: "有効期限",
        name_en: "Card expiry date", name_ja: "カード有効期限",
        default_enabled: true, priority: 70,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?:0[1-9]|1[0-2])[ ]?/[ ]?(?:20[0-9]{2}|[0-9]{2})", 0.85)],
            boundary: Boundary::Digit,
            context: ctx(&["有効期限", "expiry", "exp", "valid thru", "expiration"], true),
            ..R
        }),
        example: "有効期限 12/28",
    },
    DetectorSpec {
        id: "card_cvv", category: "finance", label: "CVV", label_ja: "セキュリティコード",
        name_en: "Card security code (CVV)", name_ja: "セキュリティコード (CVV)",
        default_enabled: true, priority: 70,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?i)(?:cvv2?|cvc2?|セキュリティコード|security code)[ 　]*[:：]?[ 　]*(?P<v>[0-9]{3,4})", 0.9)],
            boundary: Boundary::Digit,
            ..R
        }),
        example: "CVV: 123",
    },
    DetectorSpec {
        id: "money", category: "finance", label: "AMOUNT", label_ja: "金額",
        name_en: "Monetary amount", name_ja: "金額",
        default_enabled: false, priority: 35,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[
                (r"[¥￥＄$€£][ 　]?[{DC}][{DC},，]*(?:\.[0-9]+)?(?:[ 　]?(?:万|億|千)?円)?", 0.85),
                (r"[{DC}][{DC},，]*(?:\.[0-9]+)?[ 　]?(?:万|億|千)?(?:円|ドル|USD|JPY|EUR)", 0.85),
            ],
            boundary: Boundary::Digit,
            ..R
        }),
        example: "契約金額 1,200万円",
    },
    // ───────────── ネットワーク/IT ─────────────
    DetectorSpec {
        id: "ipv4", category: "network", label: "IPV4", label_ja: "IPアドレス",
        name_en: "IPv4 address", name_ja: "IPv4 アドレス",
        default_enabled: true, priority: 50,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?:(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9]?[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9]?[0-9])(?:/(?:3[0-2]|[12]?[0-9]))?", 0.9)],
            boundary: Boundary::Ip,
            validator: Some(v::ipv4),
            ..R
        }),
        example: "接続元 192.168.10.25 から",
    },
    DetectorSpec {
        id: "ipv6", category: "network", label: "IPV6", label_ja: "IPアドレス",
        name_en: "IPv6 address", name_ja: "IPv6 アドレス",
        default_enabled: true, priority: 50,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?:[0-9A-Fa-f]{1,4}:){7}[0-9A-Fa-f]{1,4}|(?:[0-9A-Fa-f]{1,4}:){1,4}:(?:[0-9]{1,3}\.){3}[0-9]{1,3}|::(?:[Ff]{4}(?::0{1,4})?:)?(?:[0-9]{1,3}\.){3}[0-9]{1,3}|[0-9A-Fa-f]{1,4}:(?::[0-9A-Fa-f]{1,4}){1,6}|(?:[0-9A-Fa-f]{1,4}:){1,2}(?::[0-9A-Fa-f]{1,4}){1,5}|(?:[0-9A-Fa-f]{1,4}:){1,3}(?::[0-9A-Fa-f]{1,4}){1,4}|(?:[0-9A-Fa-f]{1,4}:){1,4}(?::[0-9A-Fa-f]{1,4}){1,3}|(?:[0-9A-Fa-f]{1,4}:){1,5}(?::[0-9A-Fa-f]{1,4}){1,2}|(?:[0-9A-Fa-f]{1,4}:){1,6}:[0-9A-Fa-f]{1,4}|(?:[0-9A-Fa-f]{1,4}:){1,7}:|:(?::[0-9A-Fa-f]{1,4}){1,7}", 0.9)],
            boundary: Boundary::Hex,
            validator: Some(v::ipv6),
            ..R
        }),
        example: "addr 2001:db8:85a3::8a2e:370:7334",
    },
    DetectorSpec {
        id: "mac_address", category: "network", label: "MAC", label_ja: "MACアドレス",
        name_en: "MAC address", name_ja: "MAC アドレス",
        default_enabled: true, priority: 50,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[0-9A-Fa-f]{2}(?:[:\-][0-9A-Fa-f]{2}){5}|[0-9A-Fa-f]{4}\.[0-9A-Fa-f]{4}\.[0-9A-Fa-f]{4}", 0.9)],
            boundary: Boundary::Hex,
            ..R
        }),
        example: "MAC 00:1A:2B:3C:4D:5E",
    },
    DetectorSpec {
        id: "url", category: "network", label: "URL", label_ja: "URL",
        name_en: "URL (entire)", name_ja: "URL (全体)",
        default_enabled: false, priority: 45,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?i)(?:https?|ftp|sftp|ssh|smb|file|wss?)://[^\s<>\x22'「」『』（）()、。\[\]{}|\\^`]+", 0.9)],
            trim_punct: true,
            ..R
        }),
        example: "https://intranet.example.co.jp/wiki/page",
    },
    DetectorSpec {
        id: "url_credentials", category: "network", label: "URL_CREDENTIALS", label_ja: "URL認証情報",
        name_en: "Credentials in URL (user:pass@)", name_ja: "URL 内の認証情報 (user:pass@)",
        default_enabled: true, priority: 88,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[A-Za-z][A-Za-z0-9+.\-]*://(?P<v>[^:/\s@]+:[^@/\s]+)@", 0.95)],
            ..R
        }),
        example: "postgres://admin:S3cretPass@db01:5432/app",
    },
    DetectorSpec {
        id: "url_secret_params", category: "network", label: "URL_SECRET", label_ja: "URLの秘密パラメータ",
        name_en: "Secret query parameters in URL", name_ja: "URL の秘密パラメータ値 (token= 等)",
        default_enabled: true, priority: 88,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?i)[?&](?:access_token|refresh_token|id_token|token|api_?key|apikey|key|secret|client_secret|password|passwd|pwd|sig|signature|x-amz-signature|x-amz-credential|x-amz-security-token|code|auth|session_?id|sid|jsessionid)=(?P<v>[^&\s#\x22'<>]+)", 0.9)],
            ..R
        }),
        example: "https://example.com/cb?code=abc123XYZ&state=1",
    },
    DetectorSpec {
        id: "hostname", category: "network", label: "HOST", label_ja: "ホスト名",
        name_en: "Host name / FQDN", name_ja: "ホスト名 (FQDN)",
        default_enabled: true, priority: 45,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?i)(?:[a-z0-9](?:[a-z0-9\-]{0,61}[a-z0-9])?\.)+(?:com|net|org|jp|io|dev|app|cloud|ai|biz|info|co|us|uk|de|cn|kr|tw|local|localdomain|internal|intranet|corp|lan|home|test|aws|gcp)", 0.8)],
            boundary: Boundary::Ip,
            validator: Some(hostname_ok),
            ..R
        }),
        example: "db01.prod.internal に接続",
    },
    DetectorSpec {
        id: "windows_user_path", category: "network", label: "USER", label_ja: "ユーザー名",
        name_en: "User name in Windows path", name_ja: "Windows パス内のユーザー名",
        default_enabled: true, priority: 60,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?i)[A-Z]:[\\/]+(?:Users|Documents and Settings)[\\/]+(?P<v>[^\\/:*?\x22<>|\r\n]+?)(?:[\\/]|\s|$)", 0.9)],
            validator: Some(not_common_user),
            ..R
        }),
        example: r"C:\Users\yamada.taro\Documents\report.xlsx",
    },
    DetectorSpec {
        id: "unix_home_path", category: "network", label: "USER", label_ja: "ユーザー名",
        name_en: "User name in Unix home path", name_ja: "Unix ホームパス内のユーザー名",
        default_enabled: true, priority: 60,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"/(?:home|Users)/(?P<v>[A-Za-z0-9._\-]+)", 0.85)],
            boundary: Boundary::Alnum,
            validator: Some(not_common_user),
            ..R
        }),
        example: "/home/tyamada/.ssh/config",
    },
    DetectorSpec {
        id: "unc_path", category: "network", label: "HOST", label_ja: "ホスト名",
        name_en: "Server name in UNC path", name_ja: "UNC パスのサーバー名",
        default_enabled: true, priority: 60,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"\\\\(?P<v>[A-Za-z0-9][A-Za-z0-9._\-$]*)\\", 0.85)],
            ..R
        }),
        example: r"\\fileserver01\share\経理",
    },
    DetectorSpec {
        id: "domain_user", category: "network", label: "USER", label_ja: "ユーザー名",
        name_en: r"Domain\User account", name_ja: r"ドメイン\ユーザー",
        default_enabled: true, priority: 55,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[A-Z][A-Z0-9\-]{1,14}\\[A-Za-z][A-Za-z0-9._\-]{0,19}", 0.65)],
            boundary: Boundary::Path,
            validator: Some(domain_user),
            ..R
        }),
        example: r"ログオン: CORP\tyamada",
    },
    DetectorSpec {
        id: "windows_sid", category: "network", label: "SID", label_ja: "SID",
        name_en: "Windows SID", name_ja: "Windows SID",
        default_enabled: true, priority: 60,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"S-1-5-21(?:-[0-9]{1,10}){3,4}", 0.95)],
            boundary: Boundary::Digit,
            ..R
        }),
        example: "S-1-5-21-3623811015-3361044348-30300820-1013",
    },
    DetectorSpec {
        id: "uuid", category: "network", label: "UUID", label_ja: "UUID",
        name_en: "UUID / GUID", name_ja: "UUID / GUID",
        default_enabled: false, priority: 40,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}", 0.9)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "id=3f2504e0-4f89-11d3-9a0c-0305e82c3301",
    },
    // ───────────── 認証情報・シークレット ─────────────
    DetectorSpec {
        id: "private_key", category: "secret", label: "PRIVATE_KEY", label_ja: "秘密鍵",
        name_en: "Private key block (PEM/SSH/PGP)", name_ja: "秘密鍵ブロック (PEM/SSH/PGP)",
        default_enabled: true, priority: 100,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"-----BEGIN[ A-Z0-9]*PRIVATE KEY(?: BLOCK)?-----[\s\S]*?-----END[ A-Z0-9]*PRIVATE KEY(?: BLOCK)?-----", 0.99)],
            ..R
        }),
        example: "-----BEGIN RSA PRIVATE KEY-----\nMIIEdummy\n-----END RSA PRIVATE KEY-----",
    },
    DetectorSpec {
        id: "aws_access_key", category: "secret", label: "AWS_KEY", label_ja: "AWSアクセスキー",
        name_en: "AWS access key ID", name_ja: "AWS アクセスキー ID",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?:AKIA|ASIA|ABIA|ACCA|AGPA|AIDA|AIPA|ANPA|ANVA|AROA|APKA|ASCA)[A-Z0-9]{16}", 0.95)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "AKIAIOSFODNN7EXAMPLE",
    },
    DetectorSpec {
        id: "aws_secret_key", category: "secret", label: "AWS_SECRET", label_ja: "AWSシークレットキー",
        name_en: "AWS secret access key", name_ja: "AWS シークレットアクセスキー",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?i)(?:aws_?secret_?(?:access_?)?key|secret_?access_?key)[\x22']?\s*[:=]\s*[\x22']?(?P<v>[A-Za-z0-9/+=]{40})", 0.95)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
    },
    DetectorSpec {
        id: "github_token", category: "secret", label: "GITHUB_TOKEN", label_ja: "GitHubトークン",
        name_en: "GitHub token", name_ja: "GitHub トークン",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36,255}|github_pat_[A-Za-z0-9_]{22,255}", 0.98)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "ghp_0123456789abcdefghijABCDEFGHIJ012345",
    },
    DetectorSpec {
        id: "gitlab_token", category: "secret", label: "GITLAB_TOKEN", label_ja: "GitLabトークン",
        name_en: "GitLab token", name_ja: "GitLab トークン",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"glpat-[A-Za-z0-9_\-]{20,}", 0.98)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "glpat-abcdefghij0123456789",
    },
    DetectorSpec {
        id: "slack_token", category: "secret", label: "SLACK_TOKEN", label_ja: "Slackトークン",
        name_en: "Slack token", name_ja: "Slack トークン",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"xox[baprse]-[A-Za-z0-9\-]{10,}", 0.98)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "xoxb-1234567890-abcdefghijkl",
    },
    DetectorSpec {
        id: "slack_webhook", category: "secret", label: "SLACK_WEBHOOK", label_ja: "SlackのWebhook",
        name_en: "Slack webhook URL", name_ja: "Slack Webhook URL",
        default_enabled: true, priority: 96,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"https://hooks\.slack\.com/(?:services|workflows|triggers)/[A-Za-z0-9/_\-]+", 0.98)],
            ..R
        }),
        example: "https://hooks.slack.com/services/T000/B000/XXXXXXXX",
    },
    DetectorSpec {
        id: "google_api_key", category: "secret", label: "GOOGLE_API_KEY", label_ja: "GoogleのAPIキー",
        name_en: "Google API key", name_ja: "Google API キー",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"AIza[0-9A-Za-z_\-]{35}", 0.98)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "AIzaSyA-1234567890abcdefghijklmnopqrstu",
    },
    DetectorSpec {
        id: "google_oauth_secret", category: "secret", label: "GOOGLE_SECRET", label_ja: "GoogleのOAuthシークレット",
        name_en: "Google OAuth client secret", name_ja: "Google OAuth クライアントシークレット",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"GOCSPX-[A-Za-z0-9_\-]{28}", 0.98)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "GOCSPX-abcdefghijklmnopqrstuvwxyz12",
    },
    DetectorSpec {
        id: "azure_key", category: "secret", label: "AZURE_KEY", label_ja: "Azureキー",
        name_en: "Azure storage / service bus key", name_ja: "Azure ストレージ等のキー",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?i)(?:AccountKey|SharedAccessKey|PrimaryKey|SecondaryKey)\s*=\s*(?P<v>[A-Za-z0-9+/]{20,}={0,2})", 0.95)],
            ..R
        }),
        example: "AccountName=demo;AccountKey=abcdEFGHijklMNOPqrstUVWX0123456789==",
    },
    DetectorSpec {
        id: "stripe_key", category: "secret", label: "STRIPE_KEY", label_ja: "Stripeキー",
        name_en: "Stripe API key", name_ja: "Stripe API キー",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?:sk|rk|pk)_(?:live|test)_[A-Za-z0-9]{16,}", 0.98)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "sk_test_abcdefghijklmnop1234",
    },
    DetectorSpec {
        id: "anthropic_key", category: "secret", label: "API_KEY", label_ja: "APIキー",
        name_en: "Anthropic API key", name_ja: "Anthropic API キー",
        default_enabled: true, priority: 97,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"sk-ant-[A-Za-z0-9_\-]{20,}", 0.99)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "sk-ant-api03-abcdefghijklmnopqrstuvwxyz",
    },
    DetectorSpec {
        id: "openai_key", category: "secret", label: "API_KEY", label_ja: "APIキー",
        name_en: "OpenAI-style API key (sk-...)", name_ja: "OpenAI 形式の API キー (sk-...)",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"sk-(?:proj-|svcacct-|admin-)?[A-Za-z0-9_\-]{20,}", 0.95)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "sk-proj-abcdefghijklmnopqrstuvwxyz0123",
    },
    DetectorSpec {
        id: "huggingface_token", category: "secret", label: "HF_TOKEN", label_ja: "Hugging Faceトークン",
        name_en: "Hugging Face token", name_ja: "Hugging Face トークン",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"hf_[A-Za-z0-9]{30,}", 0.97)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "hf_abcdefghijklmnopqrstuvwxyzABCDEF",
    },
    DetectorSpec {
        id: "npm_token", category: "secret", label: "NPM_TOKEN", label_ja: "npmトークン",
        name_en: "npm token", name_ja: "npm トークン",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"npm_[A-Za-z0-9]{36}", 0.98)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "npm_abcdefghijklmnopqrstuvwxyz0123456789",
    },
    DetectorSpec {
        id: "sendgrid_key", category: "secret", label: "SENDGRID_KEY", label_ja: "SendGridキー",
        name_en: "SendGrid API key", name_ja: "SendGrid API キー",
        default_enabled: true, priority: 95,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"SG\.[A-Za-z0-9_\-]{22}\.[A-Za-z0-9_\-]{43}", 0.99)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "SG.abcdefghijklmnopqrstuv.abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG",
    },
    DetectorSpec {
        id: "jwt", category: "secret", label: "JWT", label_ja: "JWT",
        name_en: "JSON Web Token", name_ja: "JWT (JSON Web Token)",
        default_enabled: true, priority: 93,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"eyJ[A-Za-z0-9_\-]{5,}\.[A-Za-z0-9_\-]{5,}\.[A-Za-z0-9_\-]{5,}", 0.95)],
            boundary: Boundary::Alnum,
            validator: Some(v::jwt),
            ..R
        }),
        example: "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dummysignature123",
    },
    DetectorSpec {
        id: "bearer_token", category: "secret", label: "TOKEN", label_ja: "トークン",
        name_en: "Bearer token", name_ja: "Bearer トークン",
        default_enabled: true, priority: 92,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?i)bearer\s+(?P<v>[A-Za-z0-9\-._~+/]{16,}=*)", 0.9)],
            ..R
        }),
        example: "Authorization: Bearer abcdefghijklmnop0123456789",
    },
    DetectorSpec {
        id: "basic_auth", category: "secret", label: "BASIC_AUTH", label_ja: "Basic認証",
        name_en: "HTTP Basic auth header", name_ja: "HTTP Basic 認証ヘッダー",
        default_enabled: true, priority: 92,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?i)authorization\s*:\s*basic\s+(?P<v>[A-Za-z0-9+/]{8,}={0,2})", 0.95)],
            ..R
        }),
        example: "Authorization: Basic dXNlcjpwYXNzd29yZA==",
    },
    DetectorSpec {
        id: "password_kv", category: "secret", label: "PASSWORD", label_ja: "パスワード",
        name_en: "Password / secret in key=value", name_ja: "key=value 形式のパスワード・秘密値",
        default_enabled: true, priority: 90,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[
                (r"(?im)(?:^|[^A-Za-z0-9])(?:password|passwd|pass_?word|pwd|passphrase|secret(?:_?key)?|api_?key|apikey|access_?token|auth_?token|client_?secret|private_?key)[\x22']?\s*[:=]\s*[\x22']?(?P<v>[^\s\x22',;]{1,200})", 0.85),
                // 「token」だけのキーは技術文書やソースコードに多い (「token: &str」「Token(0)」「token: 東京」) ので、
                // トークンらしい値 (8 文字以上の英数字・記号で、区切りで終わる) のときだけ
                (r"(?im)(?:^|[^A-Za-z0-9])token[\x22']?\s*[:=]\s*[\x22']?(?P<v>[A-Za-z0-9_\-.+/=]{8,200})(?:$|[\s\x22',;&)\]}])", 0.8),
            ],
            validator: Some(not_placeholder),
            ..R
        }),
        example: "db_password=Tr0ub4dor&3",
    },
    DetectorSpec {
        id: "password_ja", category: "secret", label: "PASSWORD", label_ja: "パスワード",
        name_en: "Password (Japanese label)", name_ja: "パスワード (日本語の項目名)",
        default_enabled: true, priority: 90,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?:パスワード|暗証番号|パスフレーズ|ＰＷ|ﾊﾟｽﾜｰﾄﾞ)[ 　]*(?:は|[:：=＝])[ 　]*[「\x22']?(?P<v>[^\s「」\x22'、。，,]{1,100})", 0.9)],
            validator: Some(not_placeholder),
            ..R
        }),
        example: "初期パスワード: Abc12345",
    },
    DetectorSpec {
        id: "high_entropy", category: "secret", label: "SECRET", label_ja: "秘密値",
        name_en: "High-entropy string (possible secret)", name_ja: "高エントロピー文字列 (秘密値の可能性)",
        default_enabled: false, priority: 20,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"[A-Za-z0-9+/_\-]{20,}={0,2}", 0.6)],
            boundary: Boundary::Alnum,
            validator: Some(v::high_entropy),
            ..R
        }),
        example: "key: Zx8Qp2Lm9Vt4Rw7Ky1Hs6Nb3",
    },
    // ───────────── 位置情報 ─────────────
    DetectorSpec {
        id: "lat_lon", category: "location", label: "GEO", label_ja: "位置座標",
        name_en: "Latitude / longitude", name_ja: "緯度経度",
        default_enabled: true, priority: 60,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"-?(?:[1-8]?[0-9]\.[0-9]{4,}|90\.0{4,})\s*[,，]\s*-?(?:1[0-7][0-9]|[1-9]?[0-9])\.[0-9]{4,}", 0.85)],
            boundary: Boundary::Digit,
            validator: Some(v::lat_lon),
            ..R
        }),
        example: "位置: 35.681236, 139.767125",
    },
    DetectorSpec {
        id: "jp_plate", category: "location", label: "PLATE", label_ja: "車両番号",
        name_en: "Vehicle license plate (Japan)", name_ja: "自動車のナンバー",
        default_enabled: true, priority: 60,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"\p{Han}{1,4}[ 　]?[{DC}]{2,3}[ 　]?\p{Hiragana}[ 　]?(?:[{DC}]{1,2}[{HC}][{DC}]{2}|[・･.]{1,3}[{DC}]{1,3})", 0.8)],
            boundary: Boundary::Digit,
            ..R
        }),
        example: "品川 300 あ 12-34",
    },
    DetectorSpec {
        id: "place_name", category: "location", label: "PLACE", label_ja: "地名",
        name_en: "Place name (dictionary, e.g. 〜市/〜駅/〜在住)", name_ja: "地名 (辞書: 〜市・〜駅・〜在住 など)",
        default_enabled: false, priority: 38,
        kind: SpecKind::PlaceName,
        example: "新宿駅の近く",
    },
    // ───────────── 組織 ─────────────
    DetectorSpec {
        id: "company_jp", category: "organization", label: "COMPANY", label_ja: "会社名",
        name_en: "Company name (Japanese)", name_ja: "会社名・法人名",
        default_enabled: true, priority: 42,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[
                (r"(?:株式会社|有限会社|合同会社|合資会社|合名会社|一般社団法人|一般財団法人|公益社団法人|公益財団法人|特定非営利活動法人|NPO法人|医療法人|学校法人|社会福祉法人|独立行政法人|国立大学法人)[ 　]?[\p{Han}\p{Katakana}A-Za-zＡ-Ｚａ-ｚ0-9０-９ー・&＆]{1,20}", 0.85),
                (r"[\p{Han}\p{Katakana}A-Za-zＡ-Ｚａ-ｚ0-9０-９ー・&＆]{1,20}[ 　]?(?:株式会社|有限会社|合同会社)", 0.8),
                (r"(?:\(株\)|（株）|㈱|\(有\)|（有）|㈲)[\p{Han}\p{Katakana}A-Za-zＡ-Ｚａ-ｚー・]{1,20}", 0.8),
            ],
            ..R
        }),
        example: "株式会社サンプル商事 御中",
    },
    DetectorSpec {
        id: "company_en", category: "organization", label: "COMPANY", label_ja: "会社名",
        name_en: "Company name (English)", name_ja: "会社名 (英語表記)",
        default_enabled: true, priority: 42,
        kind: SpecKind::Regex(RegexSpec {
            patterns: &[(r"(?:[A-Z][A-Za-z0-9&\-]*\s){1,3}(?:Inc|Corp|Corporation|LLC|Ltd|Limited|Co\.,?\s?Ltd|GmbH|K\.K|PLC|LLP)\.?", 0.7)],
            boundary: Boundary::Alnum,
            ..R
        }),
        example: "contract with Example Widgets Inc.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_unique_and_categories_known() {
        let mut ids = std::collections::HashSet::new();
        for d in CATALOG {
            assert!(ids.insert(d.id), "duplicate id {}", d.id);
            assert!(category(d.category).is_some(), "unknown category {}", d.category);
        }
    }
}
