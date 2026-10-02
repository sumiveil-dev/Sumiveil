//! 検出・重なり解決・マスキングを行うエンジン。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use aho_corasick::{AhoCorasickBuilder, MatchKind};
use rayon::prelude::*;
use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

use crate::catalog::{self, Boundary, SpecKind, CATALOG};
use crate::config::Config;
use crate::custom::{CustomDetector, KeywordDetector};
use crate::detector::{boundary_ok, Detect, RawMatch, RegexDetector};
use crate::dict::{NameDict, PlaceDetector};
use crate::names::NameDetector;
use crate::template::{RenderCtx, Template};

/// 検出器のメタデータ (出力や表示に使う)。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DetectorMeta {
    pub id: String,
    pub category: String,
    pub label: String,
    pub label_ja: String,
    pub name_en: String,
    pub name_ja: String,
    pub priority: i32,
}

/// 確定した検出結果 (元テキストのバイト位置)。
#[derive(Debug, Clone)]
pub struct Detection {
    pub start: usize,
    pub end: usize,
    pub confidence: f32,
    pub meta: Arc<DetectorMeta>,
    det_index: usize,
}

/// 置換結果 1 件。
#[derive(Debug, Clone)]
pub struct Replacement {
    /// 元テキストのバイト位置
    pub start: usize,
    pub end: usize,
    /// 出力テキストのバイト位置
    pub out_start: usize,
    pub out_end: usize,
    pub confidence: f32,
    pub meta: Arc<DetectorMeta>,
    pub original: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Default)]
pub struct MaskResult {
    pub output: String,
    pub replacements: Vec<Replacement>,
}

impl MaskResult {
    /// 検出器 ID ごとの件数。
    pub fn counts(&self) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for r in &self.replacements {
            *m.entry(r.meta.id.clone()).or_insert(0) += 1;
        }
        m
    }
}

/// 連番 `{n}` の採番状態。複数ファイルで同じ番号を使いたい場合は使い回す。
#[derive(Debug, Clone, Default)]
pub struct MaskSession {
    /// ラベル → (正規化した値 → 番号)。番号はラベルごとに 1 から振る
    map: HashMap<String, HashMap<String, usize>>,
}

impl MaskSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn number(&mut self, label: &str, value: &str) -> usize {
        let key = normalize_value(value);
        if !self.map.contains_key(label) {
            self.map.insert(label.to_string(), HashMap::new());
        }
        let numbers = self.map.get_mut(label).expect("inserted above");
        if let Some(&n) = numbers.get(&key) {
            return n;
        }
        let n = numbers.len() + 1;
        numbers.insert(key, n);
        n
    }

    pub fn reset(&mut self) {
        self.map.clear();
    }
}

/// 採番用に値を正規化 (表記ゆれを同一視する)。
fn normalize_value(v: &str) -> String {
    // ASCII だけの値は NFKC で変わらないので、変換の文字列を作らない
    let nfkc;
    let n: &str = if v.is_ascii() {
        v
    } else {
        nfkc = v.nfkc().collect::<String>();
        &nfkc
    };
    let digits = n.chars().filter(|c| c.is_ascii_digit()).count();
    let alnum = n.chars().filter(|c| c.is_alphanumeric()).count();
    if digits >= 6 && digits * 10 >= alnum * 8 {
        // 電話番号・カード番号等は数字だけで比較
        n.chars().filter(|c| c.is_ascii_digit()).collect()
    } else {
        n.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect()
    }
}

struct Active {
    meta: Arc<DetectorMeta>,
    det: Box<dyn Detect>,
    min_conf: f32,
    template: Template,
}

struct Allow {
    values: HashSet<String>,
    patterns: Vec<regex::Regex>,
    domains: Vec<String>,
}

impl Allow {
    fn allows(&self, value: &str, meta: &DetectorMeta, extra: Option<&HashSet<String>>) -> bool {
        let by_value = !self.values.is_empty() || extra.is_some_and(|x| !x.is_empty());
        let by_domain = !self.domains.is_empty() && matches!(meta.id.as_str(), "email" | "hostname" | "url" | "unc_path");
        if self.patterns.iter().any(|p| p.is_match(value)) {
            return true;
        }
        // 小文字化 (文字列の確保) は、比べる対象があるときだけ
        if !by_value && !by_domain {
            return false;
        }
        let l = value.to_lowercase();
        if by_value && (self.values.contains(&l) || extra.is_some_and(|x| x.contains(&l))) {
            return true;
        }
        if by_domain {
            let host = extract_host(&l);
            return self.domains.iter().any(|d| host == *d || host.strip_suffix(d.as_str()).is_some_and(|h| h.ends_with('.')));
        }
        false
    }
}

