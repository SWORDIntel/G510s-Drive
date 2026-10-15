use std::process::Command;

pub fn get_microphone_mute() -> Option<bool> {
    if let Ok(output) = Command::new("pactl")
        .args(["get-source-mute", "@DEFAULT_SOURCE@"])
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            return Some(s.contains("yes") || s.contains("true") || s.contains("1"));
        }
    }
    None
}

pub fn toggle_microphone_mute() -> Option<bool> {
    if let Ok(output) = Command::new("pactl")
        .args(["set-source-mute", "@DEFAULT_SOURCE@", "toggle"])
        .output()
    {
        if output.status.success() {
            return get_microphone_mute();
        }
    }
    None
}

pub fn get_audio_mute() -> Option<bool> {
    if let Ok(output) = Command::new("pactl")
        .args(["get-sink-mute", "@DEFAULT_SINK@"])
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            return Some(s.contains("yes") || s.contains("true") || s.contains("1"));
        }
    }
    None
}

pub fn toggle_audio_mute() -> Option<bool> {
    if let Ok(output) = Command::new("pactl")
        .args(["set-sink-mute", "@DEFAULT_SINK@", "toggle"])
        .output()
    {
        if output.status.success() {
            return get_audio_mute();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_functions_do_not_panic() {
        let _ = get_microphone_mute();
        let _ = get_audio_mute();
    }
}
