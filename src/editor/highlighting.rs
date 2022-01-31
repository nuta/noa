

use noa_buffer::{
    buffer::Buffer,
    cursor::{Position, Range},
};

use noa_languages::{language::Language, tree_sitter};

use crate::{
    theme::{ThemeKey},
    view::View,
};

pub struct Highlighter {
    lang: &'static Language,
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

        Highlighter {
            lang,
            tree: None,
            parser,
        }
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

    fn walk_ts_node<'a, 'b, 'tree>(
        &self,
        view: &'a mut View,
        parent: tree_sitter::Node<'tree>,
        cursor: &'b mut tree_sitter::TreeCursor<'tree>,
    ) {
        for node in parent.children(cursor) {
            let node_start = node.start_position();
            let node_end = node.end_position();
            let start_pos = Position::new(node_start.row, node_start.column);
            let end_pos = Position::new(node_end.row, node_end.column);
            let range = Range::from_positions(start_pos, end_pos);

            if let Some(span) = self.lang.tree_sitter_mapping.get(node.kind()).copied() {
                view.highlight(range, ThemeKey::SyntaxSpan(span));
            }

            let mut node_cursor = node.walk();
            if node.child_count() > 0 {
                self.walk_ts_node(view, node, &mut node_cursor);
            }
        }
    }

    pub fn highlight(&mut self, view: &mut View) {
        if let Some(tree) = self.tree.as_ref() {
            let root = tree.root_node();
            self.walk_ts_node(view, root, &mut root.walk());
        }
    }
}