fn extract_host(l: &str) -> &str {
    let s = match l.rfind('@') {
        Some(i) => &l[i + 1..],
        None => l,
    };
    let s = match s.find("://") {
        Some(i) => &s[i + 3..],
        None => s,
    };
    let end = s.find(['/', ':', '?', '#']).unwrap_or(s.len());
    &s[..end]
}

pub struct Engine {
    active: Vec<Active>,
    allow: Allow,
    propagate: bool,
    names_dict: Option<Arc<NameDict>>,
    hash_key: Vec<u8>,
    /// 設定の問題 (不正なテンプレート・正規表現など)。該当項目は無視して動作する。
    pub warnings: Vec<String>,
}

fn meta_from_spec(spec: &catalog::DetectorSpec, priority: i32) -> DetectorMeta {
    DetectorMeta {
        id: spec.id.into(),
        category: spec.category.into(),
        label: spec.label.into(),
        label_ja: spec.label_ja.into(),
        name_en: spec.name_en.into(),
        name_ja: spec.name_ja.into(),
        priority,
    }
}

impl Engine {
    pub fn new(cfg: &Config) -> Engine {
        let mut warnings = vec![];
        let parse_tpl = |src: &str, fallback: &Template, warnings: &mut Vec<String>, what: &str| match Template::parse(src) {
            Ok(t) => t,
            Err(e) => {
                warnings.push(format!("{what} のテンプレートが不正なため既定値を使います: {e}"));
                fallback.clone()
            }
        };
        let builtin_default = Template::parse("<{label}_{n}>").unwrap();
        let default_tpl = parse_tpl(&cfg.masking.template, &builtin_default, &mut warnings, "masking.template");
        let names_dict = cfg.names.use_dictionary.then(NameDict::get);
        let morph = if cfg.names.use_morphology {
            match crate::morph::load_configured(&cfg.names.morphology_dict) {
                Ok(m) => Some(m),
                Err(e) => {
                    warnings.push(e);
                    None
                }
            }
        } else {
            None
        };

        let mut active = vec![];
        for (i, spec) in CATALOG.iter().enumerate() {
            if !cfg.detector_enabled(spec.id) {
                continue;
            }
            let dc = cfg.detectors.get(spec.id);
            let det: Box<dyn Detect> = match spec.kind {
                SpecKind::Regex(_) => Box::new(RegexDetector::from_catalog(i).unwrap()),
                SpecKind::PersonName => Box::new(NameDetector::new(names_dict.clone(), cfg.names.dictionary_threshold).with_morphology(morph.clone())),
                SpecKind::PlaceName => Box::new(PlaceDetector::new().with_morphology(morph.clone())),
                SpecKind::Company => Box::new(crate::company::CompanyDetector::new(names_dict.clone(), morph.clone())),
            };
            let tpl_src = dc
                .and_then(|d| d.template.clone())
                .or_else(|| cfg.categories.get(spec.category).and_then(|c| c.template.clone()));
            let template = match tpl_src {
                Some(s) => parse_tpl(&s, &default_tpl, &mut warnings, &format!("detectors.{}", spec.id)),
                None => default_tpl.clone(),
            };
            let priority = dc.and_then(|d| d.priority).unwrap_or(spec.priority);
            active.push(Active {
                meta: Arc::new(meta_from_spec(spec, priority)),
                det,
                min_conf: dc.and_then(|d| d.min_confidence).unwrap_or(cfg.masking.min_confidence),
                template,
            });
        }

        for rule in &cfg.custom_rules {
            if !rule.enabled || !cfg.category_enabled(&rule.category) {
                continue;
            }
            match CustomDetector::new(rule) {
                Ok(det) => {
                    let tpl = rule
                        .template
                        .as_deref()
                        .or(cfg.categories.get(&rule.category).and_then(|c| c.template.as_deref()))
                        .map(|s| parse_tpl(s, &default_tpl, &mut warnings, &format!("custom_rules.{}", rule.id)))
                        .unwrap_or_else(|| default_tpl.clone());
                    let name = if rule.name.is_empty() { rule.id.clone() } else { rule.name.clone() };
                    active.push(Active {
                        meta: Arc::new(DetectorMeta {
                            id: format!("custom:{}", rule.id),
                            category: rule.category.clone(),
                            label: rule.label.clone(),
                            label_ja: rule.label.clone(),
                            name_en: name.clone(),
                            name_ja: name,
                            priority: rule.priority,
                        }),
                        det: Box::new(det),
                        min_conf: 0.0,
                        template: tpl,
                    });
                }
                Err(e) => warnings.push(format!("カスタムルール '{}' の正規表現が不正なため無視します: {e}", rule.id)),
            }
        }

        if cfg.category_enabled("custom") {
            for (gi, group) in cfg.keywords.iter().enumerate() {
                if !group.enabled {
                    continue;
                }
                let Some(det) = KeywordDetector::new(group) else { continue };
                let tpl = group
                    .template
                    .as_deref()
                    .map(|s| parse_tpl(s, &default_tpl, &mut warnings, &format!("keywords[{gi}]")))
                    .unwrap_or_else(|| default_tpl.clone());
                let name = if group.name.is_empty() { group.label.clone() } else { group.name.clone() };
                active.push(Active {
                    meta: Arc::new(DetectorMeta {
                        id: format!("keyword:{}", group.label),
                        category: "custom".into(),
                        label: group.label.clone(),
                        label_ja: group.label.clone(),
                        name_en: name.clone(),
                        name_ja: name,
                        priority: 97,
                    }),
                    det: Box::new(det),
                    min_conf: 0.0,
                    template: tpl,
                });
            }
        }

        let mut patterns = vec![];
        for p in &cfg.allowlist.patterns {
            match regex::Regex::new(&format!("^(?:{p})$")) {
                Ok(r) => patterns.push(r),
                Err(e) => warnings.push(format!("allowlist.patterns '{p}' が不正なため無視します: {e}")),
            }
        }
        let allow = Allow {
            values: cfg.allowlist.values.iter().map(|v| v.to_lowercase()).collect(),
            patterns,
            domains: cfg.allowlist.domains.iter().map(|d| d.trim().trim_start_matches('.').to_lowercase()).filter(|d| !d.is_empty()).collect(),
        };

        Engine {
            active,
            allow,
            propagate: cfg.names.propagate,
            names_dict,
            hash_key: cfg.masking.hash_salt.as_bytes().to_vec(),
            warnings,
        }
    }

