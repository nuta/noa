pub struct DisplayRow {}

pub struct View {
    rows: Vec<DisplayRow>,
    logical_x: usize,
}

impl View {
    pub fn new() -> View {
        View {
            rows: Vec::new(),
            logical_x: 0,
        }
    }

    pub fn update(&mut self, rope: &ropey::Rope) {
        //
    }
}
