//! メール (.eml, RFC 5322 / MIME)。
//!
//! ヘッダーの符号化された語 (`=?ISO-2022-JP?B?…?=`) と本文 (Base64 / quoted-printable、各文字コード) を解読してマスクし、
//! UTF-8 で符号化し直した .eml を書き出す。
//! - 残すヘッダーは 件名・差出人・宛先・Cc・返信先・日付 のみ (経路情報の Received などは IP アドレスやホスト名を含むため出力しない)
//! - HTML の本文はテキストに変換して出力する
//! - 添付ファイルは中身を確認できないため取り除き、`X-Sumiveil-Removed-Attachment` に番号と拡張子だけを残す (ファイル名は出さない)

use mail_parser::{Address, MessageParser, MimeHeaders};

use super::{split_output, DocKind, Document, Inner, TextBuilder};
use crate::engine::MaskResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Subject,
    From,
    To,
    Cc,
    ReplyTo,
    Body,
    Attachment,
}

pub(crate) struct EmlDoc {
    /// (テキスト上の範囲, 種類)
    fields: Vec<((usize, usize), Field)>,
    date: Option<String>,
}

fn addresses(a: Option<&Address<'_>>) -> String {
    let Some(a) = a else { return String::new() };
    a.iter()
        .map(|x| match (x.name(), x.address()) {
            (Some(n), Some(ad)) => format!("{n} <{ad}>"),
            (Some(n), None) => n.to_string(),
            (None, Some(ad)) => ad.to_string(),
            (None, None) => String::new(),
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn open(bytes: &[u8]) -> Result<Document, String> {
    let msg = MessageParser::default().parse(bytes).ok_or("メール (.eml) として読み込めません")?;
    let mut b = TextBuilder::default();
    let mut fields = vec![];
    let mut warnings = vec![];
    let mut header = |b: &mut TextBuilder, label: &str, value: String, f: Field| {
        if value.is_empty() {
            return;
        }
        b.location(label.to_string());
        b.push(&format!("{label}: "));
        let s = b.text.len();
        b.push(&value);
        fields.push(((s, b.text.len()), f));
        b.push("\n");
    };
    header(&mut b, "件名", msg.subject().unwrap_or_default().to_string(), Field::Subject);
    header(&mut b, "差出人", addresses(msg.from()), Field::From);
    header(&mut b, "宛先", addresses(msg.to()), Field::To);
    header(&mut b, "Cc", addresses(msg.cc()), Field::Cc);
    header(&mut b, "返信先", addresses(msg.reply_to()), Field::ReplyTo);
    let date = msg.date().map(|d| d.to_rfc822());
    if let Some(d) = &date {
        b.push(&format!("日付: {d}\n"));
    }
    b.push("\n");
    b.location("本文");
    let body: Vec<String> = (0..msg.text_body_count()).filter_map(|i| msg.body_text(i).map(|t| t.into_owned())).collect();
    let body = body.join("\n\n");
    let s = b.text.len();
    b.push(&body);
    fields.push(((s, b.text.len()), Field::Body));
    if msg.html_body_count() > 0 {
        warnings.push("HTML 形式の本文は、テキスト形式に変換して書き出します".to_string());
    }
    let names: Vec<String> = msg.attachments().map(|a| a.attachment_name().unwrap_or("(名前なし)").to_string()).collect();
    if !names.is_empty() {
        b.newline();
        b.push("\n");
        b.location("添付ファイル");
        for n in &names {
            b.push("添付ファイル (取り除きます): ");
            let s = b.text.len();
            b.push(n);
            fields.push(((s, b.text.len()), Field::Attachment));
            b.push("\n");
        }
        warnings.push(format!("添付ファイル {} 件は中身を確認できないため、書き出すメールから取り除きます", names.len()));
    }
    Ok(Document {
        kind: DocKind::Eml,
        text: b.text,
        encoding_name: DocKind::Eml.label(true).to_string(),
        warnings,
        properties: vec![],
        locations: b.locations,
        inner: Inner::Eml(EmlDoc { fields, date }),
    })
}

pub(crate) fn write(doc: &Document, e: &EmlDoc, result: &MaskResult) -> Result<Vec<u8>, String> {
    let ranges: Vec<(usize, usize)> = e.fields.iter().map(|(r, _)| *r).collect();
    let pieces = split_output(&doc.text, result, &ranges);
    let mut out = String::new();
    let get = |f: Field| e.fields.iter().zip(&pieces).filter(|((_, k), _)| *k == f).map(|(_, p)| p.as_str()).collect::<Vec<_>>();
    // 差出人・宛先は、マスク後の表示名と、存在しない宛先 (.invalid ドメイン) の組にする
    for (name, f) in [("From", Field::From), ("To", Field::To), ("Cc", Field::Cc), ("Reply-To", Field::ReplyTo)] {
        if let Some(v) = get(f).first().filter(|v| !v.trim().is_empty()) {
            out.push_str(&format!("{name}: {} <undisclosed@sumiveil.invalid>\r\n", encode_word(v, name.len() + 2)));
        }
    }
    if let Some(v) = get(Field::Subject).first() {
        out.push_str(&format!("Subject: {}\r\n", encode_word(v, 9)));
    }
    if let Some(d) = &e.date {
        out.push_str(&format!("Date: {d}\r\n"));
    }
    out.push_str("MIME-Version: 1.0\r\n");
    out.push_str("Content-Type: text/plain; charset=UTF-8\r\n");
    out.push_str("Content-Transfer-Encoding: base64\r\n");
    out.push_str(&format!("X-Sumiveil: masked by Sumiveil {}\r\n", crate::VERSION));
    // ファイル名には取引先名などが入りやすく、検出しきれないため、番号と拡張子だけを残す
    for (i, a) in get(Field::Attachment).iter().enumerate() {
        let ext = a.rsplit_once('.').map(|(_, e)| e).filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or("bin");
        out.push_str(&format!("X-Sumiveil-Removed-Attachment: attachment-{}.{}\r\n", i + 1, ext.to_ascii_lowercase()));
    }
    out.push_str("\r\n");
    let body = get(Field::Body).first().map(|s| s.replace("\r\n", "\n").replace('\n', "\r\n")).unwrap_or_default();
    for line in base64(body.as_bytes()).as_bytes().chunks(76) {
        out.push_str(std::str::from_utf8(line).unwrap_or_default());
        out.push_str("\r\n");
    }
    Ok(out.into_bytes())
}

/// ヘッダー用に符号化する (ASCII のみで記号がなければそのまま、それ以外は UTF-8 の B 符号化。長い場合は折り返す)。
fn encode_word(s: &str, header_len: usize) -> String {
    let plain = s.chars().all(|c| c.is_ascii_graphic() || c == ' ') && !s.contains(['<', '>', '"', '(', ')', ',', ';', ':', '@', '[', ']', '\\', '=', '?']);
    if plain && header_len + s.len() <= 76 {
        return s.to_string();
    }
    // 1 語あたり元の文字で 45 バイトまで (符号化後 60 文字 + 前後 12 文字で 1 行 76 文字以内)
    let mut words = vec![];
    let mut cur = String::new();
    for c in s.chars() {
        if cur.len() + c.len_utf8() > 45 {
            words.push(std::mem::take(&mut cur));
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words.iter().map(|w| format!("=?UTF-8?B?{}?=", base64(w.as_bytes()))).collect::<Vec<_>>().join("\r\n ")
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encoding() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64("あ".as_bytes()), "44GC");
    }

    #[test]
    fn header_words() {
        assert_eq!(encode_word("Hello", 9), "Hello");
        assert!(encode_word("<NAME_1>", 6).starts_with("=?UTF-8?B?"));
        let long = "件名".repeat(30);
        assert!(encode_word(&long, 9).lines().all(|l| l.trim_start().len() <= 76));
    }
}
