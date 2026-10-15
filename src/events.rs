use std::io;
use std::path::PathBuf;
use evdev::{Device, KeyCode};

pub const USB_VENDOR: u16 = 0x046d;
pub const USB_PRODUCTS: [u16; 2] = [0xc22d, 0xc22e];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum G510Key {
    G(u8),      // G1..G18
    Profile(u8),// M1..M3
    Mr,         // Macro Record
    LcdMenu(u8),// L1..L5
    Media(String),
    Other(String),
}

impl G510Key {
    pub fn name(&self) -> String {
        match self {
            G510Key::G(n) => format!("G{n}"),
            G510Key::Profile(n) => format!("M{n}"),
            G510Key::Mr => "MR".to_string(),
            G510Key::LcdMenu(n) => format!("L{n}"),
            G510Key::Media(s) | G510Key::Other(s) => s.clone(),
        }
    }
}

pub fn key_from_code(code: u16) -> Option<G510Key> {
    // Linux input-event-codes:
    // KEY_MACRO1 = 0x290 (656) .. KEY_MACRO18 = 0x2a1 (673)
    if (0x290..=0x2a1).contains(&code) {
        return Some(G510Key::G((code - 0x290 + 1) as u8));
    }
    // KEY_MACRO_PRESET1 = 0x2a9 (681) .. KEY_MACRO_PRESET3 = 0x2ab (683)
    if (0x2a9..=0x2ab).contains(&code) {
        return Some(G510Key::Profile((code - 0x2a9 + 1) as u8));
    }
    // KEY_MACRO_RECORD_START = 0x2a8 (680)
    if code == 0x2a8 {
        return Some(G510Key::Mr);
    }
    // KEY_KBD_LCD_MENU1 = 0x2b8 (696) .. KEY_KBD_LCD_MENU5 = 0x2bc (700)
    if (0x2b8..=0x2bc).contains(&code) {
        return Some(G510Key::LcdMenu((code - 0x2b8 + 1) as u8));
    }

    match code {
        113 => Some(G510Key::Media("HEADPHONE_MUTE".into())),
        114 => Some(G510Key::Media("VOLUME_DOWN".into())),
        115 => Some(G510Key::Media("VOLUME_UP".into())),
        164 => Some(G510Key::Media("PLAY_PAUSE".into())),
        166 => Some(G510Key::Media("STOP".into())),
        165 => Some(G510Key::Media("PREVIOUS".into())),
        163 => Some(G510Key::Media("NEXT".into())),
        190 => Some(G510Key::Media("MICROPHONE_MUTE".into())), // KEY_F20
        _ => None,
    }
}

pub fn is_g510_gaming_device(device: &Device) -> bool {
    let input_id = device.input_id();
    if input_id.vendor() != USB_VENDOR || !USB_PRODUCTS.contains(&input_id.product()) {
        return false;
    }
    if let Some(keys) = device.supported_keys() {
        // KEY_MACRO1 = 0x290 (656), KEY_MACRO18 = 0x2a1 (673)
        keys.contains(KeyCode::new(0x290)) && keys.contains(KeyCode::new(0x2a1))
    } else {
        false
    }
}

pub fn find_gaming_device() -> io::Result<(PathBuf, Device)> {
    for (path, device) in evdev::enumerate() {
        if is_g510_gaming_device(&device) {
            return Ok((path, device));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Logitech G510 gaming key interface not found",
    ))
}
