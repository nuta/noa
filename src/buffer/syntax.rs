use std::collections::HashMap;

use crate::{
    cursor::{Position, Range},
    raw_buffer::RawBuffer,
    undoable_raw_buffer::Change,
};

use noa_languages::{
    language::Language,
    tree_sitter::{self, InputEdit, Node, Query, QueryCursor, TextProvider},
};

struct RopeByteChunks<'a>(ropey::iter::Chunks<'a>);

impl<'a> Iterator for RopeByteChunks<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(str::as_bytes)
    }
}

struct RopeTextProvider<'a>(&'a RawBuffer);

impl<'a> TextProvider<'a> for RopeTextProvider<'a> {
    type I = RopeByteChunks<'a>;

    fn text(&mut self, node: Node) -> Self::I {
        RopeByteChunks(self.0.rope_slice(node.buffer_range()).chunks())
    }
}

pub struct Syntax {
    tree: tree_sitter::Tree,
    parser: tree_sitter::Parser,
    highlight_query: tree_sitter::Query,
    highlight_query_indices: HashMap<usize, String>,
}

impl Syntax {
    pub fn new(lang: &'static Language) -> Option<Syntax> {
        lang.tree_sitter.as_ref().and_then(|def| {
            let mut parser = tree_sitter::Parser::new();
            let lang = (def.get_language)();
            match parser.set_language(lang) {
                Ok(()) => {
                    // TODO: Parse the query only once in noa_languages.
                    let highlight_query =
                        Query::new(lang, def.highlight_query).expect("invalid highlight query");

                    let mut highlight_query_indices = HashMap::new();
                    for (i, name) in highlight_query.capture_names().iter().enumerate() {
                        highlight_query_indices.insert(i, name.to_owned());
                    }

                    Some(Syntax {
                        tree: parser.parse("", None).unwrap(),
                        parser,
                        highlight_query,
                        highlight_query_indices,
                    })
                }
                Err(_) => None,
            }
        })
    }

    pub fn tree(&self) -> &tree_sitter::Tree {
        &self.tree
    }

    /// If `changes` is `None`, it will parse the full text (for the first run).
    pub fn update(&mut self, buffer: &RawBuffer, changes: Option<&[Change]>) {
        let rope = buffer.rope();
        let mut callback = |i, _| {
            if i > rope.len_bytes() {
                return &[] as &[u8];
            }

            let (chunk, start, _, _) = rope.chunk_at_byte(i);
            chunk[i - start..].as_bytes()
        };

        let old_tree = if let Some(changes) = changes {
            // Tell tree-sitter about the changes we made since the last parsing.
            for change in changes {
                self.tree.edit(&InputEdit {
                    start_byte: change.byte_range.start,
                    old_end_byte: change.byte_range.end,
                    new_end_byte: change.byte_range.start + change.insert_text.len(),
                    start_position: change.range.front().into(),
                    old_end_position: change.range.back().into(),
                    new_end_position: change.new_pos.into(),
                });
            }

            Some(&self.tree)
        } else {
            None
        };

        if let Some(new_tree) = self.parser.parse_with(&mut callback, old_tree) {
            self.tree = new_tree;
        }
    }

    pub fn highlight<F>(&mut self, mut callback: F, buffer: &RawBuffer, range: Range)
    where
        F: FnMut(Range, &str),
    {
        let mut cursor = QueryCursor::new();
        cursor.set_point_range(range.into());

        let matches = cursor.matches(
            &self.highlight_query,
            self.tree.root_node(),
            RopeTextProvider(buffer),
        );

        for m in matches {
            for cap in m.captures {
                if let Some(span) = self.highlight_query_indices.get(&m.pattern_index) {
                    callback(cap.node.buffer_range(), span);
                }
            }
        }
    }
}

impl From<Position> for tree_sitter::Point {
    fn from(pos: Position) -> Self {
        tree_sitter::Point {
            row: pos.y,
            column: pos.x,
        }
    }
}

impl From<Range> for std::ops::Range<tree_sitter::Point> {
    fn from(range: Range) -> Self {
        range.front().into()..range.back().into()
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
