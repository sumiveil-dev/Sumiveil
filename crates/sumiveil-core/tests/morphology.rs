//! 形態素解析層のテスト (辞書 target/dict/ipadic がある場合のみ実行)。

use sumiveil_core::config::Config;
use sumiveil_core::engine::{Engine, MaskSession};

fn dict_dir() -> Option<String> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/dict/ipadic");
    p.join("metadata.json").is_file().then(|| p.display().to_string())
}

fn engine(dir: &str) -> Engine {
    let mut cfg = Config::default();
    cfg.names.use_morphology = true;
    cfg.names.morphology_dict = dir.to_string();
    let e = Engine::new(&cfg);
    assert!(e.warnings.is_empty(), "{:?}", e.warnings);
    e
}

#[test]
fn morphology_finds_bare_surnames() {
    let Some(dir) = dict_dir() else {
        eprintln!("skip: dictionary not built");
        return;
    };
    let e = engine(&dir);
    let r = e.mask("昨日、山田が資料を持ってきた。", &mut MaskSession::new());
    assert!(!r.output.contains("山田"), "{}", r.output);
}

#[test]
fn morphology_keeps_ordinary_text_clean() {
    let Some(dir) = dict_dir() else {
        eprintln!("skip: dictionary not built");
        return;
    };
    let e = engine(&dir);
    let text = "本日の定例会議は午後3時から第二会議室で行います。\n資料は共有フォルダに保存しました。\n品質管理部では、検査手順の見直しを進めています。\n東京駅から徒歩5分の会場です。";
    let r = e.mask(text, &mut MaskSession::new());
    let found: Vec<String> = r.replacements.iter().map(|x| format!("{}: {}", x.meta.id, x.original)).collect();
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn missing_dictionary_is_a_warning() {
    let mut cfg = Config::default();
    cfg.names.use_morphology = true;
    cfg.names.morphology_dict = "Z:\\no\\such\\dir".into();
    let e = Engine::new(&cfg);
    assert!(!e.warnings.is_empty());
    // 辞書が無くても他の検出は動く
    let r = e.mask("a@example1.co.jp", &mut MaskSession::new());
    assert_eq!(r.output, "<EMAIL_1>");
}
