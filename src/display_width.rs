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

impl DisplayWidth for usize {
    fn display_width(&self) -> usize {
        let mut n = *self;
        match n {
            0..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            10000..=99999 => 5,
            _ => {
                let mut num = 1;
                loop {
                    n /= 10;
                    if n == 0 {
                        break;
                    }
                    num += 1;
                }
                num
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_width() {
        assert_eq!(0.display_width(), 1);
        assert_eq!(10.display_width(), 2);
        assert_eq!(101.display_width(), 3);
        assert_eq!('a'.display_width(), 1);
        assert_eq!("a   ".display_width(), 4);
        assert_eq!("aあb".display_width(), 4);
        assert_eq!("こんにちは".display_width(), 2 * 5);
        assert_eq!("こんにちは".display_width(), 2 * 5);
    }
}
