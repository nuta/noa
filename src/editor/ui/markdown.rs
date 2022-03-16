use std::fmt::{self, Display, Formatter};

use noa_proxy::lsp_types::{MarkedString, MarkupContent};

#[derive(Debug)]
pub struct Markdown(String);

impl Markdown {
    pub fn new(text: String) -> Markdown {
        Markdown(text)
    }

    /// Returns the rendered text. Each string in the returned vector is a line.
    pub fn render(&self, width: usize) -> Vec<String> {
        // TODO: Parse self.0
        textwrap::wrap(&self.0, width)
            .into_iter()
            .map(|line| line.into_owned())
            .collect()
    }
}

impl Display for Markdown {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<MarkedString> for Markdown {
    fn from(s: MarkedString) -> Self {
        match s {
            MarkedString::String(s) => Markdown(s),
            MarkedString::LanguageString(s) => Markdown(s.value),
        }
    }
}

impl From<MarkupContent> for Markdown {
    fn from(s: MarkupContent) -> Self {
        Markdown(s.value)
    }
}
