use noa_langs::{
    tree_sitter::{self},
    Lang,
};

pub struct Highlighter {
    tree: Option<tree_sitter::Tree>,
    parser: Option<tree_sitter::Parser>,
}

impl Highlighter {
    pub fn new(lang: &'static Lang) -> Highlighter {
        Highlighter {
            tree: None,
            parser: lang.syntax_highlighting_parser(),
        }
    }

    pub fn update(&mut self, rope: &ropey::Rope) {
        if let Some(parser) = self.parser.as_mut() {
            self.tree = parser.parse_with(
                &mut |i, _| {
                    if i > rope.len_bytes() {
                        return &[] as &[u8];
                    }

                    let (chunk, start, _, _) = rope.chunk_at_byte(i);
                    chunk[i - start..].as_bytes()
                },
                None,
            );
        }
    }
}
