use std::fs;
use std::io;
use std::path::Path;

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

pub fn current_time_str() -> String {
    // Read local time or /proc/uptime / format HH:MM
    // To avoid heavy chrono crate, use libc::time or libc::localtime_r
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
        // delta: total=100, idle=50, busy=50 -> 50%
        assert_eq!(cur.percentage_since(&prev), 50.0);

        let same = CpuCounters {
            total: 100,
            idle: 20,
        };
        assert_eq!(same.percentage_since(&prev), 0.0);
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
        // total 1000, available 400 -> used 600 -> 60.0%
        assert!((pct - 60.0).abs() < 0.01);
    }
}
