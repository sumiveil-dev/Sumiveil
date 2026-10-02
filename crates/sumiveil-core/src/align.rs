//! 左右比較表示のための行の対応付け。
//!
//! 置換で行数が変わる場合 (複数行の秘密鍵を 1 行のタグにする等) でも、
//! 左 (元テキスト) と右 (マスク後) の行がずれないよう空行を挟んで揃える。

use crate::engine::MaskResult;
use crate::text::LineIndex;

/// 表示上の 1 行。`None` は詰め物 (空行)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub left: Option<usize>,
    pub right: Option<usize>,
}

pub fn align(original: &str, result: &MaskResult) -> Vec<Row> {
    let li = LineIndex::new(original);
    let ri = LineIndex::new(&result.output);
    let line_of = |idx: &LineIndex, text: &str, b: usize| idx.line_col(text, b).0 - 1;

    let mut rows = Vec::with_capacity(li.line_count().max(ri.line_count()));
    let (mut ol, mut rl) = (0usize, 0usize);
    // 処理中のブロック: (左開始, 左終了, 右開始, 右終了) いずれも含む
    let mut block: Option<(usize, usize, usize, usize)> = None;

    let flush = |rows: &mut Vec<Row>, b: (usize, usize, usize, usize)| {
        let (l0, l1, r0, r1) = b;
        let ln = l1 - l0 + 1;
        let rn = r1 - r0 + 1;
        for k in 0..ln.max(rn) {
            rows.push(Row { left: (k < ln).then_some(l0 + k), right: (k < rn).then_some(r0 + k) });
        }
    };

    for r in &result.replacements {
        let a = r.original.matches('\n').count();
        let b = r.replacement.matches('\n').count();
        let l0 = line_of(&li, original, r.start);
        let r0 = line_of(&ri, &result.output, r.out_start);
        if let Some((bl0, bl1, br0, br1)) = block {
            if l0 <= bl1 {
                // 同じブロック内: 行数の差分を反映して延長
                block = Some((bl0, bl1.max(l0 + a), br0, br1.max(r0 + b)));
                continue;
            }
            flush(&mut rows, (bl0, bl1, br0, br1));
            ol = bl1 + 1;
            rl = br1 + 1;
            block = None;
        }
        if a == b {
            continue;
        }
        // ブロック前の 1:1 の行
        while ol < l0 && rl < r0 {
            rows.push(Row { left: Some(ol), right: Some(rl) });
            ol += 1;
            rl += 1;
        }
        block = Some((l0, l0 + a, r0, r0 + b));
    }
    if let Some(b) = block {
        flush(&mut rows, b);
        ol = b.1 + 1;
        rl = b.3 + 1;
    }
    let (ln, rn) = (li.line_count(), ri.line_count());
    while ol < ln || rl < rn {
        rows.push(Row { left: (ol < ln).then_some(ol), right: (rl < rn).then_some(rl) });
        ol += 1;
        rl += 1;
    }
    rows
}

#[cfg(test)]
#[path = "tests/align.rs"]
mod tests;
