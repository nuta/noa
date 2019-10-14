use crate::view::View;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub const fn new(line: usize, column: usize) -> Position {
        Position { line, column }
    }
}

pub struct Panel {
    views: Vec<View>,
    current_view_index: usize,
    top_left: Position,
    height: usize,
    width: usize,
}

impl Panel {
    pub fn new(top_left: Position, height: usize, width: usize, views: Vec<View>) -> Panel {
        Panel {
            views,
            current_view_index: 0,
            top_left,
            height,
            width,
        }
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn top_left(&self) -> &Position {
        &self.top_left
    }

    pub fn views(&self) -> &[View] {
        &self.views
    }

    pub fn current_view(&self) -> &View {
        &self.views[self.current_view_index]
    }

    pub fn current_view_mut(&mut self) -> &mut View {
        &mut self.views[self.current_view_index]
    }

    pub fn add_view(&mut self, view: View) {
        self.views.push(view);
        // Make the newly added view active.
        self.current_view_index = self.views.len() - 1;
    }
}

pub struct Layout {
    panels: Vec<Panel>,
    current_panel_index: usize,
}

impl Layout {
    pub fn new(scratch_view: View, height: usize, width: usize) -> Layout {
        let views = vec![scratch_view];
        let panel = Panel::new(Position::new(0, 0), height, width, views);
        Layout {
            panels: vec![panel],
            current_panel_index: 0,
        }
    }

    pub fn panels(&self) -> &[Panel] {
        &self.panels
    }

    pub fn current_panel(&self) -> &Panel {
        &self.panels[self.current_panel_index]
    }

    pub fn current_panel_mut(&mut self) -> &mut Panel {
        &mut self.panels[self.current_panel_index]
    }

    pub fn active_view(&self) -> &View {
        self.current_panel().current_view()
    }

    pub fn active_view_mut(&mut self) -> &mut View {
        self.current_panel_mut().current_view_mut()
    }
}
