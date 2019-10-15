use std::ops::Range;
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::highlighting::{
    Theme, ThemeSet, Style, Highlighter, HighlightState, RangedHighlightIterator
};
use lazy_static::lazy_static;
use crate::buffer::Buffer;

pub struct HighlightManager {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

lazy_static! {
    // TODO: Remove this global one.
    static ref GLOBAL_HIGHLIGHT_MANAGER: HighlightManager = {
        HighlightManager {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    };
}

impl HighlightManager {
    pub fn new() -> &'static HighlightManager {
        &GLOBAL_HIGHLIGHT_MANAGER
    }

    pub fn create_highlight(
        &'static self,
        theme_name: &str,
        file_extension: &str
    ) -> Option<Highlight> {
        let default_theme_name = "Solarized (light)";
        let theme = match self.theme_set.themes.get(theme_name) {
            Some(theme) => theme,
            // Invalid theme name.
            None => &self.theme_set.themes[default_theme_name],
        };

        self.syntax_set.find_syntax_by_extension(file_extension)
            .map(|syntax| Highlight::new(&self.syntax_set, theme, syntax))
    }
}

pub struct HighlightedLine(Vec<(Style, Range<usize>)>);

impl HighlightedLine {
    pub fn spans(&self) -> &[(Style, Range<usize>)] {
        &self.0
    }
}

pub struct Highlight {
    highlighter: Highlighter<'static>,
    syntax_set: &'static SyntaxSet,
    parse_state: ParseState,
    highlight_state: HighlightState,
    init_parse_state: ParseState,
    init_highlight_state: HighlightState,
    lines: Vec<HighlightedLine>,
}

impl Highlight {
    pub fn new(
        syntax_set: &'static SyntaxSet,
        theme: &'static Theme,
        syntax: &'static SyntaxReference
    ) -> Highlight {
        let highlighter = Highlighter::new(theme);
        let highlight_state =
            HighlightState::new(&highlighter, ScopeStack::new());

        Highlight {
            highlighter,
            syntax_set,
            parse_state: ParseState::new(syntax),
            highlight_state: highlight_state.clone(),
            init_parse_state: ParseState::new(syntax),
            init_highlight_state: highlight_state.clone(),
            lines: Vec::with_capacity(1024),
        }
    }

    pub fn parse(&mut self, _line_from: usize, buffer: &Buffer) {
        // TODO: Implement caching.
        let line_from = 0;

        self.highlight_state = self.init_highlight_state.clone();
        self.parse_state = self.init_parse_state.clone();

        self.lines.truncate(line_from);
        for line in buffer.lines_from(line_from) {
            let changes = self.parse_state.parse_line(&line, self.syntax_set);
            let iter = RangedHighlightIterator::new(&mut self.highlight_state,
                &changes, &line, &self.highlighter);
            let spans = iter.map(|e| (e.0, e.2)).collect();
            self.lines.push(HighlightedLine(spans));
        }
    }

    pub fn lines(&self) -> &[HighlightedLine] {
        &self.lines
    }
}
