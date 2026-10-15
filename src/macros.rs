use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use evdev::{
    uinput::VirtualDevice,
    AttributeSet, EventType, InputEvent, KeyCode,
};
use serde::{Deserialize, Serialize};

use crate::leds::{set_indicator, set_profile, set_rgb};

pub fn default_config_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/g510s-drive/config.json")
    } else {
        PathBuf::from(".config/g510s-drive/config.json")
    }
}

pub fn parse_key_name(name: &str) -> Option<u16> {
    match name {
        "KEY_ESC" => Some(1),
        "KEY_1" => Some(2),
        "KEY_2" => Some(3),
        "KEY_3" => Some(4),
        "KEY_4" => Some(5),
        "KEY_5" => Some(6),
        "KEY_6" => Some(7),
        "KEY_7" => Some(8),
        "KEY_8" => Some(9),
        "KEY_9" => Some(10),
        "KEY_0" => Some(11),
        "KEY_MINUS" => Some(12),
        "KEY_EQUAL" => Some(13),
        "KEY_BACKSPACE" => Some(14),
        "KEY_TAB" => Some(15),
        "KEY_Q" => Some(16),
        "KEY_W" => Some(17),
        "KEY_E" => Some(18),
        "KEY_R" => Some(19),
        "KEY_T" => Some(20),
        "KEY_Y" => Some(21),
        "KEY_U" => Some(22),
        "KEY_I" => Some(23),
        "KEY_O" => Some(24),
        "KEY_P" => Some(25),
        "KEY_ENTER" => Some(28),
        "KEY_LEFTCTRL" => Some(29),
        "KEY_A" => Some(30),
        "KEY_S" => Some(31),
        "KEY_D" => Some(32),
        "KEY_F" => Some(33),
        "KEY_G" => Some(34),
        "KEY_H" => Some(35),
        "KEY_J" => Some(36),
        "KEY_K" => Some(37),
        "KEY_L" => Some(38),
        "KEY_LEFTSHIFT" => Some(42),
        "KEY_Z" => Some(44),
        "KEY_X" => Some(45),
        "KEY_C" => Some(46),
        "KEY_V" => Some(47),
        "KEY_B" => Some(48),
        "KEY_N" => Some(49),
        "KEY_M" => Some(50),
        "KEY_LEFTALT" => Some(56),
        "KEY_SPACE" => Some(57),
        "KEY_F1" => Some(59),
        "KEY_F2" => Some(60),
        "KEY_F3" => Some(61),
        "KEY_F4" => Some(62),
        "KEY_F5" => Some(63),
        "KEY_F6" => Some(64),
        "KEY_F7" => Some(65),
        "KEY_F8" => Some(66),
        "KEY_F9" => Some(67),
        "KEY_F10" => Some(68),
        "KEY_F11" => Some(87),
        "KEY_F12" => Some(88),
        "KEY_RIGHTCTRL" => Some(97),
        "KEY_RIGHTALT" => Some(100),
        "KEY_LINEFEED" => Some(101),
        "KEY_HOME" => Some(102),
        "KEY_UP" => Some(103),
        "KEY_PAGEUP" => Some(104),
        "KEY_LEFT" => Some(105),
        "KEY_RIGHT" => Some(106),
        "KEY_END" => Some(107),
        "KEY_DOWN" => Some(108),
        "KEY_PAGEDOWN" => Some(109),
        "KEY_LEFTMETA" => Some(125),
        "KEY_RIGHTMETA" => Some(126),
        _ => name.parse::<u16>().ok(),
    }
}

pub struct KeyInjector {
    device: Option<VirtualDevice>,
}

impl Default for KeyInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyInjector {
    pub fn new() -> Self {
        let mut keys = AttributeSet::<KeyCode>::new();
        for code in 1..=255 {
            keys.insert(KeyCode::new(code));
        }
        let device = VirtualDevice::builder()
            .and_then(|b| b.name("G510s-Drive Macro Injector").with_keys(&keys))
            .and_then(|b| b.build())
            .ok();

        Self { device }
    }

