use noa_compositor::{
    canvas::CanvasViewMut,
    surface::{HandledEvent, KeyEvent, Layout, MouseEvent, RectSize, Surface},
    terminal::{KeyCode, KeyModifiers},
};
use tokio::{sync::oneshot, task};

use crate::{
    clipboard::{ClipboardData, SystemClipboardData},
    editor::Editor,
};

pub struct BufferView {
    quit_tx: Option<oneshot::Sender<()>>,
    /// `(y, x)`.
    cursor_position: (usize, usize),
}

impl BufferView {
    pub fn new(quit_tx: oneshot::Sender<()>) -> BufferView {
        BufferView {
            quit_tx: Some(quit_tx),
            cursor_position: (0, 0),
        }
    }
}

impl Surface for BufferView {
    type Context = Editor;

    fn name(&self) -> &str {
        "buffer"
    }

    fn is_visible(&self, _editor: &mut Editor) -> bool {
        true
    }

    fn layout(&self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        (Layout::Fixed { y: 0, x: 0 }, screen_size)
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        Some(self.cursor_position)
    }

    fn render(&mut self, editor: &mut Editor, mut canvas: CanvasViewMut<'_>) {
        canvas.clear();
    }

    fn handle_key_event(&mut self, editor: &mut Editor, key: KeyEvent) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let mut notifications = &mut editor.notifications;
        let mut doc = editor.documents.current_mut();

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                self.quit_tx.take().unwrap().send(());
            }
            (KeyCode::Char('s'), CTRL) => {
                notifications.maybe_error(doc.save_to_file());
            }
            (KeyCode::Char('u'), CTRL) => {
                doc.buffer_mut().undo();
            }
            (KeyCode::Char('y'), CTRL) => {
                doc.buffer_mut().redo();
            }
            (KeyCode::Char('c'), CTRL) => {
                editor
                    .clipboard
                    .copy_into_clipboard(ClipboardData::from_buffer(doc.buffer()));
            }
            (KeyCode::Char('x'), CTRL) => {
                let buffer = doc.buffer_mut();
                match editor.clipboard.copy_from_clipboard() {
                    Ok(SystemClipboardData::Ours(ClipboardData { texts })) => {
                        let strs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
                        buffer.insert_multiple(&strs);
                    }
                    Ok(SystemClipboardData::Others(string)) => {
                        buffer.insert(&string);
                    }
                    Err(err) => {
                        error!("failed to copy from clipboard: {:?}", err);
                    }
                }
            }
            (KeyCode::Char('k'), CTRL) => {
                // doc.buffer_mut().truncate();
            }
            (KeyCode::Char('a'), CTRL) => {
                //                //                f.buffer.move_to_beginning_of_line();
            }
            (KeyCode::Char('e'), CTRL) => {
                //                //                f.buffer.move_to_end_of_line();
            }
            (KeyCode::Char('f'), ALT) => {
                //                //                f.buffer.move_to_next_word();
            }
            (KeyCode::Char('b'), ALT) => {
                //                //                f.buffer.move_to_prev_word();
            }
            (KeyCode::Up, ALT) => {
                //                //                f.buffer.move_current_line_above();
            }
            (KeyCode::Down, ALT) => {
                //                //                f.buffer.move_current_line_below();
            }
            (KeyCode::Up, modifiers) if modifiers == (CTRL | ALT) => {
                // TODO:
                //                // f.buffer.add_cursor_above();
            }
            (KeyCode::Down, modifiers) if modifiers == (CTRL | ALT) => {
                // TODO:
                //                // f.buffer.add_cursor_below();
            }
            (KeyCode::Up, modifiers) if modifiers == (SHIFT | ALT) => {
                // TODO:
                //                // f.buffer.duplicate_line_above();
            }
            (KeyCode::Down, modifiers) if modifiers == (SHIFT | ALT) => {
                // TODO:
                //                // f.buffer.duplicate_line_below();
            }
            (KeyCode::Char('w'), CTRL) => {
                // doc.buffer_mut()
            }
            (KeyCode::Backspace, NONE) => {
                doc.buffer_mut().backspace();
            }
            (KeyCode::Char('d'), CTRL) | (KeyCode::Delete, _) => {
                doc.buffer_mut().delete();
            }
            (KeyCode::Up, NONE) => {
                //                f.move_cursors(-1, 0);
            }
            (KeyCode::Down, NONE) => {
                //                f.move_cursors(1, 0);
            }
            (KeyCode::Left, NONE) => {
                //                f.move_cursors(0, -1);
            }
            (KeyCode::Right, NONE) => {
                //                f.move_cursors(0, 1);
            }
            (KeyCode::Up, SHIFT) => {
                //                f.expand_selections(-1, 0);
            }
            (KeyCode::Down, SHIFT) => {
                //                f.expand_selections(1, 0);
            }
            (KeyCode::Left, SHIFT) => {
                //                f.expand_selections(0, -1);
            }
            (KeyCode::Right, SHIFT) => {
                //                f.expand_selections(0, 1);
            }
            (KeyCode::Enter, NONE) => {
                doc.buffer_mut().insert_newline_and_indent();
            }
            (KeyCode::Tab, NONE) => {
                doc.buffer_mut().deindent();
            }
            (KeyCode::BackTab, NONE) => {
                doc.buffer_mut().indent();
            }
            (KeyCode::Char(ch), NONE) => {
                doc.buffer_mut().insert_char(ch);
            }
            (KeyCode::Char(ch), SHIFT) => {
                doc.buffer_mut().insert_char(ch);
            }
            _ => {
                trace!("unhandled key = {:?}", key);
            }
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(&mut self, editor: &mut Editor, s: &str) -> HandledEvent {
        editor.documents.current_mut().buffer_mut().insert(s);
        HandledEvent::Consumed
    }

    fn handle_mouse_event(&mut self, editor: &mut Editor, _ev: MouseEvent) -> HandledEvent {
        HandledEvent::Ignored
    }
}
