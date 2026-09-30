//! 診断レポート (問題が起きたときに、利用者が自分で担当者へ送るためのテキストファイル)。
//!
//! - 自動では送信しない。ファイルを作るだけ (送るかどうかは利用者が決める)。
//! - 入力した文章・ファイルの内容・辞書やルールに登録した語は含めない (件数だけ)。
//! - パスに含まれるユーザー名は `%USERPROFILE%` などの表記に置き換える。
//! - エラーメッセージの `…` で囲まれた部分 (Rust の文字列操作のエラーは対象の文字列を含むことがある) は伏せる。

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::config::LoadedConfig;

/// レポートを作る理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// 利用者が「診断レポートを作成」を押した
    Manual,
    /// プログラムの内部エラー (パニック)
    Crash,
    /// 画面の描画方式をすべて試しても起動できなかった
    StartupFailure,
}

impl Reason {
    fn label(self) -> &'static str {
        match self {
            Reason::Manual => "手動で作成 (Manual)",
            Reason::Crash => "内部エラーで異常終了 (Crash)",
            Reason::StartupFailure => "画面を表示できずに起動に失敗 (Startup failure)",
        }
    }

    fn file_prefix(self) -> &'static str {
        match self {
            Reason::Manual => "sumiveil-diag",
            Reason::Crash => "sumiveil-crash",
            Reason::StartupFailure => "sumiveil-startup",
        }
    }
}

/// レポートに書く情報。
pub struct Info<'a> {
    pub reason: Reason,
    /// 実行したプログラム (「sumiveil-gui」「sumiveil」)
    pub program: &'a str,
    /// 設定ファイルの場所と読み込み結果 (読めなかった場合は None)
    pub config_path: &'a Path,
    pub loaded: Option<&'a LoadedConfig>,
    /// 設定ファイルのエラー
    pub config_error: Option<&'a str>,
    /// 画面の描画方式など、プログラムごとの追加情報 (見出し, 値)
    pub extra: Vec<(&'a str, String)>,
    /// エラーの内容 (パニックのメッセージ・発生場所・バックトレースなど)
    pub error: Option<String>,
}

/// 診断レポートを保存するフォルダ (設定ファイルと同じ場所の diagnostics)。
/// インストール版は %APPDATA%\Sumiveil\diagnostics、ポータブル版は exe の横の diagnostics になる。
pub fn report_dir(config_path: &Path) -> PathBuf {
    config_path.parent().map(|p| p.join("diagnostics")).unwrap_or_else(|| PathBuf::from("diagnostics"))
}

/// レポートを作ってファイルに保存し、保存先を返す。
pub fn write_report(info: &Info) -> std::io::Result<PathBuf> {
    let dir = report_dir(info.config_path);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}-{}.txt", info.reason.file_prefix(), timestamp_for_file()));
    // Windows のメモ帳でも文字化けしないよう BOM 付き UTF-8 で保存する
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend(build_report(info).replace('\n', "\r\n").into_bytes());
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// レポートの本文。
pub fn build_report(info: &Info) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "Sumiveil 診断レポート / Diagnostic report");
    let _ = writeln!(s, "======================================");
    let _ = writeln!(s, "このファイルは自動では送信されません。内容を確認してから、必要に応じて担当者に送ってください。");
    let _ = writeln!(s, "入力した文章・ファイルの内容・辞書やルールに登録した語は含まれていません。");
    let _ = writeln!(s);
    let _ = writeln!(s, "作成理由: {}", info.reason.label());
    let _ = writeln!(s, "作成日時: {}", timestamp_display());
    let _ = writeln!(s);

    let _ = writeln!(s, "[アプリ]");
    let _ = writeln!(s, "プログラム: {} {}", info.program, crate::VERSION);
    if let Ok(exe) = std::env::current_exe() {
        let _ = writeln!(s, "場所: {}", anonymize(&exe.display().to_string()));
    }
    let portable = crate::config::portable_config_path().is_some_and(|p| p == info.config_path);
    let _ = writeln!(s, "種類: {}", if portable { "ポータブル版" } else { "インストール版 (または開発版)" });
    let _ = writeln!(s, "OS: {}", os_version());
    let _ = writeln!(s, "CPU アーキテクチャ: {}", std::env::consts::ARCH);
    let _ = writeln!(s, "OS の言語: {}", sys_locale::get_locale().unwrap_or_default());
    for (k, v) in &info.extra {
        let _ = writeln!(s, "{k}: {}", anonymize(v));
    }
    let _ = writeln!(s);

    let _ = writeln!(s, "[設定]");
    let _ = writeln!(s, "設定ファイル: {}", anonymize(&info.config_path.display().to_string()));
    let _ = writeln!(s, "設定ファイルの有無: {}", if info.config_path.exists() { "あり" } else { "なし (既定値で動作)" });
    if let Some(e) = info.config_error {
        let _ = writeln!(s, "設定ファイルのエラー: {}", redact(&anonymize(e)));
    }
    if let Some(l) = info.loaded {
        write_config_summary(&mut s, l);
    }
    let _ = writeln!(s);

    if let Some(e) = &info.error {
        let _ = writeln!(s, "[エラーの内容]");
        let _ = writeln!(s, "{}", redact(&anonymize(e)).trim_end());
        let _ = writeln!(s);
    }
    s
}

