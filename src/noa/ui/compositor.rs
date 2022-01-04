use super::terminal::Terminal;

pub struct Compositor {
    backend: Terminal,
}

impl Compositor {
    pub fn new(backend: Terminal) -> Compositor {
        Compositor { backend }
    }
}
