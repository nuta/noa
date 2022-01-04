use noa_buffer::buffer::Buffer;
use noa_languages::{language::Language, tree_sitter};

pub struct Highlighter {
    tree: Option<tree_sitter::Tree>,
    parser: Option<tree_sitter::Parser>,
}

impl Highlighter {
    pub fn new(lang: &'static Language) -> Highlighter {
        let parser = lang.tree_sitter_language.as_ref().and_then(|get_lang| {
            let mut parser = tree_sitter::Parser::new();
            match parser.set_language(get_lang()) {
                Ok(()) => Some(parser),
                Err(err) => {
                    error!("failed to load tree sitter: {}", err);
                    None
                }
            }
        });

        Highlighter { tree: None, parser }
    }

    pub fn update(&mut self, buffer: &Buffer) {
        let rope = buffer.raw_buffer().rope();
        if let Some(parser) = self.parser.as_mut() {
            self.tree = parser.parse_with(
                &mut |i, _| {
                    if i > rope.len_bytes() {
                        return &[] as &[u8];
                    }

                    let (chunk, start, _, _) = rope.chunk_at_byte(i);
                    chunk[i - start..].as_bytes()
                },
                // TODO: Support incremental parsing.
                // https://github.com/mcobzarenco/zee/blob/8c21f387ee7805a185c3321f6a982bd3332701d3/core/src/syntax/parse.rs#L173-L183
                None,
            );
        }
    }
}
