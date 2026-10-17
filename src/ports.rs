//! Finding serial ports and recognising the board among them.
//!
//! The port name changes from machine to machine (`/dev/ttyUSB0`, `COM3`, ...),
//! so the application lists the ports with their USB identity and lets the
//! user pick one. A port is only a *candidate*: the CH340 identity is shared
//! by many unrelated devices, so nothing is ever written to a port the user
//! has not selected or confirmed.

use std::path::Path;

/// USB vendor id of the WCH CH340 family.
pub const CH340_VID: u16 = 0x1A86;
/// USB product id of the CH340 serial converter used on the LTDZ board.
pub const CH340_PID: u16 = 0x7523;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortInfo {
    /// Name to open, for example `/dev/ttyUSB0` or `COM3`.
    pub path: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    /// Product string reported by the USB device, when available.
    pub product: Option<String>,
}

impl PortInfo {
    /// True when the USB identity matches the converter used on the board.
    pub fn is_board_candidate(&self) -> bool {
        self.vid == Some(CH340_VID) && self.pid == Some(CH340_PID)
    }
}

/// The port to pre-select: the only candidate, if there is exactly one.
pub fn suggest(ports: &[PortInfo]) -> Option<&PortInfo> {
    let mut candidates = ports.iter().filter(|p| p.is_board_candidate());
    match (candidates.next(), candidates.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

/// Lists the serial ports of this machine, sorted by name.
pub fn list_ports() -> Vec<PortInfo> {
    let mut ports = list_platform_ports();
    ports.sort_by(|a, b| a.path.cmp(&b.path));
    ports
}

#[cfg(target_os = "linux")]
fn list_platform_ports() -> Vec<PortInfo> {
    list_linux_ports(Path::new("/sys"), Path::new("/dev"))
}

#[cfg(not(target_os = "linux"))]
fn list_platform_ports() -> Vec<PortInfo> {
    // Without USB identity: the port still can be chosen by hand.
    serial2::SerialPort::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p: std::path::PathBuf| PortInfo {
            path: p.to_string_lossy().into_owned(),
            vid: None,
            pid: None,
            product: None,
        })
        .collect()
}

/// Reads USB serial adapters from sysfs. `sys_root` and `dev_root` are
/// parameters so tests can use a fake tree.
pub fn list_linux_ports(sys_root: &Path, dev_root: &Path) -> Vec<PortInfo> {
    let Ok(entries) = std::fs::read_dir(sys_root.join("class/tty")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("ttyUSB") || name.starts_with("ttyACM")) {
            continue;
        }
        let (vid, pid, product) = usb_identity(&entry.path().join("device"));
        out.push(PortInfo {
            path: dev_root.join(&name).to_string_lossy().into_owned(),
            vid,
            pid,
            product,
        });
    }
    out
}

/// Walks up from a tty's `device` link until the USB device directory
/// (the one holding `idVendor`) is found.
fn usb_identity(device_link: &Path) -> (Option<u16>, Option<u16>, Option<String>) {
    let Ok(mut dir) = std::fs::canonicalize(device_link) else {
        return (None, None, None);
    };
    for _ in 0..5 {
        if dir.join("idVendor").exists() {
            let hex = |file: &str| {
                std::fs::read_to_string(dir.join(file))
                    .ok()
                    .and_then(|s| u16::from_str_radix(s.trim(), 16).ok())
            };
            let product = std::fs::read_to_string(dir.join("product"))
                .ok()
                .map(|s| s.trim().to_string());
            return (hex("idVendor"), hex("idProduct"), product);
        }
        if !dir.pop() {
            break;
        }
    }
    (None, None, None)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    /// Builds a fake sysfs with one USB device per entry: (tty name, vid, pid, product).
    fn fake_sys(devices: &[(&str, &str, &str, &str)]) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "pakespec-ports-{}-{}",
            std::process::id(),
            devices.len()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("class/tty")).unwrap();
        for (i, (tty, vid, pid, product)) in devices.iter().enumerate() {
            let usb = root.join(format!("devices/usb1/1-{i}"));
            let iface = usb.join(format!("1-{i}:1.0"));
            fs::create_dir_all(iface.join(tty)).unwrap();
            fs::write(usb.join("idVendor"), format!("{vid}\n")).unwrap();
            fs::write(usb.join("idProduct"), format!("{pid}\n")).unwrap();
            fs::write(usb.join("product"), format!("{product}\n")).unwrap();
            let tty_dir = root.join("class/tty").join(tty);
            fs::create_dir_all(&tty_dir).unwrap();
            symlink(&iface, tty_dir.join("device")).unwrap();
        }
        // A tty that is not a USB adapter must be ignored.
        fs::create_dir_all(root.join("class/tty/ttyS0")).unwrap();
        root
    }

    #[test]
    fn finds_the_board_among_other_usb_serial_devices() {
        let sys = fake_sys(&[
            ("ttyUSB0", "1a86", "7523", "USB Serial"),
            ("ttyACM0", "1a86", "55d3", "USB Single Serial"),
        ]);
        let ports = list_linux_ports(&sys, Path::new("/dev"));
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].vid, Some(0x1A86));
        let chosen = suggest(&ports).expect("one candidate");
        assert_eq!(chosen.pid, Some(CH340_PID));
        assert!(chosen.path.ends_with("ttyUSB0") || chosen.path.ends_with("ttyACM0"));
        assert!(chosen.is_board_candidate());
    }

    #[test]
    fn no_suggestion_when_ambiguous_or_absent() {
        let sys = fake_sys(&[
            ("ttyUSB0", "1a86", "7523", "A"),
            ("ttyUSB1", "1a86", "7523", "B"),
            ("ttyACM0", "1a86", "55d3", "C"),
        ]);
        assert!(suggest(&list_linux_ports(&sys, Path::new("/dev"))).is_none());
        let sys = fake_sys(&[("ttyACM0", "1a86", "55d3", "C")]);
        assert!(suggest(&list_linux_ports(&sys, Path::new("/dev"))).is_none());
    }

    #[test]
    fn missing_sysfs_gives_empty_list() {
        assert!(list_linux_ports(Path::new("/nonexistent"), Path::new("/dev")).is_empty());
    }
}
