use noa_buffer::display_width::DisplayWidth;

pub fn truncate_to_width(s: &str, width: usize) -> &str {
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
