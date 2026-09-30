//! 表示言語 (日本語/英語) の判定。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ja,
    En,
}

impl Lang {
    /// 設定値 ("auto" | "ja" | "en") から決める。auto は OS の表示言語に従う。
    pub fn resolve(setting: &str) -> Lang {
        match setting.trim().to_ascii_lowercase().as_str() {
            "ja" | "japanese" | "日本語" => Lang::Ja,
            "en" | "english" => Lang::En,
            _ => {
                if sys_locale::get_locale().is_some_and(|l| l.to_ascii_lowercase().starts_with("ja")) {
                    Lang::Ja
                } else {
                    Lang::En
                }
            }
        }
    }

    /// 言語に応じて文字列を選ぶ。
    pub fn t<'a>(self, ja: &'a str, en: &'a str) -> &'a str {
        match self {
            Lang::Ja => ja,
            Lang::En => en,
        }
    }

    pub fn is_ja(self) -> bool {
        self == Lang::Ja
    }
}
