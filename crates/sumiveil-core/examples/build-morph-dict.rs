//! mecab-ipadic のソース (EUC-JP) から Lindera 形式の辞書を作る。
//! cargo run --release -p sumiveil-core --example build-morph-dict -- [入力フォルダ] [出力フォルダ]
//! 既定: data/raw/mecab-ipadic → target/dict/ipadic

use std::path::PathBuf;

use lindera::dictionary::{DictionaryBuilder, Metadata};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut args = std::env::args().skip(1);
    let input = args.next().map(PathBuf::from).unwrap_or_else(|| root.join("data/raw/mecab-ipadic"));
    let output = args.next().map(PathBuf::from).unwrap_or_else(|| root.join("target/dict/ipadic"));
    std::fs::create_dir_all(&output).unwrap();
    let metadata = Metadata { name: "ipadic".into(), encoding: "EUC-JP".into(), ..Metadata::default() };
    let t = std::time::Instant::now();
    DictionaryBuilder::new(metadata).build_dictionary(&input, &output).expect("build dictionary");
    let size: u64 = std::fs::read_dir(&output).unwrap().filter_map(|e| e.ok()?.metadata().ok()).map(|m| m.len()).sum();
    println!("built {} ({:.1} MB) in {:.1?}", output.display(), size as f64 / 1048576.0, t.elapsed());
}