    /// 有効な検出器の一覧。
    pub fn active_detectors(&self) -> Vec<Arc<DetectorMeta>> {
        self.active.iter().map(|a| a.meta.clone()).collect()
    }

    pub fn detect(&self, text: &str) -> Vec<Detection> {
        self.detect_excluding(text, None)
    }

    /// `extra_allow` (小文字) に含まれる値はマスクしない (GUI の一時除外用)。
    pub fn detect_excluding(&self, text: &str, extra_allow: Option<&HashSet<String>>) -> Vec<Detection> {
        let raw: Vec<Detection> = self
            .active
            .par_iter()
            .enumerate()
            .flat_map_iter(|(i, a)| {
                let mut v: Vec<RawMatch> = vec![];
                a.det.detect(text, &mut v);
                v.into_iter().filter(move |m| m.confidence >= a.min_conf && m.start < m.end).map(move |m| Detection {
                    start: m.start,
                    end: m.end,
                    confidence: m.confidence,
                    meta: a.meta.clone(),
                    det_index: i,
                })
            })
            .filter(|d| !self.allow.allows(&text[d.start..d.end], &d.meta, extra_allow))
            .collect();
        let mut resolved = resolve_overlaps(raw);
        if self.propagate {
            let extra = self.propagate_names(text, &resolved, extra_allow);
            if !extra.is_empty() {
                resolved.extend(extra);
                resolved.sort_by_key(|d| d.start);
            }
        }
        resolved
    }

