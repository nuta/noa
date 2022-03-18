use std::cmp::min;

use noa_buffer::{cursor::Position, display_width::DisplayWidth};

use noa_terminal::{
    canvas::{CanvasViewMut, Color, Style},
    terminal::{KeyCode, KeyEvent, KeyModifiers},
};

use crate::{
    editor::Editor,
    theme::theme_for,
    ui::{
        compositor::Compositor,
        helpers::truncate_to_width,
        line_edit::LineEdit,
        surface::{HandledEvent, Layout, RectSize, Surface, UIContext},
    },
};

const HEIGHT_MAX: usize = 16;

pub type SelectedCallback = dyn FnOnce(&mut Compositor, &mut Editor) + Send;
pub type ChangedCallback = dyn FnMut(&mut Editor, &str) + Send;

pub enum SelectorContent {
    Normal {
        label: String,
        sub_label: Option<String>,
    },
    SearchMatch {
        path: String,
        pos: Position,
        line_text: String,
        before: std::ops::RangeTo<usize>,
        matched: std::ops::Range<usize>,
        after: std::ops::RangeFrom<usize>,
    },
}

pub struct SelectorItem {
    pub content: SelectorContent,
    pub selected: Box<SelectedCallback>,
}

pub struct SelectorView {
    opened_by: &'static str,
    active: bool,
    cursor_pos: (usize, usize),
    items: Vec<SelectorItem>,
    selected_index: usize,
    scroll: usize,
    items_height: usize,
    input: Option<LineEdit>,
    changed_callback: Option<Box<ChangedCallback>>,
}

impl SelectorView {
    pub fn new() -> SelectorView {
        SelectorView {
            opened_by: "",
            active: false,
            cursor_pos: (0, 0),
            items: Vec::new(),
            selected_index: 0,
            scroll: 0,
            items_height: 0,
            input: None,
            changed_callback: None,
        }
    }

    pub fn opened_by(&self) -> &'static str {
        self.opened_by
    }

    pub fn open(
        &mut self,
        opened_by: &'static str,
        eanble_input: bool,
        changed_callback: Option<Box<ChangedCallback>>,
    ) {
        self.opened_by = opened_by;
        self.active = true;
        self.selected_index = 0;
        self.scroll = 0;
        self.items = Vec::new();
        self.changed_callback = changed_callback;

        if eanble_input {
            self.input = Some(LineEdit::new());
        } else {
            self.input = None;
        }
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    pub fn set_items(&mut self, items: Vec<SelectorItem>) {
        self.items = items;
        self.selected_index = min(self.selected_index, self.items.len().saturating_sub(1));
        self.adjust_scroll();
    }

    pub fn adjust_scroll(&mut self) {
        while self.scroll + self.items_height <= self.selected_index {
            self.scroll += 1;
        }

        while self.scroll > self.selected_index {
            self.scroll = self.scroll.saturating_sub(1);
        }
    }
}

impl Surface for SelectorView {
    fn name(&self) -> &str {
        "selector"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, _ctx: &mut UIContext) -> bool {
        self.active
    }

    fn layout(&mut self, _ctx: &mut UIContext, screen_size: RectSize) -> (Layout, RectSize) {
        let height = min(
            self.items.len() + if self.input.is_some() { 1 } else { 0 },
            min(HEIGHT_MAX, screen_size.height),
        );

        self.cursor_pos = match self.input.as_ref() {
            Some(input) => (height.saturating_sub(1), 1 + input.cursor_position()),
            None => (0, 0),
        };

        (
            Layout::Fixed {
                y: screen_size.height.saturating_sub(height + 1),
                x: 0,
            },
            RectSize {
                height,
                width: screen_size.width,
            },
        )
    }

    fn cursor_position(&self, _ctx: &mut UIContext) -> Option<(usize, usize)> {
        if self.active {
            Some(self.cursor_pos)
        } else {
            None
        }
    }

    fn render(&mut self, _ctx: &mut UIContext, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        self.items_height =
            canvas
                .height()
                .saturating_sub(if self.input.is_some() { 1 } else { 0 });

        for (i, item) in self
            .items
            .iter()
            .skip(self.scroll)
            .take(self.items_height)
            .enumerate()
        {
            match &item.content {
                SelectorContent::Normal {
                    label,
                    sub_label: _,
                } => {
                    canvas.write_str(i, 1, truncate_to_width(label, canvas.width() - 2));
                }
                SelectorContent::SearchMatch {
                    path,
                    pos,
                    line_text,
                    before,
                    after,
                    matched,
                } => {
                    let before_text = &line_text[..before.end];
                    let matched_text = &line_text[matched.start..matched.end];
                    let after_text = &line_text[after.start..];
                    let s = format!(
                        "{before_text}{matched_text}{after_text} ({path}:{lineno})",
                        lineno = pos.y + 1
                    );
                    canvas.write_str(i, 1, truncate_to_width(&s, canvas.width() - 2));

                    let x = before_text.display_width();
                    canvas.apply_style(
                        i,
                        x,
                        min(canvas.width(), x + matched_text.display_width()),
                        Style {
                            fg: Color::Red,
                            ..Default::default()
                        },
                    );
                }
            }

            if self.scroll + i == self.selected_index {
                canvas.apply_style(i, 0, canvas.width(), theme_for("selector.selected"));
            }
        }

        if let Some(input) = self.input.as_mut() {
            input.relocate_scroll(canvas.width());
            canvas.write_str(
                self.items_height,
                1,
                truncate_to_width(&input.text(), canvas.width() - 2),
            );
            canvas.apply_style(
                self.items_height,
                1,
                canvas.width() - 2,
                theme_for("selector.input"),
            );
        }
    }

    fn handle_key_event(
        &mut self,
        UIContext { editor }: &mut UIContext,
        compositor: &mut Compositor,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        match (key.code, key.modifiers) {
            (KeyCode::Enter, NONE) => {
                if self.selected_index >= self.items.len() {
                    warn!("out of bounds selected_index");
                    return HandledEvent::Consumed;
                }

                let item = self.items.remove(self.selected_index);
                (item.selected)(compositor, editor);
                self.close();
            }
            (KeyCode::Down, NONE) => {
                self.selected_index =
                    min(self.selected_index + 1, self.items.len().saturating_sub(1));
                self.adjust_scroll();
            }
            (KeyCode::Up, NONE) => {
                self.selected_index = self.selected_index.saturating_sub(1);
                self.adjust_scroll();
            }
            (KeyCode::Char('q'), CTRL) => {
                self.close();
            }
            _ => {
                if let Some(input) = self.input.as_mut() {
                    let prev_text = input.text();
                    input.consume_key_event(key);
                    let text = input.text();
                    if prev_text != text {
                        if let Some(callback) = self.changed_callback.as_mut() {
                            callback(editor, &text);
                        }
                    }
                }
            }
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _ctx: &mut UIContext,
        _compositor: &mut Compositor,
        text: &str,
    ) -> HandledEvent {
        if let Some(input) = self.input.as_mut() {
            input.insert(&text.replace('\n', " "));
        }

        HandledEvent::Consumed
    }

    fn handle_mouse_event(
        &mut self,
        _ctx: &mut UIContext,
        _compositor: &mut Compositor,
        _kind: noa_terminal::terminal::MouseEventKind,
        _modifiers: noa_terminal::terminal::KeyModifiers,
        _surface_y: usize,
        _surface_x: usize,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }
}
