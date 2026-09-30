//! 形態素解析 (任意の追加層)。Lindera + IPADIC で固有名詞 (人名・地域) を抽出する。
//!
//! 辞書は exe に埋め込まず、外部フォルダからメモリマップで読み込む (未導入なら無効)。
//! 探す場所: 設定 `names.morphology_dict` → `<exe>\dict\ipadic` → `<exe>\..\dict\ipadic` → `%LOCALAPPDATA%\Sumiveil\dict\ipadic`

use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NounKind {
    Surname,
    GivenName,
    PersonOther,
    Place,
    Organization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProperNoun {
    pub start: usize,
    pub end: usize,
    pub kind: NounKind,
}

/// 辞書フォルダの候補。
pub fn dictionary_candidates(configured: &str) -> Vec<PathBuf> {
    let mut v = vec![];
    if !configured.trim().is_empty() {
        v.push(PathBuf::from(configured.trim()));
        return v;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            v.push(dir.join("dict").join("ipadic"));
            if let Some(up) = dir.parent() {
                v.push(up.join("dict").join("ipadic"));
            }
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        v.push(PathBuf::from(local).join("Sumiveil").join("dict").join("ipadic"));
    }
    v
}

/// 使用できる辞書フォルダを探す (metadata.json があるもの)。
pub fn find_dictionary(configured: &str) -> Option<PathBuf> {
    dictionary_candidates(configured).into_iter().find(|p| p.join("metadata.json").is_file())
}

pub fn is_available() -> bool {
    cfg!(feature = "morphology")
}

#[cfg(feature = "morphology")]
mod imp {
    use super::*;
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    use lindera::dictionary::load_fs_dictionary_with_options;
    use lindera::mode::Mode;
    use lindera::segmenter::Segmenter;

    pub struct Morph {
        segmenter: Segmenter,
        pub path: PathBuf,
    }

    impl Morph {
        /// 辞書を読み込む (同じパスは一度だけ)。
        pub fn load(dir: &Path) -> Result<Arc<Morph>, String> {
            static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<Morph>>>> = OnceLock::new();
            let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
            if let Some(m) = cache.lock().unwrap().get(dir) {
                return Ok(m.clone());
            }
            let dict = load_fs_dictionary_with_options(dir, true).map_err(|e| format!("{}: {e}", dir.display()))?;
            let m = Arc::new(Morph { segmenter: Segmenter::new(Mode::Normal, dict, None), path: dir.to_path_buf() });
            cache.lock().unwrap().insert(dir.to_path_buf(), m.clone());
            Ok(m)
        }

        /// 固有名詞 (人名・地域・組織) の位置を返す。行ごとに解析し、ASCII だけの行は飛ばす。
        pub fn proper_nouns(&self, text: &str) -> Vec<ProperNoun> {
            let mut out = vec![];
            let mut offset = 0usize;
            for line in text.split_inclusive('\n') {
                let base = offset;
                offset += line.len();
                if line.is_ascii() {
                    continue;
                }
                let Ok(mut tokens) = self.segmenter.segment(Cow::Borrowed(line)) else { continue };
                for t in tokens.iter_mut() {
                    let (s, e) = (t.byte_start, t.byte_end);
                    let d = t.details();
                    if d.len() < 4 || d[0] != "名詞" || d[1] != "固有名詞" {
                        continue;
                    }
                    let kind = match (d[2], d[3]) {
                        ("人名", "姓") => NounKind::Surname,
                        ("人名", "名") => NounKind::GivenName,
                        ("人名", _) => NounKind::PersonOther,
                        ("地域", "一般") => NounKind::Place,
                        ("組織", _) => NounKind::Organization,
                        _ => continue,
                    };
                    out.push(ProperNoun { start: base + s, end: base + e, kind });
                }
            }
            out
        }
    }
}

#[cfg(not(feature = "morphology"))]
mod imp {
    use super::*;

    /// 形態素解析を含めずにビルドした場合のダミー。
    pub struct Morph {
        pub path: PathBuf,
    }

    impl Morph {
        pub fn load(dir: &Path) -> Result<Arc<Morph>, String> {
            Err(format!("morphology support is not compiled in ({})", dir.display()))
        }

        pub fn proper_nouns(&self, _text: &str) -> Vec<ProperNoun> {
            vec![]
        }
    }
}

pub use imp::Morph;

/// 設定に従って辞書を読み込む。
pub fn load_configured(configured: &str) -> Result<Arc<Morph>, String> {
    match find_dictionary(configured) {
        Some(p) => Morph::load(&p),
        None => Err(format!(
            "形態素解析辞書が見つかりません / morphology dictionary not found: {}",
            dictionary_candidates(configured).iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
        )),
    }
}