    /// 確度の高い人名を文中の他の出現箇所にも適用する。
    fn propagate_names(&self, text: &str, found: &[Detection], extra_allow: Option<&HashSet<String>>) -> Vec<Detection> {
        let Some(src) = found.iter().find(|d| d.meta.id == "person_name") else {
            return vec![];
        };
        let det_index = src.det_index;
        let meta = src.meta.clone();
        let mut values: HashSet<String> = HashSet::new();
        for d in found.iter().filter(|d| d.meta.id == "person_name" && d.confidence >= 0.7) {
            let v = text[d.start..d.end].trim();
            values.insert(v.to_string());
            let parts: Vec<&str> = v.split([' ', '　']).filter(|p| !p.is_empty()).collect();
            if parts.len() > 1 {
                for p in parts {
                    let ascii = p.is_ascii();
                    if (ascii && p.len() >= 3) || (!ascii && p.chars().count() >= 2) {
                        values.insert(p.to_string());
                    }
                }
            } else if let Some(dict) = &self.names_dict {
                // 「山田太郎」→「山田」も対象に
                let idx: Vec<usize> = v.char_indices().map(|x| x.0).skip(2).collect();
                for &cut in &idx {
                    // 辞書にない珍しい姓 (「月見里花子さん」の「月見里」) も、名が 2 文字以上なら姓として扱う
                    let (sur, giv) = (&v[..cut], &v[cut..]);
                    let sur_ok = dict.is_jp_surname(sur) || (giv.chars().count() >= 2 && dict.is_surname_like(sur));
                    if sur_ok && dict.is_jp_given(giv) {
                        values.insert(v[..cut].to_string());
                        break;
                    }
                }
            }
        }
        if values.is_empty() {
            return vec![];
        }
        let words: Vec<String> = values.into_iter().collect();
        let Ok(ac) = AhoCorasickBuilder::new().match_kind(MatchKind::LeftmostLongest).build(&words) else {
            return vec![];
        };
        let mut out = vec![];
        let mut fi = 0;
        for m in ac.find_iter(text) {
            // 既存の検出と重なるものは除外
            while fi < found.len() && found[fi].end <= m.start() {
                fi += 1;
            }
            if fi < found.len() && found[fi].start < m.end() {
                continue;
            }
            let w = &text[m.start()..m.end()];
            if w.is_ascii() && !boundary_ok(text, m.start(), m.end(), Boundary::Alnum) {
                continue;
            }
            if self.allow.allows(w, &meta, extra_allow) {
                continue;
            }
            out.push(Detection { start: m.start(), end: m.end(), confidence: 0.7, meta: meta.clone(), det_index });
        }
        out
    }

    pub fn mask(&self, text: &str, session: &mut MaskSession) -> MaskResult {
        let dets = self.detect(text);
        self.apply(text, &dets, session)
    }

    /// 検出結果 (開始位置順・重なりなし) を使って置換する。
    pub fn apply(&self, text: &str, dets: &[Detection], session: &mut MaskSession) -> MaskResult {
        let mut output = String::with_capacity(text.len() + dets.len() * 8);
        let mut replacements = Vec::with_capacity(dets.len());
        let mut last = 0;
        for d in dets {
            if d.start < last || d.end > text.len() {
                continue;
            }
            output.push_str(&text[last..d.start]);
            let original = &text[d.start..d.end];
            let a = &self.active[d.det_index];
            let n = if a.template.uses_number() { session.number(&a.meta.label, original) } else { 0 };
            let cat = catalog::category(&a.meta.category);
            let replacement = a.template.render(&RenderCtx {
                original,
                id: &a.meta.id,
                label: &a.meta.label,
                label_ja: &a.meta.label_ja,
                category: &a.meta.category,
                category_ja: cat.map(|c| c.name_ja).unwrap_or(&a.meta.category),
                n,
                hash_key: &self.hash_key,
            });
            let out_start = output.len();
            output.push_str(&replacement);
            replacements.push(Replacement {
                start: d.start,
                end: d.end,
                out_start,
                out_end: output.len(),
                confidence: d.confidence,
                meta: a.meta.clone(),
                original: original.to_string(),
                replacement,
            });
            last = d.end;
        }
        output.push_str(&text[last..]);
        MaskResult { output, replacements }
    }
}

/// 優先度 → 信頼度 → 長さ → 位置 の順で採用し、重なる候補を捨てる。結果は開始位置順。
fn resolve_overlaps(mut v: Vec<Detection>) -> Vec<Detection> {
    v.sort_by(|a, b| {
        b.meta
            .priority
            .cmp(&a.meta.priority)
            .then(b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal))
            .then((b.end - b.start).cmp(&(a.end - a.start)))
            .then(a.start.cmp(&b.start))
    });
    let mut chosen: BTreeMap<usize, Detection> = BTreeMap::new();
    for d in v {
        let overlaps_prev = chosen.range(..d.end).next_back().is_some_and(|(_, p)| p.end > d.start);
        if !overlaps_prev {
            chosen.insert(d.start, d);
        }
    }
    chosen.into_values().collect()
}

/// 1 回きりのマスク処理の簡易関数。
pub fn mask_text(cfg: &Config, text: &str) -> MaskResult {
    Engine::new(cfg).mask(text, &mut MaskSession::new())
}

#[cfg(test)]
#[path = "tests/engine.rs"]
mod tests;
