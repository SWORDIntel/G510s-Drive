use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use clap::Parser;

use g510::{
    events::{find_gaming_device, key_from_code, G510Key},
    framebuffer::G510Framebuffer,
    macros::{default_config_path, load_config, MacroConfig, MacroEngine},
    render::{
        render_agent_screen, render_media_screen, render_network_screen, render_security_screen,
        render_system_screen, render_toast_screen, TOTAL_PAGES,
    },
    telemetry::{
        check_vpn, cpu_temp_celsius, current_time_str, load_average, memory_percentage,
        read_agent_status, read_default_route, read_iface_bytes, read_mpris_media,
        read_security_stats, CpuCounters,
    },
    transport::G510Transport,
};

#[derive(Parser, Debug)]
#[command(name = "g510d", version, about = "Logitech G510/G510s Native Background Daemon")]
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

const PAGE_NAMES: [&str; TOTAL_PAGES] = ["SYSTEM", "NETWORK", "SECURITY", "MPRIS", "AGENT"];

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

    let engine = Arc::new(Mutex::new(MacroEngine::new(cfg)));
    let active_page = Arc::new(AtomicUsize::new(0));
    let active_toast = Arc::new(Mutex::new(None::<(String, Instant, Duration)>));

    // Spawn evdev listener thread for G-keys, M-keys, and LCD menu buttons (L1-L5)
    let engine_keys = Arc::clone(&engine);
    let page_keys = Arc::clone(&active_page);
    let toast_keys = Arc::clone(&active_toast);

    let key_thread = thread::spawn(move || {
        if let Ok((path, mut device)) = find_gaming_device() {
            println!("[g510d] Monitoring gaming keys on {}", path.display());
            while !SHUTDOWN.load(Ordering::SeqCst) {
                if let Ok(events) = device.fetch_events() {
                    for event in events {
                        if event.event_type() == evdev::EventType::KEY && event.value() == 1 {
                            if let Some(key) = key_from_code(event.code()) {
                                match &key {
                                    G510Key::LcdMenu(1) => {
                                        // L1: Previous Page
                                        let current = page_keys.load(Ordering::SeqCst);
                                        let next = (current + TOTAL_PAGES - 1) % TOTAL_PAGES;
                                        page_keys.store(next, Ordering::SeqCst);
                                        if let Ok(mut t) = toast_keys.lock() {
                                            *t = Some((
                                                format!("◄ PAGE [{}/{}]\n  {}", next + 1, TOTAL_PAGES, PAGE_NAMES[next]),
                                                Instant::now(),
                                                Duration::from_millis(1200),
                                            ));
                                        }
                                        eprintln!("[g510d] Page -> {}", PAGE_NAMES[next]);
                                    }
                                    G510Key::LcdMenu(2) => {
                                        // L2: Next Page
                                        let current = page_keys.load(Ordering::SeqCst);
                                        let next = (current + 1) % TOTAL_PAGES;
                                        page_keys.store(next, Ordering::SeqCst);
                                        if let Ok(mut t) = toast_keys.lock() {
                                            *t = Some((
                                                format!("► PAGE [{}/{}]\n  {}", next + 1, TOTAL_PAGES, PAGE_NAMES[next]),
                                                Instant::now(),
                                                Duration::from_millis(1200),
                                            ));
                                        }
                                        eprintln!("[g510d] Page -> {}", PAGE_NAMES[next]);
                                    }
                                    G510Key::LcdMenu(5) => {
                                        // L5: Home (System)
                                        page_keys.store(0, Ordering::SeqCst);
                                        if let Ok(mut t) = toast_keys.lock() {
                                            *t = Some((
                                                format!("HOME [1/{}]\n  {}", TOTAL_PAGES, PAGE_NAMES[0]),
                                                Instant::now(),
                                                Duration::from_millis(1200),
                                            ));
                                        }
                                        eprintln!("[g510d] Page -> SYSTEM");
                                    }
                                    _ => {
                                        if let Ok(mut eng) = engine_keys.lock() {
                                            eng.handle_key(&key.name());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_millis(40));
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
    let (default_iface, local_ip) = read_default_route();
    let mut prev_net_bytes = read_iface_bytes(&default_iface);
    let mut last_net_tick = Instant::now();
    let step = Duration::from_secs_f32(args.interval.max(0.1));

    // IPC toast file paths
    let toast_file = PathBuf::from("/tmp/g510-toast");
    let page_file = PathBuf::from("/tmp/g510-page");

    while !SHUTDOWN.load(Ordering::SeqCst) {
        // Check external IPC requests
        if page_file.exists() {
            if let Ok(content) = fs::read_to_string(&page_file) {
                let _ = fs::remove_file(&page_file);
                if let Ok(p) = content.trim().parse::<usize>() {
                    if (1..=TOTAL_PAGES).contains(&p) {
                        active_page.store(p - 1, Ordering::SeqCst);
                    }
                }
            }
        }

        if toast_file.exists() {
            if let Ok(content) = fs::read_to_string(&toast_file) {
                let _ = fs::remove_file(&toast_file);
                if let Ok(mut t) = active_toast.lock() {
                    *t = Some((content.trim().to_string(), Instant::now(), Duration::from_secs(3)));
                }
            }
        }

        let clock = current_time_str();

        // Check active toast overlay
        let mut show_toast = false;
        if let Ok(mut t) = active_toast.lock() {
            if let Some((msg, start, dur)) = t.as_ref() {
                if start.elapsed() < *dur {
                    render_toast_screen(&mut fb, msg);
                    show_toast = true;
                } else {
                    *t = None;
                }
            }
        }

        if !show_toast {
            let page = active_page.load(Ordering::SeqCst);
            match page {
                0 => {
                    // System Telemetry & Thermals
                    let cur_cpu = CpuCounters::read().unwrap_or_default();
                    let cpu_pct = cur_cpu.percentage_since(&prev_cpu);
                    prev_cpu = cur_cpu;
                    let mem_pct = memory_percentage().unwrap_or(0.0);
                    let temp_c = cpu_temp_celsius();
                    let (load1, _, _) = load_average();
                    render_system_screen(&mut fb, cpu_pct, mem_pct, temp_c, load1, &clock);
                }
                1 => {
                    // Network & VPN Throughput
                    let now = Instant::now();
                    let elapsed_secs = (now - last_net_tick).as_secs_f32().max(0.1);
                    last_net_tick = now;

                    let cur_bytes = read_iface_bytes(&default_iface);
                    let rx_kbs = ((cur_bytes.0.saturating_sub(prev_net_bytes.0)) as f32 / 1024.0) / elapsed_secs;
                    let tx_kbs = ((cur_bytes.1.saturating_sub(prev_net_bytes.1)) as f32 / 1024.0) / elapsed_secs;
                    prev_net_bytes = cur_bytes;

                    let vpn_active = check_vpn();
                    render_network_screen(&mut fb, &default_iface, &local_ip, rx_kbs, tx_kbs, vpn_active, &clock);
                }
                2 => {
                    // Security & Lab Ticker
                    let sec = read_security_stats();
                    render_security_screen(&mut fb, sec.failed_ssh_hour, sec.vms, sec.containers, &clock);
                }
                3 => {
                    // MPRIS Media Player
                    let media = read_mpris_media();
                    render_media_screen(&mut fb, &media.status, &media.artist, &media.title, &clock);
                }
                4 => {
                    // AI Agent / Workbench Status
                    let agent = read_agent_status();
                    render_agent_screen(&mut fb, &agent.state, &agent.task, &agent.role, &clock);
                }
                _ => {
                    active_page.store(0, Ordering::SeqCst);
                }
            }
        }

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
