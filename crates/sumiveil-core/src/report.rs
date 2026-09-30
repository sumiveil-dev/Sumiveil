//! JSON 出力・レポートの共通形式 (CLI と GUI で共有)。

use std::collections::BTreeMap;

use serde::Serialize;

use crate::engine::MaskResult;
use crate::text::LineIndex;

#[derive(Debug, Clone, Serialize)]
pub struct JsonDetection {
    pub id: String,
    pub category: String,
    pub label: String,
    pub name: String,
    /// 1 始まりの行・桁 (文字単位)
    pub line: usize,
    pub column: usize,
    /// 元テキストのバイト位置
    pub start: usize,
    pub end: usize,
    pub confidence: f32,
    pub replacement: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original: Option<String>,
    /// 場所 (Office 文書のセル・メールのヘッダー・PDF のページなど。テキストファイルでは省略)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonStats {
    pub total: usize,
    pub by_detector: BTreeMap<String, usize>,
    pub by_category: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonReport {
    pub tool: &'static str,
    pub version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    pub profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masked: Option<String>,
    pub detections: Vec<JsonDetection>,
    pub stats: JsonStats,
    /// Office 文書のプロパティ (作成者など) の扱い: "masked" / "cleared" / "kept"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<String>,
    /// 利用者に知らせること (取り除いた添付ファイル・変更履歴など)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

pub struct ReportOptions<'a> {
    pub source: Option<String>,
    pub encoding: Option<String>,
    pub profile: &'a str,
    pub include_original: bool,
    pub include_masked: bool,
    pub japanese_names: bool,
}

pub fn build_report(original_text: &str, result: &MaskResult, opt: &ReportOptions) -> JsonReport {
    let idx = LineIndex::new(original_text);
    let mut by_detector = BTreeMap::new();
    let mut by_category = BTreeMap::new();
    let detections = result
        .replacements
        .iter()
        .map(|r| {
            *by_detector.entry(r.meta.id.clone()).or_insert(0) += 1;
            *by_category.entry(r.meta.category.clone()).or_insert(0) += 1;
            let (line, column) = idx.line_col(original_text, r.start);
            JsonDetection {
                id: r.meta.id.clone(),
                category: r.meta.category.clone(),
                label: r.meta.label.clone(),
                name: if opt.japanese_names { r.meta.name_ja.clone() } else { r.meta.name_en.clone() },
                line,
                column,
                start: r.start,
                end: r.end,
                confidence: (r.confidence * 100.0).round() / 100.0,
                replacement: r.replacement.clone(),
                original: opt.include_original.then(|| r.original.clone()),
                location: None,
            }
        })
        .collect::<Vec<_>>();
    JsonReport {
        tool: "sumiveil",
        version: crate::VERSION,
        source: opt.source.clone(),
        encoding: opt.encoding.clone(),
        profile: opt.profile.to_string(),
        masked: opt.include_masked.then(|| result.output.clone()),
        stats: JsonStats { total: detections.len(), by_detector, by_category },
        detections,
        properties: None,
        warnings: vec![],
    }
}

/// CSV の 1 フィールドをエスケープする。
pub fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
