//! A made-up laptop: a hybrid processor, an external display beside its
//! own panel, an NVMe drive with an encrypted partition, Wi-Fi and
//! ethernet, two fans on its embedded controller and a battery.
use super::*;

const GPU: At = (0, 2, 0);
const NVME: At = (1, 0, 0);
const USB: At = (0, 0x0d, 0);
const LPC: At = (0, 0x1f, 0);
const SMBUS: At = (0, 0x1f, 4);

const PCI: [Pci; 11] = [
    Pci::new((0, 0, 0), 0x060000, 0x0001, "Host Bridge"),
    // Named as pci.ids names many: a chip's code with its marketing name
    // in brackets, and a family of models sharing an id listed.
    Pci::new(
        GPU,
        0x030000,
        0x0002,
        "Generic Lake-P [Integrated Graphics]",
    ),
    Pci::new((0, 6, 0), 0x060400, 0x0006, "PCIe Root Port").driver("pcieport"),
    Pci::new(USB, 0x0c0330, 0x000d, "USB 3.2 Controller").driver("xhci_hcd"),
    Pci::new(
        (0, 0x14, 3),
        0x028000,
        0x0014,
        "Wi-Fi 6E(802.11ax) WX210/WX211* 2x2 [Generic Peak]",
    ),
    Pci::new((0, 0x1c, 0), 0x060400, 0x001c, "PCIe Root Port").driver("pcieport"),
    Pci::new(LPC, 0x060100, 0x001f, "LPC Bridge"),
    Pci::new((0, 0x1f, 3), 0x040300, 0x0020, "HD Audio Controller").driver("snd_hda_intel"),
    Pci::new(SMBUS, 0x0c0500, 0x0021, "SMBus Controller"),
    Pci::new(NVME, 0x010802, 0x0100, "NVMe SSD Controller")
        .behind((0, 6, 0))
        .driver("nvme"),
    Pci::new((2, 0, 0), 0x020000, 0x0200, "Gigabit Ethernet Controller").behind((0, 0x1c, 0)),
];

const USB_DEVICES: [Usb; 7] = [
    Usb::root("usb1", 480.0, "2.00", 12),
    Usb::new("1-3", 12.0, "2.00", 0x03, Some("USB Receiver")).removable(true),
    Usb::new("1-6", 480.0, "2.00", 0x0e, Some("Integrated Camera")).removable(false),
    Usb::new("1-10", 12.0, "2.00", 0xe0, None).removable(false),
    Usb::root("usb2", 10000.0, "3.20", 4),
    Usb::new("2-1", 5000.0, "3.20", 0x09, Some("USB 3.2 Hub"))
        .ports(4)
        .removable(true),
    Usb::new("2-1.2", 5000.0, "3.20", 0x08, Some("Card Reader")),
];

/// The drive, in 512-byte sectors: its size, and each partition's start
/// and size.
const DRIVE_SECTORS: u64 = 1_953_525_168;

fn spec() -> Spec {
    Spec {
        chassis: Chassis {
            kind: ChassisKind::Laptop,
            vendor: Some("Generic".into()),
            product: Some("Laptop 14".into()),
            board_vendor: Some("Generic".into()),
            board: Some("Mainboard".into()),
            bios: Some("1.0".into()),
        },
        smbios: 10,
        // Eight performance cores of two threads, eight efficient ones of
        // one in two clusters of four.
        cpu: Some(Processor {
            model: "Generic 16-Core Processor",
            packages: 1,
            kinds: vec![
                Kind {
                    kind: Some(CoreKind::Performance),
                    cores: 8,
                    threads: 2,
                    max_mhz: Some(5000),
                    base_mhz: Some(2200),
                    caches: vec![
                        (1, CacheKind::Data, 48, 1),
                        (1, CacheKind::Instruction, 32, 1),
                        (2, CacheKind::Unified, 2048, 1),
                    ],
                },
                Kind {
                    kind: Some(CoreKind::Efficient),
                    cores: 8,
                    threads: 1,
                    max_mhz: Some(3800),
                    base_mhz: Some(1600),
                    caches: vec![
                        (1, CacheKind::Data, 32, 1),
                        (1, CacheKind::Instruction, 64, 1),
                        (2, CacheKind::Unified, 4096, 4),
                    ],
                },
            ],
            l3_kib: Some(24 << 10),
        }),
        memory_kib: Some(32_509_360),
        pci: PCI.to_vec(),
        usb: vec![(USB, USB_DEVICES.to_vec())],
        screens: vec![
            Screen {
                name: "eDP-1",
                card: 0,
                gpu: GPU,
                on: Shown::Monitor(Monitor {
                    name: None,
                    pixels: (2560, 1600),
                    refresh: 120,
                    mm: (302, 189),
                    year: 2025,
                }),
            },
            Screen {
                name: "DP-1",
                card: 0,
                gpu: GPU,
                on: Shown::Monitor(Monitor {
                    name: Some("27 Monitor"),
                    pixels: (3840, 2160),
                    refresh: 60,
                    mm: (597, 336),
                    year: 2024,
                }),
            },
            Screen {
                name: "HDMI-A-1",
                card: 0,
                gpu: GPU,
                on: Shown::Open,
            },
        ],
        // A boot partition, and an encrypted one with btrfs on it.
        drives: vec![Disk {
            name: "nvme0n1".into(),
            model: Some("NVMe SSD 1TB"),
            sectors: DRIVE_SECTORS,
            on: Attached::Nvme(NVME, 0),
            rotational: false,
            partitions: vec![
                (2048, 2_097_152, Some("vfat"), false),
                (2_099_200, 1_951_425_934, Some("btrfs"), true),
            ],
        }],
        ports: vec![
            Port {
                name: "eth0",
                link: Link::Ethernet,
                at: (2, 0, 0),
                speed: Some(1000),
                up: true,
            },
            Port {
                name: "wlan0",
                link: Link::Wireless,
                at: (0, 0x14, 3),
                speed: None,
                up: true,
            },
        ],
        chips: vec![
            Chip::new("acpitz", On::Nothing, Site::Board).temp(1, 44_000, None),
            Chip::new(
                "cpu_thermal",
                On::Device("platform/cpu_thermal"),
                Site::Processor(0),
            )
            .temp(1, 52_000, Some("Package")),
            Chip::new("nvme", On::Drive(0), Site::Drive(0)).temp(1, 41_850, Some("Composite")),
            // DDR5 modules' monitors warn from 55 °C.
            module(0x50, Site::Module(0))
                .temp(1, 45_500, None)
                .max(55_000),
            module(0x51, Site::Module(1))
                .temp(1, 46_250, None)
                .max(55_000),
            Chip::new("ec", On::Ec(LPC), Site::Board)
                .fan(1, 2200, Some("CPU Fan"))
                .fan(2, 1870, Some("System Fan")),
            Chip::supply("BAT0", "BAT0")
                .other("in0", 16_200)
                .other("curr1", 500),
        ],
    }
}

