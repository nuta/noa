use std::{collections::HashMap, ops::ControlFlow};

use crate::{
    cursor::{Position, Range},
    mutable_raw_buffer::Change,
    raw_buffer::RawBuffer,
};

use noa_languages::{
    tree_sitter::{
        self, get_highlights_query, get_indents_query, get_tree_sitter_parser, InputEdit, Node,
        QueryCursor, TextProvider,
    },
    Language,
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

pub struct Query {
    raw_query: tree_sitter::Query,
    indices: HashMap<usize, String>,
}

impl Query {
    pub fn new(
        ts_lang: tree_sitter::Language,
        query_str: &str,
    ) -> Result<Query, tree_sitter::QueryError> {
        let raw_query = tree_sitter::Query::new(ts_lang, query_str)?;
        let mut indices = HashMap::new();
        for (i, name) in raw_query.capture_names().iter().enumerate() {
            indices.insert(i, name.to_owned());
        }

        Ok(Query { raw_query, indices })
    }

    pub fn query<F>(
        &self,
        tree: &tree_sitter::Tree,
        buffer: &RawBuffer,
        query_range: Option<Range>,
        mut callback: F,
    ) where
        F: FnMut(Range, &str),
    {
        let mut cursor = QueryCursor::new();
        if let Some(range) = query_range {
            cursor.set_point_range(range.into());
        }

        let matches = cursor.matches(&self.raw_query, tree.root_node(), RopeTextProvider(buffer));
        for m in matches {
            for cap in m.captures {
                if let Some(span) = self.indices.get(&m.pattern_index) {
                    callback(cap.node.buffer_range(), span);
                }
            }
        }
    }

    pub fn captures<F>(
        &self,
        tree: &tree_sitter::Tree,
        buffer: &RawBuffer,
        query_range: Option<Range>,
        mut callback: F,
    ) where
        F: FnMut(Range, &str),
    {
        let mut cursor = QueryCursor::new();
        if let Some(range) = query_range {
            cursor.set_point_range(range.into());
        }

        let captures = cursor.captures(&self.raw_query, tree.root_node(), RopeTextProvider(buffer));
        for (m, _) in captures {
            for cap in m.captures {
                if let Some(span) = self.indices.get(&(cap.index as usize)) {
                    callback(cap.node.buffer_range(), span);
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParserError {
    NotSupportedLanguage,
    LanguageError(tree_sitter::LanguageError),
    QueryError(tree_sitter::QueryError),
    ParseError,
}

pub struct SyntaxParser {
    parser: tree_sitter::Parser,
    ts_lang: tree_sitter::Language,
    tree: tree_sitter::Tree,
}

impl SyntaxParser {
    pub fn new(lang: &Language) -> Result<SyntaxParser, ParserError> {
        let mut parser = tree_sitter::Parser::new();
        let ts_lang = get_tree_sitter_parser(lang.name).ok_or(ParserError::NotSupportedLanguage)?;
        parser
            .set_language(ts_lang)
            .map_err(ParserError::LanguageError)?;

        Ok(SyntaxParser {
            tree: parser.parse("", None).ok_or(ParserError::ParseError)?,
            ts_lang,
            parser,
        })
    }

    pub fn tree(&self) -> &tree_sitter::Tree {
        &self.tree
    }

    pub fn parse_fully(&mut self, buffer: &RawBuffer) {
        self.parse(buffer, None);
    }

    pub fn parse_incrementally(&mut self, buffer: &RawBuffer, changes: &[Change]) {
        self.parse(buffer, Some(changes));
    }

    fn parse(&mut self, buffer: &RawBuffer, changes: Option<&[Change]>) {
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
}

pub struct Syntax {
    tree: tree_sitter::Tree,
    highlight_query: Query,
    indents_query: Query,
}

impl Syntax {
    pub fn new(lang: &'static Language) -> Result<Syntax, ParserError> {
        let parser = SyntaxParser::new(lang)?;
        let highlight_query = Query::new(
            parser.ts_lang,
            get_highlights_query(&lang.name).unwrap_or(""),
        )
        .map_err(ParserError::QueryError)?;
        let indents_query = Query::new(parser.ts_lang, get_indents_query(&lang.name).unwrap_or(""))
            .map_err(ParserError::QueryError)?;

        Ok(Syntax {
            tree: parser.tree.clone(),
            highlight_query,
            indents_query,
        })
    }

    pub fn tree(&self) -> &tree_sitter::Tree {
        &self.tree
    }

    pub fn set_tree(&mut self, tree: tree_sitter::Tree) {
        self.tree = tree;
    }

    pub fn query_highlight<F>(&self, buffer: &RawBuffer, range: Range, mut callback: F)
    where
        F: FnMut(Range, &str),
    {
        self.highlight_query
            .query(self.tree(), buffer, Some(range), &mut callback);
    }

    pub fn query_indents<F>(&self, buffer: &RawBuffer, range: Range, mut callback: F)
    where
        F: FnMut(Range, &str),
    {
        self.indents_query
            .captures(self.tree(), buffer, Some(range), &mut callback);
    }

    pub fn words<F>(&self, mut callback: F)
    where
        F: FnMut(Range) -> ControlFlow<()>,
    {
        const WORD_LEN_MAX: usize = 32;

        self.visit_all_nodes(|node, range| {
            if range.start.y != range.end.y {
                return ControlFlow::Continue(());
            }

            if range.start.x.abs_diff(range.end.x) > WORD_LEN_MAX {
                return ControlFlow::Continue(());
            }

            if !node.kind().ends_with("identifier") {
                return ControlFlow::Continue(());
            }

            callback(range)
        });
    }

    pub fn visit_all_nodes<F>(&self, mut callback: F)
    where
        F: FnMut(&tree_sitter::Node<'_>, Range) -> ControlFlow<()>,
    {
        let root = self.tree.root_node();
        self.visit_ts_node(root, &mut root.walk(), &mut callback);
    }

    fn visit_ts_node<'a, 'b, 'tree, F>(
        &self,
        parent: tree_sitter::Node<'tree>,
        cursor: &'b mut tree_sitter::TreeCursor<'tree>,
        callback: &mut F,
    ) -> ControlFlow<()>
    where
        F: FnMut(&tree_sitter::Node<'tree>, Range) -> ControlFlow<()>,
    {
        for node in parent.children(cursor) {
            let node_start = node.start_position();
            let node_end = node.end_position();
            let start_pos = Position::new(node_start.row, node_start.column);
            let end_pos = Position::new(node_end.row, node_end.column);
            let range = Range::from_positions(start_pos, end_pos);

            if callback(&node, range) == ControlFlow::Break(()) {
                return ControlFlow::Break(());
            }

            let mut node_cursor = node.walk();
            if node.child_count() > 0
                && self.visit_ts_node(node, &mut node_cursor, callback) == ControlFlow::Break(())
            {
                return ControlFlow::Break(());
            }
        }

        ControlFlow::Continue(())
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
