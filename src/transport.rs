use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::framebuffer::{G510Framebuffer, PACKET_LEN};

pub const DEFAULT_LCD_DEVICE: &str = "/dev/g510-lcd";
pub const USB_VENDOR: u16 = 0x046d;
pub const USB_PRODUCTS: [u16; 2] = [0xc22d, 0xc22e];

/// Locates the G510 LCD hidraw device node.
/// First checks `/dev/g510-lcd` (installed via udev rule).
/// If not present, scans `/sys/class/hidraw` for Logitech G510 on Interface 1 (`:1.1`).
pub fn find_hidraw_device() -> Option<PathBuf> {
    let default_path = Path::new(DEFAULT_LCD_DEVICE);
    if default_path.exists() {
        return Some(default_path.to_path_buf());
    }

    let sys_hidraw = Path::new("/sys/class/hidraw");
    if let Ok(entries) = fs::read_dir(sys_hidraw) {
        for entry in entries.flatten() {
            let device_link = entry.path().join("device");
            if let Ok(target) = fs::canonicalize(&device_link) {
                let target_str = target.to_string_lossy().to_uppercase();
                let is_interface_1 = target_str.contains(":1.1") || target_str.contains("1.4.4:1.1");
                let is_g510 = target_str.contains("046D:C22D") || target_str.contains("046D:C22E");

                if is_interface_1 && is_g510 {
                    let dev_node = PathBuf::from("/dev").join(entry.file_name());
                    if dev_node.exists() {
                        return Some(dev_node);
                    }
                }
            }
        }
    }

    None
}

pub struct G510Transport {
    file: File,
    path: PathBuf,
}

impl G510Transport {
    pub fn open() -> io::Result<Self> {
        let path = find_hidraw_device().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Logitech G510 LCD hidraw device not found (/dev/g510-lcd)",
            )
        })?;
        Self::open_path(&path)
    }

    pub fn open_path(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)?;

        Ok(Self {
            file,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write_report(&mut self, report: &[u8; PACKET_LEN]) -> io::Result<usize> {
        self.file.write(report)
    }

    pub fn write_framebuffer(&mut self, fb: &G510Framebuffer, invert: bool) -> io::Result<usize> {
        let report = fb.pack_report(invert);
        self.write_report(&report)
    }
}
