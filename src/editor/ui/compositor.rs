use super::terminal::Terminal;

pub struct Compositor {
    terminal: Terminal,
}

impl Compositor {
    pub fn new(terminal: Terminal) -> Compositor {
        Compositor { terminal }
    }
}
