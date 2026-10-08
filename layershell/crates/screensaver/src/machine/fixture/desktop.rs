//! A made-up desktop tower: a processor whose cores are all alike, a
//! graphics card behind a switch with a 4K display and a smaller one on it,
//! an NVMe drive, and a SATA SSD and hard disk behind the chipset; case
//! fans on the board's Super I/O monitor, some of its headers empty and
//! some of its inputs unwired; ethernet, and no battery.
use std::sync::LazyLock;

use super::*;

const GPU: At = (3, 0, 0);
const NVME: At = (4, 0, 0);
const ETHERNET: At = (7, 0, 0);
const USB: At = (9, 0, 0);
const SATA: At = (0x0a, 0, 0);
const SMBUS: At = (0, 0x14, 0);

const PCI: [Pci; 19] = [
    Pci::new((0, 0, 0), 0x060000, 0x0001, "Host Bridge"),
    Pci::new((0, 1, 1), 0x060400, 0x0011, "PCIe GPP Bridge").driver("pcieport"),
    Pci::new((0, 1, 2), 0x060400, 0x0011, "PCIe GPP Bridge").driver("pcieport"),
    Pci::new((0, 2, 1), 0x060400, 0x0011, "PCIe GPP Bridge").driver("pcieport"),
    Pci::new(SMBUS, 0x0c0500, 0x0014, "SMBus Controller").driver("piix4_smbus"),
    Pci::new((0, 0x14, 3), 0x060100, 0x0015, "LPC Bridge"),
    Pci::new((0, 0x18, 3), 0x060000, 0x0018, "Data Fabric").driver("k10temp"),
    Pci::new((1, 0, 0), 0x060400, 0x0101, "PCIe Switch Upstream Port")
        .behind((0, 1, 1))
        .driver("pcieport"),
    Pci::new((2, 0, 0), 0x060400, 0x0102, "PCIe Switch Downstream Port")
        .behind((1, 0, 0))
        .driver("pcieport"),
    Pci::new(GPU, 0x030000, 0x0300, "Graphics Card")
        .behind((2, 0, 0))
        .driver("amdgpu"),
    Pci::new((3, 0, 1), 0x040300, 0x0301, "HDMI Audio Controller")
        .behind((2, 0, 0))
        .driver("snd_hda_intel"),
    Pci::new(NVME, 0x010802, 0x0400, "NVMe SSD Controller")
        .behind((0, 1, 2))
        .driver("nvme"),
    Pci::new((5, 0, 0), 0x060400, 0x0500, "Chipset Upstream Port")
        .behind((0, 2, 1))
        .driver("pcieport"),
    Pci::new((6, 0, 0), 0x060400, 0x0600, "Chipset Downstream Port")
        .behind((5, 0, 0))
        .driver("pcieport"),
    Pci::new((6, 0x0c, 0), 0x060400, 0x0600, "Chipset Downstream Port")
        .behind((5, 0, 0))
        .driver("pcieport"),
    Pci::new((6, 0x0d, 0), 0x060400, 0x0600, "Chipset Downstream Port")
        .behind((5, 0, 0))
        .driver("pcieport"),
    Pci::new(ETHERNET, 0x020000, 0x0700, "2.5G Ethernet Controller")
        .behind((6, 0, 0))
        .driver("igc"),
    Pci::new(USB, 0x0c0330, 0x0900, "USB 3.2 Controller")
        .behind((6, 0x0c, 0))
        .driver("xhci_hcd"),
    Pci::new(SATA, 0x010601, 0x0a00, "SATA Controller")
        .behind((6, 0x0d, 0))
        .driver("ahci"),
];

const USB_DEVICES: [Usb; 7] = [
    Usb::root("usb1", 480.0, "2.00", 12),
    Usb::new("1-1", 12.0, "2.00", 0x03, Some("USB Keyboard")).removable(true),
    Usb::new("1-2", 12.0, "2.00", 0x03, Some("USB Mouse")).removable(true),
    Usb::new("1-4", 480.0, "2.00", 0x09, Some("USB 2.0 Hub"))
        .ports(4)
        .removable(true),
    Usb::new("1-4.1", 12.0, "2.00", 0x01, Some("USB Headset")),
    Usb::new("1-4.2", 480.0, "2.00", 0x0e, Some("HD Webcam")),
    Usb::root("usb2", 10000.0, "3.20", 4),
];

const NVME_SECTORS: u64 = 3_907_029_168;
const SSD_SECTORS: u64 = 1_953_525_168;
const HDD_SECTORS: u64 = 7_814_037_168;

