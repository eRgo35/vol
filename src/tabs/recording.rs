use crate::app::App;

pub fn recording_tab(app: &App) -> String {
    let mut text = String::new();
    text.push_str("Recording Tab\n");
    text.push_str(&format!("Counter: {}\n", app.counter));
    text
}
