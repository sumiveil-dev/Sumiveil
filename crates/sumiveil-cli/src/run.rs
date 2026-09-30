//! mask / detect / check コマンド。

use std::collections::BTreeMap;
use std::io::{BufRead, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, bail, Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use sumiveil_core::align::align;
use sumiveil_core::encoding;
use sumiveil_core::encoding_rs;
use sumiveil_core::formats::{self, Document, Output, PropertiesMode, WriteOptions};
use sumiveil_core::lang::Lang;
use sumiveil_core::report::{build_report, csv_field, JsonReport, ReportOptions};
use sumiveil_core::text::LineIndex;
use sumiveil_core::{Config, Engine, MaskResult, MaskSession};

use crate::args::{Format, GlobalOpts, MaskArgs};
use crate::util::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Mask,
    Detect,
    Check,
}

enum Source {
    Stdin,
    File(PathBuf),
    Text(String),
    Clipboard,
}

struct Input {
    source: Source,
    display: String,
    /// フォルダ入力時の相対パス (出力フォルダでの配置に使う)
    rel: Option<PathBuf>,
}

impl Input {
    /// 読み込む。ファイルは拡張子で形式を判定する (テキスト・メール・Office 文書・PDF など)。
    fn read(&self, forced: &str) -> Result<Document> {
        Ok(match &self.source {
            Source::Text(t) => formats::from_text(t.clone()),
            Source::Clipboard => formats::from_text(read_clipboard()?),
            Source::Stdin => {
                let mut buf = vec![];
                std::io::stdin().lock().read_to_end(&mut buf).context("stdin")?;
                formats::open(Path::new("stdin.txt"), buf, Some(forced)).map_err(|e| anyhow!(e))?
            }
            Source::File(p) => {
                let bytes = std::fs::read(p).with_context(|| p.display().to_string())?;
                formats::open(p, bytes, Some(forced)).map_err(|e| anyhow!("{}: {e}", p.display()))?
            }
        })
    }
}

fn globset(patterns: &[String]) -> Result<GlobSet> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        b.add(Glob::new(&p.to_lowercase()).with_context(|| format!("glob: {p}"))?);
    }
    Ok(b.build()?)
}

fn collect_inputs(a: &MaskArgs, cfg: &Config, lang: Lang) -> Result<(Vec<Input>, bool)> {
    if let Some(t) = &a.text {
        return Ok((vec![Input { source: Source::Text(t.clone()), display: "<text>".into(), rel: None }], false));
    }
    if a.from_clipboard {
        return Ok((vec![Input { source: Source::Clipboard, display: "<clipboard>".into(), rel: None }], false));
    }
    if a.inputs.is_empty() {
        if std::io::stdin().is_terminal() {
            bail!(
                "{}",
                lang.t(
                    "入力がありません。ファイルを指定するか、パイプで渡すか、-t \"テキスト\" を使ってください (詳細: sumiveil --help)",
                    "no input. Pass files, pipe text in, or use -t \"text\" (see sumiveil --help)"
                )
            );
        }
        return Ok((vec![Input { source: Source::Stdin, display: "<stdin>".into(), rel: None }], false));
    }
    let include = globset(if a.include.is_empty() { &cfg.batch.include } else { &a.include })?;
    let exclude = globset(&[cfg.batch.exclude.clone(), a.exclude.clone()].concat())?;
    let suffix = a.suffix.clone().unwrap_or_else(|| cfg.batch.suffix.clone());
    let out_dir = a.out_dir.as_ref().and_then(|d| d.canonicalize().ok());
    let mut inputs = vec![];
    let mut multi = a.inputs.len() > 1;
    for p in &a.inputs {
        if p.as_os_str() == "-" {
            inputs.push(Input { source: Source::Stdin, display: "<stdin>".into(), rel: None });
        } else if p.is_dir() {
            if !a.recursive {
                bail!("{}: {}", p.display(), lang.t("フォルダです。-r を指定してください", "is a folder; use -r to process folders"));
            }
            multi = true;
            for entry in walkdir::WalkDir::new(p).follow_links(false).into_iter().filter_map(|e| e.ok()) {
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                if let (Some(od), Ok(c)) = (&out_dir, path.canonicalize()) {
                    if c.starts_with(od) {
                        continue; // 出力フォルダ内は処理しない
                    }
                }
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if !include.is_match(&name) || exclude.is_match(&name) {
                    continue;
                }
                // 以前の出力 (report.masked.txt 等) は除外
                let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if !suffix.is_empty() && stem.ends_with(&suffix) {
                    continue;
                }
                let rel = path.strip_prefix(p).ok().map(Path::to_path_buf);
                inputs.push(Input { source: Source::File(path.to_path_buf()), display: path.display().to_string(), rel });
            }
        } else if p.is_file() {
            let rel = p.file_name().map(PathBuf::from);
            inputs.push(Input { source: Source::File(p.clone()), display: p.display().to_string(), rel });
        } else {
            bail!("{}: {}", p.display(), lang.t("見つかりません", "not found"));
        }
    }
    if multi && inputs.iter().any(|i| matches!(i.source, Source::Stdin)) {
        bail!("{}", lang.t("標準入力 (-) は他の入力と同時に指定できません", "stdin (-) cannot be combined with other inputs"));
    }
    Ok((inputs, multi))
}

