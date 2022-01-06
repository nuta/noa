pub trait DisplayWidth {
    fn display_width(&self) -> usize;
}

impl DisplayWidth for str {
    fn display_width(&self) -> usize {
        unicode_width::UnicodeWidthStr::width_cjk(self)
    }
}

impl DisplayWidth for char {
    fn display_width(&self) -> usize {
        unicode_width::UnicodeWidthChar::width_cjk(*self).unwrap_or(1)
    }
}
