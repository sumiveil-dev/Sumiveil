//! `{fake}` 用のダミー値生成。実在しない (文書用に予約された) 値を返す。

use crate::template::{hmac_hex, RenderCtx};

const JA_NAMES: &[&str] = &[
    "甲野一郎", "乙川花子", "丙田次郎", "丁村桜", "戊山三郎", "己沢陽子", "庚野四郎", "辛島直美",
];
const EN_NAMES: &[&str] = &[
    "John Doe", "Jane Roe", "Richard Roe", "Alex Example", "Sam Sample", "Pat Placeholder",
];

fn pick<'a>(list: &[&'a str], n: usize) -> String {
    let base = list[(n.max(1) - 1) % list.len()];
    let round = (n.max(1) - 1) / list.len();
    if round == 0 {
        base.to_string()
    } else {
        format!("{base}{}", round + 1)
    }
}

fn is_ascii_text(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii())
}

pub fn fake(ctx: &RenderCtx) -> String {
    let n = ctx.n.max(1);
    let o = ctx.original;
    match ctx.id {
        "email" => format!("user{n}@example.com"),
        "phone_jp" => format!("090-0000-{:04}", n % 10000),
        "phone_intl" => format!("+1-555-01{:02}", n % 100),
        "postal_code_jp" => {
            if o.starts_with('〒') {
                format!("〒000-{:04}", n % 10000)
            } else {
                format!("000-{:04}", n % 10000)
            }
        }
        "ipv4" => format!("192.0.2.{}", (n - 1) % 254 + 1),
        "ipv6" => format!("2001:db8::{n:x}"),
        "mac_address" => format!("00:00:5E:00:53:{:02X}", n % 256),
        "hostname" | "unc_path" => format!("host{n}.example.com"),
        "url" => format!("https://example.com/path{n}"),
        "person_name" => {
            if is_ascii_text(o) {
                pick(EN_NAMES, n)
            } else {
                pick(JA_NAMES, n)
            }
        }
        "company_jp" => format!("株式会社サンプル{n}"),
        "company_en" => format!("Example Corp {n}"),
        "address_jp" | "address_jp_city" => format!("東京都架空区見本町{n}-{n}"),
        "address_en" => format!("{n} Example Street"),
        "windows_user_path" | "unix_home_path" | "domain_user" => format!("user{n}"),
        "credit_card" => {
            let pat = "4111111111111111";
            let mut digits = pat.chars();
            o.chars()
                .map(|c| if c.is_ascii_digit() || ('０'..='９').contains(&c) { digits.next().unwrap_or('1') } else { c })
                .collect()
        }
        _ => format_preserving(ctx),
    }
}

/// 形式保持のダミー値: 数字→数字、英字→英字 (大文字小文字維持)、その他はそのまま。HMAC で決定的に生成。
fn format_preserving(ctx: &RenderCtx) -> String {
    let mut seed = hmac_hex(ctx.hash_key, ctx.label, ctx.original).into_bytes();
    let mut idx = 0usize;
    let mut next = |m: u8| -> u8 {
        if idx >= seed.len() {
            seed = hmac_hex(ctx.hash_key, ctx.label, &String::from_utf8_lossy(&seed)).into_bytes();
            idx = 0;
        }
        let v = seed[idx];
        idx += 1;
        v % m
    };
    ctx.original
        .chars()
        .map(|c| match c {
            '0'..='9' => (b'0' + next(10)) as char,
            '０'..='９' => char::from_u32('０' as u32 + next(10) as u32).unwrap(),
            'a'..='z' => (b'a' + next(26)) as char,
            'A'..='Z' => (b'A' + next(26)) as char,
            c if c.is_alphabetic() => '○',
            c => c,
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/fake.rs"]
mod tests;