    pub fn emit_combo(&mut self, codes: &[u16]) {
        if let Some(dev) = self.device.as_mut() {
            let mut events = Vec::new();
            for &c in codes {
                events.push(InputEvent::new(EventType::KEY.0, c, 1));
            }
            events.push(InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0));
            let _ = dev.emit(&events);

            thread::sleep(Duration::from_millis(20));

            let mut release_events = Vec::new();
            for &c in codes.iter().rev() {
                release_events.push(InputEvent::new(EventType::KEY.0, c, 0));
            }
            release_events.push(InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0));
            let _ = dev.emit(&release_events);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MacroAction {
    #[serde(rename = "exec")]
    Exec { argv: Vec<String> },
    #[serde(rename = "keys")]
    Keys { keys: Vec<String> },
    #[serde(rename = "recorded")]
    Recorded {
        #[serde(default)]
        events: Vec<RecordedEvent>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordedEvent {
    pub code: String,
    pub value: i32,
    pub delay_ms: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MacroConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_profiles")]
    pub profiles: HashMap<String, HashMap<String, MacroAction>>,
    #[serde(default = "default_profile_rgb")]
    pub profile_rgb: HashMap<String, String>,
    #[serde(default)]
    pub special_actions: HashMap<String, MacroAction>,
}

fn default_version() -> u32 {
    1
}

fn exec_action(cmd: &[&str]) -> MacroAction {
    MacroAction::Exec {
        argv: cmd.iter().map(|s| s.to_string()).collect(),
    }
}

fn keys_action(keys: &[&str]) -> MacroAction {
    MacroAction::Keys {
        keys: keys.iter().map(|s| s.to_string()).collect(),
    }
}

pub fn default_profiles() -> HashMap<String, HashMap<String, MacroAction>> {
    let mut profiles = HashMap::new();

    // -------------------------------------------------------------
    // PROFILE M1: Development & Daily Driving (Green #00dc00)
    // -------------------------------------------------------------
    let mut m1 = HashMap::new();
    m1.insert("G1".to_string(), exec_action(&["/usr/bin/x-terminal-emulator"]));
    m1.insert("G2".to_string(), exec_action(&["/usr/bin/konsole"]));
    m1.insert("G3".to_string(), exec_action(&["/usr/local/bin/g510ctl", "page", "1"]));
    m1.insert("G4".to_string(), exec_action(&["/usr/bin/git", "status"]));
    m1.insert("G5".to_string(), exec_action(&["/usr/bin/git", "diff", "--stat"]));
    m1.insert("G6".to_string(), exec_action(&["/usr/bin/git", "pull"]));
    m1.insert("G7".to_string(), exec_action(&["/home/john/.cargo/bin/cargo", "check"]));
    m1.insert("G8".to_string(), exec_action(&["/home/john/.cargo/bin/cargo", "test"]));
    m1.insert("G9".to_string(), exec_action(&["/home/john/.cargo/bin/cargo", "build", "--release"]));
    m1.insert("G10".to_string(), keys_action(&["KEY_LEFTCTRL", "KEY_L"])); // Clear terminal
    m1.insert("G11".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "DEV PROFILE\nM1 Active"]));
    m1.insert("G12".to_string(), exec_action(&["/usr/local/bin/g510ctl", "page", "5"])); // Agent page
    m1.insert("G13".to_string(), keys_action(&["KEY_LEFTCTRL", "KEY_C"])); // Copy
    m1.insert("G14".to_string(), keys_action(&["KEY_LEFTCTRL", "KEY_V"])); // Paste
    m1.insert("G15".to_string(), keys_action(&["KEY_LEFTCTRL", "KEY_F"])); // Find
    m1.insert("G16".to_string(), keys_action(&["KEY_LEFTCTRL", "KEY_S"])); // Save
    m1.insert("G17".to_string(), keys_action(&["KEY_LEFTCTRL", "KEY_Z"])); // Undo
    m1.insert("G18".to_string(), keys_action(&["KEY_LEFTCTRL", "KEY_W"])); // Close tab
    profiles.insert("M1".to_string(), m1);

    // -------------------------------------------------------------
    // PROFILE M2: Workbench & Security Lab (Red #dc0000)
    // -------------------------------------------------------------
    let mut m2 = HashMap::new();
    m2.insert("G1".to_string(), exec_action(&["/usr/local/bin/g510ctl", "page", "3"])); // Security page
    m2.insert("G2".to_string(), exec_action(&["/usr/local/bin/g510ctl", "page", "2"])); // Network page
    m2.insert("G3".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "SECURITY LAB\nRunning Diagnostics"]));
    m2.insert("G4".to_string(), exec_action(&["/usr/bin/docker", "ps"]));
    m2.insert("G5".to_string(), exec_action(&["/usr/bin/journalctl", "-u", "ssh", "-n", "10"]));
    m2.insert("G6".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "NETWORK AUDIT\nInterfaces Verified"]));
    m2.insert("G7".to_string(), exec_action(&["/usr/local/bin/g510ctl", "page", "1"])); // System page
    m2.insert("G8".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "SANDBOX CHECK\nVMs Verified"]));
    m2.insert("G9".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "SNAPSHOT\nState Saved"]));
    m2.insert("G10".to_string(), keys_action(&["KEY_LEFTCTRL", "KEY_C"]));
    m2.insert("G11".to_string(), keys_action(&["KEY_LEFTCTRL", "KEY_V"]));
    m2.insert("G12".to_string(), keys_action(&["KEY_LEFTCTRL", "KEY_L"]));
    m2.insert("G13".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "LAB M2\nG13 Trigger"]));
    m2.insert("G14".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "LAB M2\nG14 Trigger"]));
    m2.insert("G15".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "LAB M2\nG15 Trigger"]));
    m2.insert("G16".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "LAB M2\nG16 Trigger"]));
    m2.insert("G17".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "LAB M2\nG17 Trigger"]));
    m2.insert("G18".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "LAB M2\nG18 Trigger"]));
    profiles.insert("M2".to_string(), m2);

    // -------------------------------------------------------------
    // PROFILE M3: Communications & Multimedia (Blue #0000dc)
    // -------------------------------------------------------------
    let mut m3 = HashMap::new();
    m3.insert("G1".to_string(), exec_action(&["/usr/bin/playerctl", "play-pause"]));
    m3.insert("G2".to_string(), exec_action(&["/usr/bin/playerctl", "next"]));
    m3.insert("G3".to_string(), exec_action(&["/usr/bin/playerctl", "previous"]));
    m3.insert("G4".to_string(), exec_action(&["/usr/bin/playerctl", "volume", "0.05+"]));
    m3.insert("G5".to_string(), exec_action(&["/usr/bin/playerctl", "volume", "0.05-"]));
    m3.insert("G6".to_string(), exec_action(&["/usr/local/bin/g510ctl", "page", "4"])); // Media Page
    m3.insert("G7".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "MUTED\nAudio Toggle"]));
    m3.insert("G8".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "MIC MUTED\nMic Toggle"]));
    m3.insert("G9".to_string(), keys_action(&["KEY_LEFTCTRL", "KEY_LEFTALT", "KEY_L"])); // Screen Lock
    m3.insert("G10".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "COMM M3\nG10 Trigger"]));
    m3.insert("G11".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "COMM M3\nG11 Trigger"]));
    m3.insert("G12".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "COMM M3\nG12 Trigger"]));
    m3.insert("G13".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "COMM M3\nG13 Trigger"]));
    m3.insert("G14".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "COMM M3\nG14 Trigger"]));
    m3.insert("G15".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "COMM M3\nG15 Trigger"]));
    m3.insert("G16".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "COMM M3\nG16 Trigger"]));
    m3.insert("G17".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "COMM M3\nG17 Trigger"]));
    m3.insert("G18".to_string(), exec_action(&["/usr/local/bin/g510ctl", "toast", "COMM M3\nG18 Trigger"]));
    profiles.insert("M3".to_string(), m3);

    profiles
}

