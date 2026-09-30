//! `sumiveil config ...` サブコマンド。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use sumiveil_core::catalog::{self, CATALOG, CATEGORIES};
use sumiveil_core::config::{self, profile_key, ConfigDoc, DEFAULT_TOML};
use sumiveil_core::lang::Lang;

use crate::args::{ConfigCmd, GlobalOpts};
use crate::util::*;

const KNOWN_SECTIONS: &[&str] = &[
    "general", "masking", "categories", "detectors", "custom_rules", "keywords", "allowlist", "names", "gui", "batch", "profiles", "include",
];

fn check_key(lang: Lang, quiet: bool, key: &str) {
    let parts: Vec<&str> = key.split('.').collect();
    let mut p = parts.as_slice();
    if p.first() == Some(&"profiles") && p.len() > 2 {
        p = &p[2..];
    }
    let ok = match p {
        [first, ..] if !KNOWN_SECTIONS.contains(first) => false,
        ["detectors", id, ..] => catalog::detector(id).is_some(),
        ["categories", id, ..] => catalog::category(id).is_some(),
        _ => true,
    };
    if !ok {
        warn(lang, quiet, &format!("{}: {key}", lang.t("未知のキーです (無視される可能性があります)", "unknown key (may be ignored)")));
    }
}

/// 一時ファイルに保存して読み込みを確認してから置き換える。
fn save_validated(doc: &ConfigDoc, lang: Lang, quiet: bool) -> Result<()> {
    let tmp = doc.path.with_extension("toml.check");
    if let Some(dir) = doc.path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(&tmp, doc.to_toml_string())?;
    let check = config::load(&tmp, None);
    let _ = std::fs::remove_file(&tmp);
    let loaded = check.with_context(|| lang.t("変更後の設定を読み込めないため保存しませんでした", "the change was not saved because the result is invalid").to_string())?;
    for w in &loaded.warnings {
        warn(lang, quiet, w);
    }
    doc.save()?;
    Ok(())
}

fn set_ids(doc: &mut ConfigDoc, profile: &str, ids: &[String], on: bool) -> Result<()> {
    for id in ids {
        let id = id.trim();
        if id == "all" {
            for c in CATEGORIES {
                doc.set(&profile_key(profile, &format!("categories.{}.enabled", c.id)), on);
            }
            for d in CATALOG {
                doc.set(&profile_key(profile, &format!("detectors.{}.enabled", d.id)), on);
            }
        } else if catalog::category(id).is_some() {
            doc.set(&profile_key(profile, &format!("categories.{id}.enabled")), on);
        } else if catalog::detector(id).is_some() {
            doc.set(&profile_key(profile, &format!("detectors.{id}.enabled")), on);
        } else {
            bail!("unknown detector or category: {id} (see `sumiveil list-detectors`)");
        }
    }
    Ok(())
}

fn resolved_value(cfg: &sumiveil_core::Config, key: &str) -> Option<String> {
    let v = toml::Value::try_from(cfg).ok()?;
    let mut cur = &v;
    for k in key.split('.') {
        cur = cur.get(k)?;
    }
    Some(match cur {
        toml::Value::String(s) => format!("{s:?}"),
        other => other.to_string(),
    })
}

fn open_in_editor(path: &Path) -> Result<()> {
    let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).unwrap_or_else(|_| "notepad.exe".into());
    std::process::Command::new(editor).arg(path).spawn().context("failed to start editor")?;
    Ok(())
}

