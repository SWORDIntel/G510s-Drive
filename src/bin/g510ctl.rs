use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use clap::{Parser, Subcommand};

use g510::{
    events::{find_gaming_device, key_from_code},
    framebuffer::G510Framebuffer,
    leds::{set_profile, set_rgb, snapshot},
    macros::{default_config_path, load_config, save_config, MacroConfig, MacroEngine},
    render::render_text_screen,
    transport::G510Transport,
};

#[derive(Parser, Debug)]
#[command(name = "g510ctl", version, about = "Logitech G510/G510s CLI Controller")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Probe Logitech G510 USB and evdev devices
    Probe,
    /// Display current LED and hardware status
    Status,
    /// Set RGB backlighting color
    Rgb {
        /// Color in hex format (e.g. #00dc00 or 00dc00)
        color: String,
        /// Brightness level (0-255)
        #[arg(long, default_value_t = 255)]
        brightness: u8,
        /// Target device: live or power-on
        #[arg(long, default_value = "live")]
        target: String,
    },
    /// Switch active M-profile indicator LED (1, 2, or 3)
    Profile {
        /// Profile number (1, 2, or 3)
        profile: u8,
    },
    /// Render up to 3 lines of text on the LCD screen
    Text {
        /// Text message to display
        message: String,
        /// Invert pixels (black text on white background)
        #[arg(long)]
        invert: bool,
        /// Hold screen until interrupted (SIGINT)
        #[arg(long)]
        hold: bool,
    },
    /// Initialize default macro configuration file
    InitConfig {
        /// Custom destination path
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Listen for G-keys, M-keys, and LCD buttons
    Listen {
        /// Output events in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Run foreground macro execution engine
    Run {
        /// Path to macro config file
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Probe => {
            println!("Probing Logitech G510 devices...");
            match g510::transport::find_hidraw_device() {
                Some(path) => println!("  [+] LCD hidraw device found: {}", path.display()),
                None => println!("  [-] LCD hidraw device not found (/dev/g510-lcd)"),
            }

            match find_gaming_device() {
                Ok((path, dev)) => println!(
                    "  [+] Gaming evdev device found: {} (Name: {:?})",
                    path.display(),
                    dev.name()
                ),
                Err(e) => println!("  [-] Gaming evdev device not found: {e}"),
            }
        }
        Commands::Status => {
            let snap = snapshot(None);
            println!("{}", serde_json::to_string_pretty(&snap)?);
        }
        Commands::Rgb {
            color,
            brightness,
            target,
        } => {
            let state = set_rgb(&color, brightness, &target, None)?;
            println!(
                "RGB set: {} (brightness: {}/{}) at {}",
                state.hex, state.brightness, state.max_brightness, state.led_path
            );
        }
        Commands::Profile { profile } => {
            set_profile(profile, None)?;
            println!("Switched to profile M{profile}");
        }
        Commands::Text {
            message,
            invert,
            hold,
        } => {
            let mut fb = G510Framebuffer::new();
            render_text_screen(&mut fb, &message);

            let mut transport = G510Transport::open()?;
            transport.write_framebuffer(&fb, invert)?;

            if hold {
                println!("Text displayed. Holding (Ctrl+C to stop)...");
                while transport.write_framebuffer(&fb, invert).is_ok() {
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }
        Commands::InitConfig { path } => {
            let config_path = path.unwrap_or_else(default_config_path);
            if config_path.exists() {
                eprintln!("Config already exists at {}", config_path.display());
            } else {
                let default_cfg = MacroConfig::default();
                save_config(&default_cfg, &config_path)?;
                println!("Created default configuration at {}", config_path.display());
            }
        }
        Commands::Listen { json } => {
            let (path, mut device) = find_gaming_device()?;
            eprintln!("Listening for special keys on {}...", path.display());

            loop {
                for event in device.fetch_events()? {
                    if event.event_type() == evdev::EventType::KEY {
                        let value = event.value();
                        if value == 0 || value == 1 {
                            if let Some(key) = key_from_code(event.code()) {
                                if json {
                                    println!(
                                        "{{\"key\": \"{}\", \"pressed\": {}}}",
                                        key.name(),
                                        value == 1
                                    );
                                } else {
                                    println!(
                                        "Key: {:<6} -> {}",
                                        key.name(),
                                        if value == 1 { "PRESSED" } else { "RELEASED" }
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        Commands::Run { config } => {
            let config_path = config.unwrap_or_else(default_config_path);
            let cfg = if config_path.exists() {
                load_config(&config_path)?
            } else {
                eprintln!("Config not found at {}; using defaults", config_path.display());
                MacroConfig::default()
            };

            let mut engine = MacroEngine::new(cfg);
            let (path, mut device) = find_gaming_device()?;
            eprintln!("Running G510 Macro Engine on {}...", path.display());

            loop {
                for event in device.fetch_events()? {
                    if event.event_type() == evdev::EventType::KEY && event.value() == 1 {
                        if let Some(key) = key_from_code(event.code()) {
                            engine.handle_key(&key.name());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
