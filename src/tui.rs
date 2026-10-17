use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Gauge, List, ListItem, Paragraph, Tabs},
    Frame, Terminal,
};

use crate::{
    audio::{get_audio_mute, get_microphone_mute, toggle_audio_mute, toggle_microphone_mute},
    events::find_gaming_device,
    ipc::{send_ipc_request, socket_path, DynamicPage, IpcRequest},
    leds::{set_profile, set_rgb, snapshot, LedSnapshot},
    macros::{
        default_config_path, load_config, save_config, MacroAction, MacroConfig,
    },
    transport::find_hidraw_device,
};

pub struct ColorPreset {
    pub name: &'static str,
    pub hex: &'static str,
    pub rgb: (u8, u8, u8),
}

pub const COLOR_PRESETS: &[ColorPreset] = &[
    ColorPreset { name: "Dev Green", hex: "#00dc00", rgb: (0, 220, 0) },
    ColorPreset { name: "Security Red", hex: "#dc0000", rgb: (220, 0, 0) },
    ColorPreset { name: "Media Blue", hex: "#0000dc", rgb: (0, 0, 220) },
    ColorPreset { name: "Amber / Orange", hex: "#ff8800", rgb: (255, 136, 0) },
    ColorPreset { name: "Cyan / Aqua", hex: "#00ffff", rgb: (0, 255, 255) },
    ColorPreset { name: "Purple / Violet", hex: "#9900ff", rgb: (153, 0, 255) },
    ColorPreset { name: "Electric Yellow", hex: "#ffff00", rgb: (255, 255, 0) },
    ColorPreset { name: "Cold White", hex: "#ffffff", rgb: (255, 255, 255) },
    ColorPreset { name: "Backlight Off", hex: "#000000", rgb: (0, 0, 0) },
];

pub struct MacroPresetDef {
    pub label: &'static str,
    pub description: &'static str,
    pub action: Option<MacroAction>,
}

fn make_keys_action(keys: &[&str]) -> MacroAction {
    MacroAction::Keys {
        keys: keys.iter().map(|s| s.to_string()).collect(),
    }
}

fn make_exec_action(argv: &[&str]) -> MacroAction {
    MacroAction::Exec {
        argv: argv.iter().map(|s| s.to_string()).collect(),
    }
}

