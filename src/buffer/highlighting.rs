use crate::{
    cursor::{Position, Range},
    raw_buffer::RawBuffer,
};

use noa_languages::{
    language::{Language, SyntaxSpan},
    tree_sitter,
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
                Err(_) => None,
            }
        });

        Highlighter {
            lang,
            tree: None,
            parser,
        }
    }

    pub fn tree(&self) -> Option<&tree_sitter::Tree> {
        self.tree.as_ref()
    }

    pub fn update(&mut self, buffer: &RawBuffer) {
        let rope = buffer.rope();
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

    fn walk_ts_node<'tree, F>(
        &self,
        parent: tree_sitter::Node<'tree>,
        cursor: &mut tree_sitter::TreeCursor<'tree>,
        callback: &mut F,
    ) where
        F: FnMut(Range, SyntaxSpan),
    {
        for node in parent.children(cursor) {
            let range = node.buffer_range();

            if let Some(span) = self.lang.tree_sitter_mapping.get(node.kind()).copied() {
                callback(range, span);
            }

            let mut node_cursor = node.walk();
            if node.child_count() > 0 {
                self.walk_ts_node(node, &mut node_cursor, callback);
            }
        }
    }

    pub fn highlight<F>(&self, mut callback: F)
    where
        F: FnMut(Range, SyntaxSpan),
    {
        if let Some(tree) = self.tree.as_ref() {
            let root = tree.root_node();
            self.walk_ts_node(root, &mut root.walk(), &mut callback);
        }
    }
}

pub trait TsNodeExt {
    fn buffer_range(&self) -> Range;
}

impl<'tree> TsNodeExt for tree_sitter::Node<'tree> {
    fn buffer_range(&self) -> Range {
        let node_start = self.start_position();
        let node_end = self.end_position();
        let start_pos = Position::new(node_start.row, node_start.column);
        let end_pos = Position::new(node_end.row, node_end.column);
        Range::from_positions(start_pos, end_pos)
    }
}
