use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

pub const SYSFS_LEDS: &str = "/sys/class/leds";
pub const KBD_BACKLIGHT: &str = "g15::kbd_backlight";
pub const POWER_ON_BACKLIGHT: &str = "g15::power_on_backlight_val";
pub const MR_LED: &str = "g15::macro_record";
pub const PRESET_PREFIX: &str = "g15::macro_preset";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RgbState {
    pub hex: String,
    pub brightness: u8,
    pub max_brightness: u8,
    pub led_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LedSnapshot {
    pub live_rgb: Option<RgbState>,
    pub power_on_rgb: Option<RgbState>,
    pub m1: Option<bool>,
    pub m2: Option<bool>,
    pub m3: Option<bool>,
    pub mr: Option<bool>,
}

pub fn resolve_led(name: &str, root: Option<&Path>) -> io::Result<PathBuf> {
    let base = root.unwrap_or_else(|| Path::new(SYSFS_LEDS));
    let direct = base.join(name);
    if direct.is_dir() {
        return Ok(direct);
    }

    // Check for duplicate device suffixes like "g15::kbd_backlight-1"
    let prefix = format!("{}-", name);
    if let Ok(entries) = fs::read_dir(base) {
        let matches: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .map_or(false, |s| s.starts_with(&prefix))
            })
            .collect();

        if matches.len() == 1 {
            return Ok(matches[0].clone());
        } else if matches.len() > 1 {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("ambiguous LED devices for {name}: {:?}", matches),
            ));
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("{name} unavailable under {base:?}"),
    ))
}

fn write_attr(led: &Path, attribute: &str, value: &str) -> io::Result<()> {
    let target = led.join(attribute);
    fs::write(&target, format!("{value}\n"))
}

pub fn parse_hex_color(hex_str: &str) -> Result<[u8; 3], String> {
    let s = hex_str.trim().trim_start_matches('#');
    if s.len() != 6 {
        return Err("RGB color must be in 6-digit hex format (#RRGGBB)".to_string());
    }
    let r = u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?;
    Ok([r, g, b])
}

pub fn read_rgb(target: &str, root: Option<&Path>) -> io::Result<RgbState> {
    let led_name = match target {
        "live" => KBD_BACKLIGHT,
        "power-on" => POWER_ON_BACKLIGHT,
        other => other,
    };
    let led = resolve_led(led_name, root)?;

    let multi_raw = fs::read_to_string(led.join("multi_intensity"))?;
    let levels: Vec<u8> = multi_raw
        .split_whitespace()
        .filter_map(|s| s.parse::<u8>().ok())
        .collect();

    if levels.len() != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported multi_intensity layout: {multi_raw:?}"),
        ));
    }

    let brightness: u8 = fs::read_to_string(led.join("brightness"))?
        .trim()
        .parse()
        .unwrap_or(0);

    let max_brightness: u8 = fs::read_to_string(led.join("max_brightness"))?
        .trim()
        .parse()
        .unwrap_or(255);

    Ok(RgbState {
        hex: format!("#{:02x}{:02x}{:02x}", levels[0], levels[1], levels[2]),
        brightness,
        max_brightness,
        led_path: led.to_string_lossy().to_string(),
    })
}

pub fn set_rgb(
    color_hex: &str,
    brightness: u8,
    target: &str,
    root: Option<&Path>,
) -> io::Result<RgbState> {
    let [r, g, b] = parse_hex_color(color_hex)
        .map_err(|msg| io::Error::new(io::ErrorKind::InvalidInput, msg))?;

    let led_name = match target {
        "live" => KBD_BACKLIGHT,
        "power-on" => POWER_ON_BACKLIGHT,
        other => other,
    };
    let led = resolve_led(led_name, root)?;

    write_attr(&led, "multi_intensity", &format!("{r} {g} {b}"))?;
    write_attr(&led, "brightness", &brightness.to_string())?;

    read_rgb(target, root)
}

pub fn set_profile(profile: u8, root: Option<&Path>) -> io::Result<()> {
    if !(1..=3).contains(&profile) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "profile must be 1, 2, or 3",
        ));
    }

    for n in 1..=3 {
        let name = format!("{PRESET_PREFIX}{n}");
        if let Ok(led) = resolve_led(&name, root) {
            let _ = write_attr(&led, "brightness", if n == profile { "1" } else { "0" });
        }
    }
    Ok(())
}

pub fn set_indicator(name: &str, enabled: bool, root: Option<&Path>) -> io::Result<()> {
    let led_name = match name {
        "MR" => MR_LED.to_string(),
        "M1" => format!("{PRESET_PREFIX}1"),
        "M2" => format!("{PRESET_PREFIX}2"),
        "M3" => format!("{PRESET_PREFIX}3"),
        other => other.to_string(),
    };
    let led = resolve_led(&led_name, root)?;
    write_attr(&led, "brightness", if enabled { "1" } else { "0" })
}

pub fn snapshot(root: Option<&Path>) -> LedSnapshot {
    let live_rgb = read_rgb("live", root).ok();
    let power_on_rgb = read_rgb("power-on", root).ok();

    let read_preset = |n: u8| -> Option<bool> {
        let name = format!("{PRESET_PREFIX}{n}");
        let led = resolve_led(&name, root).ok()?;
        let val = fs::read_to_string(led.join("brightness")).ok()?;
        Some(val.trim() == "1")
    };

    let read_mr = || -> Option<bool> {
        let led = resolve_led(MR_LED, root).ok()?;
        let val = fs::read_to_string(led.join("brightness")).ok()?;
        Some(val.trim() == "1")
    };

    LedSnapshot {
        live_rgb,
        power_on_rgb,
        m1: read_preset(1),
        m2: read_preset(2),
        m3: read_preset(3),
        mr: read_mr(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_parsing() {
        assert_eq!(parse_hex_color("#00dc00").unwrap(), [0, 220, 0]);
        assert_eq!(parse_hex_color("ff0000").unwrap(), [255, 0, 0]);
        assert_eq!(parse_hex_color("#123456").unwrap(), [0x12, 0x34, 0x56]);

        assert!(parse_hex_color("red").is_err());
        assert!(parse_hex_color("#123").is_err());
        assert!(parse_hex_color("#12345678").is_err());
        assert!(parse_hex_color("invalid").is_err());
    }

    #[test]
    fn test_sysfs_fake_leds() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        let live_led = root.join("g15::kbd_backlight");
        fs::create_dir_all(&live_led).unwrap();
        fs::write(live_led.join("brightness"), "255\n").unwrap();
        fs::write(live_led.join("max_brightness"), "255\n").unwrap();
        fs::write(live_led.join("multi_intensity"), "0 220 0\n").unwrap();

        let state = read_rgb("live", Some(root)).unwrap();
        assert_eq!(state.hex, "#00dc00");
        assert_eq!(state.brightness, 255);

        let updated = set_rgb("#ff0000", 128, "live", Some(root)).unwrap();
        assert_eq!(updated.hex, "#ff0000");
        assert_eq!(updated.brightness, 128);

        let re_read = read_rgb("live", Some(root)).unwrap();
        assert_eq!(re_read.hex, "#ff0000");
        assert_eq!(re_read.brightness, 128);
    }
}
