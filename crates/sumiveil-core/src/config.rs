//! 設定ファイル (TOML) の読み込み・合成・編集。
//!
//! - 場所の優先順位: 明示指定 > 環境変数 `SUMIVEIL_CONFIG` > exe と同じフォルダの `sumiveil.toml` (ポータブル) > `%APPDATA%\Sumiveil\config.toml`
//! - `include = ["team.toml"]` で別ファイルを先に読み込んで合成できる (相対パスは設定ファイルのフォルダ基準)
//! - `[profiles.<名前>]` は基本設定の上に重ねる差分。`general.active_profile` または `--profile` で選択

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_TOML: &str = include_str!("../../../config/default.toml");
pub const ENV_CONFIG: &str = "SUMIVEIL_CONFIG";
pub const PORTABLE_FILE: &str = "sumiveil.toml";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("設定ファイルを読み込めません: {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("設定ファイルの書式エラー: {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("設定値が不正です: {0}")]
    Invalid(String),
    #[error("include が深すぎるか循環しています: {0}")]
    IncludeLoop(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// "auto" | "ja" | "en"
    pub language: String,
    pub theme: ThemeMode,
    pub active_profile: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self { language: "auto".into(), theme: ThemeMode::System, active_profile: "default".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MaskingConfig {
    /// 既定のマスク表示テンプレート
    pub template: String,
    /// この信頼度未満の検出は無視
    pub min_confidence: f32,
    /// `{hash}` と `{fake}` に使う秘密のソルト
    pub hash_salt: String,
}

impl Default for MaskingConfig {
    fn default() -> Self {
        Self { template: "<{label}_{n}>".into(), min_confidence: 0.5, hash_salt: String::new() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CategoryConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DetectorConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomRule {
    pub id: String,
    pub name: String,
    pub label: String,
    pub pattern: String,
    /// マスクする範囲のキャプチャグループ名 (省略時はマッチ全体、`v` があればそれ)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub category: String,
    pub priority: i32,
    pub confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    pub case_insensitive: bool,
    pub enabled: bool,
}

impl Default for CustomRule {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            label: "CUSTOM".into(),
            pattern: String::new(),
            group: None,
            category: "custom".into(),
            priority: 70,
            confidence: 0.9,
            template: None,
            case_insensitive: false,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeywordGroup {
    pub label: String,
    pub name: String,
    pub words: Vec<String>,
    pub case_sensitive: bool,
    /// 英数字の単語境界でのみ一致させる
    pub whole_word: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    pub enabled: bool,
}

impl Default for KeywordGroup {
    fn default() -> Self {
        Self {
            label: "KEYWORD".into(),
            name: String::new(),
            words: vec![],
            case_sensitive: false,
            whole_word: false,
            template: None,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AllowlistConfig {
    /// 完全一致 (大文字小文字無視) でマスク対象外
    pub values: Vec<String>,
    /// 正規表現 (値全体に一致) でマスク対象外
    pub patterns: Vec<String>,
    /// メールアドレス・ホスト名・URL のドメインがこれ (またはサブドメイン) ならマスク対象外
    pub domains: Vec<String>,
}

impl Default for AllowlistConfig {
    fn default() -> Self {
        Self {
            values: vec![],
            patterns: vec![],
            domains: ["example.com", "example.org", "example.net", "example.jp", "localhost"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NamesConfig {
    /// 確度の高い人名が見つかったら、文中の同じ名前もすべてマスク
    pub propagate: bool,
    /// 内蔵の姓名・地名辞書を使う
    pub use_dictionary: bool,
    /// 辞書層の最低スコア (0.0〜1.0)
    pub dictionary_threshold: f32,
    /// 形態素解析 (追加コンポーネント) を使う
    pub use_morphology: bool,
    /// 形態素解析辞書のパス (空なら exe と同じフォルダの dict/ipadic)
    pub morphology_dict: String,
}

impl Default for NamesConfig {
    fn default() -> Self {
        Self {
            propagate: true,
            use_dictionary: true,
            dictionary_threshold: 0.6,
            use_morphology: false,
            morphology_dict: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GuiConfig {
    pub font_size: f32,
    pub auto_mask: bool,
    pub debounce_ms: u64,
    pub sync_scroll: bool,
    pub word_wrap: bool,
    pub show_line_numbers: bool,
    pub show_detection_panel: bool,
    pub tray_enabled: bool,
    pub close_to_tray: bool,
    pub start_in_tray: bool,
    pub hotkey_enabled: bool,
    pub hotkey: String,
    pub hotkey_notify: bool,
    /// アクセントカラー: "auto" (Windows の設定に従う) | "#RRGGBB"
    pub accent_color: String,
    /// アプリ内のショートカットキー
    pub shortcuts: ShortcutsConfig,
    /// 描画方式: "auto" | "opengl" | "directx" | "software" (再起動後に反映)
    pub renderer: String,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            auto_mask: true,
            debounce_ms: 250,
            sync_scroll: true,
            word_wrap: false,
            show_line_numbers: true,
            show_detection_panel: true,
            tray_enabled: true,
            close_to_tray: false,
            start_in_tray: false,
            hotkey_enabled: true,
            hotkey: "Ctrl+Alt+M".into(),
            hotkey_notify: true,
            accent_color: "auto".into(),
            shortcuts: ShortcutsConfig::default(),
            renderer: "auto".into(),
        }
    }
}

/// アプリ内のショートカットキー。"Ctrl+Shift+C" の形式、空文字なら無効。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutsConfig {
    /// ファイルを開く
    pub open: String,
    /// クリップボードから貼り付け (置き換え)
    pub paste: String,
    /// マスク結果をコピー
    pub copy: String,
    /// マスク結果を保存
    pub save: String,
    /// マスクを再実行
    pub run: String,
    /// 設定を開く
    pub settings: String,
    /// 検索バーを開く
    pub find: String,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            open: "Ctrl+O".into(),
            paste: "Ctrl+Shift+V".into(),
            copy: "Ctrl+Shift+C".into(),
            save: "Ctrl+S".into(),
            run: "F5".into(),
            settings: "Ctrl+,".into(),
            find: "Ctrl+F".into(),
        }
    }
}

impl ShortcutsConfig {
    /// (設定キー, 値) の一覧。
    pub fn entries(&self) -> [(&'static str, &str); 7] {
        [
            ("open", &self.open),
            ("paste", &self.paste),
            ("copy", &self.copy),
            ("save", &self.save),
            ("run", &self.run),
            ("settings", &self.settings),
            ("find", &self.find),
        ]
    }

    pub fn get(&self, id: &str) -> Option<&str> {
        self.entries().into_iter().find(|(k, _)| *k == id).map(|(_, v)| v)
    }
}

/// ショートカットキーの問題。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutError {
    /// 書式が不正 (修飾キーは Ctrl / Shift / Alt、最後にキー名)
    Syntax,
    /// 文字入力やコピー・貼り付けなど、入力欄の基本操作と重なる
    Reserved,
}

/// "ctrl + shift + c" などを比較用の形 (小文字・修飾キーの順序を固定) にする。空なら None。
pub fn normalize_shortcut(spec: &str) -> Result<Option<String>, ShortcutError> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok(None);
    }
    // 「Ctrl++」のようにキー自体が + の場合
    let (mods, key) = if let Some(m) = spec.strip_suffix("++") {
        (m, "+")
    } else {
        match spec.rsplit_once('+') {
            Some((m, k)) => (m, k),
            None => ("", spec),
        }
    };
    let (mut ctrl, mut shift, mut alt) = (false, false, false);
    for t in mods.split('+').map(str::trim).filter(|t| !t.is_empty()) {
        match t.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            _ => return Err(ShortcutError::Syntax),
        }
    }
    let key = key.trim().to_ascii_lowercase();
    let key = match key.as_str() {
        "," => "comma",
        "." => "period",
        "-" => "minus",
        "+" => "plus",
        "=" => "equals",
        "/" => "slash",
        ";" => "semicolon",
        "esc" => "escape",
        "return" => "enter",
        "del" => "delete",
        k => k,
    }
    .to_string();
    // 修飾キーそのもの (「Ctrl+ControlLeft」のような組み合わせ) はキーとして使えない
    let modifier_key = key == "ctrl" || ["control", "shift", "alt", "super", "win", "meta"].iter().any(|m| key.starts_with(m));
    if key.is_empty() || key.contains(' ') || modifier_key {
        return Err(ShortcutError::Syntax);
    }
    let is_fkey = key.len() >= 2 && key.starts_with('f') && key[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n));
    // Ctrl / Alt なしで使えるのはファンクションキーだけ (文字入力の妨げになるため)
    let reserved = (!ctrl && !alt && !is_fkey)
        || (ctrl && !shift && !alt && ["c", "x", "v", "a", "z", "y"].contains(&key.as_str()));
    if reserved {
        return Err(ShortcutError::Reserved);
    }
    let mut out = String::new();
    for (on, name) in [(ctrl, "ctrl+"), (alt, "alt+"), (shift, "shift+")] {
        if on {
            out.push_str(name);
        }
    }
    out.push_str(&key);
    Ok(Some(out))
}

/// `spec` を操作 `id` に割り当てたとき、既に同じキーを使っている他の操作 (またはグローバルホットキー = "hotkey")。
pub fn shortcut_conflict(gui: &GuiConfig, id: &str, spec: &str) -> Option<&'static str> {
    let target = normalize_shortcut(spec).ok().flatten()?;
    let hotkey = if gui.hotkey_enabled { Some(("hotkey", gui.hotkey.as_str())) } else { None };
    gui.shortcuts
        .entries()
        .into_iter()
        .chain(hotkey)
        .filter(|(k, _)| *k != id)
        .find(|(_, v)| normalize_shortcut(v).ok().flatten().as_deref() == Some(target.as_str()))
        .map(|(k, _)| k)
}

impl GuiConfig {
    /// 手動指定のアクセントカラー (RGB)。"auto" や不正な値なら None。
    pub fn manual_accent(&self) -> Option<[u8; 3]> {
        parse_hex_color(&self.accent_color)
    }
}

/// "#RRGGBB" (先頭の # は省略可) を RGB に変換する。
pub fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BatchConfig {
    /// 対象ファイルのパターン (ファイル名に対する glob)
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    /// 出力フォルダを指定しない場合のファイル名接尾辞
    pub suffix: String,
    /// "same" (入力と同じ) | "utf-8" | "utf-8-bom" | "shift_jis" | "utf-16le"
    pub output_encoding: String,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            include: DEFAULT_BATCH_INCLUDE.iter().map(|s| s.to_string()).collect(),
            exclude: vec![],
            suffix: ".masked".into(),
            output_encoding: "same".into(),
        }
    }
}

/// フォルダ一括処理の既定の対象。
pub const DEFAULT_BATCH_INCLUDE: &[&str] = &[
    // テキスト・データ
    "*.txt", "*.log", "*.csv", "*.tsv", "*.json", "*.jsonl", "*.ndjson", "*.xml", "*.yaml", "*.yml", "*.har",
    // 文書 (マークアップ)
    "*.md", "*.markdown", "*.mdx", "*.rst", "*.adoc", "*.html", "*.htm",
    // 設定ファイル
    "*.ini", "*.conf", "*.cfg", "*.config", "*.properties", "*.toml", "*.env", "*.sql",
    // ソースコード・スクリプト
    "*.py", "*.js", "*.ts", "*.java", "*.cs", "*.go", "*.rb", "*.php", "*.ps1", "*.sh", "*.bat",
    // メール・Office 文書・PDF
    "*.eml", "*.msg", "*.docx", "*.xlsx", "*.pptx", "*.pdf",
];

/// テキスト以外のファイル (Office 文書など) の扱い。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilesConfig {
    /// Office 文書のプロパティ (作成者など): "ask" (その都度確認) | "mask" | "clear" (消す) | "keep" (残す)
    pub properties: String,
}

impl Default for FilesConfig {
    fn default() -> Self {
        Self { properties: "ask".into() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub masking: MaskingConfig,
    pub categories: BTreeMap<String, CategoryConfig>,
    pub detectors: BTreeMap<String, DetectorConfig>,
    pub custom_rules: Vec<CustomRule>,
    pub keywords: Vec<KeywordGroup>,
    pub allowlist: AllowlistConfig,
    pub names: NamesConfig,
    pub gui: GuiConfig,
    pub batch: BatchConfig,
    pub files: FilesConfig,
}

impl Config {
    /// カテゴリ設定を考慮した検出器の有効/無効。
    pub fn detector_enabled(&self, id: &str) -> bool {
        let Some(spec) = crate::catalog::detector(id) else {
            return false;
        };
        let cat = self.categories.get(spec.category).and_then(|c| c.enabled).unwrap_or(true);
        let own = self.detectors.get(id).and_then(|d| d.enabled).unwrap_or(spec.default_enabled);
        cat && own
    }

    pub fn category_enabled(&self, id: &str) -> bool {
        self.categories.get(id).and_then(|c| c.enabled).unwrap_or(true)
    }
}

/// 読み込んだ設定と付随情報。
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: Config,
    pub path: Option<PathBuf>,
    /// 定義されているプロファイル名 ("default" を含む)
    pub profiles: Vec<ProfileInfo>,
    pub active_profile: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileInfo {
    pub name: String,
    pub description: String,
}

/// 実際に使う設定ファイルのパスを決める。
pub fn resolve_config_path(explicit: Option<&Path>) -> PathBuf {
    if let Some(p) = explicit {
        return p.to_path_buf();
    }
    if let Ok(p) = std::env::var(ENV_CONFIG) {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Some(p) = portable_config_path() {
        if p.exists() {
            return p;
        }
    }
    user_config_path()
}

pub fn portable_config_path() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|d| d.join(PORTABLE_FILE))
}

pub fn user_config_path() -> PathBuf {
    match directories::ProjectDirs::from("", "", "Sumiveil") {
        // Windows: %APPDATA%\Sumiveil\config\config.toml になるのを避け、%APPDATA%\Sumiveil\config.toml に置く
        Some(d) => {
            let cfg = d.config_dir();
            let base = if cfg.file_name().is_some_and(|n| n == "config") { cfg.parent().unwrap_or(cfg) } else { cfg };
            base.join("config.toml")
        }
        None => PathBuf::from("sumiveil-config.toml"),
    }
}

/// 設定ファイルを読み込む。ファイルが無ければ既定値で返す。
pub fn load(path: &Path, profile: Option<&str>) -> Result<LoadedConfig, ConfigError> {
    let table = if path.exists() {
        load_table(path, 0)?
    } else {
        toml::Table::new()
    };
    let mut loaded = resolve(table, profile)?;
    loaded.path = Some(path.to_path_buf());
    Ok(loaded)
}

/// 文字列から読み込む (include はカレントディレクトリ基準)。
pub fn load_str(s: &str, profile: Option<&str>) -> Result<LoadedConfig, ConfigError> {
    let table: toml::Table = s
        .parse()
        .map_err(|e: toml::de::Error| ConfigError::Parse { path: PathBuf::from("<string>"), message: e.to_string() })?;
    let table = process_includes(table, Path::new("."), 0)?;
    resolve(table, profile)
}

fn load_table(path: &Path, depth: usize) -> Result<toml::Table, ConfigError> {
    if depth > 8 {
        return Err(ConfigError::IncludeLoop(path.to_path_buf()));
    }
    let bytes = std::fs::read(path).map_err(|e| ConfigError::Io { path: path.to_path_buf(), source: e })?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_start_matches('\u{feff}');
    let table: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| ConfigError::Parse { path: path.to_path_buf(), message: e.to_string() })?;
    let base = path.parent().unwrap_or(Path::new("."));
    process_includes(table, base, depth)
}

fn process_includes(mut table: toml::Table, base: &Path, depth: usize) -> Result<toml::Table, ConfigError> {
    let includes: Vec<String> = match table.remove("include") {
        Some(toml::Value::String(s)) => vec![s],
        Some(toml::Value::Array(a)) => a.into_iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        _ => vec![],
    };
    if includes.is_empty() {
        return Ok(table);
    }
    let mut merged = toml::Table::new();
    for inc in includes {
        let p = expand_env(&inc);
        let p = if Path::new(&p).is_absolute() { PathBuf::from(p) } else { base.join(p) };
        let t = load_table(&p, depth + 1)?;
        merge(&mut merged, t, "");
    }
    merge(&mut merged, table, "");
    Ok(merged)
}

fn expand_env(s: &str) -> String {
    // %VAR% 形式の環境変数を展開
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        if let Some(j) = after.find('%') {
            let name = &after[..j];
            match std::env::var(name) {
                Ok(v) if !name.is_empty() => out.push_str(&v),
                _ => {
                    out.push('%');
                    out.push_str(name);
                    out.push('%');
                }
            }
            rest = &after[j + 1..];
        } else {
            out.push('%');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// 配列を連結するキー (それ以外の配列は上書き)
const APPEND_KEYS: &[&str] = &["custom_rules", "keywords", "allowlist.values", "allowlist.patterns", "allowlist.domains"];

pub(crate) fn merge(base: &mut toml::Table, overlay: toml::Table, prefix: &str) {
    for (k, v) in overlay {
        let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
        match (base.get_mut(&k), v) {
            (Some(toml::Value::Table(bt)), toml::Value::Table(ot)) => merge(bt, ot, &path),
            (Some(toml::Value::Array(ba)), toml::Value::Array(oa)) if APPEND_KEYS.contains(&path.as_str()) => {
                for item in oa {
                    if !ba.contains(&item) {
                        ba.push(item);
                    }
                }
            }
            (_, v) => {
                base.insert(k, v);
            }
        }
    }
}

fn resolve(mut table: toml::Table, profile: Option<&str>) -> Result<LoadedConfig, ConfigError> {
    let profiles_table = match table.remove("profiles") {
        Some(toml::Value::Table(t)) => t,
        _ => toml::Table::new(),
    };
    let mut profiles = vec![ProfileInfo { name: "default".into(), description: String::new() }];
    for (name, v) in &profiles_table {
        let description = v.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
        profiles.push(ProfileInfo { name: name.clone(), description });
    }
    let base_active = table
        .get("general")
        .and_then(|g| g.get("active_profile"))
        .and_then(|p| p.as_str())
        .unwrap_or("default")
        .to_string();
    let active = profile.map(String::from).unwrap_or(base_active);
    let mut warnings = vec![];
    if active != "default" {
        match profiles_table.get(&active) {
            Some(toml::Value::Table(pt)) => {
                let mut pt = pt.clone();
                pt.remove("description");
                // プロファイル内で active_profile は変更させない
                if let Some(toml::Value::Table(g)) = pt.get_mut("general") {
                    g.remove("active_profile");
                }
                merge(&mut table, pt, "");
            }
            _ => warnings.push(format!("プロファイル '{active}' が見つかりません。既定の設定を使います。")),
        }
    }
    let mut config: Config = toml::Value::Table(table)
        .try_into()
        .map_err(|e: toml::de::Error| ConfigError::Parse { path: PathBuf::from("<merged>"), message: e.to_string() })?;
    config.general.active_profile = active.clone();
    warnings.extend(validate(&config));
    Ok(LoadedConfig { config, path: None, profiles, active_profile: active, warnings })
}

/// 設定内容の検証。問題があれば警告文の一覧を返す。
pub fn validate(cfg: &Config) -> Vec<String> {
    let mut w = vec![];
    for id in cfg.detectors.keys() {
        if crate::catalog::detector(id).is_none() {
            w.push(format!("未知の検出器 ID: detectors.{id}"));
        }
    }
    for id in cfg.categories.keys() {
        if crate::catalog::category(id).is_none() {
            w.push(format!("未知のカテゴリ ID: categories.{id}"));
        }
    }
    let check_tpl = |w: &mut Vec<String>, where_: &str, t: &str| {
        if let Err(e) = crate::template::Template::parse(t) {
            w.push(format!("{where_} のテンプレートが不正です: {e}"));
        }
    };
    check_tpl(&mut w, "masking.template", &cfg.masking.template);
    for (id, c) in &cfg.categories {
        if let Some(t) = &c.template {
            check_tpl(&mut w, &format!("categories.{id}"), t);
        }
    }
    for (id, d) in &cfg.detectors {
        if let Some(t) = &d.template {
            check_tpl(&mut w, &format!("detectors.{id}"), t);
        }
    }
    for (i, r) in cfg.custom_rules.iter().enumerate() {
        if r.pattern.is_empty() {
            w.push(format!("custom_rules[{i}] の pattern が空です"));
        } else if let Err(e) = crate::custom::compile_rule(r) {
            w.push(format!("custom_rules[{i}] ({}) の正規表現が不正です: {e}", r.id));
        }
        if let Some(t) = &r.template {
            check_tpl(&mut w, &format!("custom_rules[{i}]"), t);
        }
    }
    for p in &cfg.allowlist.patterns {
        if let Err(e) = regex::Regex::new(p) {
            w.push(format!("allowlist.patterns の正規表現が不正です: {p}: {e}"));
        }
    }
    if !["ask", "mask", "clear", "keep"].contains(&cfg.files.properties.trim().to_ascii_lowercase().as_str()) {
        w.push(format!("files.properties は \"ask\" / \"mask\" / \"clear\" / \"keep\" のいずれかで指定してください: {}", cfg.files.properties));
    }
    let accent = cfg.gui.accent_color.trim();
    if !accent.eq_ignore_ascii_case("auto") && parse_hex_color(accent).is_none() {
        w.push(format!("gui.accent_color は \"auto\" か \"#RRGGBB\" で指定してください: {accent}"));
    }
    for (id, spec) in cfg.gui.shortcuts.entries() {
        match normalize_shortcut(spec) {
            Err(ShortcutError::Syntax) => w.push(format!("gui.shortcuts.{id} の書式が不正です (例: \"Ctrl+Shift+C\"): {spec}")),
            Err(ShortcutError::Reserved) => w.push(format!("gui.shortcuts.{id} は文字入力やコピー・貼り付けと重なるため使えません: {spec}")),
            Ok(_) => {
                // 重複は 1 組につき 1 回だけ報告する (後ろの項目で報告)
                if let Some(other) = shortcut_conflict(&cfg.gui, id, spec) {
                    let earlier = cfg.gui.shortcuts.entries().iter().position(|(k, _)| *k == other) < cfg.gui.shortcuts.entries().iter().position(|(k, _)| *k == id);
                    if other == "hotkey" || earlier {
                        let other = if other == "hotkey" { "gui.hotkey".to_string() } else { format!("gui.shortcuts.{other}") };
                        w.push(format!("gui.shortcuts.{id} ({spec}) は {other} と同じキーです"));
                    }
                }
            }
        }
    }
    w
}

// ───────────────────────── 編集 (コメント保持) ─────────────────────────

/// コメントや書式を保ったまま設定ファイルを編集するためのラッパー。
pub struct ConfigDoc {
    pub path: PathBuf,
    doc: toml_edit::DocumentMut,
}

impl ConfigDoc {
    /// 既存ファイルを開く。無ければ既定のテンプレートから開始する (保存はしない)。
    pub fn open(path: &Path) -> Result<Self, ConfigError> {
        let text = if path.exists() {
            let bytes = std::fs::read(path).map_err(|e| ConfigError::Io { path: path.to_path_buf(), source: e })?;
            String::from_utf8_lossy(&bytes).trim_start_matches('\u{feff}').to_string()
        } else {
            DEFAULT_TOML.to_string()
        };
        let doc = text
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| ConfigError::Parse { path: path.to_path_buf(), message: e.to_string() })?;
        Ok(Self { path: path.to_path_buf(), doc })
    }

    pub fn from_str(path: &Path, text: &str) -> Result<Self, ConfigError> {
        let doc = text
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| ConfigError::Parse { path: path.to_path_buf(), message: e.to_string() })?;
        Ok(Self { path: path.to_path_buf(), doc })
    }

    fn table_path_mut(&mut self, keys: &[&str]) -> &mut toml_edit::Table {
        let mut t = self.doc.as_table_mut();
        for k in keys {
            let needs_new = !matches!(t.get(k), Some(toml_edit::Item::Table(_)));
            if needs_new {
                // インラインテーブルなら通常テーブルに変換、それ以外は置換
                let converted = match t.get(k) {
                    Some(toml_edit::Item::Value(toml_edit::Value::InlineTable(it))) => Some(it.clone().into_table()),
                    _ => None,
                };
                let mut nt = converted.unwrap_or_default();
                nt.set_implicit(true);
                t.insert(k, toml_edit::Item::Table(nt));
            }
            t = t.get_mut(k).and_then(|i| i.as_table_mut()).unwrap();
        }
        t
    }

    /// ドット区切りのキーに値を設定する (途中のテーブルは自動作成)。
    pub fn set(&mut self, key: &str, value: impl Into<toml_edit::Value>) {
        let parts: Vec<&str> = key.split('.').collect();
        let (last, parents) = parts.split_last().unwrap();
        let t = self.table_path_mut(parents);
        match t.get_mut(last) {
            Some(toml_edit::Item::Value(v)) => {
                // 既存のコメント・装飾を保持
                let decor = v.decor().clone();
                let mut nv: toml_edit::Value = value.into();
                *nv.decor_mut() = decor;
                *v = nv;
            }
            _ => {
                t.insert(last, toml_edit::Item::Value(value.into()));
            }
        }
    }

    /// 文字列を TOML の値として解釈して設定 (解釈できなければ文字列として設定)。
    pub fn set_raw(&mut self, key: &str, raw: &str) {
        let parsed = format!("x = {raw}").parse::<toml_edit::DocumentMut>().ok().and_then(|d| d.get("x").and_then(|i| i.as_value().cloned()));
        match parsed {
            Some(v) => self.set(key, v),
            None => self.set(key, raw),
        }
    }

    /// Serialize 可能な値 (配列・テーブル・テーブルの配列) をまとめて設定する。
    pub fn set_serialized<T: Serialize>(&mut self, key: &str, value: &T) -> Result<(), ConfigError> {
        #[derive(Serialize)]
        struct Wrap<'a, T: Serialize> {
            x: &'a T,
        }
        let s = toml::to_string(&Wrap { x: value }).map_err(|e| ConfigError::Invalid(e.to_string()))?;
        let d = s.parse::<toml_edit::DocumentMut>().map_err(|e| ConfigError::Invalid(e.to_string()))?;
        let item = d.get("x").cloned().unwrap_or(toml_edit::Item::None);
        let parts: Vec<&str> = key.split('.').collect();
        let (last, parents) = parts.split_last().unwrap();
        let t = self.table_path_mut(parents);
        match item {
            toml_edit::Item::None => {
                t.remove(last);
            }
            item => {
                t.insert(last, item);
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> bool {
        let parts: Vec<&str> = key.split('.').collect();
        let (last, parents) = parts.split_last().unwrap();
        let mut t = self.doc.as_table_mut();
        for k in parents {
            match t.get_mut(k).and_then(|i| i.as_table_mut()) {
                Some(nt) => t = nt,
                None => return false,
            }
        }
        t.remove(last).is_some()
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let mut item = self.doc.as_item();
        for k in key.split('.') {
            item = item.get(k)?;
        }
        Some(match item {
            toml_edit::Item::Value(v) => {
                let mut v = v.clone();
                v.decor_mut().clear();
                v.to_string().trim().to_string()
            }
            other => other.to_string().trim().to_string(),
        })
    }

    pub fn to_toml_string(&self) -> String {
        self.doc.to_string()
    }

    /// 一時ファイルに書いてから置き換える (書き込み途中で壊れないように)。
    pub fn save(&self) -> Result<(), ConfigError> {
        if let Some(dir) = self.path.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir).map_err(|e| ConfigError::Io { path: dir.to_path_buf(), source: e })?;
            }
        }
        let tmp = self.path.with_extension("toml.tmp");
        std::fs::write(&tmp, self.doc.to_string()).map_err(|e| ConfigError::Io { path: tmp.clone(), source: e })?;
        std::fs::rename(&tmp, &self.path).map_err(|e| ConfigError::Io { path: self.path.clone(), source: e })?;
        Ok(())
    }
}

/// 編集対象のキーの前置き。active_profile が default 以外ならそのプロファイル内を編集する。
pub fn profile_key(profile: &str, key: &str) -> String {
    if profile.is_empty() || profile == "default" {
        key.to_string()
    } else {
        format!("profiles.{profile}.{key}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_toml_loads_cleanly() {
        let l = load_str(DEFAULT_TOML, None).unwrap();
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        assert!(l.profiles.iter().any(|p| p.name == "llm"));
        assert_eq!(l.config.masking.template, "<{label}_{n}>");
    }

    #[test]
    fn empty_is_default() {
        let l = load_str("", None).unwrap();
        assert_eq!(l.config.masking, MaskingConfig::default());
        assert!(l.config.detector_enabled("email"));
        assert!(!l.config.detector_enabled("uuid"));
    }

    #[test]
    fn profiles_overlay() {
        let s = r#"
[detectors.email]
enabled = true
[allowlist]
values = ["a"]
[profiles.strict]
description = "厳しめ"
[profiles.strict.detectors.email]
enabled = false
[profiles.strict.allowlist]
values = ["b"]
"#;
        let l = load_str(s, Some("strict")).unwrap();
        assert!(!l.config.detector_enabled("email"));
        assert_eq!(l.config.allowlist.values, vec!["a", "b"]);
        assert_eq!(l.config.general.active_profile, "strict");
        let l = load_str(s, None).unwrap();
        assert!(l.config.detector_enabled("email"));
    }

    #[test]
    fn category_disable() {
        let l = load_str("[categories.network]\nenabled = false", None).unwrap();
        assert!(!l.config.detector_enabled("ipv4"));
        assert!(l.config.detector_enabled("email"));
    }

    #[test]
    fn includes_merge() {
        let dir = std::env::temp_dir().join(format!("sumiveil-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("team.toml"), "[[keywords]]\nlabel = \"CLIENT\"\nwords = [\"Acme\"]\n[masking]\ntemplate = \"[{label}]\"").unwrap();
        std::fs::write(dir.join("me.toml"), "include = [\"team.toml\"]\n[[keywords]]\nlabel = \"MINE\"\nwords = [\"Foo\"]").unwrap();
        let l = load(&dir.join("me.toml"), None).unwrap();
        assert_eq!(l.config.keywords.len(), 2);
        assert_eq!(l.config.masking.template, "[{label}]");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn edit_preserves_comments() {
        let src = "# 先頭コメント\n[detectors.email]\nenabled = true # 行末コメント\n";
        let mut d = ConfigDoc::from_str(Path::new("x.toml"), src).unwrap();
        d.set("detectors.email.enabled", false);
        d.set("detectors.ipv4.template", "[IP]");
        d.set_raw("masking.min_confidence", "0.7");
        let out = d.to_toml_string();
        assert!(out.contains("# 先頭コメント"));
        assert!(out.contains("enabled = false # 行末コメント"));
        let l = load_str(&out, None).unwrap();
        assert!(!l.config.detector_enabled("email"));
        assert_eq!(l.config.detectors["ipv4"].template.as_deref(), Some("[IP]"));
        assert!((l.config.masking.min_confidence - 0.7).abs() < 1e-6);
        assert_eq!(d.get("detectors.email.enabled").as_deref(), Some("false"));
    }

    #[test]
    fn accent_color_and_obsolete_backdrop() {
        // 1.0.0 の設定ファイルにある gui.backdrop は読み飛ばす
        let l = load_str("[gui]\nbackdrop = true\n", None).unwrap();
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        assert_eq!(l.config.gui.accent_color, "auto");
        assert_eq!(l.config.gui.manual_accent(), None);
        let l = load_str("[gui]\naccent_color = \"#E81123\"\n", None).unwrap();
        assert_eq!(l.config.gui.manual_accent(), Some([0xE8, 0x11, 0x23]));
        let l = load_str("[gui]\naccent_color = \"red\"\n", None).unwrap();
        assert_eq!(l.warnings.len(), 1);
        assert_eq!(parse_hex_color("0078d4"), Some([0x00, 0x78, 0xD4]));
        assert_eq!(parse_hex_color("#12345"), None);
    }

    #[test]
    fn shortcuts() {
        let l = load_str("", None).unwrap();
        assert_eq!(l.config.gui.shortcuts.run, "F5");
        // 一部だけ変更。残りは既定値
        let l = load_str("[gui.shortcuts]\nrun = \"F6\"\nsave = \"\"\n", None).unwrap();
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        assert_eq!((l.config.gui.shortcuts.run.as_str(), l.config.gui.shortcuts.save.as_str(), l.config.gui.shortcuts.open.as_str()), ("F6", "", "Ctrl+O"));

        assert_eq!(normalize_shortcut(" shift + ctrl + c "), Ok(Some("ctrl+shift+c".into())));
        assert_eq!(normalize_shortcut("Ctrl+,"), normalize_shortcut("Control+Comma"));
        assert_eq!(normalize_shortcut("Ctrl++"), Ok(Some("ctrl+plus".into())));
        assert_eq!(normalize_shortcut(""), Ok(None));
        assert_eq!(normalize_shortcut("Win+C"), Err(ShortcutError::Syntax));
        assert_eq!(normalize_shortcut("Ctrl+Shift"), Err(ShortcutError::Syntax));
        assert_eq!(normalize_shortcut("Ctrl+ControlLeft"), Err(ShortcutError::Syntax));
        assert_eq!(normalize_shortcut("Ctrl+C"), Err(ShortcutError::Reserved));
        assert_eq!(normalize_shortcut("A"), Err(ShortcutError::Reserved));
        assert_eq!(normalize_shortcut("Shift+Tab"), Err(ShortcutError::Reserved));
        assert!(normalize_shortcut("Ctrl+Shift+V").is_ok());

        let g = GuiConfig::default();
        assert_eq!(shortcut_conflict(&g, "run", "Ctrl+S"), Some("save"));
        assert_eq!(shortcut_conflict(&g, "save", "Ctrl+S"), None);
        assert_eq!(shortcut_conflict(&g, "run", "ctrl+alt+m"), Some("hotkey"));

        let l = load_str("[gui]\nhotkey = \"Ctrl+Alt+M\"\n[gui.shortcuts]\nrun = \"Ctrl+S\"\ncopy = \"Ctrl+C\"\nopen = \"Ctrl+Alt+M\"\npaste = \"Hyper+V\"\n", None).unwrap();
        assert_eq!(l.warnings.len(), 4, "{:?}", l.warnings);
    }

    #[test]
    fn edit_serialized_lists() {
        let mut d = ConfigDoc::from_str(Path::new("x.toml"), "").unwrap();
        let rules = vec![CustomRule { id: "prj".into(), label: "PROJECT".into(), pattern: "PRJ-\\d{4}".into(), ..Default::default() }];
        d.set_serialized("custom_rules", &rules).unwrap();
        d.set_serialized("allowlist.values", &vec!["a".to_string()]).unwrap();
        let l = load_str(&d.to_toml_string(), None).unwrap();
        assert_eq!(l.config.custom_rules, rules);
        assert!(l.config.allowlist.values.contains(&"a".to_string()));
    }
}
