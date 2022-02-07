use std::{collections::HashMap, str::FromStr};

use crate::{
    cursor::{Position, Range},
    raw_buffer::RawBuffer,
};

use noa_languages::{
    language::{Language, SyntaxSpan},
    tree_sitter::{self, Node, Query, QueryCursor, TextProvider},
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

    pub fn update(&mut self, buffer: &RawBuffer) {
        let rope = buffer.rope();
        if let Some(new_tree) = self.parser.parse_with(
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
        ) {
            self.tree = new_tree;
        }
    }

    pub fn highlight<F>(&mut self, mut callback: F, buffer: &RawBuffer)
    where
        F: FnMut(Range, SyntaxSpan),
    {
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(
            &self.highlight_query,
            self.tree.root_node(),
            RopeTextProvider(buffer),
        );

        for m in matches {
            for cap in m.captures {
                if let Some(span) = self
                    .highlight_query_indices
                    .get(&m.pattern_index)
                    .and_then(|name| SyntaxSpan::from_str(name).ok())
                {
                    callback(cap.node.buffer_range(), span);
                }
            }
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
