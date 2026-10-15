use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Copy, Debug, Default)]
pub struct CpuCounters {
    pub total: u64,
    pub idle: u64,
}

impl CpuCounters {
    pub fn read() -> io::Result<Self> {
        Self::read_from_path(Path::new("/proc/stat"))
    }

    pub fn read_from_path(path: &Path) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.first() == Some(&"cpu") && parts.len() >= 5 {
                let values: Vec<u64> = parts[1..]
                    .iter()
                    .filter_map(|s| s.parse::<u64>().ok())
                    .collect();
                let total: u64 = values.iter().sum();
                let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
                return Ok(CpuCounters { total, idle });
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to parse /proc/stat",
        ))
    }

    pub fn percentage_since(&self, previous: &CpuCounters) -> f32 {
        if self.total <= previous.total {
            return 0.0;
        }
        let total_delta = (self.total - previous.total) as f32;
        let idle_delta = (self.idle - previous.idle) as f32;
        let busy = (total_delta - idle_delta).max(0.0);
        ((busy / total_delta) * 100.0).clamp(0.0, 100.0)
    }
}

pub fn memory_percentage() -> io::Result<f32> {
    memory_percentage_from_path(Path::new("/proc/meminfo"))
}

pub fn memory_percentage_from_path(path: &Path) -> io::Result<f32> {
    let content = fs::read_to_string(path)?;
    let mut total: Option<u64> = None;
    let mut available: Option<u64> = None;

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            match parts[0] {
                "MemTotal:" => total = parts[1].parse().ok(),
                "MemAvailable:" => available = parts[1].parse().ok(),
                _ => {}
            }
        }
    }

    let tot = total.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "MemTotal not found in /proc/meminfo")
    })?;
    let avail = available.unwrap_or(0);

    if tot == 0 {
        return Ok(0.0);
    }

    let used = tot.saturating_sub(avail) as f32;
    Ok(((used / tot as f32) * 100.0).clamp(0.0, 100.0))
}

pub fn cpu_temp_celsius() -> Option<u32> {
    for zone in 0..8 {
        let path = format!("/sys/class/thermal/thermal_zone{zone}/temp");
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(val) = content.trim().parse::<u32>() {
                return Some(if val > 1000 { val / 1000 } else { val });
            }
        }
    }
    None
}

pub fn load_average() -> (f32, f32, f32) {
    if let Ok(content) = fs::read_to_string("/proc/loadavg") {
        let parts: Vec<f32> = content
            .split_whitespace()
            .take(3)
            .filter_map(|s| s.parse::<f32>().ok())
            .collect();
        if parts.len() == 3 {
            return (parts[0], parts[1], parts[2]);
        }
    }
    (0.0, 0.0, 0.0)
}

#[derive(Clone, Debug, Default)]
pub struct NetThroughput {
    pub iface: String,
    pub ip: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_rate_kbs: f32,
    pub tx_rate_kbs: f32,
    pub vpn_active: bool,
}

pub fn read_default_route() -> (String, String) {
    if let Ok(output) = Command::new("ip")
        .args(["-4", "route", "get", "1.1.1.1"])
        .output()
    {
        if output.status.success() {
            let line = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = line.split_whitespace().collect();
            let mut iface = "lo".to_string();
            let mut ip = "127.0.0.1".to_string();

            for i in 0..parts.len() {
                if parts[i] == "dev" && i + 1 < parts.len() {
                    iface = parts[i + 1].to_string();
                } else if parts[i] == "src" && i + 1 < parts.len() {
                    ip = parts[i + 1].to_string();
                }
            }
            return (iface, ip);
        }
    }
    ("vmbr0".into(), "192.168.1.91".into())
}

pub fn check_vpn() -> bool {
    if let Ok(entries) = fs::read_dir("/sys/class/net") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("wg")
                || name.starts_with("tailscale")
                || name.starts_with("tun")
                || name.starts_with("hc-")
            {
                let oper = fs::read_to_string(entry.path().join("operstate")).unwrap_or_default();
                if oper.trim() == "up" || oper.trim() == "unknown" {
                    return true;
                }
            }
        }
    }
    false
}

