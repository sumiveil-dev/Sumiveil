//! sumiveil: テキスト中の個人情報・機密情報をマスクするコマンドラインツール。

mod args;
mod config_cmd;
mod run;
mod util;

use std::process::ExitCode;

use anstream::ColorChoice;
use clap::{CommandFactory, FromArgMatches};
use sumiveil_core::catalog::{CATALOG, CATEGORIES};
use sumiveil_core::lang::Lang;

use args::{Cli, ColorWhen, Command};
use run::Mode;
use util::*;

/// 診断レポートのファイルを作る (送信はしない)。設定が壊れていても作れるよう、読み込みの失敗も記録する。
fn diagnose(g: &args::GlobalOpts, lang: Lang) -> anyhow::Result<ExitCode> {
    use sumiveil_core::diag::{self, Info, Reason};
    let path = sumiveil_core::config::resolve_config_path(g.config.as_deref());
    let (loaded, err) = match sumiveil_core::config::load(&path, g.profile.as_deref()) {
        Ok(l) => (Some(l), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let info = Info { reason: Reason::Manual, program: "sumiveil", config_path: &path, loaded: loaded.as_ref(), config_error: err.as_deref(), extra: vec![], error: None };
    let out = diag::write_report(&info)?;
    anstream::println!("{}: {}", lang.t("診断レポートを作成しました (自動では送信されません)", "Created a diagnostic report (never sent automatically)"), out.display());
    Ok(ExitCode::SUCCESS)
}

fn list_detectors(g: &args::GlobalOpts, json: bool, lang: Lang) -> anyhow::Result<ExitCode> {
    let loaded = load_config(g, lang)?;
    let cfg = &loaded.config;
    if json {
        let items: Vec<serde_json::Value> = CATALOG
            .iter()
            .map(|d| {
                serde_json::json!({
                    "id": d.id, "category": d.category, "label": d.label, "label_ja": d.label_ja,
                    "name_en": d.name_en, "name_ja": d.name_ja, "default_enabled": d.default_enabled,
                    "enabled": cfg.detector_enabled(d.id), "priority": d.priority, "example": d.example,
                })
            })
            .collect();
        write_stdout(format!("{}\n", serde_json::to_string_pretty(&items)?).as_bytes())?;
        return Ok(ExitCode::SUCCESS);
    }
    let mut out = String::new();
    for c in CATEGORIES {
        let cat_on = cfg.category_enabled(c.id);
        let cname = if lang.is_ja() { c.name_ja } else { c.name_en };
        let mark = if cat_on { format!("{OK}●{OK:#}") } else { format!("{DIM}○{DIM:#}") };
        out.push_str(&format!("{mark} {BOLD}{}{BOLD:#}  {DIM}{cname}{DIM:#}\n", c.id));
        for d in CATALOG.iter().filter(|d| d.category == c.id) {
            let on = cfg.detector_enabled(d.id);
            let mark = if on { format!("{OK}✓{OK:#}") } else { format!("{DIM}-{DIM:#}") };
            let name = if lang.is_ja() { d.name_ja } else { d.name_en };
            out.push_str(&format!("   {mark} {:<22} {name}\n", d.id));
        }
        if c.id == "custom" {
            for r in &cfg.custom_rules {
                let mark = if r.enabled { format!("{OK}✓{OK:#}") } else { format!("{DIM}-{DIM:#}") };
                out.push_str(&format!("   {mark} {:<22} {}\n", format!("custom:{}", r.id), r.name));
            }
            for k in &cfg.keywords {
                let mark = if k.enabled { format!("{OK}✓{OK:#}") } else { format!("{DIM}-{DIM:#}") };
                out.push_str(&format!("   {mark} {:<22} {} ({})\n", format!("keyword:{}", k.label), k.name, k.words.len()));
            }
        }
    }
    let mut stdout = anstream::stdout();
    use std::io::Write;
    let _ = stdout.write_all(out.as_bytes());
    Ok(ExitCode::SUCCESS)
}

fn open_gui(file: Option<std::path::PathBuf>, lang: Lang) -> anyhow::Result<ExitCode> {
    let exe = std::env::current_exe()?;
    let gui = exe.with_file_name(if cfg!(windows) { "sumiveil-gui.exe" } else { "sumiveil-gui" });
    if !gui.exists() {
        anyhow::bail!("{}: {}", lang.t("GUI が見つかりません", "GUI executable not found"), gui.display());
    }
    let mut cmd = std::process::Command::new(gui);
    if let Some(f) = file {
        cmd.arg(std::fs::canonicalize(&f).unwrap_or(f));
    }
    cmd.spawn()?;
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let lang = early_lang();
    let mut cmd = Cli::command();
    if lang.is_ja() {
        cmd = args::localize(cmd);
    }
    let matches = cmd.get_matches();
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(c) => c,
        Err(e) => e.exit(),
    };
    match cli.global.color {
        ColorWhen::Always => ColorChoice::Always.write_global(),
        ColorWhen::Never => ColorChoice::Never.write_global(),
        ColorWhen::Auto => {
            if std::env::var_os("NO_COLOR").is_some() {
                ColorChoice::Never.write_global();
            }
        }
    }
    let g = &cli.global;
    let result = match cli.command {
        None => run::run(g, cli.mask, lang, Mode::Mask),
        Some(Command::Mask(a)) => run::run(g, a, lang, Mode::Mask),
        Some(Command::Detect(a)) => run::run(g, a, lang, Mode::Detect),
        Some(Command::Check(a)) => run::run(g, a, lang, Mode::Check),
        Some(Command::Config(c)) => config_cmd::run(g, c, lang),
        Some(Command::ListDetectors { json }) => list_detectors(g, json, lang),
        Some(Command::Diagnose) => diagnose(g, lang),
        Some(Command::Gui { file }) => open_gui(file, lang),
        Some(Command::Completions { shell }) => {
            let mut c = Cli::command();
            clap_complete::generate(shell, &mut c, "sumiveil", &mut std::io::stdout());
            Ok(ExitCode::SUCCESS)
        }
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            anstream::eprintln!("{ERR}sumiveil:{ERR:#} {e:#}");
            ExitCode::from(2)
        }
    }
}