pub fn default_profile_rgb() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("M1".to_string(), "#00dc00".to_string());
    map.insert("M2".to_string(), "#dc0000".to_string());
    map.insert("M3".to_string(), "#0000dc".to_string());
    map
}

impl Default for MacroConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            profiles: default_profiles(),
            profile_rgb: default_profile_rgb(),
            special_actions: HashMap::new(),
        }
    }
}

pub fn load_config(path: &Path) -> io::Result<MacroConfig> {
    let data = fs::read_to_string(path)?;
    serde_json::from_str(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn save_config(config: &MacroConfig, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json_bytes = serde_json::to_vec_pretty(config)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(path, json_bytes)
}

pub struct MacroEngine {
    pub config: MacroConfig,
    pub active_profile: String,
    pub injector: KeyInjector,
    pub mr_armed: bool,
    pub recording_key: Option<String>,
}

impl MacroEngine {
    pub fn new(config: MacroConfig) -> Self {
        let mut engine = Self {
            config,
            active_profile: "M1".to_string(),
            injector: KeyInjector::new(),
            mr_armed: false,
            recording_key: None,
        };
        engine.activate_profile("M1");
        engine
    }

    pub fn activate_profile(&mut self, profile: &str) {
        if !["M1", "M2", "M3"].contains(&profile) {
            return;
        }
        self.active_profile = profile.to_string();

        let num: u8 = profile[1..].parse().unwrap_or(1);
        let _ = set_profile(num, None);

        if let Some(color) = self.config.profile_rgb.get(profile) {
            let _ = set_rgb(color, 255, "live", None);
        }
        eprintln!("[g510ctl] Active profile: {profile}");
    }

    pub fn toggle_mr(&mut self) -> Option<String> {
        if self.mr_armed || self.recording_key.is_some() {
            self.mr_armed = false;
            let target = self.recording_key.take();
            let _ = set_indicator("MR", false, None);
            eprintln!("[g510ctl] MR recording cleared");
            target.map(|k| format!("SAVED {k}"))
        } else {
            self.mr_armed = true;
            let _ = set_indicator("MR", true, None);
            eprintln!("[g510ctl] MR armed: press G1-G18 to assign");
            Some("MR ARMED\nPress G1-G18".to_string())
        }
    }

    pub fn execute_action(&mut self, action: &MacroAction) {
        match action {
            MacroAction::Exec { argv } => {
                if let Some((cmd, args)) = argv.split_first() {
                    let _ = Command::new(cmd)
                        .args(args)
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                }
            }
            MacroAction::Keys { keys } => {
                let codes: Vec<u16> = keys.iter().filter_map(|k| parse_key_name(k)).collect();
                if !codes.is_empty() {
                    self.injector.emit_combo(&codes);
                }
            }
            MacroAction::Recorded { events } => {
                for ev in events {
                    if let Some(code) = parse_key_name(&ev.code) {
                        if ev.delay_ms > 0 {
                            thread::sleep(Duration::from_millis(ev.delay_ms as u64));
                        }
                        self.injector.emit_combo(&[code]);
                    }
                }
            }
        }
    }

    pub fn handle_key(&mut self, key_name: &str) {
        if ["M1", "M2", "M3"].contains(&key_name) {
            self.activate_profile(key_name);
            return;
        }

        if key_name == "MR" {
            let _ = self.toggle_mr();
            return;
        }

        if self.mr_armed && key_name.starts_with('G') {
            self.mr_armed = false;
            self.recording_key = Some(key_name.to_string());
            eprintln!("[g510ctl] Assigned {key_name} for MR recording");
            return;
        }

        if let Some(profile_map) = self.config.profiles.get(&self.active_profile) {
            if let Some(action) = profile_map.get(key_name) {
                self.execute_action(&action.clone());
                return;
            }
        }

        if let Some(action) = self.config.special_actions.get(key_name) {
            self.execute_action(&action.clone());
            return;
        }

        if key_name.starts_with('L') {
            eprintln!("[g510ctl] LCD button {key_name} pressed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validity() {
        let config = MacroConfig::default();
        assert_eq!(config.version, 1);
        assert!(config.profiles.contains_key("M1"));
        assert!(config.profiles.contains_key("M2"));
        assert!(config.profiles.contains_key("M3"));

        // Verify 18 G-keys configured for each profile
        for prof in &["M1", "M2", "M3"] {
            let map = config.profiles.get(*prof).unwrap();
            for i in 1..=18 {
                let g = format!("G{i}");
                assert!(map.contains_key(&g), "missing {} in profile {}", g, prof);
            }
        }
    }

    #[test]
    fn test_parse_key_name() {
        assert_eq!(parse_key_name("KEY_LEFTCTRL"), Some(29));
        assert_eq!(parse_key_name("KEY_C"), Some(46));
        assert_eq!(parse_key_name("KEY_V"), Some(47));
        assert_eq!(parse_key_name("KEY_ENTER"), Some(28));
        assert_eq!(parse_key_name("99"), Some(99));
        assert_eq!(parse_key_name("UNKNOWN_KEY"), None);
    }

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let mut config = MacroConfig::default();
        config.profile_rgb.insert("M1".to_string(), "#123456".to_string());

        save_config(&config, &config_path).unwrap();
        assert!(config_path.exists());

        let loaded = load_config(&config_path).unwrap();
        assert_eq!(loaded.profile_rgb.get("M1").unwrap(), "#123456");
    }
}
