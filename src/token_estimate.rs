//! 粗略 token 估算：区分 CJK 与拉丁字符，避免中文场景下 `chars/4` 严重偏松。

const CHARS_PER_TOKEN_LATIN: usize = 4;
const CHARS_PER_TOKEN_CJK: usize = 2;

/// 估算单段文本的 token 数（非空文本至少为 1，空串为 0）。
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text_tokens(text).max(1)
}

/// 估算多段文本合计 token 数。
#[allow(dead_code)]
pub fn estimate_texts_tokens(texts: &[&str]) -> u64 {
    let combined: String = texts.iter().copied().collect();
    estimate_tokens(&combined) as u64
}

fn text_tokens(text: &str) -> usize {
    let mut cjk = 0usize;
    let mut latin = 0usize;
    for ch in text.chars() {
        if is_cjk(ch) {
            cjk += 1;
        } else {
            latin += 1;
        }
    }
    cjk.div_ceil(CHARS_PER_TOKEN_CJK) + latin.div_ceil(CHARS_PER_TOKEN_LATIN)
}

fn is_cjk(ch: char) -> bool {
    let code = ch as u32;
    (0x4E00..=0x9FFF).contains(&code)
        || (0x3400..=0x4DBF).contains(&code)
        || (0x20000..=0x2A6DF).contains(&code)
        || (0x3040..=0x30FF).contains(&code)
        || (0xAC00..=0xD7AF).contains(&code)
        || (0xFF00..=0xFFEF).contains(&code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_uses_four_chars_per_token() {
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
        assert_eq!(estimate_tokens("abcde"), 2);
    }

    #[test]
    fn cjk_uses_two_chars_per_token() {
        assert_eq!(estimate_tokens("你好"), 1);
        assert_eq!(estimate_tokens("你好世界"), 2);
        assert_eq!(estimate_tokens("你好世"), 2);
    }

    #[test]
    fn mixed_text_combines_rates() {
        assert_eq!(estimate_tokens("abcd你好"), 2);
        assert_eq!(estimate_tokens("abc你好世"), 3);
    }

    #[test]
    fn empty_is_zero() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_texts_tokens(&[]), 0);
    }
}
