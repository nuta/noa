// use clipboard::ClipboardProvider;
// use clipboard::ClipboardContext;

pub struct Clipboard {
    // ctx: ClipboardContext,
}

impl Clipboard {
    pub fn new() -> Clipboard {
        Clipboard {
            // ctx: ClipboardProvider::new().unwrap(),
        }
    }

    pub fn get(&mut self) -> String {
        String::new()
    }

    pub fn set(&mut self, text: &str) {
    }
}