/// 明示的に指定された出力の文字コード (`--output-encoding`、ファイル入力では設定の batch.output_encoding も)。
fn explicit_output_encoding(a: &MaskArgs, cfg: &Config, input: &Input) -> Result<Option<(&'static encoding_rs::Encoding, bool)>> {
    let label = a.output_encoding.clone().or_else(|| match input.source {
        Source::File(_) if cfg.batch.output_encoding != "same" => Some(cfg.batch.output_encoding.clone()),
        _ => None,
    });
    label.map(|l| encoding::encoding_for_label(&l).with_context(|| format!("unknown encoding: {l}"))).transpose()
}

/// テキストとして出力するときの文字コード。指定がなければ、ファイルは入力と同じ、それ以外は UTF-8。
fn output_encoding(a: &MaskArgs, cfg: &Config, input: &Input, doc: &Document) -> Result<(&'static encoding_rs::Encoding, bool)> {
    Ok(match explicit_output_encoding(a, cfg, input)? {
        Some(e) => e,
        None => match (&input.source, doc.text_encoding()) {
            (Source::File(_), Some(e)) => e,
            _ => (encoding_rs::UTF_8, false),
        },
    })
}

/// Office 文書のプロパティの扱い (`--properties` → 設定の files.properties。"ask" は確認できないので clear)。
/// 2 つ目は「確認の代わりに clear にした」かどうか。
fn properties_mode(a: &MaskArgs, cfg: &Config) -> (PropertiesMode, bool) {
    match a.properties.as_deref().and_then(PropertiesMode::parse) {
        Some(m) => (m, false),
        None => match PropertiesMode::parse(&cfg.files.properties) {
            Some(m) => (m, false),
            None => (PropertiesMode::Clear, true),
        },
    }
}

fn out_path_for(input: &Input, a: &MaskArgs, cfg: &Config, json: bool) -> Result<PathBuf> {
    let Source::File(src) = &input.source else { bail!("no output path for {}", input.display) };
    let suffix = a.suffix.clone().unwrap_or_else(|| cfg.batch.suffix.clone());
    let p = if let Some(dir) = &a.out_dir {
        let rel = input.rel.clone().unwrap_or_else(|| src.file_name().map(PathBuf::from).unwrap_or_default());
        let mut p = dir.join(rel);
        if json {
            let name = format!("{}.json", p.file_name().unwrap_or_default().to_string_lossy());
            p.set_file_name(name);
        }
        p
    } else {
        let stem = src.file_stem().unwrap_or_default().to_string_lossy();
        let ext = if json { "json".to_string() } else { src.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default() };
        let name = if ext.is_empty() { format!("{stem}{suffix}") } else { format!("{stem}{suffix}.{ext}") };
        src.with_file_name(name)
    };
    // PDF / .msg はテキストで書き出す (report.pdf → report.pdf.txt)
    let p = if !json && formats::DocKind::from_path(src).writes_text() {
        let name = formats::text_output_name(&p.file_name().unwrap_or_default().to_string_lossy());
        p.with_file_name(name)
    } else {
        p
    };
    if let (Ok(a), Ok(b)) = (p.canonicalize(), src.canonicalize()) {
        if a == b {
            bail!("refusing to overwrite the input file: {}", src.display());
        }
    }
    Ok(p)
}

