//! Finding the mouse's hidraw node, and the reconnect backoff (12.3.12).
//!
//! The uevent's `HID_ID` is parsed as `bus:vendor:product` and compared field by field; a
//! substring search for "004c" and "0269" (what the Python daemon did) would also match a
//! device whose name or another id merely contains those digits.

use crate::protocol::DeviceInfo;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Apple's USB vendor id.
pub const VENDOR: u32 = 0x004c;
/// Magic Mouse 2 over Bluetooth (the USB-C model's id is unverified).
pub const PRODUCT: u32 = 0x0269;
/// Where hidraw class devices are listed.
pub const SYS_HIDRAW: &str = "/sys/class/hidraw";

/// The identity fields of a HID uevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidId {
    pub bus: u32,
    pub vendor: u32,
    pub product: u32,
    pub name: String,
}

/// Parses a `device/uevent` file's text.
pub fn parse_uevent(text: &str) -> Option<HidId> {
    let value = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .map(str::trim)
    };
    let mut fields = value("HID_ID")?
        .split(':')
        .map(|f| u32::from_str_radix(f, 16).ok());
    let (bus, vendor, product) = (fields.next()??, fields.next()??, fields.next()??);
    Some(HidId {
        bus,
        vendor,
        product,
        name: value("HID_NAME").unwrap_or("").to_owned(),
    })
}

/// Whether this is a Magic Mouse 2.
pub fn is_magic_mouse(id: &HidId) -> bool {
    id.vendor == VENDOR && id.product == PRODUCT
}

/// The first Magic Mouse under `sys` (normally `/sys/class/hidraw`), as `/dev/hidrawN`.
pub fn find(sys: &Path) -> Option<(PathBuf, DeviceInfo)> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(sys)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    entries.into_iter().find_map(|dir| {
        let id = parse_uevent(&std::fs::read_to_string(dir.join("device/uevent")).ok()?)?;
        let node = Path::new("/dev").join(dir.file_name()?);
        is_magic_mouse(&id).then(|| {
            let info = DeviceInfo {
                name: id.name.clone(),
                path: node.display().to_string(),
                vendor: id.vendor as u16,
                product: id.product as u16,
            };
            (node, info)
        })
    })
}

/// Exponential backoff: 1 s, doubling, capped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    next: Duration,
    cap: Duration,
}

impl Backoff {
    /// Device reconnects: 1 s x2 up to 30 s.
    pub const DEVICE: Backoff = Backoff {
        next: Duration::from_secs(1),
        cap: Duration::from_secs(30),
    };
    /// uinput ACL wait at boot: 1 s x2 up to 5 s.
    pub const UINPUT: Backoff = Backoff {
        next: Duration::from_secs(1),
        cap: Duration::from_secs(5),
    };

    /// The delay to wait now, and the backoff after it.
    pub fn step(self) -> (Duration, Backoff) {
        (
            self.next,
            Backoff {
                next: (self.next * 2).min(self.cap),
                ..self
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uevent_table() {
        let mm = "DRIVER=magicmouse\nHID_ID=0005:0000004C:00000269\nHID_NAME=Someone's Magic Mouse\nHID_PHYS=aa\n";
        let id = parse_uevent(mm).unwrap();
        assert_eq!((id.bus, id.vendor, id.product), (5, 0x4c, 0x269));
        assert_eq!(id.name, "Someone's Magic Mouse");
        assert!(is_magic_mouse(&id));
        const OTHERS: &[&str] = &[
            "HID_ID=0003:00000DB0:00000076\nHID_NAME=Lights 004c 0269",
            "HID_ID=0003:0000004C:00000265",
            "HID_ID=0005:0000104C:00000269",
            "HID_NAME=no id",
            "HID_ID=zz:4c:269",
        ];
        for text in OTHERS {
            assert!(
                !parse_uevent(text).is_some_and(|i| is_magic_mouse(&i)),
                "{text:?}"
            );
        }
    }

    #[test]
    fn backoff_sequences() {
        let seq = |mut b: Backoff, n: usize| {
            (0..n)
                .map(|_| {
                    let (d, next) = b.step();
                    b = next;
                    d.as_secs()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(seq(Backoff::DEVICE, 7), [1, 2, 4, 8, 16, 30, 30]);
        assert_eq!(seq(Backoff::UINPUT, 5), [1, 2, 4, 5, 5]);
    }

    #[test]
    fn find_reads_a_sysfs_tree() {
        let root = std::env::temp_dir().join(format!("palmrest-sys-{}", std::process::id()));
        for (n, id) in [
            ("hidraw0", "0003:00000DB0:00000076"),
            ("hidraw3", "0005:0000004C:00000269"),
        ] {
            std::fs::create_dir_all(root.join(n).join("device")).unwrap();
            std::fs::write(
                root.join(n).join("device/uevent"),
                format!("HID_ID={id}\nHID_NAME=m\n"),
            )
            .unwrap();
        }
        let found = find(&root).map(|(p, i)| (p, i.product));
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(found, Some((PathBuf::from("/dev/hidraw3"), 0x269)));
    }
}
