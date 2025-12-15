pub struct StatusLine {
    text: String,
}

impl StatusLine {
    pub fn new(text: String) -> Self {
        Self { text }
    }
}

impl Widget for StatusLine {
    fn render(&self, frame: &mut Frame) {
        frame.draw_text(self.text, 0, 0);
    }
}
