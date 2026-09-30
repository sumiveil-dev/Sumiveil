//! CLI 共通の小物: 言語判定・設定読み込み・上書き・出力。

use std::io::Write;
use std::path::Path;

use anstyle::{AnsiColor, Style};
use anyhow::{bail, Context, Result};
use sumiveil_core::catalog::{self, CATALOG, CATEGORIES};
use sumiveil_core::config::{self, Config, LoadedConfig};
use sumiveil_core::lang::Lang;

use crate::args::{GlobalOpts, MaskArgs};

pub const WARN: Style = AnsiColor::Yellow.on_default().bold();
pub const ERR: Style = AnsiColor::Red.on_default().bold();
pub const OK: Style = AnsiColor::Green.on_default().bold();
pub const DIM: Style = Style::new().dimmed();
pub const DEL: Style = AnsiColor::Red.on_default();
pub const ADD: Style = AnsiColor::Green.on_default();
pub const BOLD: Style = Style::new().bold();

/// 引数解析前に表示言語を決める (環境変数 SUMIVEIL_LANG → 既定の設定ファイル → OS)。
pub fn early_lang() -> Lang {
    if let Ok(l) = std::env::var("SUMIVEIL_LANG") {
        return Lang::resolve(&l);
    }
    let path = config::resolve_config_path(None);
    let setting = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim_start_matches('\u{feff}').parse::<toml::Table>().ok())
        .and_then(|t| t.get("general")?.get("language")?.as_str().map(String::from))
        .unwrap_or_else(|| "auto".into());
    Lang::resolve(&setting)
}

pub fn warn(lang: Lang, quiet: bool, msg: &str) {
    if !quiet {
        anstream::eprintln!("{WARN}{}{WARN:#} {msg}", lang.t("警告:", "warning:"));
    }
}

pub fn load_config(g: &GlobalOpts, lang: Lang) -> Result<LoadedConfig> {
    let path = config::resolve_config_path(g.config.as_deref());
    if g.config.is_some() && !path.exists() {
        bail!("{}: {}", lang.t("設定ファイルが見つかりません", "config file not found"), path.display());
    }
    let loaded = config::load(&path, g.profile.as_deref())?;
    for w in &loaded.warnings {
        warn(lang, g.quiet, w);
    }
    Ok(loaded)
}

/// 検出器 ID またはカテゴリ ID を有効/無効にする。
pub fn set_enabled(cfg: &mut Config, id: &str, on: bool) -> Result<()> {
    let id = id.trim();
    if id == "all" {
        for c in CATEGORIES {
            cfg.categories.entry(c.id.into()).or_default().enabled = Some(on);
        }
        for d in CATALOG {
            cfg.detectors.entry(d.id.into()).or_default().enabled = Some(on);
        }
    } else if catalog::category(id).is_some() {
        cfg.categories.entry(id.into()).or_default().enabled = Some(on);
        if id == "custom" {
            cfg.custom_rules.iter_mut().for_each(|r| r.enabled = on);
            cfg.keywords.iter_mut().for_each(|k| k.enabled = on);
        }
    } else if catalog::detector(id).is_some() {
        cfg.detectors.entry(id.into()).or_default().enabled = Some(on);
        if on {
            // 所属カテゴリが無効なら有効にする (個別指定を優先)
            let cat = catalog::detector(id).unwrap().category;
            if !cfg.category_enabled(cat) {
                cfg.categories.entry(cat.into()).or_default().enabled = Some(true);
                for d in CATALOG.iter().filter(|d| d.category == cat && d.id != id) {
                    cfg.detectors.entry(d.id.into()).or_default().enabled.get_or_insert(false);
                }
            }
        }
    } else if let Some(r) = cfg.custom_rules.iter_mut().find(|r| r.id == id || format!("custom:{}", r.id) == id) {
        r.enabled = on;
    } else {
        bail!("unknown detector or category: {id} (see `sumiveil list-detectors`)");
    }
    Ok(())
}

pub fn apply_overrides(cfg: &mut Config, a: &MaskArgs) -> Result<()> {
    if !a.only.is_empty() {
        for d in CATALOG {
            cfg.detectors.entry(d.id.into()).or_default().enabled = Some(false);
        }
        for c in CATEGORIES {
            cfg.categories.entry(c.id.into()).or_default().enabled = Some(true);
        }
        let custom_listed = a.only.iter().any(|x| x == "custom");
        cfg.custom_rules.iter_mut().for_each(|r| r.enabled = custom_listed);
        cfg.keywords.iter_mut().for_each(|k| k.enabled = custom_listed);
        for id in &a.only {
            if catalog::category(id).is_some() {
                for d in CATALOG.iter().filter(|d| d.category == id) {
                    cfg.detectors.entry(d.id.into()).or_default().enabled = Some(true);
                }
            } else {
                set_enabled(cfg, id, true)?;
            }
        }
    }
    for id in &a.enable {
        set_enabled(cfg, id, true)?;
    }
    for id in &a.disable {
        set_enabled(cfg, id, false)?;
    }
    if let Some(t) = &a.template {
        sumiveil_core::template::Template::parse(t).map_err(|e| anyhow::anyhow!("--template: {e}"))?;
        cfg.masking.template = t.clone();
        // 個別テンプレートより優先させる
        cfg.detectors.values_mut().for_each(|d| d.template = None);
        cfg.categories.values_mut().for_each(|c| c.template = None);
        cfg.custom_rules.iter_mut().for_each(|r| r.template = None);
        cfg.keywords.iter_mut().for_each(|k| k.template = None);
    }
    if let Some(m) = a.min_confidence {
        cfg.masking.min_confidence = m.clamp(0.0, 1.0);
        cfg.detectors.values_mut().for_each(|d| d.min_confidence = None);
    }
    Ok(())
}

/// 標準出力へ書く。パイプが閉じられた場合は静かに終了する。
pub fn write_stdout(bytes: &[u8]) -> Result<()> {
    let mut out = std::io::stdout().lock();
    match out.write_all(bytes).and_then(|_| out.flush()) {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => std::process::exit(0),
        r => r.context("stdout"),
    }
}

pub fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
        }
    }
    std::fs::write(path, bytes).with_context(|| path.display().to_string())
}

pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut cb = arboard::Clipboard::new().context("clipboard")?;
    cb.set_text(text.to_string()).context("clipboard")?;
    Ok(())
}

pub fn read_clipboard() -> Result<String> {
    let mut cb = arboard::Clipboard::new().context("clipboard")?;
    cb.get_text().context("clipboard")
}
