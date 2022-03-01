use noa_buffer::display_width::DisplayWidth;

pub fn truncate_to_width(s: &str, width: usize) -> &str {
    if s.display_width() <= width {
        return s;
    }

    let mut prev_substr = None;
    for (offset, _) in s.char_indices() {
        let substr = &s[..offset];
        if substr.display_width() > width {
            return prev_substr.unwrap_or("");
        }
        prev_substr = Some(substr);
    }

    prev_substr.unwrap_or(s)
}

pub fn truncate_to_width_suffix(s: &str, width: usize) -> &str {
    if s.display_width() <= width {
        return s;
    }

    let mut prev_substr = None;
    for (offset, _) in s.char_indices() {
        let substr = &s[s.len() - offset..];
        if substr.display_width() > width {
            return prev_substr.unwrap_or("");
        }
        prev_substr = Some(substr);
    }

    prev_substr.unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_to_width() {
        assert_eq!(truncate_to_width("", 0), "");
        assert_eq!(truncate_to_width("a", 0), "");
        assert_eq!(truncate_to_width("abc", 3), "abc");
        assert_eq!(truncate_to_width("abc", 2), "ab");
        assert_eq!(truncate_to_width("あい", 3), "あ");
        assert_eq!(truncate_to_width("あ", 1), "");
    }

    #[test]
    fn test_truncate_to_width_reserve() {
        assert_eq!(truncate_to_width_suffix("", 0), "");
        assert_eq!(truncate_to_width_suffix("a", 0), "");
        assert_eq!(truncate_to_width_suffix("abc", 3), "abc");
        assert_eq!(truncate_to_width_suffix("abc", 2), "bc");
        assert_eq!(truncate_to_width_suffix("あい", 3), "い");
        assert_eq!(truncate_to_width_suffix("あ", 1), "");
    }
}
