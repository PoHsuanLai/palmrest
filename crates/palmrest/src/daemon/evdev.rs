//! Finding and opening a multitouch mouse's evdev node (`device.source = "evdev"`).
//!
// TODO(untested): run only against the Magic Mouse's node. The capability test below is what
// the kernel's multitouch protocol B promises; that any other mouse passes it, or that a
// device this reads should have been left alone, is unverified.
//!
//! A node qualifies when it is a mouse (`REL_X`, `REL_Y`) with multitouch slots
//! (`ABS_MT_SLOT`, `ABS_MT_TRACKING_ID`, `ABS_MT_POSITION_X/Y`). Touchpads never do: a node
//! udev tags `ID_INPUT_TOUCHPAD=1`, or that has `BTN_TOOL_FINGER` (what that tag is derived
//! from), is skipped, so touchpads stay with libinput and shell-host's own scroll physics. The
//! node is opened read-only and never grabbed.

use super::sys::{self, AbsInfo, MtAxis};
use crate::sensor::evdev::{AbsAxis, EvdevSpec};
use palmrest_protocol::DeviceInfo;
use rustix::fd::OwnedFd;
use rustix::fs::{Mode, OFlags};
use std::io;
use std::path::{Path, PathBuf};

/// Where input class devices are listed.
pub const SYS_INPUT: &str = "/sys/class/input";
/// udev's database of device properties.
pub const UDEV_DATA: &str = "/run/udev/data";

const REL_X: u32 = 0x00;
const REL_Y: u32 = 0x01;
const ABS_MT_SLOT: u32 = 0x2f;
const ABS_MT_POSITION_X: u32 = 0x35;
const ABS_MT_POSITION_Y: u32 = 0x36;
const ABS_MT_TRACKING_ID: u32 = 0x39;
const BTN_TOOL_FINGER: u32 = 0x145;
/// Bits per word of a sysfs capability mask (`BITS_PER_LONG`).
// TODO(untested): 64-bit kernels only; a 32-bit kernel prints 32-bit words.
const WORD_BITS: u32 = 64;

/// A sysfs capability mask (`device/capabilities/{abs,rel,key}`): hex words, most significant
/// first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Bits(Vec<u64>);

impl Bits {
    /// Parses the mask's text; an unreadable word counts as empty.
    pub fn parse(text: &str) -> Self {
        Self(
            text.split_whitespace()
                .rev()
                .map(|w| u64::from_str_radix(w, 16).unwrap_or(0))
                .collect(),
        )
    }

    /// Whether bit `n` is set.
    pub fn has(&self, n: u32) -> bool {
        let word = usize::try_from(n / WORD_BITS).unwrap_or(usize::MAX);
        self.0
            .get(word)
            .is_some_and(|w| w >> (n % WORD_BITS) & 1 == 1)
    }
}

/// A node's capabilities and what udev says about it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Caps {
    pub abs: Bits,
    pub rel: Bits,
    pub key: Bits,
    /// The node's udev database entry (`E:KEY=VALUE` lines), empty if unreadable.
    pub udev: String,
}

/// Whether palmrest may read a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eligibility {
    MultitouchMouse,
    /// Left to libinput.
    Touchpad,
    NotMultitouch,
    NotAMouse,
}

/// The rule, on capabilities alone.
pub fn classify(c: &Caps) -> Eligibility {
    let touchpad =
        c.udev.lines().any(|l| l.trim() == "E:ID_INPUT_TOUCHPAD=1") || c.key.has(BTN_TOOL_FINGER);
    let multitouch = [
        ABS_MT_SLOT,
        ABS_MT_TRACKING_ID,
        ABS_MT_POSITION_X,
        ABS_MT_POSITION_Y,
    ]
    .iter()
    .all(|&b| c.abs.has(b));
    let mouse = c.rel.has(REL_X) && c.rel.has(REL_Y);
    match (touchpad, multitouch, mouse) {
        (true, _, _) => Eligibility::Touchpad,
        (false, false, _) => Eligibility::NotMultitouch,
        (false, true, false) => Eligibility::NotAMouse,
        (false, true, true) => Eligibility::MultitouchMouse,
    }
}

/// The first multitouch mouse node under `sys` (normally `/sys/class/input`), as
/// `/dev/input/eventN`, with udev's data read from `udev`.
pub fn find(sys: &Path, udev: &Path) -> Option<(PathBuf, DeviceInfo)> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(sys)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("event"))
        })
        .collect();
    entries.sort();
    entries.into_iter().find_map(|dir| {
        let read = |rel: &str| std::fs::read_to_string(dir.join(rel)).unwrap_or_default();
        let udev_entry = udev.join(format!("c{}", read("dev").trim()));
        let caps = Caps {
            abs: Bits::parse(&read("device/capabilities/abs")),
            rel: Bits::parse(&read("device/capabilities/rel")),
            key: Bits::parse(&read("device/capabilities/key")),
            udev: std::fs::read_to_string(udev_entry).unwrap_or_default(),
        };
        (classify(&caps) == Eligibility::MultitouchMouse).then(|| {
            let hex = |rel: &str| u16::from_str_radix(read(rel).trim(), 16).unwrap_or(0);
            let node = Path::new("/dev/input").join(dir.file_name().unwrap_or_default());
            let info = DeviceInfo {
                name: read("device/name").trim().to_owned(),
                path: node.display().to_string(),
                vendor: hex("device/id/vendor"),
                product: hex("device/id/product"),
            };
            (node, info)
        })
    })
}

