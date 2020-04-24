
enum Context {

}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Normal,
}

pub struct Highlight {
    context: Vec<Context>,
}

impl Highlight {
    pub fn new() -> Highlight {
        Highlight {
            context: Vec::new(),
        }
    }

    pub fn highlight_line<'a>(&mut self, line: &'a str) -> Vec<(Style, &'a str)> {
        let mut spans = Vec::new();
        let mut start = 0;
        let mut style = Style::Normal;
        let mut prev_style = Style::Normal;
        for (i, c) in line.char_indices() {
            style = Style::Normal;
            if prev_style != style {
                spans.push((prev_style, &line[start..i - 1]));
                prev_style = style;
                start = i;
            }
        }

        let last_span = &line[start..];
        if !last_span.is_empty() {
            spans.push((style, last_span));
        }

        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight() {
        let mut h = Highlight::new();
        assert_eq!(h.highlight_line("hello world"), vec![]);
    }
}