fn render_diff(original: &str, r: &MaskResult, color: bool) -> String {
    let rows = align(original, r);
    let li = LineIndex::new(original);
    let ri = LineIndex::new(&r.output);
    let line = |idx: &LineIndex, text: &str, n: usize| -> String {
        let s = idx.line_start(n);
        let e = if n + 1 < idx.line_count() { idx.line_start(n + 1) } else { text.len() };
        text[s..e].trim_end_matches(['\r', '\n']).to_string()
    };
    let mut out = String::new();
    for row in rows {
        let l = row.left.map(|n| line(&li, original, n));
        let rt = row.right.map(|n| line(&ri, &r.output, n));
        if l == rt {
            continue;
        }
        let (del, add, dim) = if color { (DEL.render().to_string(), ADD.render().to_string(), DIM.render().to_string()) } else { Default::default() };
        let reset = if color { anstyle::Reset.render().to_string() } else { String::new() };
        if let (Some(n), Some(t)) = (row.left, &l) {
            out.push_str(&format!("{dim}{:>5}{reset} {del}- {t}{reset}\n", n + 1));
        }
        if let (Some(n), Some(t)) = (row.right, &rt) {
            out.push_str(&format!("{dim}{:>5}{reset} {add}+ {t}{reset}\n", n + 1));
        }
    }
    out
}

fn summary(lang: Lang, counts: &BTreeMap<String, usize>) -> String {
    let total: usize = counts.values().sum();
    let detail: Vec<String> = counts.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    if lang.is_ja() {
        format!("{total} 件検出 ({})", detail.join(", "))
    } else {
        format!("{total} detection(s) ({})", detail.join(", "))
    }
}