/// Opens a node read-only, switches its timestamps to `CLOCK_MONOTONIC` and reads its
/// geometry. Never `EVIOCGRAB`: the kernel and libinput keep every event.
pub fn open(path: &Path) -> io::Result<(OwnedFd, EvdevSpec)> {
    let flags = OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::RDONLY;
    let fd = rustix::fs::open(path, flags, Mode::empty())?;
    sys::monotonic_events(&fd)?;
    let axis = |a: AbsInfo| AbsAxis {
        min: a.minimum,
        max: a.maximum,
        resolution: a.resolution,
    };
    let x = axis(sys::abs_info(&fd, MtAxis::PositionX)?);
    let y = axis(sys::abs_info(&fd, MtAxis::PositionY)?);
    let slot_max = sys::abs_info(&fd, MtAxis::Slot)?.maximum;
    let spec = EvdevSpec::new(x, y, slot_max).map_err(io::Error::other)?;
    Ok((fd, spec))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Magic Mouse's node as sysfs and udev describe it on the development machine.
    fn magic_mouse() -> Caps {
        Caps {
            abs: Bits::parse("273800000000000\n"),
            rel: Bits::parse("1943\n"),
            key: Bits::parse("70001 0 0 0 0\n"),
            udev: "E:ID_INPUT=1\nE:ID_INPUT_MOUSE=1\n".to_owned(),
        }
    }

    #[test]
    fn capability_masks() {
        let c = magic_mouse();
        assert!(c.abs.has(ABS_MT_SLOT) && c.abs.has(ABS_MT_TRACKING_ID));
        assert!(!c.abs.has(0x00), "no ABS_X on the mouse");
        assert!(c.key.has(0x110) && c.key.has(0x100), "BTN_LEFT and BTN_0");
        assert!(!c.key.has(BTN_TOOL_FINGER));
        assert!(!Bits::parse("").has(0));
    }

    #[test]
    fn classify_table() {
        let mm = magic_mouse();
        let cases: Vec<(&str, Caps, Eligibility)> = vec![
            ("the Magic Mouse", mm.clone(), Eligibility::MultitouchMouse),
            (
                "udev says touchpad",
                Caps {
                    udev: "E:ID_INPUT=1\nE:ID_INPUT_TOUCHPAD=1\n".to_owned(),
                    ..mm.clone()
                },
                Eligibility::Touchpad,
            ),
            (
                "BTN_TOOL_FINGER without udev data",
                Caps {
                    key: Bits::parse("e520 10000 0 0 0 0"),
                    udev: String::new(),
                    ..mm.clone()
                },
                Eligibility::Touchpad,
            ),
            (
                "a uinput wheel device",
                Caps {
                    abs: Bits::parse("0"),
                    rel: Bits::parse("1940"),
                    key: Bits::parse("0"),
                    udev: String::new(),
                },
                Eligibility::NotMultitouch,
            ),
            (
                "a touchscreen",
                Caps {
                    rel: Bits::parse("0"),
                    key: Bits::parse("400 0 0 0 0 0"),
                    udev: "E:ID_INPUT_TOUCHSCREEN=1\n".to_owned(),
                    ..mm
                },
                Eligibility::NotAMouse,
            ),
        ];
        for (name, caps, want) in cases {
            assert_eq!(classify(&caps), want, "{name}");
        }
    }

    #[test]
    fn find_reads_a_sysfs_tree() {
        let root = std::env::temp_dir().join(format!("palmrest-input-{}", std::process::id()));
        let (sys, udev) = (root.join("sys"), root.join("udev"));
        let node = |n: &str, dev: &str, abs: &str, rel: &str, key: &str| {
            let d = sys.join(n);
            std::fs::create_dir_all(d.join("device/capabilities")).unwrap();
            std::fs::create_dir_all(d.join("device/id")).unwrap();
            std::fs::write(d.join("dev"), format!("{dev}\n")).unwrap();
            std::fs::write(d.join("device/name"), format!("{n} name\n")).unwrap();
            std::fs::write(d.join("device/id/vendor"), "004c\n").unwrap();
            std::fs::write(d.join("device/id/product"), "0269\n").unwrap();
            for (f, v) in [("abs", abs), ("rel", rel), ("key", key)] {
                std::fs::write(d.join("device/capabilities").join(f), v).unwrap();
            }
        };
        node(
            "event3",
            "13:67",
            "273800000000000",
            "1943",
            "70001 0 0 0 0",
        );
        node(
            "event5",
            "13:69",
            "273800000000000",
            "1943",
            "70001 0 0 0 0",
        );
        std::fs::create_dir_all(&udev).unwrap();
        std::fs::write(udev.join("c13:67"), "E:ID_INPUT_TOUCHPAD=1\n").unwrap();
        let found = find(&sys, &udev);
        std::fs::remove_dir_all(&root).unwrap();
        let (path, info) = found.unwrap();
        assert_eq!(
            path,
            PathBuf::from("/dev/input/event5"),
            "event3 is a touchpad"
        );
        assert_eq!((info.name.as_str(), info.product), ("event5 name", 0x269));
    }
}
