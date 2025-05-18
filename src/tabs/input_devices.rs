use crate::app::App;

pub fn input_devices_tab(app: &App) -> String {
    let mut text = String::new();
    text.push_str("Input Devices Tab\n");
    text.push_str(&format!("Counter: {}\n", app.counter));
    text
}
