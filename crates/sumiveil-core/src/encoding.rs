//! 文字コードの判定・変換 (UTF-8 / UTF-8 BOM / UTF-16 / Shift_JIS(CP932) / EUC-JP / ISO-2022-JP ほか)。

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE, UTF_8};

#[derive(Debug, Clone)]
pub struct Decoded {
    pub text: String,
    pub encoding: &'static Encoding,
    pub bom: bool,
    /// 変換できない文字があった
    pub had_errors: bool,
}

impl Decoded {
    /// 表示用の文字コード名 (例: "UTF-8", "UTF-8 (BOM)", "Shift_JIS")
    pub fn display_name(&self) -> String {
        display_name(self.encoding, self.bom)
    }
}

pub fn display_name(enc: &'static Encoding, bom: bool) -> String {
    let base = match enc.name() {
        "UTF-8" => "UTF-8",
        "UTF-16LE" => "UTF-16LE",
        "UTF-16BE" => "UTF-16BE",
        "Shift_JIS" => "Shift_JIS",
        "EUC-JP" => "EUC-JP",
        "ISO-2022-JP" => "ISO-2022-JP",
        other => other,
    };
    if bom && enc == UTF_8 {
        format!("{base} (BOM)")
    } else {
        base.to_string()
    }
}

/// ラベルから文字コードを得る。`utf-8-bom` 等の独自ラベルにも対応。戻り値は (文字コード, BOM を付けるか)。
pub fn encoding_for_label(label: &str) -> Option<(&'static Encoding, bool)> {
    let l = label.trim().to_ascii_lowercase().replace('_', "-");
    match l.as_str() {
        "utf-8-bom" | "utf8-bom" | "utf-8-sig" | "utf8bom" => Some((UTF_8, true)),
        "utf8" | "utf-8" => Some((UTF_8, false)),
        "sjis" | "shift-jis" | "cp932" | "ms932" | "windows-31j" => Some((encoding_rs::SHIFT_JIS, false)),
        "utf-16" | "utf16" | "utf-16le" | "utf16le" => Some((UTF_16LE, true)),
        "utf-16be" | "utf16be" => Some((UTF_16BE, true)),
        "eucjp" | "euc-jp" => Some((encoding_rs::EUC_JP, false)),
        "jis" | "iso-2022-jp" => Some((encoding_rs::ISO_2022_JP, false)),
        other => Encoding::for_label(other.as_bytes()).map(|e| (e, false)),
    }
}

/// バイト列を文字列に変換する。`forced` が Some ならその文字コードを使う (auto は自動判定)。
pub fn decode(bytes: &[u8], forced: Option<&str>) -> Decoded {
    if let Some(label) = forced.filter(|l| !l.eq_ignore_ascii_case("auto")) {
        if let Some((enc, _)) = encoding_for_label(label) {
            let (bom_enc, bom_len) = Encoding::for_bom(bytes).unwrap_or((enc, 0));
            let enc = if bom_len > 0 { bom_enc } else { enc };
            let (text, had_errors) = enc.decode_without_bom_handling(&bytes[bom_len..]);
            return Decoded { text: text.into_owned(), encoding: enc, bom: bom_len > 0, had_errors };
        }
    }
    if let Some((enc, bom_len)) = Encoding::for_bom(bytes) {
        let (text, had_errors) = enc.decode_without_bom_handling(&bytes[bom_len..]);
        return Decoded { text: text.into_owned(), encoding: enc, bom: true, had_errors };
    }
    if let Some(enc) = sniff_utf16(bytes) {
        let (text, had_errors) = enc.decode_without_bom_handling(bytes);
        return Decoded { text: text.into_owned(), encoding: enc, bom: false, had_errors };
    }
    if std::str::from_utf8(bytes).is_ok() {
        return Decoded { text: String::from_utf8(bytes.to_vec()).unwrap(), encoding: UTF_8, bom: false, had_errors: false };
    }
    let mut det = EncodingDetector::new(Iso2022JpDetection::Allow);
    det.feed(bytes, true);
    let enc = det.guess(Some(b"jp"), Utf8Detection::Allow);
    let (text, had_errors) = enc.decode_without_bom_handling(bytes);
    Decoded { text: text.into_owned(), encoding: enc, bom: false, had_errors }
}

/// BOM なし UTF-16 の簡易判定 (ASCII 文字の上位バイトが 0 になる性質を使う)。
fn sniff_utf16(bytes: &[u8]) -> Option<&'static Encoding> {
    if bytes.len() < 4 || bytes.len() % 2 != 0 {
        return None;
    }
    let sample = &bytes[..bytes.len().min(4096)];
    let pairs = sample.len() / 2;
    let even_zero = sample.iter().step_by(2).filter(|&&b| b == 0).count();
    let odd_zero = sample.iter().skip(1).step_by(2).filter(|&&b| b == 0).count();
    if odd_zero * 10 >= pairs * 4 && even_zero * 20 < pairs {
        Some(UTF_16LE)
    } else if even_zero * 10 >= pairs * 4 && odd_zero * 20 < pairs {
        Some(UTF_16BE)
    } else {
        None
    }
}

/// 文字列を指定の文字コードでバイト列にする (UTF-16 にも対応)。
pub fn encode(text: &str, enc: &'static Encoding, bom: bool) -> Vec<u8> {
    if enc == UTF_16LE || enc == UTF_16BE {
        let le = enc == UTF_16LE;
        let mut out = Vec::with_capacity(text.len() * 2 + 2);
        if bom {
            out.extend_from_slice(if le { &[0xFF, 0xFE] } else { &[0xFE, 0xFF] });
        }
        for u in text.encode_utf16() {
            out.extend_from_slice(&if le { u.to_le_bytes() } else { u.to_be_bytes() });
        }
        return out;
    }
    if enc == UTF_8 {
        let mut out = Vec::with_capacity(text.len() + 3);
        if bom {
            out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        out.extend_from_slice(text.as_bytes());
        return out;
    }
    let (bytes, _, _) = enc.encode(text);
    bytes.into_owned()
}

#[cfg(test)]
#[path = "tests/encoding.rs"]
mod tests;
