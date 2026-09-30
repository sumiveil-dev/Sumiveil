//! Windows のシステムフォントを読み込む (再配布しない)。メモリマップで必要な部分だけ読む。

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

pub const SEMIBOLD: &str = "semibold";

fn fonts_dir() -> PathBuf {
    let windir = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    windir.join("Fonts")
}

fn map_file(name: &str) -> Option<&'static [u8]> {
    let candidates = [
        fonts_dir().join(name),
        std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join(r"Microsoft\Windows\Fonts").join(name)).unwrap_or_default(),
    ];
    for p in candidates {
        if let Ok(f) = std::fs::File::open(&p) {
            // Safety: フォントファイルは実行中に変更されない前提で読み取り専用マップする
            if let Ok(m) = unsafe { memmap2::Mmap::map(&f) } {
                let m: &'static memmap2::Mmap = Box::leak(Box::new(m));
                return Some(&m[..]);
            }
        }
    }
    None
}

/// TTC 内でファミリー名に `want` を含むフォントの番号を探す。
fn ttc_index(data: &[u8], want: &str) -> Option<u32> {
    let rd16 = |o: usize| data.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let rd32 = |o: usize| data.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    if data.get(0..4)? != b"ttcf" {
        return None;
    }
    let n = rd32(8)? as usize;
    for i in 0..n.min(16) {
        let off = rd32(12 + i * 4)? as usize;
        let num_tables = rd16(off + 4)? as usize;
        for t in 0..num_tables {
            let rec = off + 12 + t * 16;
            if data.get(rec..rec + 4)? != b"name" {
                continue;
            }
            let noff = rd32(rec + 8)? as usize;
            let count = rd16(noff + 2)? as usize;
            let storage = noff + rd16(noff + 4)? as usize;
            for r in 0..count {
                let nr = noff + 6 + r * 12;
                let (platform, name_id) = (rd16(nr)?, rd16(nr + 6)?);
                if platform != 3 || !(name_id == 1 || name_id == 4) {
                    continue;
                }
                let len = rd16(nr + 8)? as usize;
                let start = storage + rd16(nr + 10)? as usize;
                let raw = data.get(start..start + len)?;
                let s = String::from_utf16_lossy(&raw.chunks(2).map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)])).collect::<Vec<_>>());
                if s.contains(want) {
                    return Some(i as u32);
                }
            }
        }
    }
    None
}

struct Loader {
    defs: FontDefinitions,
}

impl Loader {
    /// 候補 (ファイル名, TTC 内のファミリー名) を順に試し、読めた最初のものを登録して名前を返す。
    fn add(&mut self, key: &str, candidates: &[(&str, Option<&str>)]) -> Option<String> {
        for (file, family) in candidates {
            if let Some(data) = map_file(file) {
                let index = family.and_then(|f| ttc_index(data, f)).unwrap_or(0);
                let mut fd = FontData::from_static(data);
                fd.index = index;
                self.defs.font_data.insert(key.to_string(), Arc::new(fd));
                return Some(key.to_string());
            }
        }
        None
    }
}

/// フォントを設定する。
pub fn install(ctx: &egui::Context) {
    let mut l = Loader { defs: FontDefinitions::default() };
    let ui_latin = l.add("ui-latin", &[("SegUIVar.ttf", None), ("segoeui.ttf", None)]);
    let ui_jp = l.add("ui-jp", &[("YuGothM.ttc", Some("Yu Gothic UI")), ("YuGothR.ttc", Some("Yu Gothic UI")), ("meiryo.ttc", Some("Meiryo UI")), ("msgothic.ttc", Some("MS UI Gothic"))]);
    let mono_latin = l.add("mono-latin", &[("CascadiaMono.ttf", None), ("consola.ttf", None)]);
    let mono_jp = l.add("mono-jp", &[("BIZ-UDGothicR.ttc", Some("BIZ UDGothic")), ("msgothic.ttc", Some("MS Gothic"))]);
    let semi_latin = l.add("semi-latin", &[("seguisb.ttf", None), ("segoeuib.ttf", None)]);
    let semi_jp = l.add("semi-jp", &[("YuGothB.ttc", Some("Yu Gothic UI")), ("meiryob.ttc", Some("Meiryo UI"))]);

    // 予備の日本語 (CJK) フォント。見つかったものをすべて順に後ろへ足し、上のフォントに無い字形を補う。
    // Yu Gothic・MS Gothic は Windows 10/11 の全言語版に標準で入っている。YaHei・JhengHei・SimSun も
    // 仮名と漢字を含むため、日本語フォントが削られた環境 (サンドボックス等) の最後の砦になる。
    const FALLBACKS: &[(&str, &str, Option<&str>)] = &[
        ("fb-yugothic", "YuGothR.ttc", Some("Yu Gothic")),
        ("fb-meiryo", "meiryo.ttc", Some("Meiryo UI")),
        ("fb-msgothic", "msgothic.ttc", Some("MS UI Gothic")),
        ("fb-bizud", "BIZ-UDGothicR.ttc", Some("BIZ UDPGothic")),
        ("fb-yumin", "yumin.ttf", None),
        ("fb-msmincho", "msmincho.ttc", Some("MS Mincho")),
        ("fb-yahei", "msyh.ttc", Some("Microsoft YaHei UI")),
        ("fb-jhenghei", "msjh.ttc", Some("Microsoft JhengHei UI")),
        ("fb-simsun", "simsun.ttc", Some("SimSun")),
        ("fb-malgun", "malgun.ttf", None),
        // 記号 (✓ ▾ … など) と絵文字
        ("fb-symbol", "seguisym.ttf", None),
        ("fb-emoji", "seguiemj.ttf", None),
    ];
    let mut fallbacks: Vec<Option<String>> = vec![];
    for (key, file, family) in FALLBACKS {
        fallbacks.push(l.add(key, &[(file, *family)]));
    }
    let has_japanese = ui_jp.is_some() || mono_jp.is_some() || fallbacks.iter().any(Option::is_some);

    let defaults: Vec<String> = l.defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mono_defaults: Vec<String> = l.defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();

    // 先頭のフォント → (use_fallbacks なら) 予備の日本語フォント の順に並べる (egui 同梱のフォントは使わない)
    let chain = |items: &[&Option<String>], use_fallbacks: bool, tail: &[String]| -> Vec<String> {
        let mut v: Vec<String> = vec![];
        let fb: &[Option<String>] = if use_fallbacks { &fallbacks } else { &[] };
        for name in items.iter().copied().chain(fb.iter()).flatten().chain(tail.iter()) {
            if !v.contains(name) {
                v.push(name.clone());
            }
        }
        v
    };
    let prop = chain(&[&ui_latin, &ui_jp], true, &defaults);
    let mono = chain(&[&mono_latin, &mono_jp, &ui_jp], true, &mono_defaults);
    let semi = chain(&[&semi_latin, &semi_jp, &ui_latin, &ui_jp], true, &defaults);
    l.defs.families.insert(FontFamily::Proportional, prop);
    l.defs.families.insert(FontFamily::Monospace, mono);
    l.defs.families.insert(FontFamily::Name(SEMIBOLD.into()), semi);
    ctx.set_fonts(l.defs);
    HAS_JAPANESE_FONT.store(has_japanese, std::sync::atomic::Ordering::Relaxed);
}

static HAS_JAPANESE_FONT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// 日本語を表示できるフォントが 1 つでも見つかったか。
pub fn has_japanese() -> bool {
    HAS_JAPANESE_FONT.load(std::sync::atomic::Ordering::Relaxed)
}
