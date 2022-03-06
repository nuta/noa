use std::fmt::{self, Display, Formatter};

use noa_proxy::lsp_types::{MarkedString, MarkupContent};

pub struct Markdown(String);

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
