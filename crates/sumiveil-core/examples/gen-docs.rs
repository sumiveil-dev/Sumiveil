//! docs/detectors.md を検出器カタログから生成する: cargo run -p sumiveil-core --example gen-docs

use std::fmt::Write;

use sumiveil_core::catalog::{CATALOG, CATEGORIES};

fn main() {
    let mut s = String::new();
    writeln!(s, "# 検出器一覧 / Detectors\n").unwrap();
    writeln!(s, "> このファイルは `cargo run -p sumiveil-core --example gen-docs` で自動生成されます。\n").unwrap();
    writeln!(s, "- 検出器数: {}", CATALOG.len()).unwrap();
    writeln!(s, "- カテゴリ単位 (`categories.<id>.enabled`) と検出器単位 (`detectors.<id>.enabled`) で有効/無効を切り替えられます。").unwrap();
    writeln!(s, "- 「既定」列が ✓ のものは初期状態で有効です。\n").unwrap();
    for c in CATEGORIES.iter().filter(|c| c.id != "custom") {
        writeln!(s, "## {} ({}) — `{}`\n", c.name_ja, c.name_en, c.id).unwrap();
        writeln!(s, "| ID | 名前 | ラベル | 既定 | 優先度 | 例 |").unwrap();
        writeln!(s, "|---|---|---|:---:|---:|---|").unwrap();
        for d in CATALOG.iter().filter(|d| d.category == c.id) {
            let ex = d.example.replace('\n', " ⏎ ").replace('|', "\\|");
            writeln!(
                s,
                "| `{}` | {}<br><sub>{}</sub> | `{}` | {} | {} | `{}` |",
                d.id,
                d.name_ja,
                d.name_en,
                d.label,
                if d.default_enabled { "✓" } else { "" },
                d.priority,
                ex
            )
            .unwrap();
        }
        writeln!(s).unwrap();
    }
    writeln!(s, "## カスタム — `custom`\n").unwrap();
    writeln!(s, "設定ファイルの `[[custom_rules]]` (正規表現) と `[[keywords]]` (キーワード辞書) で追加します。ID はそれぞれ `custom:<id>`、`keyword:<label>` になります。").unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/detectors.md");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, s).unwrap();
    println!("written {}", path.display());
}