/// A DDR5 module's monitor on the SMBus at `address`.
fn module(address: u16, site: Site) -> Chip {
    Chip::new(
        "spd5118",
        On::Module {
            smbus: SMBUS,
            bus: 0,
            address,
            generation: Some("DDR5"),
        },
        site,
    )
}

pub(super) fn machine() -> Machine {
    Machine {
        batteries: vec![Battery {
            chemistry: Some("Li-poly".into()),
            maker: None,
            design_wh: Some(60.0),
            full_wh: Some(54.6),
            cycles: Some(212),
        }],
        chargers: vec![Charger {
            kind: ChargerKind::Usb,
            watts: Some(65.0),
        }],
        ..spec().machine(sample)
    }
}

/// The laptop's values at `t`: a processor that warms and cools over about
/// forty seconds with the fans following it, a battery running down, and
/// bursts of I/O.
fn sample(t: f32) -> Snapshot {
    let wave = |period: f32, phase: f32| (TAU * t / period + phase).sin();
    // Up to one, and zero for most of each period.
    let burst = |period: f32, phase: f32| wave(period, phase).max(0.0).powi(4);
    let processor = 52.0 + 10.0 * wave(37.0, 0.0) + 3.0 * wave(4.7, 1.3);
    let fan = 2200.0 + 90.0 * (processor - 52.0);

    Snapshot {
        sensors: vec![
            Some(44.0 + 3.0 * wave(61.0, 0.4)),
            Some(processor),
            Some(41.0 + 2.5 * wave(83.0, 2.0)),
            Some(46.0 + 2.0 * wave(71.0, 0.0)),
            Some(46.5 + 2.0 * wave(71.0, 1.0)),
            Some(fan),
            Some(0.85 * fan),
        ],
        batteries: vec![Charge {
            fraction: Some((0.8 - 0.0002 * t).clamp(0.05, 1.0)),
            state: Some(ChargeState::Discharging),
            watts: Some(9.5 + 2.0 * wave(13.0, 0.0)),
        }],
        chargers: vec![Some(false)],
        drives: vec![Some(Transfer {
            read: 4.0e6 * burst(7.0, 0.0),
            written: 1.5e6 * burst(11.0, 2.0),
        })],
        interfaces: vec![
            Some(Traffic {
                received: 2.5e6 * (0.5 + 0.5 * wave(9.0, 0.0)),
                sent: 3.0e5 * (0.5 + 0.5 * wave(5.0, 1.0)),
            }),
            Some(Traffic {
                received: 4.0e5 * (0.5 + 0.5 * wave(6.0, 2.0)),
                sent: 6.0e4 * (0.5 + 0.5 * wave(4.0, 0.5)),
            }),
        ],
    }
}

/// Writes the laptop as a sysfs tree in `fake`: its spec's, and its
/// battery and charger beside a mouse's battery.
#[cfg(test)]
pub(super) fn tree(fake: &super::super::tests::Fake) {
    use super::super::tests::IDENTIFIERS;

    spec().tree(fake);

    let supply = "sys/class/power_supply";
    fake.file(&format!("{supply}/BAT0/type"), "Battery\n")
        .file(&format!("{supply}/BAT0/technology"), "Li-poly\n")
        .file(&format!("{supply}/BAT0/energy_full_design"), "60000000\n")
        .file(&format!("{supply}/BAT0/energy_full"), "54600000\n")
        .file(&format!("{supply}/BAT0/cycle_count"), "212\n")
        .file(&format!("{supply}/BAT0/capacity"), "80\n")
        .file(&format!("{supply}/BAT0/status"), "Discharging\n")
        .file(&format!("{supply}/BAT0/power_now"), "9500000\n")
        .file(&format!("{supply}/BAT0/serial_number"), IDENTIFIERS[7])
        .file(
            &format!("{supply}/ucsi-source-psy-USBC000:001/type"),
            "USB\n",
        )
        .file(
            &format!("{supply}/ucsi-source-psy-USBC000:001/online"),
            "0\n",
        )
        .file(
            &format!("{supply}/ucsi-source-psy-USBC000:001/voltage_max"),
            "20000000\n",
        )
        .file(
            &format!("{supply}/ucsi-source-psy-USBC000:001/current_max"),
            "3250000\n",
        )
        .file(&format!("{supply}/hidpp_battery_0/type"), "Battery\n")
        .file(&format!("{supply}/hidpp_battery_0/scope"), "Device\n")
        .file(&format!("{supply}/hidpp_battery_0/capacity"), "50\n");
}
