use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use clap::Parser;

use g510::{
    events::{find_gaming_device, key_from_code},
    framebuffer::G510Framebuffer,
    macros::{default_config_path, load_config, MacroConfig, MacroEngine},
    render::render_status_screen,
    telemetry::{current_time_str, memory_percentage, CpuCounters},
    transport::G510Transport,
};

#[derive(Parser, Debug)]
#[command(name = "g510d", version, about = "Logitech G510/G510s Background Daemon")]
struct Args {
    /// Telemetry refresh interval in seconds
    #[arg(short, long, default_value_t = 1.0)]
    interval: f32,

    /// Invert LCD pixels
    #[arg(long)]
    invert: bool,

    /// Custom macro configuration path
    #[arg(short, long)]
    config: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    install_signal_handlers();

    println!("[g510d] Starting G510 background daemon (refresh: {}s)...", args.interval);

    // Load macro configuration
    let config_path = args.config.unwrap_or_else(default_config_path);
    let cfg = if config_path.exists() {
        match load_config(&config_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[g510d] Warning: failed to parse {}: {e}", config_path.display());
                MacroConfig::default()
            }
        }
    } else {
        MacroConfig::default()
    };

    let engine = Arc::new(std::sync::Mutex::new(MacroEngine::new(cfg)));

    // Spawn evdev listener thread for G-keys, M-keys, and L-keys
    let engine_keys = Arc::clone(&engine);
    let key_thread = thread::spawn(move || {
        if let Ok((path, mut device)) = find_gaming_device() {
            println!("[g510d] Monitoring gaming keys on {}", path.display());
            while !SHUTDOWN.load(Ordering::SeqCst) {
                if let Ok(events) = device.fetch_events() {
                    for event in events {
                        if event.event_type() == evdev::EventType::KEY && event.value() == 1 {
                            if let Some(key) = key_from_code(event.code()) {
                                if let Ok(mut eng) = engine_keys.lock() {
                                    eng.handle_key(&key.name());
                                }
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_millis(50));
            }
        } else {
            eprintln!("[g510d] Gaming evdev node not accessible; macro listening disabled");
        }
    });

    // Open LCD transport
    let mut transport = match G510Transport::open() {
        Ok(t) => {
            println!("[g510d] LCD transport active on {}", t.path().display());
            t
        }
        Err(e) => {
            eprintln!("[g510d] Error: cannot open LCD transport: {e}");
            return Err(e.into());
        }
    };

    let mut fb = G510Framebuffer::new();
    let mut prev_cpu = CpuCounters::read().unwrap_or_default();
    let step = Duration::from_secs_f32(args.interval.max(0.1));

    while !SHUTDOWN.load(Ordering::SeqCst) {
        let cur_cpu = CpuCounters::read().unwrap_or_default();
        let cpu_pct = cur_cpu.percentage_since(&prev_cpu);
        prev_cpu = cur_cpu;

        let mem_pct = memory_percentage().unwrap_or(0.0);
        let clock = current_time_str();

        render_status_screen(&mut fb, cpu_pct, mem_pct, &clock);
        if let Err(e) = transport.write_framebuffer(&fb, args.invert) {
            eprintln!("[g510d] LCD write error: {e}");
            thread::sleep(Duration::from_millis(500));
        }

        thread::sleep(step);
    }

    println!("[g510d] Shutting down cleanly...");
    let _ = key_thread.join();
    Ok(())
}

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

extern "C" fn sig_handler(_: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

fn install_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGINT, sig_handler as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, sig_handler as *const () as libc::sighandler_t);
    }
}
