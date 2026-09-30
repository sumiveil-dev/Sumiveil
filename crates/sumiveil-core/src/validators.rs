//! 検出候補の妥当性検証 (チェックディジット等)。誤検出を減らすために使う。

use crate::text::{digits_only, shannon_entropy};

/// 検証結果。`Strong` は文脈キーワードが無くても確定してよいことを示す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Reject,
    Accept,
    Strong,
}

pub type Validator = fn(&str) -> Verdict;

fn b(ok: bool) -> Verdict {
    if ok {
        Verdict::Accept
    } else {
        Verdict::Reject
    }
}

/// Luhn アルゴリズム。
pub fn luhn(digits: &str) -> bool {
    let mut sum = 0u32;
    let mut double = false;
    for c in digits.chars().rev() {
        let Some(mut d) = c.to_digit(10) else {
            return false;
        };
        if double {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
        double = !double;
    }
    !digits.is_empty() && sum % 10 == 0
}

pub fn credit_card(s: &str) -> Verdict {
    let d = digits_only(s);
    if !(13..=19).contains(&d.len()) || !luhn(&d) {
        return Verdict::Reject;
    }
    // 主要ブランドの IIN 範囲
    let p2: u32 = d[..2].parse().unwrap_or(0);
    let p4: u32 = d[..4].parse().unwrap_or(0);
    let known = d.starts_with('4')
        || (51..=55).contains(&p2)
        || (2221..=2720).contains(&p4)
        || p2 == 34
        || p2 == 37
        || (3528..=3589).contains(&p4)
        || p2 == 36
        || p2 == 38
        || p2 == 39
        || (3000..=3059).contains(&p4)
        || p4 == 6011
        || p2 == 65
        || p2 == 62;
    b(known)
}

/// マイナンバー (個人番号) 12 桁のチェックディジット。
pub fn my_number(s: &str) -> Verdict {
    let d: Vec<u32> = digits_only(s).chars().filter_map(|c| c.to_digit(10)).collect();
    if d.len() != 12 {
        return Verdict::Reject;
    }
    // 同一数字の繰り返しは除外
    if d.iter().all(|&x| x == d[0]) {
        return Verdict::Reject;
    }
    let mut sum = 0u32;
    for n in 1..=11usize {
        let p = d[11 - n];
        let q = if n <= 6 { n as u32 + 1 } else { n as u32 - 5 };
        sum += p * q;
    }
    let r = sum % 11;
    let cd = if r <= 1 { 0 } else { 11 - r };
    b(cd == d[11])
}

/// 法人番号 13 桁 (先頭がチェックディジット)。先頭に T (適格請求書発行事業者登録番号) があれば Strong。
pub fn corporate_number(s: &str) -> Verdict {
    let d: Vec<u32> = digits_only(s).chars().filter_map(|c| c.to_digit(10)).collect();
    if d.len() != 13 {
        return Verdict::Reject;
    }
    let body = &d[1..];
    let mut sum = 0u32;
    for (i, &x) in body.iter().rev().enumerate() {
        let w = if i % 2 == 0 { 1 } else { 2 };
        sum += x * w;
    }
    let cd = 9 - (sum % 9);
    if cd != d[0] {
        return Verdict::Reject;
    }
    if s.trim_start().starts_with(['T', 'Ｔ']) {
        Verdict::Strong
    } else {
        Verdict::Accept
    }
}

/// IBAN の mod 97 検証。
pub fn iban(s: &str) -> Verdict {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() < 15 || compact.len() > 34 {
        return Verdict::Reject;
    }
    let rearranged = format!("{}{}", &compact[4..], &compact[..4]);
    let mut rem: u32 = 0;
    for c in rearranged.chars() {
        let v = match c {
            '0'..='9' => c as u32 - '0' as u32,
            'A'..='Z' => c as u32 - 'A' as u32 + 10,
            _ => return Verdict::Reject,
        };
        if v >= 10 {
            rem = (rem * 100 + v) % 97;
        } else {
            rem = (rem * 10 + v) % 97;
        }
    }
    if rem == 1 {
        Verdict::Strong
    } else {
        Verdict::Reject
    }
}

/// 米国 SSN の形式的な妥当性。
pub fn us_ssn(s: &str) -> Verdict {
    let d = digits_only(s);
    if d.len() != 9 {
        return Verdict::Reject;
    }
    let area = &d[0..3];
    let group = &d[3..5];
    let serial = &d[5..9];
    b(area != "000" && area != "666" && !area.starts_with('9') && group != "00" && serial != "0000")
}

/// 電話番号 (日本)。桁数と先頭の番号帯で判定。
pub fn phone_jp(s: &str) -> Verdict {
    let t = s.trim();
    let mut d = digits_only(t);
    let intl = t.starts_with('+');
    if intl {
        // +81 を 0 に置換 (+81 0x... という書き方も許容)
        if !d.starts_with("81") {
            return Verdict::Reject;
        }
        d = d[2..].to_string();
        if !d.starts_with('0') {
            d.insert(0, '0');
        }
    }
    if !d.starts_with('0') {
        return Verdict::Reject;
    }
    let has_sep = t.chars().any(|c| !crate::text::is_digit_like(c) && c != '+');
    let ok = match d.len() {
        10 => !d.starts_with("00"),
        11 => {
            ["070", "080", "090", "050", "060", "020"].iter().any(|p| d.starts_with(p))
                || d.starts_with("0800")
        }
        _ => false,
    };
    if !ok {
        return Verdict::Reject;
    }
    if has_sep || intl {
        Verdict::Strong
    } else {
        Verdict::Accept
    }
}

/// 国際電話番号 (E.164 相当)。
pub fn phone_intl(s: &str) -> Verdict {
    let d = digits_only(s);
    b((8..=15).contains(&d.len()))
}

/// IPv4。ループバック・未指定・ブロードキャストは対象外。
pub fn ipv4(s: &str) -> Verdict {
    let addr = s.split('/').next().unwrap_or(s);
    let parts: Vec<&str> = addr.split('.').collect();
    if parts.len() != 4 {
        return Verdict::Reject;
    }
    if parts.iter().any(|p| p.len() > 1 && p.starts_with('0')) {
        // 先頭ゼロ (バージョン番号 1.02.3.4 等) は除外
        return Verdict::Reject;
    }
    if addr.starts_with("127.") || addr == "0.0.0.0" || addr == "255.255.255.255" {
        return Verdict::Reject;
    }
    Verdict::Accept
}

pub fn ipv6(s: &str) -> Verdict {
    let colons = s.matches(':').count();
    if s == "::" || s == "::1" || colons < 2 {
        return Verdict::Reject;
    }
    if !s.contains("::") && colons != 7 && !s.contains('.') {
        return Verdict::Reject;
    }
    // 16 進数字が少なすぎるもの (C++ の std::xx 等) を除外
    let hex = s.chars().filter(|c| c.is_ascii_hexdigit()).count();
    b(hex >= 4)
}

pub fn jwt(s: &str) -> Verdict {
    let parts: Vec<&str> = s.split('.').collect();
    b(parts.len() == 3 && parts[0].starts_with("eyJ"))
}

/// 高エントロピー文字列 (API キー等の汎用検出)。
pub fn high_entropy(s: &str) -> Verdict {
    if s.len() < 20 {
        return Verdict::Reject;
    }
    let has_lower = s.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = s.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = s.chars().any(|c| c.is_ascii_digit());
    let classes = has_lower as u8 + has_upper as u8 + has_digit as u8;
    let e = shannon_entropy(s);
    let is_hex = s.chars().all(|c| c.is_ascii_hexdigit());
    if is_hex {
        return b(s.len() >= 32 && e >= 3.0 && has_digit);
    }
    b(classes >= 3 && e >= 4.0)
}

/// 緯度経度の範囲チェック。
pub fn lat_lon(s: &str) -> Verdict {
    let mut it = s.split([',', '，']);
    let lat: f64 = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(999.0);
    let lon: f64 = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(999.0);
    b(lat.abs() <= 90.0 && lon.abs() <= 180.0)
}

/// 住所候補: 番地らしい数字を含むこと。
pub fn has_digit(s: &str) -> Verdict {
    b(s.chars().any(|c| crate::text::is_digit_like(c) || "一二三四五六七八九十".contains(c)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luhn_cards() {
        assert_eq!(credit_card("4111 1111 1111 1111"), Verdict::Accept);
        assert_eq!(credit_card("5555-5555-5555-4444"), Verdict::Accept);
        assert_eq!(credit_card("3530111333300000"), Verdict::Accept); // JCB テスト番号
        assert_eq!(credit_card("378282246310005"), Verdict::Accept); // Amex テスト番号
        assert_eq!(credit_card("4111 1111 1111 1112"), Verdict::Reject);
        assert_eq!(credit_card("1234567812345670"), Verdict::Reject); // IIN 不明
    }

    #[test]
    fn my_number_cd() {
        // チェックディジット計算で作成したダミー番号
        assert_eq!(my_number("1234 5678 9018"), Verdict::Accept);
        assert_eq!(my_number("123456789012"), Verdict::Reject);
        assert_eq!(my_number("111111111111"), Verdict::Reject);
    }

    #[test]
    fn corporate_cd() {
        // 国税庁の法人番号の例: 7000012050002 (国税庁自身)
        assert_eq!(corporate_number("7000012050002"), Verdict::Accept);
        assert_eq!(corporate_number("T7000012050002"), Verdict::Strong);
        assert_eq!(corporate_number("1000012050002"), Verdict::Reject);
    }

    #[test]
    fn iban_mod97() {
        assert_eq!(iban("GB82 WEST 1234 5698 7654 32"), Verdict::Strong);
        assert_eq!(iban("GB82 WEST 1234 5698 7654 33"), Verdict::Reject);
    }

    #[test]
    fn phones() {
        assert_eq!(phone_jp("090-1234-5678"), Verdict::Strong);
        assert_eq!(phone_jp("03-1234-5678"), Verdict::Strong);
        assert_eq!(phone_jp("+81 90 1234 5678"), Verdict::Strong);
        assert_eq!(phone_jp("0312345678"), Verdict::Accept);
        assert_eq!(phone_jp("031234567"), Verdict::Reject);
        assert_eq!(phone_jp("03-123-45678-9"), Verdict::Reject);
    }

    #[test]
    fn ips() {
        assert_eq!(ipv4("192.168.1.10"), Verdict::Accept);
        assert_eq!(ipv4("127.0.0.1"), Verdict::Reject);
        assert_eq!(ipv4("1.02.3.4"), Verdict::Reject);
        assert_eq!(ipv6("2001:db8::1"), Verdict::Accept);
        assert_eq!(ipv6("::1"), Verdict::Reject);
        assert_eq!(ipv6("std::io"), Verdict::Reject);
    }

    #[test]
    fn ssn() {
        assert_eq!(us_ssn("123-45-6789"), Verdict::Accept);
        assert_eq!(us_ssn("666-45-6789"), Verdict::Reject);
    }
}