pub fn read_iface_bytes(target_iface: &str) -> (u64, u64) {
    if let Ok(content) = fs::read_to_string("/proc/net/dev") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            let name = parts[0].trim_end_matches(':');
            if name == target_iface && parts.len() >= 10 {
                let rx: u64 = parts[1].parse().unwrap_or(0);
                let tx: u64 = parts[9].parse().unwrap_or(0);
                return (rx, tx);
            }
        }
    }
    (0, 0)
}

#[derive(Clone, Debug, Default)]
pub struct SecurityStats {
    pub failed_ssh_hour: u32,
    pub vms: u32,
    pub containers: u32,
}

pub fn read_security_stats() -> SecurityStats {
    let mut stats = SecurityStats::default();

    // Docker containers
    if let Ok(output) = Command::new("docker").args(["ps", "-q"]).output() {
        if output.status.success() {
            stats.containers = String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|s| !s.trim().is_empty())
                .count() as u32;
        }
    }

    // QEMU VMs
    if let Ok(output) = Command::new("pgrep").args(["-c", "qemu"]).output() {
        if output.status.success() {
            stats.vms = String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(0);
        }
    }

    // Failed SSH attempts
    if let Ok(output) = Command::new("journalctl")
        .args(["-u", "ssh", "--since", "1 hour ago", "-q"])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            stats.failed_ssh_hour = text
                .lines()
                .filter(|line| line.contains("Failed") || line.contains("Invalid user"))
                .count() as u32;
        }
    }

    stats
}

#[derive(Clone, Debug, Default)]
pub struct MediaInfo {
    pub status: String,
    pub artist: String,
    pub title: String,
}

pub fn read_mpris_media() -> MediaInfo {
    if let Ok(output) = Command::new("playerctl")
        .args(["metadata", "--format", "{{status}}|{{artist}}|{{title}}"])
        .output()
    {
        if output.status.success() {
            let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 3 {
                return MediaInfo {
                    status: parts[0].to_string(),
                    artist: parts[1].to_string(),
                    title: parts[2].to_string(),
                };
            }
        }
    }
    MediaInfo {
        status: "Stopped".into(),
        artist: "No Artist".into(),
        title: "No Media Playing".into(),
    }
}

#[derive(Clone, Debug)]
pub struct AgentStatus {
    pub state: String,
    pub task: String,
    pub role: String,
}

pub fn read_agent_status() -> AgentStatus {
    // Check state file /run/user/<uid>/g510-agent.json or /tmp/g510-agent.json
    let paths = [
        format!("/run/user/{}/g510-agent.json", unsafe { libc::getuid() }),
        "/tmp/g510-agent.json".to_string(),
    ];

    for p in &paths {
        if let Ok(content) = fs::read_to_string(p) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                return AgentStatus {
                    state: val["state"].as_str().unwrap_or("ACTIVE").to_string(),
                    task: val["task"].as_str().unwrap_or("Autonomous Agent").to_string(),
                    role: val["role"].as_str().unwrap_or("Workbench Pair").to_string(),
                };
            }
        }
    }

    AgentStatus {
        state: "IDLE".into(),
        task: "Workbench Ready".into(),
        role: "Pair Programmer".into(),
    }
}

pub fn current_time_str() -> String {
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_cpu_delta() {
        let prev = CpuCounters {
            total: 100,
            idle: 20,
        };
        let cur = CpuCounters {
            total: 200,
            idle: 70,
        };
        assert_eq!(cur.percentage_since(&prev), 50.0);
    }

    #[test]
    fn test_memory_percentage() {
        let temp_dir = tempfile::tempdir().unwrap();
        let meminfo_path = temp_dir.path().join("meminfo");

        let mut file = fs::File::create(&meminfo_path).unwrap();
        writeln!(file, "MemTotal:       1000 kB").unwrap();
        writeln!(file, "MemFree:         200 kB").unwrap();
        writeln!(file, "MemAvailable:    400 kB").unwrap();

        let pct = memory_percentage_from_path(&meminfo_path).unwrap();
        assert!((pct - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_time_string_format() {
        let time_str = current_time_str();
        assert_eq!(time_str.len(), 5);
        assert_eq!(&time_str[2..3], ":");
    }
}