pub fn run(g: &GlobalOpts, a: MaskArgs, lang: Lang, mode: Mode) -> Result<ExitCode> {
    let loaded = load_config(g, lang)?;
    let mut cfg = loaded.config.clone();
    apply_overrides(&mut cfg, &a)?;
    let engine = Engine::new(&cfg);
    for w in &engine.warnings {
        warn(lang, g.quiet, w);
    }
    let (inputs, multi) = collect_inputs(&a, &cfg, lang)?;
    if multi && a.output.is_some() {
        bail!("{}", lang.t("入力が複数のときは -o ではなく --out-dir を使ってください", "use --out-dir instead of -o for multiple inputs"));
    }
    let json = a.format == Format::Json;
    let (props_mode, props_defaulted) = properties_mode(&a, &cfg);
    let mut props_notice_shown = false;
    let report_opts = |display: &str, doc: &Document, include_masked: bool| ReportOptions {
        source: Some(display.to_string()),
        encoding: Some(doc.encoding_name.clone()),
        profile: &loaded.active_profile,
        include_original: a.include_original,
        include_masked,
        japanese_names: lang.is_ja(),
    };

    // tail -f 用の行単位処理
    if mode == Mode::Mask && a.line_buffered && !json && !multi && matches!(inputs[0].source, Source::Stdin) {
        let mut session = MaskSession::new();
        let stdin = std::io::stdin();
        let mut buf = Vec::new();
        let mut reader = stdin.lock();
        loop {
            buf.clear();
            if reader.read_until(b'\n', &mut buf).context("stdin")? == 0 {
                break;
            }
            let line = encoding::decode(&buf, Some(&a.encoding)).text;
            let r = engine.mask(&line, &mut session);
            write_stdout(r.output.as_bytes())?;
        }
        return Ok(ExitCode::SUCCESS);
    }

    let mut session = MaskSession::new();
    let mut total_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut reports: Vec<JsonReport> = vec![];
    let mut json_outputs: Vec<JsonReport> = vec![];
    let mut clipboard_buf = String::new();
    let stdout_color = std::io::stdout().is_terminal() && g.color != crate::args::ColorWhen::Never || g.color == crate::args::ColorWhen::Always;

    for input in &inputs {
        let doc = match input.read(&a.encoding) {
            Ok(d) => d,
            Err(e) if multi => {
                warn(lang, g.quiet, &format!("{e:#}"));
                continue;
            }
            Err(e) => return Err(e),
        };
        for w in &doc.warnings {
            warn(lang, g.quiet, &format!("{}: {w}", input.display));
        }
        if a.separate_numbering {
            session.reset();
        }
        let result = engine.mask(&doc.text, &mut session);
        for (k, v) in result.counts() {
            *total_counts.entry(k).or_insert(0) += v;
        }
        if a.report.is_some() {
            let mut rep = build_report(&doc.text, &result, &report_opts(&input.display, &doc, false));
            formats::annotate_report(&mut rep, &doc, &result, None);
            reports.push(rep);
        }

        match mode {
            Mode::Mask => {
                let to_stdout = !multi && a.output.is_none();
                let body: Vec<u8> = if json {
                    let mut rep = build_report(&doc.text, &result, &report_opts(&input.display, &doc, !a.no_masked));
                    formats::annotate_report(&mut rep, &doc, &result, None);
                    if to_stdout {
                        json_outputs.push(rep);
                        vec![]
                    } else {
                        let mut s = serde_json::to_string_pretty(&rep)?;
                        s.push('\n');
                        s.into_bytes()
                    }
                } else if a.diff {
                    render_diff(&doc.text, &result, to_stdout && stdout_color).into_bytes()
                } else if !doc.kind.is_structured() || to_stdout {
                    // テキストファイル、または Office 文書などを画面 (標準出力) に出すとき: マスクしたテキスト
                    let (enc, bom) = output_encoding(&a, &cfg, input, &doc)?;
                    encoding::encode(&result.output, enc, bom)
                } else {
                    // メール・Office 文書は元の形式で、PDF / .msg はテキストで書き出す
                    let text_encoding = explicit_output_encoding(&a, &cfg, input)?;
                    let mut opts = WriteOptions { engine: &engine, session: &mut session, properties: props_mode, text_encoding };
                    let (out, wr) = formats::write(&doc, &result, &mut opts).map_err(|e| anyhow!("{}: {e}", input.display))?;
                    for w in &wr.warnings {
                        warn(lang, g.quiet, &format!("{}: {w}", input.display));
                    }
                    if wr.properties.is_some() && props_defaulted && !props_notice_shown {
                        props_notice_shown = true;
                        warn(
                            lang,
                            g.quiet,
                            lang.t(
                                "Office 文書のプロパティ (作成者など) を消去しました (--properties mask|clear|keep、または設定の files.properties で変更できます)",
                                "cleared Office document properties such as author (change with --properties mask|clear|keep or files.properties)",
                            ),
                        );
                    }
                    match out {
                        Output::Bytes(b) => b,
                        Output::Text(t) => match text_encoding {
                            Some((enc, bom)) => encoding::encode(&t, enc, bom),
                            None => t.into_bytes(),
                        },
                    }
                };
                if a.to_clipboard {
                    clipboard_buf.push_str(&result.output);
                }
                if multi {
                    let out = out_path_for(input, &a, &cfg, json)?;
                    write_file(&out, &body)?;
                    if !g.quiet {
                        let n: usize = result.replacements.len();
                        anstream::eprintln!("{OK}✓{OK:#} {} → {} {DIM}({n}){DIM:#}", input.display, out.display());
                    }
                } else if let Some(out) = &a.output {
                    if let Source::File(src) = &input.source {
                        if let (Ok(x), Ok(y)) = (out.canonicalize(), src.canonicalize()) {
                            if x == y {
                                bail!("refusing to overwrite the input file: {}", src.display());
                            }
                        }
                    }
                    write_file(out, &body)?;
                } else if !json {
                    write_stdout(&body)?;
                }
            }
            Mode::Detect | Mode::Check => {
                if json {
                    let mut rep = build_report(&doc.text, &result, &report_opts(&input.display, &doc, false));
                    formats::annotate_report(&mut rep, &doc, &result, None);
                    json_outputs.push(rep);
                } else {
                    let idx = LineIndex::new(&doc.text);
                    let mut out = anstream::stdout().lock();
                    for r in &result.replacements {
                        // Office 文書などは取り出したテキストの行ではなく、セル・ページなどの場所を示す
                        let place = match doc.location_at(r.start).filter(|_| doc.kind.is_structured()) {
                            Some(loc) => format!("[{loc}]"),
                            None => {
                                let (line, col) = idx.line_col(&doc.text, r.start);
                                format!("{line}:{col}")
                            }
                        };
                        let name = if lang.is_ja() { &r.meta.name_ja } else { &r.meta.name_en };
                        let value = if a.show_values { format!("  {BOLD}{}{BOLD:#}", r.original.replace('\n', "\\n")) } else { String::new() };
                        let res = writeln!(
                            out,
                            "{}:{place}: {WARN}{}{WARN:#} {DIM}[{name}] {:.2}{DIM:#}{value}",
                            input.display, r.meta.id, r.confidence
                        );
                        if let Err(e) = res {
                            if e.kind() == std::io::ErrorKind::BrokenPipe {
                                std::process::exit(0);
                            }
                            return Err(e.into());
                        }
                    }
                }
            }
        }
    }

    // JSON を標準出力へ (単一入力なら 1 オブジェクト、複数なら配列)
    if !json_outputs.is_empty() || (json && mode != Mode::Mask && !multi) {
        let s = if json_outputs.len() == 1 && !multi {
            serde_json::to_string_pretty(&json_outputs[0])?
        } else {
            serde_json::to_string_pretty(&json_outputs)?
        };
        match (&a.output, mode) {
            (Some(p), Mode::Mask) => write_file(p, format!("{s}\n").as_bytes())?,
            _ => write_stdout(format!("{s}\n").as_bytes())?,
        }
    }

    if a.to_clipboard && mode == Mode::Mask {
        copy_to_clipboard(&clipboard_buf)?;
        if !g.quiet {
            anstream::eprintln!("{OK}✓{OK:#} {}", lang.t("クリップボードにコピーしました", "copied to clipboard"));
        }
    }

    if let Some(rp) = &a.report {
        let is_csv = rp.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv"));
        let body = if is_csv {
            let mut s = String::from("\u{feff}file,line,column,id,category,name,confidence,replacement");
            if a.include_original {
                s.push_str(",original");
            }
            s.push_str("\r\n");
            for rep in &reports {
                for d in &rep.detections {
                    let mut row = vec![
                        csv_field(rep.source.as_deref().unwrap_or("")),
                        d.line.to_string(),
                        d.column.to_string(),
                        csv_field(&d.id),
                        csv_field(&d.category),
                        csv_field(&d.name),
                        format!("{:.2}", d.confidence),
                        csv_field(&d.replacement),
                    ];
                    if let Some(o) = &d.original {
                        row.push(csv_field(o));
                    }
                    s.push_str(&row.join(","));
                    s.push_str("\r\n");
                }
            }
            s
        } else {
            serde_json::to_string_pretty(&reports)? + "\n"
        };
        write_file(rp, body.as_bytes())?;
    }

    let total: usize = total_counts.values().sum();
    match mode {
        Mode::Mask => {
            if a.stats && !g.quiet {
                anstream::eprintln!("{}", summary(lang, &total_counts));
            }
            Ok(ExitCode::SUCCESS)
        }
        Mode::Detect => {
            if !g.quiet && !json {
                anstream::eprintln!("{DIM}{}{DIM:#}", summary(lang, &total_counts));
            }
            Ok(ExitCode::SUCCESS)
        }
        Mode::Check => {
            if total > 0 {
                if !g.quiet {
                    anstream::eprintln!("{ERR}✗{ERR:#} {}", summary(lang, &total_counts));
                }
                Ok(ExitCode::from(1))
            } else {
                if !g.quiet {
                    anstream::eprintln!("{OK}✓{OK:#} {}", lang.t("機密情報は見つかりませんでした", "no sensitive information found"));
                }
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}