pub(super) fn spec() -> Spec {
    Spec {
        chassis: Chassis {
            kind: ChassisKind::Desktop,
            vendor: Some("Generic".into()),
            product: Some("Tower".into()),
            board_vendor: Some("Generic".into()),
            board: Some("Desktop Board".into()),
            bios: Some("1.0".into()),
        },
        smbios: 3,
        cpu: Some(Processor {
            model: "Generic 8-Core Processor",
            packages: 1,
            kinds: vec![Kind {
                kind: None,
                cores: 8,
                threads: 2,
                max_mhz: Some(5400),
                base_mhz: None,
                caches: vec![
                    (1, CacheKind::Data, 32, 1),
                    (1, CacheKind::Instruction, 32, 1),
                    (2, CacheKind::Unified, 1024, 1),
                ],
            }],
            l3_kib: Some(32 << 10),
        }),
        memory_kib: Some(31_960_000),
        pci: PCI.to_vec(),
        usb: vec![(USB, USB_DEVICES.to_vec())],
        screens: vec![
            Screen {
                name: "DP-1",
                card: 1,
                gpu: GPU,
                on: Shown::Monitor(Monitor {
                    name: Some("UHD Monitor"),
                    pixels: (3840, 2160),
                    refresh: 60,
                    mm: (597, 336),
                    year: 2023,
                }),
            },
            Screen {
                name: "DP-2",
                card: 1,
                gpu: GPU,
                on: Shown::Open,
            },
            Screen {
                name: "DP-3",
                card: 1,
                gpu: GPU,
                on: Shown::Open,
            },
            Screen {
                name: "HDMI-A-1",
                card: 1,
                gpu: GPU,
                on: Shown::Monitor(Monitor {
                    name: Some("FHD Monitor"),
                    pixels: (1920, 1080),
                    refresh: 144,
                    mm: (531, 299),
                    year: 2021,
                }),
            },
        ],
        drives: vec![
            Disk {
                name: "nvme0n1".into(),
                model: Some("NVMe SSD 2TB"),
                sectors: NVME_SECTORS,
                on: Attached::Nvme(NVME, 0),
                rotational: false,
                partitions: efi_and(NVME_SECTORS, "ext4"),
            },
            Disk {
                name: "sda".into(),
                model: Some("SATA SSD 1TB"),
                sectors: SSD_SECTORS,
                on: Attached::Sata(SATA, 0),
                rotational: false,
                partitions: whole(SSD_SECTORS, Some("ext4")),
            },
            Disk {
                name: "sdb".into(),
                model: Some("SATA HDD 4TB"),
                sectors: HDD_SECTORS,
                on: Attached::Sata(SATA, 1),
                rotational: true,
                partitions: whole(HDD_SECTORS, Some("xfs")),
            },
        ],
        ports: vec![Port {
            name: "enp7s0",
            link: Link::Ethernet,
            at: ETHERNET,
            speed: Some(2500),
            up: true,
        }],
        chips: vec![
            Chip::new("k10temp", On::Pci((0, 0x18, 3)), Site::Processor(0))
                .temp(1, 48_750, Some("Tctl"))
                .temp(3, 46_500, Some("Tccd1")),
            module(0x51, 0).temp(1, 41_000, None).max(55_000),
            module(0x53, 1).temp(1, 42_250, None).max(55_000),
            Chip::new("nvme", On::Drive(0), Site::Drive(0))
                .temp(1, 44_850, Some("Composite"))
                .temp(2, 44_850, Some("Sensor 1"))
                .temp(3, 52_850, Some("Sensor 2")),
            // The graphics card's own fan, turning slowly.
            Chip::new("amdgpu", On::Pci(GPU), Site::Device(pci(GPU)))
                .temp(1, 46_000, Some("edge"))
                .temp(2, 51_000, Some("junction"))
                .temp(3, 58_000, Some("mem"))
                .fan(1, 980, None)
                .other("in0", 806)
                .other("power1", 21_000_000),
            // Seven fan headers, four with a fan turning: the processor's
            // cooler and three case fans. Of its temperature inputs, three
            // that the firmware never fills read 0 and two with nothing
            // wired to them read their register's limits.
            Chip::new("nct6799", On::Device("platform/nct6775.656"), Site::Board)
                .temp(1, 32_000, Some("SYSTIN"))
                .temp(2, 41_500, Some("CPUTIN"))
                .unwired(3, 127_000, Some("AUXTIN0"))
                .temp(4, 26_000, Some("AUXTIN1"))
                .unwired(5, -128_000, Some("AUXTIN2"))
                .temp(6, 25_000, Some("AUXTIN3"))
                .temp(7, 48_500, Some("PECI/TSI Agent 0 Calibration"))
                .unwired(8, 0, Some("PCH_CHIP_CPU_MAX_TEMP"))
                .unwired(9, 0, Some("PCH_CHIP_TEMP"))
                .unwired(10, 0, Some("PCH_CPU_TEMP"))
                .fan(1, 0, None)
                .fan(2, 1180, None)
                .fan(3, 0, None)
                .fan(4, 812, None)
                .fan(5, 790, None)
                .fan(6, 0, None)
                .fan(7, 1650, None)
                .other("in0", 1368)
                .other("in1", 1016)
                .other("in2", 3392),
            Chip::new("acpitz", On::Nothing, Site::Board).temp(1, 16_800, None),
            Chip::new("drivetemp", On::Drive(1), Site::Drive(1)).temp(1, 33_000, None),
            Chip::new("drivetemp", On::Drive(2), Site::Drive(2)).temp(1, 38_000, None),
        ],
    }
}

/// DDR5 module `n`'s monitor, on the SMBus at `address`.
fn module(address: u16, n: usize) -> Chip {
    Chip::new(
        "spd5118",
        On::Module {
            smbus: SMBUS,
            bus: 0,
            address,
            generation: Some("DDR5"),
        },
        Site::Module(n),
    )
}

static SPEC: LazyLock<Spec> = LazyLock::new(spec);

pub(super) fn machine() -> Machine {
    SPEC.machine(|t| SPEC.snapshot(t))
}