/// 設定の要約: オン/オフや件数だけを書き、登録した語や正規表現の中身は書かない。
fn write_config_summary(s: &mut String, l: &LoadedConfig) {
    let c = &l.config;
    let _ = writeln!(s, "プロファイル: {} (全 {} 件)", if l.active_profile == "default" { "既定" } else { "利用者が作成したもの" }, l.profiles.len());
    let _ = writeln!(s, "表示言語: {} / テーマ: {:?}", c.general.language, c.general.theme);
    let g = &c.gui;
    let _ = writeln!(s, "描画方式の設定: {}", g.renderer);
    let _ = writeln!(
        s,
        "エディタ: 自動マスク={} 待ち時間={}ms 折り返し={} 行番号={} スクロール同期={} 検出一覧={} 文字サイズ={}",
        g.auto_mask, g.debounce_ms, g.word_wrap, g.show_line_numbers, g.sync_scroll, g.show_detection_panel, g.font_size
    );
    let _ = writeln!(s, "トレイ: 表示={} 閉じると格納={} 起動時に格納={} / ホットキー: 使う={} ({})", g.tray_enabled, g.close_to_tray, g.start_in_tray, g.hotkey_enabled, g.hotkey);
    let _ = writeln!(s, "最低信頼度: {}", c.masking.min_confidence);
    let disabled_cats: Vec<&str> = crate::catalog::CATEGORIES.iter().filter(|x| !c.category_enabled(x.id)).map(|x| x.id).collect();
    let disabled: Vec<&str> = crate::catalog::CATALOG.iter().filter(|d| !c.detector_enabled(d.id)).map(|d| d.id).collect();
    let _ = writeln!(s, "無効のカテゴリ: {}", if disabled_cats.is_empty() { "なし".to_string() } else { disabled_cats.join(", ") });
    let _ = writeln!(s, "無効の検出器: {} 件 ({})", disabled.len(), disabled.join(", "));
    let n = &c.names;
    let _ = writeln!(s, "人名: 辞書={} しきい値={} 伝播={} 形態素解析={}", n.use_dictionary, n.dictionary_threshold, n.propagate, n.use_morphology);
    if n.use_morphology {
        let found = crate::morph::find_dictionary(&n.morphology_dict).map(|p| anonymize(&p.display().to_string()));
        let _ = writeln!(s, "形態素解析の辞書: {}", found.unwrap_or_else(|| "見つからない".into()));
    }
    let words: usize = c.keywords.iter().map(|k| k.words.len()).sum();
    let _ = writeln!(s, "キーワード辞書: {} グループ / {} 語 (語の内容は含めない)", c.keywords.len(), words);
    let _ = writeln!(s, "カスタムルール: {} 件 (パターンは含めない)", c.custom_rules.len());
    let a = &c.allowlist;
    let _ = writeln!(s, "許可リスト: 値 {} 件 / ドメイン {} 件 / 正規表現 {} 件 (内容は含めない)", a.values.len(), a.domains.len(), a.patterns.len());
    let _ = writeln!(s, "Office 文書のプロパティ: {}", c.files.properties);
    let _ = writeln!(s, "設定の警告: {} 件", l.warnings.len());
    for w in &l.warnings {
        let _ = writeln!(s, "  - {}", redact(&anonymize(w)));
    }
}

