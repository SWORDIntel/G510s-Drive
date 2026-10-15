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
    ipc::{IpcServer, IpcState},
    macros::{default_config_path, load_config, MacroConfig, MacroEngine},
    render::{
        render_agent_screen, render_dynamic_screen, render_media_screen, render_network_screen,
        render_security_screen, render_system_screen, render_toast_screen, TOTAL_BUILTIN_PAGES,
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

const PAGE_NAMES: [&str; TOTAL_BUILTIN_PAGES] = ["SYSTEM", "NETWORK", "SECURITY", "MPRIS", "AGENT"];

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

    // Initialize IPC State & Server
    let ipc_state = IpcState {
        dynamic_pages: Arc::new(Mutex::new(Vec::new())),
        pending_toast: Arc::new(Mutex::new(None)),
        pending_page: Arc::new(Mutex::new(None)),
        pending_alert: Arc::new(Mutex::new(None)),
        active_page: Arc::clone(&active_page),
    };

    let ipc_server = IpcServer::new(ipc_state.clone());
    let ipc_running = Arc::new(AtomicBool::new(true));
    let ipc_handle = match ipc_server.start(Arc::clone(&ipc_running)) {
        Ok(h) => {
            println!("[g510d] IPC socket server active at {}", g510::ipc::socket_path().display());
            Some(h)
        }
        Err(e) => {
            eprintln!("[g510d] Warning: failed to start IPC socket: {e}");
            None
        }
    };

    // Spawn evdev listener thread for G-keys, M-keys, and LCD menu buttons (L1-L5)
    let engine_keys = Arc::clone(&engine);
    let page_keys = Arc::clone(&active_page);
    let toast_keys = Arc::clone(&active_toast);
    let dyn_pages_keys = Arc::clone(&ipc_state.dynamic_pages);

    let _key_thread = thread::spawn(move || {
        let mut dev_opt = None;
        for _ in 0..10 {
            if SHUTDOWN.load(Ordering::SeqCst) {
                return;
            }
            if let Ok(pair) = find_gaming_device() {
                dev_opt = Some(pair);
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }

        if let Some((path, mut device)) = dev_opt {
            println!("[g510d] Monitoring gaming keys on {}", path.display());
            while !SHUTDOWN.load(Ordering::SeqCst) {
                if let Ok(events) = device.fetch_events() {
                    for event in events {
                        if event.event_type() == evdev::EventType::KEY && event.value() == 1 {
                            if let Some(key) = key_from_code(event.code()) {
                                match &key {
                                    G510Key::LcdMenu(1) => {
                                        // L1: Previous Page
                                        let dyn_count = dyn_pages_keys.lock().map_or(0, |p| p.len());
                                        let total = TOTAL_BUILTIN_PAGES + dyn_count;
                                        let current = page_keys.load(Ordering::SeqCst);
                                        let next = (current + total - 1) % total;
                                        page_keys.store(next, Ordering::SeqCst);

                                        let name = if next < TOTAL_BUILTIN_PAGES {
                                            PAGE_NAMES[next].to_string()
                                        } else if let Ok(pages) = dyn_pages_keys.lock() {
                                            pages.get(next - TOTAL_BUILTIN_PAGES)
                                                .map(|p| p.title.clone())
                                                .unwrap_or_else(|| "PLUGIN".into())
                                        } else {
                                            "PLUGIN".into()
                                        };

                                        if let Ok(mut t) = toast_keys.lock() {
                                            *t = Some((
                                                format!("◄ PAGE [{}/{}]\n  {}", next + 1, total, name),
                                                Instant::now(),
                                                Duration::from_millis(1200),
                                            ));
                                        }
                                        eprintln!("[g510d] Page -> {}", name);
                                    }
                                    G510Key::LcdMenu(2) => {
                                        // L2: Next Page
                                        let dyn_count = dyn_pages_keys.lock().map_or(0, |p| p.len());
                                        let total = TOTAL_BUILTIN_PAGES + dyn_count;
                                        let current = page_keys.load(Ordering::SeqCst);
                                        let next = (current + 1) % total;
                                        page_keys.store(next, Ordering::SeqCst);

                                        let name = if next < TOTAL_BUILTIN_PAGES {
                                            PAGE_NAMES[next].to_string()
                                        } else if let Ok(pages) = dyn_pages_keys.lock() {
                                            pages.get(next - TOTAL_BUILTIN_PAGES)
                                                .map(|p| p.title.clone())
                                                .unwrap_or_else(|| "PLUGIN".into())
                                        } else {
                                            "PLUGIN".into()
                                        };

                                        if let Ok(mut t) = toast_keys.lock() {
                                            *t = Some((
                                                format!("► PAGE [{}/{}]\n  {}", next + 1, total, name),
                                                Instant::now(),
                                                Duration::from_millis(1200),
                                            ));
                                        }
                                        eprintln!("[g510d] Page -> {}", name);
                                    }
                                    G510Key::LcdMenu(5) => {
                                        // L5: Home (System)
                                        let dyn_count = dyn_pages_keys.lock().map_or(0, |p| p.len());
                                        let total = TOTAL_BUILTIN_PAGES + dyn_count;
                                        page_keys.store(0, Ordering::SeqCst);
                                        if let Ok(mut t) = toast_keys.lock() {
                                            *t = Some((
                                                format!("HOME [1/{}]\n  {}", total, PAGE_NAMES[0]),
                                                Instant::now(),
                                                Duration::from_millis(1200),
                                            ));
                                        }
                                        eprintln!("[g510d] Page -> SYSTEM");
                                    }
                                    G510Key::Profile(n) => {
                                        if let Ok(mut eng) = engine_keys.lock() {
                                            let prof = format!("M{n}");
                                            eng.activate_profile(&prof);
                                            let desc = match n {
                                                1 => "DEV / GENERAL",
                                                2 => "SECURITY LAB",
                                                3 => "MULTIMEDIA",
                                                _ => "CUSTOM",
                                            };
                                            if let Ok(mut t) = toast_keys.lock() {
                                                *t = Some((
                                                    format!("PROFILE: M{n}\n{desc}"),
                                                    Instant::now(),
                                                    Duration::from_millis(1500),
                                                ));
                                            }
                                        }
                                    }
                                    G510Key::Mr => {
                                        if let Ok(mut eng) = engine_keys.lock() {
                                            if let Some(msg) = eng.toggle_mr() {
                                                if let Ok(mut t) = toast_keys.lock() {
                                                    *t = Some((msg, Instant::now(), Duration::from_millis(1500)));
                                                }
                                            }
                                        }
                                    }
                                    G510Key::Media(name) if name == "MICROPHONE_MUTE" => {
                                        let state = g510::audio::toggle_microphone_mute();
                                        let msg = match state {
                                            Some(true) => "MICROPHONE\n[ MUTED ]",
                                            Some(false) => "MICROPHONE\n[ ACTIVE ]",
                                            None => "MICROPHONE\n[ TOGGLED ]",
                                        };
                                        if let Ok(mut t) = toast_keys.lock() {
                                            *t = Some((msg.to_string(), Instant::now(), Duration::from_millis(1500)));
                                        }
                                        if state == Some(true) {
                                            let restore = if let Ok(eng) = engine_keys.lock() {
                                                eng.config.profile_rgb.get(&eng.active_profile).cloned().unwrap_or_else(|| "#00dc00".into())
                                            } else {
                                                "#00dc00".into()
                                            };
                                            thread::spawn(move || {
                                                g510::alerts::run_flash_alert("#ff8800", 2, &restore);
                                            });
                                        }
                                    }
                                    G510Key::Media(name) if name == "HEADPHONE_MUTE" => {
                                        let state = g510::audio::toggle_audio_mute();
                                        let msg = match state {
                                            Some(true) => "AUDIO OUTPUT\n[ MUTED ]",
                                            Some(false) => "AUDIO OUTPUT\n[ ACTIVE ]",
                                            None => "AUDIO OUTPUT\n[ TOGGLED ]",
                                        };
                                        if let Ok(mut t) = toast_keys.lock() {
                                            *t = Some((msg.to_string(), Instant::now(), Duration::from_millis(1500)));
                                        }
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

    // Open LCD transport (retry for up to 10 seconds during system boot)
    let mut transport = {
        let mut attempts = 0;
        loop {
            match G510Transport::open() {
                Ok(t) => {
                    println!("[g510d] LCD transport active on {}", t.path().display());
                    break t;
                }
                Err(e) => {
                    attempts += 1;
                    if attempts >= 10 || SHUTDOWN.load(Ordering::SeqCst) {
                        eprintln!("[g510d] Error: cannot open LCD transport after {attempts} attempts: {e}");
                        return Err(e.into());
                    }
                    eprintln!("[g510d] Waiting for LCD transport ({attempts}/10): {e}...");
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    };

    let mut fb = G510Framebuffer::new();
    let mut prev_cpu = CpuCounters::read().unwrap_or_default();
    let (default_iface, local_ip) = read_default_route();
    let mut prev_net_bytes = read_iface_bytes(&default_iface);
    let mut last_net_tick = Instant::now();
    let step = Duration::from_secs_f32(args.interval.max(0.1));

    // File-based IPC fallback paths
    let toast_file = PathBuf::from("/tmp/g510-toast");
    let page_file = PathBuf::from("/tmp/g510-page");
    let alert_file = PathBuf::from("/tmp/g510-alert");

    while !SHUTDOWN.load(Ordering::SeqCst) {
        // 1. Process pending IPC Requests from Socket
        if let Ok(mut pt) = ipc_state.pending_toast.lock() {
            if let Some((msg, dur)) = pt.take() {
                if let Ok(mut at) = active_toast.lock() {
                    *at = Some((msg, Instant::now(), dur));
                }
            }
        }

        if let Ok(mut pp) = ipc_state.pending_page.lock() {
            if let Some(target_page) = pp.take() {
                let dyn_count = ipc_state.dynamic_pages.lock().map_or(0, |p| p.len());
                let total = TOTAL_BUILTIN_PAGES + dyn_count;
                if target_page >= 1 && target_page <= total {
                    active_page.store(target_page - 1, Ordering::SeqCst);
                }
            }
        }

        if let Ok(mut pa) = ipc_state.pending_alert.lock() {
            if let Some(alert) = pa.take() {
                let restore = if let Ok(eng) = engine.lock() {
                    eng.config.profile_rgb.get(&eng.active_profile).cloned().unwrap_or_else(|| "#00dc00".into())
                } else {
                    "#00dc00".into()
                };
                thread::spawn(move || match alert {
                    g510::alerts::RgbAlert::Flash { color, times } => {
                        g510::alerts::run_flash_alert(&color, times, &restore);
                    }
                    g510::alerts::RgbAlert::Pulse { color, duration } => {
                        g510::alerts::run_pulse_alert(&color, duration, &restore);
                    }
                });
            }
        }

        // 2. Check File-based IPC fallbacks
        if page_file.exists() {
            if let Ok(content) = fs::read_to_string(&page_file) {
                let _ = fs::remove_file(&page_file);
                if let Ok(p) = content.trim().parse::<usize>() {
                    let dyn_count = ipc_state.dynamic_pages.lock().map_or(0, |p| p.len());
                    let total = TOTAL_BUILTIN_PAGES + dyn_count;
                    if (1..=total).contains(&p) {
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

        if alert_file.exists() {
            if let Ok(content) = fs::read_to_string(&alert_file) {
                let _ = fs::remove_file(&alert_file);
                if let Ok(alert) = serde_json::from_str::<g510::alerts::RgbAlert>(&content) {
                    let restore = if let Ok(eng) = engine.lock() {
                        eng.config.profile_rgb.get(&eng.active_profile).cloned().unwrap_or_else(|| "#00dc00".into())
                    } else {
                        "#00dc00".into()
                    };
                    thread::spawn(move || {
                        match alert {
                            g510::alerts::RgbAlert::Flash { color, times } => {
                                g510::alerts::run_flash_alert(&color, times, &restore);
                            }
                            g510::alerts::RgbAlert::Pulse { color, duration } => {
                                g510::alerts::run_pulse_alert(&color, duration, &restore);
                            }
                        }
                    });
                }
            }
        }

        let clock = current_time_str();

        // 3. Check active toast overlay
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

        // 4. Render Active Carousel Page (Built-in or Dynamic Plugin)
        if !show_toast {
            let dyn_pages = ipc_state.dynamic_pages.lock().map_or_else(|_| Vec::new(), |p| p.clone());
            let total_pages = TOTAL_BUILTIN_PAGES + dyn_pages.len();
            let mut page = active_page.load(Ordering::SeqCst);
            if page >= total_pages {
                page = 0;
                active_page.store(0, Ordering::SeqCst);
            }

            match page {
                0 => {
                    // System Telemetry & Thermals
                    let cur_cpu = CpuCounters::read().unwrap_or_default();
                    let cpu_pct = cur_cpu.percentage_since(&prev_cpu);
                    prev_cpu = cur_cpu;
                    let mem_pct = memory_percentage().unwrap_or(0.0);
                    let temp_c = cpu_temp_celsius();
                    let (load1, _, _) = load_average();
                    render_system_screen(&mut fb, 0, total_pages, cpu_pct, mem_pct, temp_c, load1, &clock);
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
                    render_network_screen(&mut fb, 1, total_pages, &default_iface, &local_ip, rx_kbs, tx_kbs, vpn_active, &clock);
                }
                2 => {
                    // Security & Lab Ticker
                    let sec = read_security_stats();
                    render_security_screen(&mut fb, 2, total_pages, sec.failed_ssh_hour, sec.vms, sec.containers, &clock);
                }
                3 => {
                    // MPRIS Media Player
                    let media = read_mpris_media();
                    render_media_screen(&mut fb, 3, total_pages, &media.status, &media.artist, &media.title, &clock);
                }
                4 => {
                    // AI Agent / Workbench Status
                    let agent = read_agent_status();
                    render_agent_screen(&mut fb, 4, total_pages, &agent.state, &agent.task, &agent.role, &clock);
                }
                dynamic_idx => {
                    let dp_idx = dynamic_idx - TOTAL_BUILTIN_PAGES;
                    if let Some(dp) = dyn_pages.get(dp_idx) {
                        render_dynamic_screen(
                            &mut fb,
                            dynamic_idx,
                            total_pages,
                            &dp.title,
                            &dp.line1,
                            &dp.line2,
                            &dp.line3,
                            dp.bar,
                            &clock,
                        );
                    } else {
                        active_page.store(0, Ordering::SeqCst);
                    }
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
    ipc_running.store(false, Ordering::SeqCst);
    if let Some(h) = ipc_handle {
        let _ = h.join();
    }
    Ok(())
}
