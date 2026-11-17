use settings_migrator_engine::{Os, VERSION};

fn main() {
    let os = match Os::current() {
        Some(Os::Windows) => "Windows",
        Some(Os::MacOs) => "macOS",
        None => "unsupported OS",
    };
    println!("settings-migrator {VERSION} ({os})");
}
