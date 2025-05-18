use crate::app::App;

pub fn playback_tab(app: &App) -> String {
    let mut text = String::new();
    text.push_str("Playback Tab\n");
    text.push_str(&format!("Counter: {}\n", app.counter));
    text
}