pub fn get_macro_presets() -> Vec<MacroPresetDef> {
    vec![
        MacroPresetDef {
            label: "Terminal",
            description: "Spawn terminal window (x-terminal-emulator)",
            action: Some(make_exec_action(&["/usr/bin/x-terminal-emulator"])),
        },
        MacroPresetDef {
            label: "Git Status",
            description: "Show repository status in terminal",
            action: Some(make_exec_action(&["/usr/bin/git", "status"])),
        },
        MacroPresetDef {
            label: "Cargo Check",
            description: "Run cargo check fast compilation test",
            action: Some(make_exec_action(&["/usr/bin/cargo", "check"])),
        },
        MacroPresetDef {
            label: "Cargo Test",
            description: "Execute cargo test runner",
            action: Some(make_exec_action(&["/usr/bin/cargo", "test"])),
        },
        MacroPresetDef {
            label: "Copy (Ctrl+C)",
            description: "Inject Ctrl+C keystroke chord",
            action: Some(make_keys_action(&["KEY_LEFTCTRL", "KEY_C"])),
        },
        MacroPresetDef {
            label: "Paste (Ctrl+V)",
            description: "Inject Ctrl+V keystroke chord",
            action: Some(make_keys_action(&["KEY_LEFTCTRL", "KEY_V"])),
        },
        MacroPresetDef {
            label: "Cut (Ctrl+X)",
            description: "Inject Ctrl+X keystroke chord",
            action: Some(make_keys_action(&["KEY_LEFTCTRL", "KEY_X"])),
        },
        MacroPresetDef {
            label: "Undo (Ctrl+Z)",
            description: "Inject Ctrl+Z keystroke chord",
            action: Some(make_keys_action(&["KEY_LEFTCTRL", "KEY_Z"])),
        },
        MacroPresetDef {
            label: "Save (Ctrl+S)",
            description: "Inject Ctrl+S save chord",
            action: Some(make_keys_action(&["KEY_LEFTCTRL", "KEY_S"])),
        },
        MacroPresetDef {
            label: "Find (Ctrl+F)",
            description: "Inject Ctrl+F find chord",
            action: Some(make_keys_action(&["KEY_LEFTCTRL", "KEY_F"])),
        },
        MacroPresetDef {
            label: "Close Tab (Ctrl+W)",
            description: "Inject Ctrl+W close chord",
            action: Some(make_keys_action(&["KEY_LEFTCTRL", "KEY_W"])),
        },
        MacroPresetDef {
            label: "Lock Screen (Ctrl+Alt+L)",
            description: "Inject lock screen shortcut",
            action: Some(make_keys_action(&["KEY_LEFTCTRL", "KEY_LEFTALT", "KEY_L"])),
        },
        MacroPresetDef {
            label: "Media Play/Pause",
            description: "Toggle playback via playerctl",
            action: Some(make_exec_action(&["/usr/bin/playerctl", "play-pause"])),
        },
        MacroPresetDef {
            label: "Media Next Track",
            description: "Skip to next track via playerctl",
            action: Some(make_exec_action(&["/usr/bin/playerctl", "next"])),
        },
        MacroPresetDef {
            label: "Media Previous Track",
            description: "Skip to previous track via playerctl",
            action: Some(make_exec_action(&["/usr/bin/playerctl", "previous"])),
        },
        MacroPresetDef {
            label: "Volume +5%",
            description: "Increase media volume",
            action: Some(make_exec_action(&["/usr/bin/playerctl", "volume", "0.05+"])),
        },
        MacroPresetDef {
            label: "Volume -5%",
            description: "Decrease media volume",
            action: Some(make_exec_action(&["/usr/bin/playerctl", "volume", "0.05-"])),
        },
        MacroPresetDef {
            label: "Toggle Mic Mute",
            description: "Toggle microphone mute via g510ctl",
            action: Some(make_exec_action(&["/usr/local/bin/g510ctl", "mute", "mic", "--toggle"])),
        },
        MacroPresetDef {
            label: "LCD Page 1 (System)",
            description: "Switch LCD carousel to System & Thermals",
            action: Some(make_exec_action(&["/usr/local/bin/g510ctl", "page", "1"])),
        },
        MacroPresetDef {
            label: "LCD Page 2 (Network)",
            description: "Switch LCD carousel to Network & VPN",
            action: Some(make_exec_action(&["/usr/local/bin/g510ctl", "page", "2"])),
        },
        MacroPresetDef {
            label: "LCD Page 3 (Security)",
            description: "Switch LCD carousel to Security & Lab",
            action: Some(make_exec_action(&["/usr/local/bin/g510ctl", "page", "3"])),
        },
        MacroPresetDef {
            label: "LCD Page 4 (Media)",
            description: "Switch LCD carousel to MPRIS Media",
            action: Some(make_exec_action(&["/usr/local/bin/g510ctl", "page", "4"])),
        },
        MacroPresetDef {
            label: "LCD Page 5 (Agent)",
            description: "Switch LCD carousel to AI Agent status",
            action: Some(make_exec_action(&["/usr/local/bin/g510ctl", "page", "5"])),
        },
        MacroPresetDef {
            label: "[ Clear / Unassign ]",
            description: "Remove macro action from this key",
            action: None,
        },
    ]
}

pub struct App {
    pub selected_tab: usize,
    // Lighting
    pub color_idx: usize,
    pub brightness: u8,
    pub target_mode: &'static str, // "live" or "power-on"
    // Macros
    pub active_profile: usize, // 0: M1, 1: M2, 2: M3
    pub selected_g_key: usize, // 0..17 (G1..G18)
    pub macro_config: MacroConfig,
    pub config_path: PathBuf,
    pub show_preset_modal: bool,
    pub preset_modal_idx: usize,
    // LCD & IPC
    pub selected_lcd_page: usize, // 0..4 (1..5)
    pub dynamic_pages: Vec<DynamicPage>,
    // Audio & Status
    pub mic_muted: Option<bool>,
    pub audio_muted: Option<bool>,
    pub hardware_snap: LedSnapshot,
    pub ipc_status: Option<serde_json::Value>,
    // Status Feedback
    pub status_msg: Option<(String, Instant)>,
}

impl App {
    pub fn new() -> Self {
        let config_path = default_config_path();
        let macro_config = load_config(&config_path).unwrap_or_default();
        let hardware_snap = snapshot(None);
        let mic_muted = get_microphone_mute();
        let audio_muted = get_audio_mute();

        let initial_brightness = hardware_snap.live_rgb.as_ref().map_or(255, |s| s.brightness);

        let mut app = Self {
            selected_tab: 0,
            color_idx: 0,
            brightness: initial_brightness,
            target_mode: "live",
            active_profile: 0,
            selected_g_key: 0,
            macro_config,
            config_path,
            show_preset_modal: false,
            preset_modal_idx: 0,
            selected_lcd_page: 0,
            dynamic_pages: Vec::new(),
            mic_muted,
            audio_muted,
            hardware_snap,
            ipc_status: None,
            status_msg: Some(("Logitech G510s Interactive TUI Initialized".to_string(), Instant::now())),
        };

        app.refresh_status();
        app
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_msg = Some((msg.into(), Instant::now()));
    }