/// パスなどに含まれる利用者固有の部分を一般的な表記に置き換える。
pub fn anonymize(s: &str) -> String {
    let mut out = s.to_string();
    // 長いものから順に置き換える (%LOCALAPPDATA% は %USERPROFILE% の下にあるため)
    for var in ["LOCALAPPDATA", "APPDATA", "TEMP", "USERPROFILE", "OneDrive"] {
        if let Ok(v) = std::env::var(var) {
            if v.len() > 3 {
                out = replace_ignore_case(&out, &v, &format!("%{var}%"));
            }
        }
    }
    if let Ok(user) = std::env::var("USERNAME") {
        if user.len() >= 2 {
            out = replace_ignore_case(&out, &user, "<ユーザー名>");
        }
    }
    if let Ok(host) = std::env::var("COMPUTERNAME") {
        if host.len() >= 2 {
            out = replace_ignore_case(&out, &host, "<PC名>");
        }
    }
    out
}

fn replace_ignore_case(s: &str, from: &str, to: &str) -> String {
    let re = regex::RegexBuilder::new(&regex::escape(from)).case_insensitive(true).build();
    match re {
        Ok(re) => re.replace_all(s, regex::NoExpand(to)).into_owned(),
        Err(_) => s.replace(from, to),
    }
}

/// エラーメッセージ中の `…` や "…" で囲まれた部分を伏せる (入力した文章の一部が入ることがあるため)。
pub fn redact(s: &str) -> String {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r#"`[^`\n]*`|"[^"\n]{4,}""#).unwrap());
    re.replace_all(s, "<省略>").into_owned()
}

/// Windows のバージョン (「Microsoft Windows [Version 10.0.26100.1234]」)。
fn os_version() -> String {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        if let Ok(o) = std::process::Command::new("cmd").args(["/d", "/c", "ver"]).creation_flags(CREATE_NO_WINDOW).output() {
            let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !t.is_empty() {
                return t;
            }
        }
    }
    std::env::consts::OS.to_string()
}

fn now_jst() -> (i64, u32, u32, u32, u32, u32) {
    // 日本時間 (JST = UTC+9。夏時間はない) で記録する。日時ライブラリは追加しない
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) + 9 * 3600;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    (y, m, d, (rem / 3600) as u32, (rem % 3600 / 60) as u32, (rem % 60) as u32)
}

fn timestamp_for_file() -> String {
    let (y, m, d, hh, mm, ss) = now_jst();
    format!("{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}")
}

fn timestamp_display() -> String {
    let (y, m, d, hh, mm, ss) = now_jst();
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02} (JST)")
}

/// 1970-01-01 からの日数 → (年, 月, 日)。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_quoted_text() {
        let msg = "byte index 5 is not a char boundary; it is inside 'あ' (bytes 3..6) of `山田太郎 090-1234-5678`";
        let r = redact(msg);
        assert!(!r.contains("山田") && r.contains("<省略>"), "{r}");
    }

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_356), (2025, 9, 25));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn report_has_no_registered_words() {
        let mut cfg = crate::Config::default();
        cfg.keywords.push(crate::config::KeywordGroup { label: "X".into(), name: "秘密".into(), words: vec!["プロジェクト黒猫".into()], ..Default::default() });
        cfg.allowlist.values.push("boss@corp.example".into());
        let loaded = LoadedConfig { config: cfg, path: None, active_profile: "default".into(), profiles: vec![], warnings: vec![] };
        let path = PathBuf::from(r"C:\tmp\sumiveil\config.toml");
        let info = Info { reason: Reason::Manual, program: "test", config_path: &path, loaded: Some(&loaded), config_error: None, extra: vec![], error: Some("panicked at `黒猫の件`".into()) };
        let r = build_report(&info);
        assert!(!r.contains("黒猫") && !r.contains("boss@") && !r.contains("秘密"), "{r}");
        assert!(r.contains("1 グループ / 1 語"), "{r}");
    }
}
