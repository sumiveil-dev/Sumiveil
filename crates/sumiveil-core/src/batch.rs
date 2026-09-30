//! フォルダ一括処理の共通部品 (ファイル列挙・1 ファイルの処理)。

use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::encoding;
use crate::engine::{Engine, MaskResult, MaskSession};
use crate::formats::{self, DocKind, Document, Output, PropertiesMode, WriteOptions, WriteReport};

#[derive(Debug, Clone)]
pub struct BatchFile {
    pub path: PathBuf,
    /// 入力フォルダからの相対パス
    pub rel: PathBuf,
}

pub fn globset(patterns: &[String]) -> Result<GlobSet, String> {
    let mut b = GlobSetBuilder::new();
    for p in patterns.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
        b.add(Glob::new(&p.to_lowercase()).map_err(|e| format!("{p}: {e}"))?);
    }
    b.build().map_err(|e| e.to_string())
}

/// 以前の出力 (report.masked.txt・report.masked.pdf.txt など) か。
fn is_previous_output(path: &Path, suffix: &str) -> bool {
    if suffix.is_empty() {
        return false;
    }
    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    stem.ends_with(suffix) || name.contains(&format!("{suffix}."))
}

/// 対象ファイルを列挙する。`skip_suffix` で終わるファイル名 (以前の出力) と `out_dir` 配下は除外。
pub fn collect_files(
    root: &Path,
    recursive: bool,
    include: &[String],
    exclude: &[String],
    skip_suffix: &str,
    out_dir: Option<&Path>,
) -> Result<Vec<BatchFile>, String> {
    let inc = globset(include)?;
    let exc = globset(exclude)?;
    let out_dir = out_dir.and_then(|d| d.canonicalize().ok());
    let mut files = vec![];
    let walker = walkdir::WalkDir::new(root).follow_links(false).max_depth(if recursive { usize::MAX } else { 1 });
    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if let (Some(od), Ok(c)) = (&out_dir, path.canonicalize()) {
            if c.starts_with(od) {
                continue;
            }
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if (!include.is_empty() && !inc.is_match(&name)) || exc.is_match(&name) {
            continue;
        }
        if is_previous_output(path, skip_suffix) {
            continue;
        }
        let rel = path.strip_prefix(root).map(Path::to_path_buf).unwrap_or_else(|_| PathBuf::from(entry.file_name()));
        files.push(BatchFile { path: path.to_path_buf(), rel });
    }
    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(files)
}

/// 出力先パス。`out_dir` があればその下に相対パスで、なければ入力の横に接尾辞付きで。
/// PDF / .msg はテキストで書き出すため `.txt` を付ける (`report.pdf` → `report.pdf.txt` / `report.masked.pdf.txt`)。
pub fn output_path(file: &BatchFile, out_dir: Option<&Path>, suffix: &str) -> PathBuf {
    let text_out = DocKind::from_path(&file.path).writes_text();
    let p = match out_dir {
        Some(d) => d.join(&file.rel),
        None => {
            let stem = file.path.file_stem().unwrap_or_default().to_string_lossy();
            let name = match file.path.extension() {
                Some(ext) => format!("{stem}{suffix}.{}", ext.to_string_lossy()),
                None => format!("{stem}{suffix}"),
            };
            file.path.with_file_name(name)
        }
    };
    if text_out {
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        p.with_file_name(formats::text_output_name(&name))
    } else {
        p
    }
}

pub struct FileOutcome {
    pub doc: Document,
    pub result: MaskResult,
    pub write: WriteReport,
    pub out_path: PathBuf,
}

impl FileOutcome {
    pub fn text(&self) -> &str {
        &self.doc.text
    }

    pub fn encoding(&self) -> &str {
        &self.doc.encoding_name
    }
}

/// 1 ファイルを読み込み・マスクし・書き出す。`output_encoding` は "same" またはラベル (テキスト形式のみ)。
pub fn process_file(
    engine: &Engine,
    session: &mut MaskSession,
    file: &BatchFile,
    out_path: &Path,
    output_encoding: &str,
    properties: PropertiesMode,
) -> Result<FileOutcome, String> {
    let bytes = std::fs::read(&file.path).map_err(|e| format!("{}: {e}", file.path.display()))?;
    let doc = formats::open(&file.path, bytes, None).map_err(|e| format!("{}: {e}", file.path.display()))?;
    let result = engine.mask(&doc.text, session);
    let text_encoding = if output_encoding.eq_ignore_ascii_case("same") {
        None
    } else {
        Some(encoding::encoding_for_label(output_encoding).ok_or_else(|| format!("unknown encoding: {output_encoding}"))?)
    };
    if let (Ok(a), Ok(b)) = (out_path.canonicalize(), file.path.canonicalize()) {
        if a == b {
            return Err(format!("refusing to overwrite the input file: {}", file.path.display()));
        }
    }
    let mut opts = WriteOptions { engine, session, properties, text_encoding };
    let (out, write) = formats::write(&doc, &result, &mut opts).map_err(|e| format!("{}: {e}", file.path.display()))?;
    let bytes = match out {
        Output::Bytes(b) => b,
        Output::Text(t) => match text_encoding {
            Some((enc, bom)) => encoding::encode(&t, enc, bom),
            None => t.into_bytes(),
        },
    };
    if let Some(dir) = out_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(out_path, bytes).map_err(|e| format!("{}: {e}", out_path.display()))?;
    Ok(FileOutcome { doc, result, write, out_path: out_path.to_path_buf() })
}
