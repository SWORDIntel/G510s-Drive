use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use clap::{Parser, Subcommand};

use g510::{
    events::{find_gaming_device, key_from_code},
    framebuffer::G510Framebuffer,
    ipc::{send_ipc_request, IpcRequest},
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
    /// Display current LED, daemon, and hardware status
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
    /// Switch active LCD carousel page (1=SYS, 2=NET, 3=SEC, 4=MEDIA, 5=AGENT, 6+=Dynamic)
    Page {
        /// Page number (1-based index)
        number: usize,
    },
    /// Display a pop-up toast notification overlay on the LCD
    Toast {
        /// Message to display (up to 3 lines)
        message: String,
        /// Duration in milliseconds (default: 2500)
        #[arg(long)]
        duration_ms: Option<u64>,
    },
    /// Trigger reactive RGB lighting alert (flash or pulse)
    Alert {
        /// Alert animation type: flash or pulse
        #[arg(default_value = "flash")]
        mode: String,
        /// Alert color hex (e.g. #ff0000, #00ffff, #00ff00)
        #[arg(long, default_value = "#ff0000")]
        color: String,
        /// Flash repetitions (for flash mode)
        #[arg(long, default_value_t = 3)]
        times: u32,
        /// Pulse duration in seconds (for pulse mode)
        #[arg(long, default_value_t = 3.0)]
        duration: f32,
    },
    /// Query or toggle microphone / audio mute state
    Mute {
        /// Target device: mic or audio
        target: String,
        /// Toggle mute state instead of querying
        #[arg(long)]
        toggle: bool,
    },
    /// Manage dynamic LCD applet plugins over IPC
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

#[derive(Subcommand, Debug)]
enum PluginAction {
    /// Register a new dynamic page in the LCD carousel
    Register {
        /// Unique plugin page ID (e.g. disk, weather, docker)
        id: String,
        /// Screen title shown in header [X/Y:TITLE] (default: upper ID)
        #[arg(long)]
        title: Option<String>,
        /// Content line 1 (up to 26 chars)
        #[arg(long, default_value = "")]
        line1: String,
        /// Content line 2 (up to 26 chars)
        #[arg(long, default_value = "")]
        line2: String,
        /// Content line 3 (up to 26 chars)
        #[arg(long, default_value = "")]
        line3: String,
        /// Progress / Gauge percentage (0.0 to 100.0)
        #[arg(long)]
        bar: Option<f32>,
    },
    /// Update an existing dynamic page
    Update {
        /// Unique plugin page ID
        id: String,
        /// New screen title
        #[arg(long)]
        title: Option<String>,
        /// New line 1
        #[arg(long)]
        line1: Option<String>,
        /// New line 2
        #[arg(long)]
        line2: Option<String>,
        /// New line 3
        #[arg(long)]
        line3: Option<String>,
        /// New progress / Gauge percentage (0.0 to 100.0)
        #[arg(long)]
        bar: Option<f32>,
    },
    /// Unregister a dynamic page and remove it from the carousel
    Unregister {
        /// Unique plugin page ID
        id: String,
    },
    /// List all currently registered dynamic pages
    List,
    /// Ping the IPC daemon
    Ping,
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
            let ipc_info = match send_ipc_request(&IpcRequest::GetStatus) {
                Ok(resp) if resp.ok => resp.data,
                _ => None,
            };

            let out = serde_json::json!({
                "hardware": snap,
                "daemon_ipc": ipc_info.unwrap_or_else(|| serde_json::json!({"status": "offline"})),
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
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
        Commands::Page { number } => {
            let req = IpcRequest::SetPage { number };
            match send_ipc_request(&req) {
                Ok(resp) if resp.ok => {
                    println!("Requested carousel switch to page {number} via g510d IPC");
                }
                _ => {
                    std::fs::write("/tmp/g510-page", number.to_string())?;
                    println!("Requested carousel switch to page {number} (fallback)");
                }
            }
        }
        Commands::Toast { message, duration_ms } => {
            let dur = duration_ms.unwrap_or(2500);
            let req = IpcRequest::Toast {
                message: message.clone(),
                duration_ms: dur,
            };
            match send_ipc_request(&req) {
                Ok(resp) if resp.ok => {
                    println!("Toast notification displayed via g510d IPC (duration: {dur}ms)");
                }
                _ => {
                    std::fs::write("/tmp/g510-toast", message)?;
                    println!("Toast notification queued for G510 LCD (fallback)");
                }
            }
        }
        Commands::Alert {
            mode,
            color,
            times,
            duration,
        } => {
            let req = IpcRequest::Alert {
                mode: mode.clone(),
                color: color.clone(),
                times,
                duration,
            };
            match send_ipc_request(&req) {
                Ok(resp) if resp.ok => {
                    println!("Alert {mode} ({color}) dispatched via g510d IPC");
                }
                _ => {
                    let alert_payload = if mode == "pulse" {
                        format!("{{\"type\": \"pulse\", \"color\": \"{color}\", \"duration\": {duration}}}")
                    } else {
                        format!("{{\"type\": \"flash\", \"color\": \"{color}\", \"times\": {times}}}")
                    };
                    std::fs::write("/tmp/g510-alert", alert_payload)?;
                    println!("Alert {mode} ({color}) queued for G510 keyboard (fallback)");
                }
            }
        }
        Commands::Mute { target, toggle } => {
            match target.to_lowercase().as_str() {
                "mic" | "microphone" => {
                    let state = if toggle {
                        g510::audio::toggle_microphone_mute()
                    } else {
                        g510::audio::get_microphone_mute()
                    };
                    match state {
                        Some(muted) => {
                            println!("Microphone is {}", if muted { "MUTED" } else { "ACTIVE" });
                        }
                        None => eprintln!("Could not query microphone state from PipeWire/PulseAudio"),
                    }
                }
                "audio" | "sink" | "speaker" => {
                    let state = if toggle {
                        g510::audio::toggle_audio_mute()
                    } else {
                        g510::audio::get_audio_mute()
                    };
                    match state {
                        Some(muted) => {
                            println!("Audio output is {}", if muted { "MUTED" } else { "ACTIVE" });
                        }
                        None => eprintln!("Could not query audio sink state from PipeWire/PulseAudio"),
                    }
                }
                other => eprintln!("Unknown mute target '{other}': use 'mic' or 'audio'"),
            }
        }
        Commands::Plugin { action } => match action {
            PluginAction::Ping => {
                let start = std::time::Instant::now();
                match send_ipc_request(&IpcRequest::Ping) {
                    Ok(resp) if resp.ok => {
                        let elapsed = start.elapsed();
                        println!("g510d IPC: PONG ({:?})", elapsed);
                    }
                    Ok(resp) => {
                        eprintln!("g510d IPC error: {}", resp.error.unwrap_or_else(|| "unknown".into()));
                    }
                    Err(e) => {
                        eprintln!("Failed to connect to g510d IPC socket: {e}");
                        eprintln!("Check if g510d daemon is running (`systemctl status g510-status`)");
                    }
                }
            }
            PluginAction::Register {
                id,
                title,
                line1,
                line2,
                line3,
                bar,
            } => {
                let t = title.unwrap_or_else(|| id.to_uppercase());
                let req = IpcRequest::RegisterPage {
                    id: id.clone(),
                    title: t,
                    line1,
                    line2,
                    line3,
                    bar,
                };
                match send_ipc_request(&req) {
                    Ok(resp) if resp.ok => {
                        println!("Registered dynamic page '{id}' in LCD carousel");
                    }
                    Ok(resp) => {
                        eprintln!("Error registering page: {}", resp.error.unwrap_or_default());
                    }
                    Err(e) => eprintln!("IPC connection error: {e}"),
                }
            }
            PluginAction::Update {
                id,
                title,
                line1,
                line2,
                line3,
                bar,
            } => {
                let req = IpcRequest::UpdatePage {
                    id: id.clone(),
                    title,
                    line1,
                    line2,
                    line3,
                    bar,
                };
                match send_ipc_request(&req) {
                    Ok(resp) if resp.ok => {
                        println!("Updated dynamic page '{id}'");
                    }
                    Ok(resp) => {
                        eprintln!("Error updating page: {}", resp.error.unwrap_or_default());
                    }
                    Err(e) => eprintln!("IPC connection error: {e}"),
                }
            }
            PluginAction::Unregister { id } => {
                let req = IpcRequest::UnregisterPage { id: id.clone() };
                match send_ipc_request(&req) {
                    Ok(resp) if resp.ok => {
                        println!("Unregistered dynamic page '{id}'");
                    }
                    Ok(resp) => {
                        eprintln!("Error unregistering page: {}", resp.error.unwrap_or_default());
                    }
                    Err(e) => eprintln!("IPC connection error: {e}"),
                }
            }
            PluginAction::List => {
                match send_ipc_request(&IpcRequest::ListPages) {
                    Ok(resp) if resp.ok => {
                        if let Some(val) = resp.data {
                            println!("{}", serde_json::to_string_pretty(&val)?);
                        } else {
                            println!("[]");
                        }
                    }
                    Ok(resp) => eprintln!("Error: {}", resp.error.unwrap_or_default()),
                    Err(e) => eprintln!("IPC connection error: {e}"),
                }
            }
        },
    }

    Ok(())
}
