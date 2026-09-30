//! 文字列処理の小さなヘルパー群。

/// 全角数字を半角に変換し、数字以外を取り除いた文字列を返す。
pub fn digits_only(s: &str) -> String {
    s.chars().filter_map(to_ascii_digit).collect()
}

/// 半角/全角の数字を ASCII 数字に変換する。数字でなければ None。
pub fn to_ascii_digit(c: char) -> Option<char> {
    match c {
        '0'..='9' => Some(c),
        '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32),
        _ => None,
    }
}

pub fn is_digit_like(c: char) -> bool {
    to_ascii_digit(c).is_some()
}

pub fn is_hyphen_like(c: char) -> bool {
    matches!(
        c,
        '-' | '‐' | '‑' | '–' | '—' | '―' | '−' | 'ー' | '－' | 'ｰ'
    )
}

/// 位置 `idx` (バイト) の直前の文字。
pub fn char_before(text: &str, idx: usize) -> Option<char> {
    text[..idx].chars().next_back()
}

/// 位置 `idx` (バイト) の直後の文字。
pub fn char_after(text: &str, idx: usize) -> Option<char> {
    text[idx..].chars().next()
}

/// `idx` から前方 `n` 文字ぶん戻ったバイト位置。
pub fn back_chars(text: &str, idx: usize, n: usize) -> usize {
    let mut pos = idx;
    for (count, (i, _)) in text[..idx].char_indices().rev().enumerate() {
        pos = i;
        if count + 1 >= n {
            break;
        }
    }
    pos
}

/// `idx` から後方 `n` 文字ぶん進んだバイト位置。
pub fn forward_chars(text: &str, idx: usize, n: usize) -> usize {
    match text[idx..].char_indices().nth(n) {
        Some((i, _)) => idx + i,
        None => text.len(),
    }
}

/// バイト位置から (1始まりの行, 1始まりの桁[文字単位]) を求めるための索引。
pub struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self { starts }
    }

    /// (行番号, 桁) どちらも 1 始まり。
    pub fn line_col(&self, text: &str, byte: usize) -> (usize, usize) {
        let line = match self.starts.binary_search(&byte) {
            Ok(l) => l,
            Err(l) => l - 1,
        };
        let col = text[self.starts[line]..byte].chars().count();
        (line + 1, col + 1)
    }

    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    pub fn line_start(&self, line0: usize) -> usize {
        self.starts[line0]
    }
}

/// Shannon エントロピー (bit/char)。
pub fn shannon_entropy(s: &str) -> f64 {
    let mut counts = std::collections::HashMap::new();
    let mut total = 0usize;
    for c in s.chars() {
        *counts.entry(c).or_insert(0usize) += 1;
        total += 1;
    }
    if total == 0 {
        return 0.0;
    }
    counts
        .values()
        .map(|&n| {
            let p = n as f64 / total as f64;
            -p * p.log2()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits() {
        assert_eq!(digits_only("０９０-1234－５６７８"), "09012345678");
    }

    #[test]
    fn line_col() {
        let t = "abc\nあいう\nx";
        let idx = LineIndex::new(t);
        assert_eq!(idx.line_col(t, 0), (1, 1));
        let p = t.find('い').unwrap();
        assert_eq!(idx.line_col(t, p), (2, 2));
        assert_eq!(idx.line_col(t, t.len() - 1), (3, 1));
    }

    #[test]
    fn back_forward() {
        let t = "あいうえお";
        assert_eq!(back_chars(t, t.len(), 2), "あいう".len());
        assert_eq!(forward_chars(t, 0, 2), "あい".len());
        assert_eq!(forward_chars(t, 0, 99), t.len());
    }
}
