//! `catalog.rs` の単体テスト (本体から分離。`catalog.rs` の子モジュール `tests` として組み込まれる)

use super::*;

#[test]
fn ids_unique_and_categories_known() {
    let mut ids = std::collections::HashSet::new();
    for d in CATALOG {
        assert!(ids.insert(d.id), "duplicate id {}", d.id);
        assert!(category(d.category).is_some(), "unknown category {}", d.category);
    }
}