pub fn run(g: &GlobalOpts, cmd: ConfigCmd, lang: Lang) -> Result<ExitCode> {
    let path = config::resolve_config_path(g.config.as_deref());
    // --profile を指定した場合はそのプロファイル内を編集する
    let profile = g.profile.clone().unwrap_or_default();
    match cmd {
        ConfigCmd::Path => {
            println!("{}", path.display());
            if !path.exists() && !g.quiet {
                anstream::eprintln!("{DIM}{}{DIM:#}", lang.t("(未作成: 既定値を使用中。`sumiveil config init` で作成できます)", "(not created yet: using defaults. Create it with `sumiveil config init`)"));
            }
        }
        ConfigCmd::Show { resolved } => {
            if resolved {
                let loaded = load_config(g, lang)?;
                let s = toml::to_string_pretty(&loaded.config)?;
                write_stdout(format!("# profile: {}\n{s}", loaded.active_profile).as_bytes())?;
            } else if path.exists() {
                write_stdout(&std::fs::read(&path)?)?;
            } else {
                write_stdout(DEFAULT_TOML.as_bytes())?;
            }
        }
        ConfigCmd::Get { key } => {
            let doc = ConfigDoc::open(&path)?;
            let k = profile_key(&profile, &key);
            match doc.get(&k).filter(|_| path.exists()) {
                Some(v) => println!("{v}"),
                None => {
                    let loaded = load_config(g, lang)?;
                    match resolved_value(&loaded.config, &key) {
                        Some(v) => {
                            println!("{v}");
                            if !g.quiet {
                                anstream::eprintln!("{DIM}{}{DIM:#}", lang.t("(既定値)", "(default)"));
                            }
                        }
                        None => bail!("{}: {key}", lang.t("キーが見つかりません", "key not found")),
                    }
                }
            }
        }
        ConfigCmd::Set { key, value } => {
            check_key(lang, g.quiet, &key);
            let mut doc = ConfigDoc::open(&path)?;
            doc.set_raw(&profile_key(&profile, &key), &value);
            save_validated(&doc, lang, g.quiet)?;
            if !g.quiet {
                anstream::eprintln!("{OK}✓{OK:#} {} = {value}", profile_key(&profile, &key));
            }
        }
        ConfigCmd::Unset { key } => {
            let mut doc = ConfigDoc::open(&path)?;
            if doc.remove(&profile_key(&profile, &key)) {
                save_validated(&doc, lang, g.quiet)?;
            } else if !g.quiet {
                warn(lang, g.quiet, &format!("{}: {key}", lang.t("設定されていません", "not set")));
            }
        }
        ConfigCmd::Enable { ids } => {
            let mut doc = ConfigDoc::open(&path)?;
            set_ids(&mut doc, &profile, &ids, true)?;
            save_validated(&doc, lang, g.quiet)?;
        }
        ConfigCmd::Disable { ids } => {
            let mut doc = ConfigDoc::open(&path)?;
            set_ids(&mut doc, &profile, &ids, false)?;
            save_validated(&doc, lang, g.quiet)?;
        }
        ConfigCmd::Init { force, portable } => {
            let target: PathBuf = if portable { config::portable_config_path().context("exe path")? } else { path.clone() };
            if target.exists() && !force {
                bail!("{}: {} (--force)", lang.t("既に存在します", "already exists"), target.display());
            }
            write_file(&target, DEFAULT_TOML.as_bytes())?;
            if !g.quiet {
                anstream::eprintln!("{OK}✓{OK:#} {}", target.display());
            }
        }
        ConfigCmd::Export { file, resolved } => {
            if resolved {
                let loaded = load_config(g, lang)?;
                write_file(&file, toml::to_string_pretty(&loaded.config)?.as_bytes())?;
            } else if path.exists() {
                std::fs::copy(&path, &file).with_context(|| file.display().to_string())?;
            } else {
                write_file(&file, DEFAULT_TOML.as_bytes())?;
            }
            if !g.quiet {
                anstream::eprintln!("{OK}✓{OK:#} {}", file.display());
            }
        }
        ConfigCmd::Import { file, merge } => {
            let imported = config::load(&file, None).with_context(|| file.display().to_string())?;
            for w in &imported.warnings {
                warn(lang, g.quiet, w);
            }
            if merge {
                let table: toml::Table = std::fs::read_to_string(&file)?.trim_start_matches('\u{feff}').parse()?;
                let mut doc = ConfigDoc::open(&path)?;
                fn walk(doc: &mut ConfigDoc, prefix: &str, t: &toml::Table) -> Result<()> {
                    for (k, v) in t {
                        let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                        match v {
                            toml::Value::Table(sub) => walk(doc, &key, sub)?,
                            other => doc.set_serialized(&key, other)?,
                        }
                    }
                    Ok(())
                }
                walk(&mut doc, "", &table)?;
                save_validated(&doc, lang, g.quiet)?;
            } else {
                if path.exists() {
                    let bak = path.with_extension("toml.bak");
                    std::fs::copy(&path, &bak)?;
                    if !g.quiet {
                        anstream::eprintln!("{DIM}backup: {}{DIM:#}", bak.display());
                    }
                }
                let bytes = std::fs::read(&file)?;
                write_file(&path, &bytes)?;
            }
            if !g.quiet {
                anstream::eprintln!("{OK}✓{OK:#} {} → {}", file.display(), path.display());
            }
        }
        ConfigCmd::Validate { file } => {
            let target = file.unwrap_or(path);
            match config::load(&target, g.profile.as_deref()) {
                Ok(l) if l.warnings.is_empty() => {
                    anstream::println!("{OK}✓{OK:#} {} {}", target.display(), lang.t("問題ありません", "is valid"));
                }
                Ok(l) => {
                    for w in &l.warnings {
                        anstream::println!("{WARN}!{WARN:#} {w}");
                    }
                    return Ok(ExitCode::from(1));
                }
                Err(e) => {
                    anstream::println!("{ERR}✗{ERR:#} {e}");
                    return Ok(ExitCode::from(1));
                }
            }
        }
        ConfigCmd::Edit => {
            if !path.exists() {
                write_file(&path, DEFAULT_TOML.as_bytes())?;
            }
            open_in_editor(&path)?;
        }
        ConfigCmd::Profiles => {
            let loaded = load_config(g, lang)?;
            for p in &loaded.profiles {
                let mark = if p.name == loaded.active_profile { "*" } else { " " };
                anstream::println!("{mark} {BOLD}{}{BOLD:#}  {DIM}{}{DIM:#}", p.name, p.description);
            }
        }
        ConfigCmd::Use { profile: name } => {
            let loaded = config::load(&path, None)?;
            if !loaded.profiles.iter().any(|p| p.name == name) {
                bail!("{}: {name}", lang.t("プロファイルがありません", "no such profile"));
            }
            let mut doc = ConfigDoc::open(&path)?;
            doc.set("general.active_profile", name.as_str());
            save_validated(&doc, lang, g.quiet)?;
            if !g.quiet {
                anstream::eprintln!("{OK}✓{OK:#} active_profile = {name}");
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}
