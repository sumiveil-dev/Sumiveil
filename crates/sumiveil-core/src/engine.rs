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
    map: HashMap<(String, String), usize>,
    counters: HashMap<String, usize>,
}

impl MaskSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn number(&mut self, label: &str, value: &str) -> usize {
        let key = (label.to_string(), normalize_value(value));
        if let Some(&n) = self.map.get(&key) {
            return n;
        }
        let c = self.counters.entry(label.to_string()).or_insert(0);
        *c += 1;
        self.map.insert(key, *c);
        *c
    }

    pub fn reset(&mut self) {
        self.map.clear();
        self.counters.clear();
    }
}

/// 採番用に値を正規化 (表記ゆれを同一視する)。
fn normalize_value(v: &str) -> String {
    let n: String = v.nfkc().collect::<String>().to_lowercase();
    let digits = n.chars().filter(|c| c.is_ascii_digit()).count();
    let alnum = n.chars().filter(|c| c.is_alphanumeric()).count();
    if digits >= 6 && digits * 10 >= alnum * 8 {
        // 電話番号・カード番号等は数字だけで比較
        n.chars().filter(|c| c.is_ascii_digit()).collect()
    } else {
        n.chars().filter(|c| !c.is_whitespace()).collect()
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
        let l = value.to_lowercase();
        if self.values.contains(&l) || extra.is_some_and(|x| x.contains(&l)) {
            return true;
        }
        if self.patterns.iter().any(|p| p.is_match(value)) {
            return true;
        }
        if matches!(meta.id.as_str(), "email" | "hostname" | "url" | "unc_path") && !self.domains.is_empty() {
            let host = extract_host(&l);
            return self.domains.iter().any(|d| host == *d || host.ends_with(&format!(".{d}")));
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
mod tests {
    use super::*;
    use crate::config::{load_str, AllowlistConfig, CustomRule, KeywordGroup};

    fn cfg(s: &str) -> Config {
        load_str(s, None).unwrap().config
    }

    #[test]
    fn basic_masking() {
        let r = mask_text(&Config::default(), "連絡先: taro@corp.co.jp / 090-1234-5678");
        assert_eq!(r.output, "連絡先: <EMAIL_1> / <PHONE_1>");
        assert_eq!(r.replacements.len(), 2);
        assert_eq!(&r.output[r.replacements[0].out_start..r.replacements[0].out_end], "<EMAIL_1>");
    }

    #[test]
    fn consistent_numbering() {
        let r = mask_text(&Config::default(), "a@example1.co.jp b@example1.co.jp A@EXAMPLE1.CO.JP 090-1111-2222 09011112222");
        assert_eq!(r.output, "<EMAIL_1> <EMAIL_2> <EMAIL_1> <PHONE_1> <PHONE_1>");
    }

    #[test]
    fn templates_per_detector_and_category() {
        let c = cfg("[detectors.email]\ntemplate = \"[{label_ja}]\"\n[categories.contact]\ntemplate = \"{shape:*}\"");
        let r = mask_text(&c, "a@example1.co.jp 090-1234-5678");
        assert_eq!(r.output, "[メールアドレス] ***-****-****");
    }

    #[test]
    fn allowlist() {
        let mut c = Config::default();
        c.allowlist = AllowlistConfig { values: vec!["10.0.0.1".into()], patterns: vec![r"192\.168\..*".into()], ..Default::default() };
        let r = mask_text(&c, "info@example.com admin@sub.example.com x@corp.co.jp 10.0.0.1 192.168.0.5 172.16.0.1");
        assert_eq!(r.output, "info@example.com admin@sub.example.com <EMAIL_1> 10.0.0.1 192.168.0.5 <IPV4_1>");
    }

    #[test]
    fn overlap_priority() {
        // URL 認証情報 (優先度 88) がメール (60) より優先される
        let r = mask_text(&Config::default(), "postgres://admin:pa55@db.corp.local/app");
        assert!(r.output.contains("<URL_CREDENTIALS_1>"), "{}", r.output);
    }

    #[test]
    fn custom_and_keywords() {
        let mut c = Config::default();
        c.custom_rules.push(CustomRule { id: "prj".into(), label: "PROJECT".into(), pattern: r"PRJ-\d{4}".into(), ..Default::default() });
        c.keywords.push(KeywordGroup { label: "CLIENT".into(), words: vec!["アクメ商事".into()], ..Default::default() });
        let r = mask_text(&c, "PRJ-1234 はアクメ商事向け");
        assert_eq!(r.output, "<PROJECT_1> は<CLIENT_1>向け");
    }

    #[test]
    fn name_propagation() {
        let r = mask_text(&Config::default(), "山田様、お世話になっております。山田が伺います。");
        assert_eq!(r.output, "<NAME_1>様、お世話になっております。<NAME_1>が伺います。");
    }

    #[test]
    fn disabled_category() {
        let c = cfg("[categories.contact]\nenabled = false");
        let r = mask_text(&c, "TEL 090-1234-5678 / 10.1.2.3");
        assert_eq!(r.output, "TEL 090-1234-5678 / <IPV4_1>");
    }

    #[test]
    fn invalid_template_warns_but_works() {
        let c = cfg("[detectors.email]\ntemplate = \"{bogus}\"");
        let e = Engine::new(&c);
        assert!(!e.warnings.is_empty());
        let r = e.mask("a@example1.co.jp", &mut MaskSession::new());
        assert_eq!(r.output, "<EMAIL_1>");
    }

    #[test]
    fn realistic_mixed_text() {
        let text = "\
From: 鈴木 一郎 <ichiro.suzuki@corp.example.co.jp>
To: 株式会社サンプル商事 田中様

お世話になっております。
サーバー db01.prod.internal (10.20.30.40) のパスワードは password=Hunter2! です。
カード番号 4111 1111 1111 1111、マイナンバー 1234 5678 9018。
住所: 東京都千代田区架空町1丁目2-3
C:\\Users\\ichiro\\Desktop\\memo.txt
AKIAIOSFODNN7EXAMPLE
";
        let r = mask_text(&Config::default(), text);
        for secret in ["ichiro.suzuki", "田中", "10.20.30.40", "db01.prod.internal", "Hunter2!", "4111", "1234 5678 9018", "架空町", "AKIAIOSFODNN7EXAMPLE", "サンプル商事"] {
            assert!(!r.output.contains(secret), "{secret} leaked:\n{}", r.output);
        }
        assert!(!r.output.contains("\\ichiro\\"), "{}", r.output);
    }
}
