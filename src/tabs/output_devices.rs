use crate::App;

pub fn output_devices_tab(app: &App) -> String {
    let mut text = String::new();
    text.push_str("Output Devices Tab\n");
    text.push_str(&format!("Counter: {}\n", app.counter));
    text
}
