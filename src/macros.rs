use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use serde::{Deserialize, Serialize};

use crate::leds::{set_profile, set_rgb};

pub fn default_config_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/g510s-drive/config.json")
    } else {
        PathBuf::from(".config/g510s-drive/config.json")
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

fn default_profiles() -> HashMap<String, HashMap<String, MacroAction>> {
    let mut map = HashMap::new();
    let mut m1 = HashMap::new();
    m1.insert(
        "G1".to_string(),
        MacroAction::Exec {
            argv: vec![
                "/usr/bin/notify-send".to_string(),
                "G510".to_string(),
                "G1 pressed".to_string(),
            ],
        },
    );
    map.insert("M1".to_string(), m1);
    map.insert("M2".to_string(), HashMap::new());
    map.insert("M3".to_string(), HashMap::new());
    map
}

fn default_profile_rgb() -> HashMap<String, String> {
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
}

impl MacroEngine {
    pub fn new(config: MacroConfig) -> Self {
        let mut engine = Self {
            config,
            active_profile: "M1".to_string(),
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

    pub fn execute_action(&self, action: &MacroAction) {
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
                eprintln!("[g510ctl] Simulating keys: {:?}", keys);
            }
            MacroAction::Recorded { events } => {
                eprintln!("[g510ctl] Replaying {} recorded events", events.len());
            }
        }
    }

    pub fn handle_key(&mut self, key_name: &str) {
        if ["M1", "M2", "M3"].contains(&key_name) {
            self.activate_profile(key_name);
            return;
        }

        if let Some(profile_map) = self.config.profiles.get(&self.active_profile) {
            if let Some(action) = profile_map.get(key_name) {
                self.execute_action(action);
                return;
            }
        }

        if let Some(action) = self.config.special_actions.get(key_name) {
            self.execute_action(action);
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
        assert_eq!(config.profile_rgb.get("M1").unwrap(), "#00dc00");
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
