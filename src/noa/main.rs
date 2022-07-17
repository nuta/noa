mod editor;
mod ui;

#[tokio::main]
async fn main() {
    let mut editor = editor::Editor::new();
    let mut ui = ui::Ui::new(editor);
    ui.run();
}
