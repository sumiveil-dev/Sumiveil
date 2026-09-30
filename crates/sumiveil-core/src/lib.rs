//! Sumiveil のコアライブラリ: 機密情報の検出とマスキング。
//!
//! ネットワークアクセスは一切行わない。

pub mod align;
pub mod batch;
pub mod catalog;
pub mod config;
pub mod custom;
pub mod detector;
pub mod diag;
pub mod dict;
pub mod encoding;
pub mod engine;
pub mod fake;
pub mod formats;
pub mod lang;
pub mod morph;
pub mod names;
pub mod report;
pub mod search;
pub mod template;
pub mod text;
pub mod validators;

pub use config::{Config, ConfigDoc, LoadedConfig};
pub use engine::{Detection, DetectorMeta, Engine, MaskResult, MaskSession, Replacement};

pub use encoding_rs;
pub use regex;
pub use toml_edit;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
