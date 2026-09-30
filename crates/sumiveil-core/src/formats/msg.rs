//! Outlook のメール (.msg, 複合ファイル形式 + MAPI プロパティ)。マスクしたテキストとして書き出す。

use std::io::{Cursor, Read};

use cfb::CompoundFile;

use super::{DocKind, Document, Inner, TextBuilder};

type Cf<'a> = CompoundFile<Cursor<&'a [u8]>>;

fn read_bytes(cf: &mut Cf<'_>, path: &str) -> Option<Vec<u8>> {
    let mut s = cf.open_stream(path).ok()?;
    let mut v = vec![];
    s.read_to_end(&mut v).ok()?;
    Some(v)
}

/// 文字列プロパティ (Unicode 001F、なければ 8 ビット 001E)。
fn read_str(cf: &mut Cf<'_>, dir: &str, tag: &str) -> Option<String> {
    if let Some(b) = read_bytes(cf, &format!("{dir}/__substg1.0_{tag}001F")) {
        let u: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let s = String::from_utf16_lossy(&u);
        return Some(s.trim_end_matches('\0').to_string()).filter(|s| !s.is_empty());
    }
    let b = read_bytes(cf, &format!("{dir}/__substg1.0_{tag}001E"))?;
    let s = match std::str::from_utf8(&b) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::SHIFT_JIS.decode(&b).0.into_owned(),
    };
    Some(s.trim_end_matches('\0').to_string()).filter(|s| !s.is_empty())
}

/// 送信日時 (PidTagClientSubmitTime 0x0039、なければ配信日時 0x0E06)。UTC の「YYYY-MM-DD hh:mm」。
fn sent_time(cf: &mut Cf<'_>) -> Option<String> {
    let b = read_bytes(cf, "/__properties_version1.0")?;
    // 先頭 32 バイトのヘッダーの後に、16 バイトずつ (型 2 + ID 2 + フラグ 4 + 値 8)
    let find = |id: u16| {
        b.get(32..)?.chunks_exact(16).find(|e| u16::from_le_bytes([e[0], e[1]]) == 0x0040 && u16::from_le_bytes([e[2], e[3]]) == id).map(|e| u64::from_le_bytes(e[8..16].try_into().unwrap()))
    };
    let ft = find(0x0039).or_else(|| find(0x0E06))?;
    let secs = (ft / 10_000_000) as i64 - 11_644_473_600; // 1601-01-01 → 1970-01-01
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // 日数 → 年月日 (Howard Hinnant のアルゴリズム)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Some(format!("{y:04}-{m:02}-{d:02} {:02}:{:02} (UTC)", rem / 3600, rem % 3600 / 60))
}

pub(crate) fn open(bytes: &[u8]) -> Result<Document, String> {
    let mut cf = CompoundFile::open(Cursor::new(bytes)).map_err(|e| format!("Outlook のメール (.msg) として読み込めません: {e}"))?;
    let root: Vec<String> = cf.read_root_storage().map(|e| e.name().to_string()).collect();
    let mut b = TextBuilder::default();
    let mut warnings = vec![];
    let line = |b: &mut TextBuilder, label: &str, value: Option<String>| {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            b.location(label.to_string());
            b.push(&format!("{label}: {v}\n"));
        }
    };
    let subject = read_str(&mut cf, "", "0037");
    line(&mut b, "件名", subject);
    let sender = match (read_str(&mut cf, "", "0C1A"), read_str(&mut cf, "", "5D01").or_else(|| read_str(&mut cf, "", "0065"))) {
        (Some(n), Some(a)) => Some(format!("{n} <{a}>")),
        (n, a) => n.or(a),
    };
    line(&mut b, "差出人", sender);
    let to = read_str(&mut cf, "", "0E04");
    line(&mut b, "宛先", to);
    let cc = read_str(&mut cf, "", "0E03");
    line(&mut b, "Cc", cc);
    let date = sent_time(&mut cf);
    line(&mut b, "送信日時", date);
    // 受信者のメールアドレス
    let mut addrs = vec![];
    for dir in root.iter().filter(|n| n.starts_with("__recip_version1.0_")) {
        let d = format!("/{dir}");
        let name = read_str(&mut cf, &d, "3001");
        let addr = read_str(&mut cf, &d, "39FE").or_else(|| read_str(&mut cf, &d, "3003")).filter(|a| a.contains('@'));
        match (name, addr) {
            (Some(n), Some(a)) => addrs.push(format!("{n} <{a}>")),
            (n, a) => addrs.extend(n.or(a)),
        }
    }
    if !addrs.is_empty() {
        line(&mut b, "受信者", Some(addrs.join(", ")));
    }
    // 添付ファイル (名前だけ)
    let mut names = vec![];
    for dir in root.iter().filter(|n| n.starts_with("__attach_version1.0_")) {
        let d = format!("/{dir}");
        names.extend(read_str(&mut cf, &d, "3707").or_else(|| read_str(&mut cf, &d, "3704")).or_else(|| read_str(&mut cf, &d, "3001")));
    }
    if !names.is_empty() {
        line(&mut b, "添付ファイル (内容は含みません)", Some(names.join(", ")));
        warnings.push(format!("添付ファイル {} 件の中身は書き出しません", names.len()));
    }
    b.push("\n");
    b.location("本文");
    match read_str(&mut cf, "", "1000") {
        Some(body) => b.push(&body),
        None => match read_bytes(&mut cf, "/__substg1.0_10130102") {
            Some(html) => {
                b.push(&mail_parser::decoders::html::html_to_text(&String::from_utf8_lossy(&html)));
                warnings.push("HTML 形式の本文を、テキスト形式に変換しました".to_string());
            }
            None => warnings.push("本文を読み取れませんでした (リッチテキスト形式のみのメール)".to_string()),
        },
    }
    b.newline();
    Ok(Document {
        kind: DocKind::Msg,
        text: b.text,
        encoding_name: DocKind::Msg.label(true).to_string(),
        warnings,
        properties: vec![],
        locations: b.locations,
        inner: Inner::Plain,
    })
}
