//! The mice palmrest knows by id, and how far each is verified.
//!
//! `HID_ID` is parsed as `bus:vendor:product` and compared field by field; a substring search
//! for "004c" and "0269" would also match a device whose name or
//! another id merely contains those digits.

/// The HID bus a device is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bus {
    Usb,
    Bluetooth,
}

impl Bus {
    /// The bus number in `HID_ID` (linux/input.h `BUS_*`).
    pub fn number(self) -> u32 {
        match self {
            Bus::Usb => 0x03,
            Bus::Bluetooth => 0x05,
        }
    }
}

/// Which mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    MagicMouse1,
    MagicMouse2,
    /// The USB-C Magic Mouse.
    MagicMouseUsbC,
}

impl Model {
    /// Whether `decode` reads the model's touch report: report 0x12 (and 0xF7 doubles). The
    /// Magic Mouse 1 sends report 0x29, with a different header and touch blocks.
    pub fn decodable(self) -> bool {
        self != Model::MagicMouse1
    }
}

/// One known id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownDevice {
    pub bus: Bus,
    pub vendor: u32,
    pub product: u32,
    pub model: Model,
    /// Whether palmrest has ever run against this device.
    pub verified: bool,
}

/// Apple's vendor id on Bluetooth (the Magic Mouse 2 and later).
pub const BT_VENDOR: u32 = 0x004c;
/// Apple's USB vendor id (also what the Magic Mouse 1 reports over Bluetooth).
pub const USB_VENDOR: u32 = 0x05ac;

/// Every id palmrest matches, in the order discovery prefers them.
pub const KNOWN: &[KnownDevice] = &[
    KnownDevice {
        bus: Bus::Bluetooth,
        vendor: BT_VENDOR,
        product: 0x0269,
        model: Model::MagicMouse2,
        verified: true,
    },
    // TODO(untested): the Magic Mouse 2 on its cable. The kernel driver matches this id; that
    // it sends the same 0x12 reports and accepts the same F1 02 01 feature report over USB is
    // assumed, never read.
    KnownDevice {
        bus: Bus::Usb,
        vendor: USB_VENDOR,
        product: 0x0269,
        model: Model::MagicMouse2,
        verified: false,
    },
    // TODO(untested): the USB-C Magic Mouse. Product id 0x0323 is the one listed for it, not
    // read from a device, and its report layout and surface are assumed to be the Magic Mouse
    // 2's.
    KnownDevice {
        bus: Bus::Bluetooth,
        vendor: BT_VENDOR,
        product: 0x0323,
        model: Model::MagicMouseUsbC,
        verified: false,
    },
    KnownDevice {
        bus: Bus::Usb,
        vendor: USB_VENDOR,
        product: 0x0323,
        model: Model::MagicMouseUsbC,
        verified: false,
    },
    // TODO(untested): the Magic Mouse 1 sends report 0x29, whose header and 6-byte touch
    // blocks differ from report 0x12, and `decode` has no reader for it. It is listed so the
    // daemon can say so; the kernel driver decodes it through evdev, which the daemon does not
    // read; neither path has run against one.
    KnownDevice {
        bus: Bus::Bluetooth,
        vendor: USB_VENDOR,
        product: 0x030d,
        model: Model::MagicMouse1,
        verified: false,
    },
];

/// The known device with these ids, if any.
pub fn known(bus: u32, vendor: u32, product: u32) -> Option<&'static KnownDevice> {
    KNOWN
        .iter()
        .find(|k| k.bus.number() == bus && k.vendor == vendor && k.product == product)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_magic_mouse_1_is_not_decoded() {
        let undecodable: Vec<Model> = KNOWN
            .iter()
            .map(|k| k.model)
            .filter(|m| !m.decodable())
            .collect();
        assert_eq!(undecodable, [Model::MagicMouse1]);
    }

    #[test]
    fn known_table() {
        const CASES: &[(u32, u32, u32, Option<Model>)] = &[
            (5, 0x4c, 0x269, Some(Model::MagicMouse2)),
            (3, 0x5ac, 0x269, Some(Model::MagicMouse2)),
            (5, 0x5ac, 0x30d, Some(Model::MagicMouse1)),
            (5, 0x4c, 0x323, Some(Model::MagicMouseUsbC)),
            (3, 0x4c, 0x269, None),
            (5, 0x4c, 0x265, None),
            (5, 0x104c, 0x269, None),
        ];
        for &(bus, vendor, product, want) in CASES {
            assert_eq!(
                known(bus, vendor, product).map(|k| k.model),
                want,
                "{bus:#x}:{vendor:#x}:{product:#x}"
            );
        }
        assert_eq!(
            KNOWN.iter().filter(|k| k.verified).count(),
            1,
            "only the Bluetooth Magic Mouse 2 has been run"
        );
    }
}