    pub fn refresh_status(&mut self) {
        self.hardware_snap = snapshot(None);
        self.mic_muted = get_microphone_mute();
        self.audio_muted = get_audio_mute();

        if let Ok(resp) = send_ipc_request(&IpcRequest::GetStatus) {
            if resp.ok {
                self.ipc_status = resp.data;
            }
        }

        if let Ok(resp) = send_ipc_request(&IpcRequest::ListPages) {
            if resp.ok {
                if let Some(val) = resp.data {
                    if let Ok(pages) = serde_json::from_value::<Vec<DynamicPage>>(val) {
                        self.dynamic_pages = pages;
                    }
                }
            }
        }
    }

    pub fn profile_name(&self) -> &'static str {
        match self.active_profile {
            0 => "M1",
            1 => "M2",
            2 => "M3",
            _ => "M1",
        }
    }

    pub fn apply_color(&mut self) {
        let preset = &COLOR_PRESETS[self.color_idx];
        match set_rgb(preset.hex, self.brightness, self.target_mode, None) {
            Ok(state) => {
                self.set_status(format!("Set {} RGB to {} (brightness: {}/255)", self.target_mode, state.hex, state.brightness));
            }
            Err(e) => {
                self.set_status(format!("Error setting RGB: {e}"));
            }
        }
        self.refresh_status();
    }

    pub fn apply_profile_led(&mut self, prof: u8) {
        if let Err(e) = set_profile(prof, None) {
            self.set_status(format!("Error setting profile LED: {e}"));
        } else {
            self.set_status(format!("Switched hardware indicator to M{prof}"));
        }
        self.refresh_status();
    }

    pub fn test_alert(&mut self, mode: &str) {
        let color = COLOR_PRESETS[self.color_idx].hex.to_string();
        let req = if mode == "pulse" {
            IpcRequest::Alert {
                mode: "pulse".into(),
                color: color.clone(),
                times: 1,
                duration: 3.0,
            }
        } else {
            IpcRequest::Alert {
                mode: "flash".into(),
                color: color.clone(),
                times: 3,
                duration: 1.5,
            }
        };

        match send_ipc_request(&req) {
            Ok(resp) if resp.ok => {
                self.set_status(format!("Dispatched {} alert ({}) via IPC", mode, color));
            }
            _ => {
                self.set_status(format!("Could not send alert to g510d (check daemon)"));
            }
        }
    }

    pub fn toggle_mic(&mut self) {
        let new_state = toggle_microphone_mute();
        self.mic_muted = new_state;
        match new_state {
            Some(true) => self.set_status("Microphone: [ MUTED ]"),
            Some(false) => self.set_status("Microphone: [ ACTIVE ]"),
            None => self.set_status("Could not toggle microphone via PipeWire/PulseAudio"),
        }
    }

    pub fn toggle_audio(&mut self) {
        let new_state = toggle_audio_mute();
        self.audio_muted = new_state;
        match new_state {
            Some(true) => self.set_status("Audio Output: [ MUTED ]"),
            Some(false) => self.set_status("Audio Output: [ ACTIVE ]"),
            None => self.set_status("Could not toggle audio sink via PipeWire/PulseAudio"),
        }
    }

    pub fn switch_lcd_page(&mut self, page: usize) {
        let req = IpcRequest::SetPage { number: page };
        match send_ipc_request(&req) {
            Ok(resp) if resp.ok => {
                self.set_status(format!("Switched LCD carousel to page {page}"));
            }
            _ => {
                self.set_status(format!("Failed to switch page (is g510d running?)"));
            }
        }
        self.refresh_status();
    }

    pub fn send_test_toast(&mut self) {
        let req = IpcRequest::Toast {
            message: "G510s TUI TEST\nConfiguration Live!".into(),
            duration_ms: 2500,
        };
        match send_ipc_request(&req) {
            Ok(resp) if resp.ok => {
                self.set_status("Test toast displayed on G510 LCD");
            }
            _ => {
                self.set_status("Failed to send toast to g510d");
            }
        }
    }

    pub fn save_macros(&mut self) {
        match save_config(&self.macro_config, &self.config_path) {
            Ok(_) => {
                self.set_status(format!("Saved macro config to {}", self.config_path.display()));
            }
            Err(e) => {
                self.set_status(format!("Failed to save config: {e}"));
            }
        }
    }

    pub fn reload_macros(&mut self) {
        match load_config(&self.config_path) {
            Ok(cfg) => {
                self.macro_config = cfg;
                self.set_status("Reloaded macro configuration from disk");
            }
            Err(e) => {
                self.set_status(format!("Failed to reload config: {e}"));
            }
        }
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let tick_rate = Duration::from_millis(150);

    let res = run_app(&mut terminal, &mut app, tick_rate);

    // Restore terminal cleanly
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("[g510tui] Error: {err:?}");
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();

    loop {
        terminal
            .draw(|f| ui(f, app))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Modal dialog key intercept
                    if app.show_preset_modal {
                        let presets = get_macro_presets();
                        match key.code {
                            KeyCode::Esc => {
                                app.show_preset_modal = false;
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.preset_modal_idx > 0 {
                                    app.preset_modal_idx -= 1;
                                } else {
                                    app.preset_modal_idx = presets.len() - 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.preset_modal_idx + 1 < presets.len() {
                                    app.preset_modal_idx += 1;
                                } else {
                                    app.preset_modal_idx = 0;
                                }
                            }
                            KeyCode::Enter | KeyCode::Char(' ') => {
                                let chosen = &presets[app.preset_modal_idx];
                                let g_key = format!("G{}", app.selected_g_key + 1);
                                let prof = app.profile_name().to_string();

                                let p_map = app.macro_config.profiles.entry(prof.clone()).or_default();
                                if let Some(act) = &chosen.action {
                                    p_map.insert(g_key.clone(), act.clone());
                                    app.set_status(format!("Assigned '{}' to {} in {}", chosen.label, g_key, prof));
                                } else {
                                    p_map.remove(&g_key);
                                    app.set_status(format!("Cleared macro for {} in {}", g_key, prof));
                                }
                                app.show_preset_modal = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Global navigation
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Tab => {
                            app.selected_tab = (app.selected_tab + 1) % 4;
                            app.refresh_status();
                        }
                        KeyCode::BackTab => {
                            app.selected_tab = (app.selected_tab + 3) % 4;
                            app.refresh_status();
                        }
                        KeyCode::Char('1') | KeyCode::F(1) => {
                            app.selected_tab = 0;
                            app.refresh_status();
                        }
                        KeyCode::Char('2') | KeyCode::F(2) => {
                            app.selected_tab = 1;
                            app.refresh_status();
                        }
                        KeyCode::Char('3') | KeyCode::F(3) => {
                            app.selected_tab = 2;
                            app.refresh_status();
                        }
                        KeyCode::Char('4') | KeyCode::F(4) => {
                            app.selected_tab = 3;
                            app.refresh_status();
                        }
                        _ => {
                            // Tab-specific key handlers
                            match app.selected_tab {
                                0 => handle_lighting_keys(app, key.code),
                                1 => handle_macro_keys(app, key.code),
                                2 => handle_lcd_keys(app, key.code),
                                3 => handle_audio_keys(app, key.code),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

fn handle_lighting_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.color_idx > 0 {
                app.color_idx -= 1;
            } else {
                app.color_idx = COLOR_PRESETS.len() - 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.color_idx + 1 < COLOR_PRESETS.len() {
                app.color_idx += 1;
            } else {
                app.color_idx = 0;
            }
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('-') => {
            app.brightness = app.brightness.saturating_sub(15);
            app.apply_color();
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('+') | KeyCode::Char('=') => {
            app.brightness = app.brightness.saturating_add(15);
            app.apply_color();
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            app.apply_color();
        }
        KeyCode::Char('t') => {
            app.target_mode = if app.target_mode == "live" { "power-on" } else { "live" };
            app.set_status(format!("Target mode set to: {}", app.target_mode));
        }
        KeyCode::Char('f') => {
            app.test_alert("flash");
        }
        KeyCode::Char('b') => {
            app.test_alert("pulse");
        }
        _ => {}
    }
}

fn handle_macro_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.selected_g_key > 0 {
                app.selected_g_key -= 1;
            } else {
                app.selected_g_key = 17;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.selected_g_key < 17 {
                app.selected_g_key += 1;
            } else {
                app.selected_g_key = 0;
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.active_profile = (app.active_profile + 2) % 3;
            let prof_num = (app.active_profile + 1) as u8;
            app.apply_profile_led(prof_num);
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('m') => {
            app.active_profile = (app.active_profile + 1) % 3;
            let prof_num = (app.active_profile + 1) as u8;
            app.apply_profile_led(prof_num);
        }
        KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('a') => {
            app.show_preset_modal = true;
            app.preset_modal_idx = 0;
        }
        KeyCode::Char('d') | KeyCode::Delete | KeyCode::Backspace => {
            let g_key = format!("G{}", app.selected_g_key + 1);
            let prof = app.profile_name().to_string();
            if let Some(p_map) = app.macro_config.profiles.get_mut(&prof) {
                p_map.remove(&g_key);
                app.set_status(format!("Cleared macro for {} in {}", g_key, prof));
            }
        }
        KeyCode::Char('s') => {
            app.save_macros();
        }
        KeyCode::Char('r') => {
            app.reload_macros();
        }
        KeyCode::Char('c') => {
            // Assign active lighting color to this profile
            let hex = COLOR_PRESETS[app.color_idx].hex.to_string();
            let prof = app.profile_name().to_string();
            app.macro_config.profile_rgb.insert(prof.clone(), hex.clone());
            app.set_status(format!("Assigned {} theme color to profile {}", hex, prof));
        }
        _ => {}
    }
}

fn handle_lcd_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.selected_lcd_page > 0 {
                app.selected_lcd_page -= 1;
            } else {
                app.selected_lcd_page = 4;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.selected_lcd_page < 4 {
                app.selected_lcd_page += 1;
            } else {
                app.selected_lcd_page = 0;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            app.switch_lcd_page(app.selected_lcd_page + 1);
        }
        KeyCode::Char('t') => {
            app.send_test_toast();
        }
        KeyCode::Char('p') => {
            let start = Instant::now();
            match send_ipc_request(&IpcRequest::Ping) {
                Ok(resp) if resp.ok => {
                    let elapsed = start.elapsed();
                    app.set_status(format!("Daemon IPC PONG ({:?})", elapsed));
                }
                _ => app.set_status("IPC Ping failed (g510d offline?)"),
            }
        }
        _ => {}
    }
}

fn handle_audio_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('m') => {
            app.toggle_mic();
        }
        KeyCode::Char('a') => {
            app.toggle_audio();
        }
        KeyCode::Char('r') => {
            app.refresh_status();
            app.set_status("Diagnostics refreshed");
        }
        _ => {}
    }
}

fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header & Tabs
            Constraint::Min(12),   // Body
            Constraint::Length(3), // Status & Shortcuts
        ])
        .split(size);

    // 1. Header with Tabs
    let tab_titles = vec![
        Span::raw(" 1: Lighting & RGB "),
        Span::raw(" 2: Macro Controls "),
        Span::raw(" 3: LCD & IPC Applets "),
        Span::raw(" 4: Audio & Status "),
    ];

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Logitech G510 / G510s Interactive Control Suite ")
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .select(app.selected_tab)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray),
        );
    f.render_widget(tabs, chunks[0]);

    // 2. Active Tab Body
    match app.selected_tab {
        0 => render_lighting_tab(f, chunks[1], app),
        1 => render_macro_tab(f, chunks[1], app),
        2 => render_lcd_tab(f, chunks[1], app),
        3 => render_audio_tab(f, chunks[1], app),
        _ => {}
    }

    // Modal popup if active
    if app.show_preset_modal {
        render_preset_modal(f, size, app);
    }

    // 3. Status Bar & Shortcuts
    let (msg, age) = match &app.status_msg {
        Some((m, t)) if t.elapsed() < Duration::from_secs(8) => (m.as_str(), Color::Green),
        _ => ("Ready. Press Tab/1-4 to navigate, q to quit.", Color::DarkGray),
    };

    let status_bar = Paragraph::new(Line::from(vec![
        Span::styled(" [STATUS] ", Style::default().fg(Color::Black).bg(age).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(msg, Style::default().fg(Color::White)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Gray)),
    );
    f.render_widget(status_bar, chunks[2]);
}

fn render_lighting_tab(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left column: Preset color list
    let items: Vec<ListItem> = COLOR_PRESETS
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let selected = i == app.color_idx;
            let marker = if selected { "▶ " } else { "  " };
            let swatch_style = Style::default().fg(Color::Rgb(p.rgb.0, p.rgb.1, p.rgb.2));
            let line = Line::from(vec![
                Span::raw(marker),
                Span::styled("███ ", swatch_style),
                Span::styled(format!("{:<18}", p.name), if selected { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default() }),
                Span::styled(format!("({})", p.hex), Style::default().fg(Color::Gray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Color Presets (Up/Down, Enter: Apply) ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );
    f.render_widget(list, chunks[0]);

    // Right column: Brightness slider, target, alert actions
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Brightness Gauge
            Constraint::Length(5), // Target & Hardware info
            Constraint::Min(4),    // Action buttons & alert test
        ])
        .split(chunks[1]);

    // Brightness gauge
    let pct = (app.brightness as f64 / 255.0) * 100.0;
    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(" Brightness Level (Left/Right: -/+) ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent(pct as u16)
        .label(format!("{}/255 ({:.0}%)", app.brightness, pct));
    f.render_widget(gauge, right_chunks[0]);

    // Target mode & current hardware color
    let cur_hex = app.hardware_snap.live_rgb.as_ref().map_or("N/A", |s| s.hex.as_str());
    let power_hex = app.hardware_snap.power_on_rgb.as_ref().map_or("N/A", |s| s.hex.as_str());

    let info_text = Text::from(vec![
        Line::from(vec![
            Span::raw("Target Device Mode: "),
            Span::styled(format!("[ {} ]", app.target_mode.to_uppercase()), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" (Press 't' to toggle)"),
        ]),
        Line::from(vec![
            Span::raw("Hardware Active Color: "),
            Span::styled(cur_hex, Style::default().fg(Color::Green)),
            Span::raw("  |  Power-on Reset: "),
            Span::styled(power_hex, Style::default().fg(Color::Magenta)),
        ]),
    ]);
    let info_para = Paragraph::new(info_text).block(
        Block::default()
            .title(" Live Hardware State ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    f.render_widget(info_para, right_chunks[1]);

    // Action hints
    let hints_text = Text::from(vec![
        Line::from(vec![
            Span::styled("Enter / Space", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" : Apply selected color & brightness immediately"),
        ]),
        Line::from(vec![
            Span::styled("f", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("           : Test Flash Alert (flashes 3x in selected color)"),
        ]),
        Line::from(vec![
            Span::styled("b", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("           : Test Breathing Pulse (3.0s smooth wave)"),
        ]),
    ]);
    let hints_para = Paragraph::new(hints_text).block(
        Block::default()
            .title(" Actions ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    f.render_widget(hints_para, right_chunks[2]);
}

fn render_macro_tab(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let prof = app.profile_name();
    let p_map = app.macro_config.profiles.get(prof);
    let theme_rgb = app.macro_config.profile_rgb.get(prof).map_or("#00dc00", |s| s.as_str());

    // Left Column: G-Keys list
    let mut items = Vec::new();
    for i in 1..=18 {
        let g_str = format!("G{i}");
        let selected = (i - 1) == app.selected_g_key;
        let marker = if selected { "▶ " } else { "  " };

        let action_desc = match p_map.and_then(|m| m.get(&g_str)) {
            Some(MacroAction::Keys { keys }) => {
                format!("[KEYS] {}", keys.join("+"))
            }
            Some(MacroAction::Exec { argv }) => {
                format!("[EXEC] {}", argv.join(" "))
            }
            Some(MacroAction::Recorded { events }) => {
                format!("[RECORDED] {} keystrokes", events.len())
            }
            None => "[ UNASSIGNED ]".to_string(),
        };

        let style = if selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if action_desc.starts_with("[ UNASSIGNED") {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };

        items.push(ListItem::new(Line::from(vec![
            Span::raw(marker),
            Span::styled(format!("{:<4}: ", g_str), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(action_desc, style),
        ])));
    }

    let g_list = List::new(items).block(
        Block::default()
            .title(format!(" Profile {} Macros (Up/Down, Enter: Edit/Assign) ", prof))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    f.render_widget(g_list, chunks[0]);

    // Right Column: Profile Selector, theme color, controls
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Profile selector
            Constraint::Length(7), // Selected G-key details
            Constraint::Min(6),    // Commands & Instructions
        ])
        .split(chunks[1]);

    // Profile selector bar
    let prof_tabs = vec![
        Span::raw(" M1: Dev "),
        Span::raw(" M2: SecLab "),
        Span::raw(" M3: Media "),
    ];
    let p_tabs = Tabs::new(prof_tabs)
        .select(app.active_profile)
        .block(
            Block::default()
                .title(" Active Profile (m / Left / Right) ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(p_tabs, right_chunks[0]);

    // Selected key detail
    let current_g = format!("G{}", app.selected_g_key + 1);
    let detail_text = match p_map.and_then(|m| m.get(&current_g)) {
        Some(MacroAction::Keys { keys }) => {
            vec![
                Line::from(vec![Span::raw("Type:    "), Span::styled("Keystroke Injection (uinput)", Style::default().fg(Color::Green))]),
                Line::from(vec![Span::raw("Chord:   "), Span::styled(keys.join(" + "), Style::default().fg(Color::Yellow))]),
            ]
        }
        Some(MacroAction::Exec { argv }) => {
            vec![
                Line::from(vec![Span::raw("Type:    "), Span::styled("Command Execution", Style::default().fg(Color::Cyan))]),
                Line::from(vec![Span::raw("Command: "), Span::styled(argv.join(" "), Style::default().fg(Color::White))]),
            ]
        }
        Some(MacroAction::Recorded { events }) => {
            vec![
                Line::from(vec![Span::raw("Type:    "), Span::styled("MR Recorded Sequence", Style::default().fg(Color::Magenta))]),
                Line::from(vec![Span::raw("Events:  "), Span::styled(format!("{} recorded input events", events.len()), Style::default().fg(Color::White))]),
            ]
        }
        None => {
            vec![
                Line::from(vec![Span::styled("No action assigned to this G-key.", Style::default().fg(Color::DarkGray))]),
                Line::from(vec![Span::raw("Press Enter to choose from preset actions.")]),
            ]
        }
    };

    let detail_para = Paragraph::new(Text::from(detail_text)).block(
        Block::default()
            .title(format!(" {} Configuration ", current_g))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    f.render_widget(detail_para, right_chunks[1]);

    // Instructions
    let instr_lines = vec![
        Line::from(vec![
            Span::styled("Enter / a", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" : Open Action Preset Picker"),
        ]),
        Line::from(vec![
            Span::styled("d / Del", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("   : Clear / Unassign key"),
        ]),
        Line::from(vec![
            Span::styled("s", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("         : Save config to config.json"),
        ]),
        Line::from(vec![
            Span::styled("r", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("         : Reload config from disk"),
        ]),
        Line::from(vec![
            Span::styled("c", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::raw("         : Set profile RGB to active color ("),
            Span::styled(theme_rgb, Style::default().fg(Color::Yellow)),
            Span::raw(")"),
        ]),
    ];
    let instr_para = Paragraph::new(Text::from(instr_lines)).block(
        Block::default()
            .title(" Macro Shortcuts ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    f.render_widget(instr_para, right_chunks[2]);
}

fn render_lcd_tab(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left Column: Built-in Carousel Pages
    let pages = [
        ("Page 1: [1/N:SYS]", "System Telemetry (CPU%, RAM%, Temp °C, Load)"),
        ("Page 2: [2/N:NET]", "Network Throughput (IP, IF, RX/TX, VPN status)"),
        ("Page 3: [3/N:SEC]", "Security Ticker (SSH failures, VMs, Docker)"),
        ("Page 4: [4/N:MEDIA]", "MPRIS Media Player (Playing track, artist, status)"),
        ("Page 5: [5/N:AGENT]", "AI Workbench Status (STATE, TASK, ROLE)"),
    ];

    let items: Vec<ListItem> = pages
        .iter()
        .enumerate()
        .map(|(i, (title, desc))| {
            let selected = i == app.selected_lcd_page;
            let marker = if selected { "▶ " } else { "  " };
            ListItem::new(vec![
                Line::from(vec![
                    Span::raw(marker),
                    Span::styled(*title, if selected { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Cyan) }),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(*desc, Style::default().fg(Color::Gray)),
                ]),
            ])
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Built-in LCD Carousel Pages (Enter: Jump to page) ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    f.render_widget(list, chunks[0]);

    // Right Column: Dynamic Plugins & Testing Actions
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    // Dynamic Pages
    let dyn_items: Vec<ListItem> = if app.dynamic_pages.is_empty() {
        vec![ListItem::new(Span::styled("No dynamic plugins registered over IPC.", Style::default().fg(Color::DarkGray)))]
    } else {
        app.dynamic_pages
            .iter()
            .map(|dp| {
                let bar_str = dp.bar.map_or("".into(), |b| format!(" [Bar: {:.0}%]", b));
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(format!("[ID: {}] TITLE: {}", dp.id, dp.title), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::styled(bar_str, Style::default().fg(Color::Cyan)),
                    ]),
                    Line::from(vec![
                        Span::raw("  L1: "),
                        Span::styled(&dp.line1, Style::default().fg(Color::White)),
                    ]),
                ])
            })
            .collect()
    };

    let dyn_list = List::new(dyn_items).block(
        Block::default()
            .title(format!(" Dynamic Plugins Registered over IPC ({}) ", app.dynamic_pages.len()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    f.render_widget(dyn_list, right_chunks[0]);

    // Testing Actions
    let action_text = Text::from(vec![
        Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" : Jump LCD carousel to selected page index"),
        ]),
        Line::from(vec![
            Span::styled("t", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("     : Dispatch modal toast overlay notification"),
        ]),
        Line::from(vec![
            Span::styled("p", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("     : Ping daemon IPC socket (/run/user/1000/g510d.sock)"),
        ]),
    ]);
    let action_para = Paragraph::new(action_text).block(
        Block::default()
            .title(" LCD Operations ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    f.render_widget(action_para, right_chunks[1]);
}

fn render_audio_tab(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left Column: Audio & Media mute status
    let mic_style = match app.mic_muted {
        Some(true) => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        Some(false) => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        None => Style::default().fg(Color::DarkGray),
    };
    let mic_text = match app.mic_muted {
        Some(true) => "[ MUTED ]",
        Some(false) => "[ ACTIVE / UNMUTED ]",
        None => "[ UNKNOWN / UNAVAILABLE ]",
    };

    let audio_style = match app.audio_muted {
        Some(true) => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        Some(false) => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        None => Style::default().fg(Color::DarkGray),
    };
    let audio_text = match app.audio_muted {
        Some(true) => "[ MUTED ]",
        Some(false) => "[ ACTIVE / UNMUTED ]",
        None => "[ UNKNOWN / UNAVAILABLE ]",
    };

    let audio_content = Text::from(vec![
        Line::from(vec![
            Span::raw("Microphone Source:   "),
            Span::styled(mic_text, mic_style),
        ]),
        Line::from(vec![
            Span::raw("                     (Press 'm' to toggle)"),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("Headphone / Sink:    "),
            Span::styled(audio_text, audio_style),
        ]),
        Line::from(vec![
            Span::raw("                     (Press 'a' to toggle)"),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Subsystem: ", Style::default().fg(Color::Gray)),
            Span::raw("PipeWire / PulseAudio synchronized"),
        ]),
    ]);

    let audio_block = Paragraph::new(audio_content).block(
        Block::default()
            .title(" Hardware Audio Synchronization ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    f.render_widget(audio_block, chunks[0]);

    // Right Column: Daemon & Hardware diagnostics
    let daemon_online = app.ipc_status.is_some();
    let daemon_style = if daemon_online {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };

    let lcd_path = find_hidraw_device().map_or("Not Found".into(), |p| p.display().to_string());
    let evdev_path = find_gaming_device().map_or("Not Found".into(), |(p, _)| p.display().to_string());

    let diag_content = Text::from(vec![
        Line::from(vec![
            Span::raw("Daemon (g510d):      "),
            Span::styled(if daemon_online { "ONLINE (Active)" } else { "OFFLINE" }, daemon_style),
        ]),
        Line::from(vec![
            Span::raw("IPC Socket:          "),
            Span::styled(socket_path().display().to_string(), Style::default().fg(Color::Cyan)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("LCD Transport Node:  "),
            Span::styled(lcd_path, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("Gaming Keys evdev:   "),
            Span::styled(evdev_path, Style::default().fg(Color::Yellow)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Press 'r' to refresh diagnostics", Style::default().fg(Color::DarkGray)),
        ]),
    ]);

    let diag_block = Paragraph::new(diag_content).block(
        Block::default()
            .title(" System & Device Diagnostics ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    f.render_widget(diag_block, chunks[1]);
}

fn render_preset_modal(f: &mut Frame, area: Rect, app: &App) {
    let presets = get_macro_presets();
    let modal_area = centered_rect(65, 75, area);

    f.render_widget(Clear, modal_area);

    let items: Vec<ListItem> = presets
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let selected = i == app.preset_modal_idx;
            let marker = if selected { "▶ " } else { "  " };
            ListItem::new(vec![
                Line::from(vec![
                    Span::raw(marker),
                    Span::styled(p.label, if selected { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(p.description, Style::default().fg(Color::Gray)),
                ]),
            ])
        })
        .collect();

    let g_name = format!("G{}", app.selected_g_key + 1);
    let list = List::new(items).block(
        Block::default()
            .title(format!(" Assign Action to {} in Profile {} (Enter: Assign, Esc: Cancel) ", g_name, app.profile_name()))
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(list, modal_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_presets_validity() {
        assert_eq!(COLOR_PRESETS.len(), 9);
        for p in COLOR_PRESETS {
            assert!(p.hex.starts_with('#'));
            assert_eq!(p.hex.len(), 7);
            // Verify hex parsing matches RGB
            let r = u8::from_str_radix(&p.hex[1..3], 16).unwrap();
            let g = u8::from_str_radix(&p.hex[3..5], 16).unwrap();
            let b = u8::from_str_radix(&p.hex[5..7], 16).unwrap();
            assert_eq!((r, g, b), p.rgb);
        }
    }

    #[test]
    fn test_macro_presets_validity() {
        let presets = get_macro_presets();
        assert!(presets.len() >= 15);
        for p in &presets {
            assert!(!p.label.is_empty());
            assert!(!p.description.is_empty());
        }
    }

    #[test]
    fn test_app_state_defaults() {
        let app = App::new();
        assert_eq!(app.selected_tab, 0);
        assert_eq!(app.active_profile, 0);
        assert_eq!(app.profile_name(), "M1");
        assert_eq!(app.selected_g_key, 0);
        assert!(!app.show_preset_modal);
    }

    #[test]
    fn test_app_profile_name_cycling() {
        let mut app = App::new();
        app.active_profile = 0;
        assert_eq!(app.profile_name(), "M1");
        app.active_profile = 1;
        assert_eq!(app.profile_name(), "M2");
        app.active_profile = 2;
        assert_eq!(app.profile_name(), "M3");
    }
}

