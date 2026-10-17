fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Logitech G510 Control Deck (Interactive TUI)");
        println!();
        println!("Usage: g510deck");
        println!("       g510tui");
        println!("       g510ctl tui");
        println!();
        println!("Keybindings:");
        println!("  Tab / Shift-Tab / 1-4 : Switch tabs (Lighting, Macros, LCD, Audio)");
        println!("  Up / Down (k / j)     : Navigate items & G-keys");
        println!("  Left / Right (h / l)  : Adjust brightness / cycle profiles");
        println!("  Enter / Space         : Apply color / open macro preset picker");
        println!("  q / Esc               : Quit Control Deck");
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("g510deck {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    g510::tui::run()
}
