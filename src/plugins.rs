use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

pub fn default_plugins_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/g510s-drive/plugins")
    } else {
        PathBuf::from(".config/g510s-drive/plugins")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginConfig {
    pub name: String,
    #[serde(default = "default_plugin_interval")]
    pub interval_sec: u64,
    pub command: Vec<String>,
}

fn default_plugin_interval() -> u64 {
    2
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PluginScreenData {
    pub title: String,
    #[serde(default)]
    pub line1: String,
    #[serde(default)]
    pub line2: String,
    #[serde(default)]
    pub line3: String,
    pub bar: Option<f32>,
}

pub struct ActivePlugin {
    pub config: PluginConfig,
    pub last_run: Option<Instant>,
    pub cached_data: PluginScreenData,
}

impl ActivePlugin {
    pub fn new(config: PluginConfig) -> Self {
        let name = config.name.clone();
        Self {
            config,
            last_run: None,
            cached_data: PluginScreenData {
                title: name,
                line1: "INITIALIZING...".into(),
                line2: "".into(),
                line3: "".into(),
                bar: None,
            },
        }
    }

    pub fn poll_if_due(&mut self) {
        let interval = Duration::from_secs(self.config.interval_sec.max(1));
        let should_run = match self.last_run {
            Some(last) => last.elapsed() >= interval,
            None => true,
        };

        if should_run {
            self.last_run = Some(Instant::now());
            if let Some(data) = execute_plugin_command(&self.config.command, &self.config.name) {
                self.cached_data = data;
            }
        }
    }

    pub fn handle_action(&mut self, action_name: &str) {
        if let Some((cmd, args)) = self.config.command.split_first() {
            let mut full_args: Vec<String> = args.to_vec();
            full_args.push("--action".to_string());
            full_args.push(action_name.to_string());

            let _ = Command::new(cmd)
                .args(&full_args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
    }
}

pub fn parse_plugin_output(stdout: &str, default_title: &str) -> PluginScreenData {
    let trimmed = stdout.trim();

    // 1. Try parsing JSON format
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        if let Ok(mut json_data) = serde_json::from_str::<PluginScreenData>(trimmed) {
            if json_data.title.is_empty() {
                json_data.title = default_title.to_string();
            }
            return json_data;
        }
    }

    // 2. Parse plain text line-by-line
    let mut data = PluginScreenData {
        title: default_title.to_string(),
        line1: "".into(),
        line2: "".into(),
        line3: "".into(),
        bar: None,
    };

    let mut text_lines = Vec::new();
    for line in trimmed.lines() {
        let l = line.trim();
        if l.starts_with("BAR:") {
            if let Ok(val) = l.trim_start_matches("BAR:").trim().parse::<f32>() {
                data.bar = Some(val.clamp(0.0, 100.0));
            }
        } else if l.starts_with("TITLE:") {
            data.title = l.trim_start_matches("TITLE:").trim().to_string();
        } else if !l.is_empty() {
            text_lines.push(l.to_string());
        }
    }

    if let Some(l1) = text_lines.first() {
        data.line1 = l1.clone();
    }
    if let Some(l2) = text_lines.get(1) {
        data.line2 = l2.clone();
    }
    if let Some(l3) = text_lines.get(2) {
        data.line3 = l3.clone();
    }

    data
}

pub fn execute_plugin_command(argv: &[String], default_title: &str) -> Option<PluginScreenData> {
    if argv.is_empty() {
        return None;
    }
    let (cmd, args) = argv.split_first()?;

    let output = Command::new(cmd)
        .args(args)
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        Some(parse_plugin_output(&text, default_title))
    } else {
        None
    }
}

/// Automatically discover executable scripts in ~/.config/g510s-drive/plugins/
pub fn discover_directory_plugins() -> Vec<PluginConfig> {
    let dir = default_plugins_dir();
    let mut discovered = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                // Check if file is executable or has .sh / .py
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("PLUGIN")
                    .to_uppercase();

                discovered.push(PluginConfig {
                    name,
                    interval_sec: 2,
                    command: vec![path.to_string_lossy().to_string()],
                });
            }
        }
    }

    discovered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_plugin_output() {
        let json = r#"{"title": "DOCKER", "line1": "Containers: 5", "line2": "CPU: 12%", "bar": 50.0}"#;
        let data = parse_plugin_output(json, "DEFAULT");
        assert_eq!(data.title, "DOCKER");
        assert_eq!(data.line1, "Containers: 5");
        assert_eq!(data.line2, "CPU: 12%");
        assert_eq!(data.bar, Some(50.0));
    }

    #[test]
    fn test_parse_text_plugin_output() {
        let text = "TITLE: SENSORS\nBAR: 85.5\nTemp: 45°C\nFan: 2200 RPM\nPump: Normal\n";
        let data = parse_plugin_output(text, "DEFAULT");
        assert_eq!(data.title, "SENSORS");
        assert_eq!(data.bar, Some(85.5));
        assert_eq!(data.line1, "Temp: 45°C");
        assert_eq!(data.line2, "Fan: 2200 RPM");
        assert_eq!(data.line3, "Pump: Normal");
    }
}
