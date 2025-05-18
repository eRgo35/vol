use crate::app::App;

pub fn configuration_tab(app: &App) -> String {
    let mut text = String::new();
    text.push_str("Configuration Tab\n");
    text.push_str(&format!("Counter: {}\n", app.counter));
    text
}
