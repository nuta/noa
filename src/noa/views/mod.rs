use noa_buffer::display_width::DisplayWidth;

pub mod buffer_view;
pub mod metaline_view;

pub(super) fn truncate_to_width_suffix(s: &str, width: usize) -> &str {
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
