use std::thread;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

use crate::leds::set_rgb;

fn default_flash_color() -> String {
    "#ff0000".to_string()
}

fn default_flash_times() -> u32 {
    3
}

fn default_pulse_color() -> String {
    "#00ffff".to_string()
}

fn default_pulse_duration() -> f32 {
    3.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RgbAlert {
    #[serde(rename = "flash")]
    Flash {
        #[serde(default = "default_flash_color")]
        color: String,
        #[serde(default = "default_flash_times")]
        times: u32,
    },
    #[serde(rename = "pulse")]
    Pulse {
        #[serde(default = "default_pulse_color")]
        color: String,
        #[serde(default = "default_pulse_duration")]
        duration: f32,
    },
}

pub fn run_flash_alert(color: &str, times: u32, restore_color: &str) {
    let count = times.clamp(1, 10);
    for _ in 0..count {
        let _ = set_rgb(color, 255, "live", None);
        thread::sleep(Duration::from_millis(150));
        let _ = set_rgb(color, 10, "live", None);
        thread::sleep(Duration::from_millis(150));
    }
    let _ = set_rgb(restore_color, 255, "live", None);
}

pub fn run_pulse_alert(color: &str, duration_secs: f32, restore_color: &str) {
    let duration = Duration::from_secs_f32(duration_secs.clamp(0.5, 30.0));
    let start = Instant::now();
    let _ = set_rgb(color, 255, "live", None);

    let step = Duration::from_millis(40);
    let mut phase: f32 = 0.0;

    while start.elapsed() < duration {
        // Sine wave breathing: 30 to 255
        let brightness = ((phase.sin() * 0.5 + 0.5) * 215.0 + 40.0).round() as u8;
        let _ = set_rgb(color, brightness, "live", None);
        phase += 0.25;
        thread::sleep(step);
    }

    let _ = set_rgb(restore_color, 255, "live", None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_json_parsing() {
        let flash_json = r##"{"type": "flash", "color": "#ff0000", "times": 3}"##;
        let alert: RgbAlert = serde_json::from_str(flash_json).unwrap();
        match alert {
            RgbAlert::Flash { color, times } => {
                assert_eq!(color, "#ff0000");
                assert_eq!(times, 3);
            }
            _ => panic!("expected flash"),
        }

        let pulse_json = r##"{"type": "pulse", "color": "#00ffff", "duration": 4.5}"##;
        let alert2: RgbAlert = serde_json::from_str(pulse_json).unwrap();
        match alert2 {
            RgbAlert::Pulse { color, duration } => {
                assert_eq!(color, "#00ffff");
                assert_eq!(duration, 4.5);
            }
            _ => panic!("expected pulse"),
        }
    }
}
