use crate::definitions::LANGUAGES;

pub struct Lsp {
    pub language_id: &'static str,
    pub command: &'static [&'static str],
}

pub fn get_lsp_config_by_lsp_lang_id(id: &str) -> Option<&'static Lsp> {
    LANGUAGES
        .iter()
        .find(|lang| match lang.lsp.as_ref() {
            Some(lsp) => lsp.language_id == id,
            None => false,
        })
        .map(|lang| lang.lsp.as_ref().unwrap())
}
