//! A made-up two-socket rack server: two packages of 32 cores of two
//! threads, sixteen registered modules, eight NVMe drives each on a root
//! port of its own, a SAS controller with eight hard disks, two boot SSDs,
//! six network ports, the management controller's graphics with no
//! display on it, and no fan the kernel sees (the management controller
//! runs them).
use std::sync::LazyLock;

use super::*;

const BMC: At = (2, 0, 0);
const USB: At = (0, 0x14, 0);
const SATA: At = (0, 0x17, 0);
const SMBUS: At = (0, 0x1f, 4);
const SAS: At = (0x5e, 0, 0);

/// The root ports the NVMe drives are on, a drive behind each: four on
/// each package's root complex.
const NVME_PORTS: [At; 8] = [
    (0x30, 2, 0),
    (0x30, 3, 0),
    (0x30, 4, 0),
    (0x30, 5, 0),
    (0xb0, 2, 0),
    (0xb0, 3, 0),
    (0xb0, 4, 0),
    (0xb0, 5, 0),
];

const NVME_SECTORS: u64 = 7_501_476_528;
const HDD_SECTORS: u64 = 31_251_759_104;
const SSD_SECTORS: u64 = 937_703_088;

/// The NVMe controller behind root port `port`.
const fn nvme(port: At) -> At {
    (port.0 + port.1 - 1, 0, 0)
}

fn functions() -> Vec<Pci> {
    let mut functions = vec![
        Pci::new((0, 0, 0), 0x060000, 0x0001, "Host Bridge"),
        Pci::new(USB, 0x0c0330, 0x0014, "USB 3.0 Controller").driver("xhci_hcd"),
        Pci::new(SATA, 0x010601, 0x0017, "SATA Controller").driver("ahci"),
        Pci::new((0, 0x1c, 0), 0x060400, 0x001c, "PCIe Root Port").driver("pcieport"),
        Pci::new((0, 0x1f, 0), 0x060100, 0x001f, "LPC Bridge"),
        Pci::new(SMBUS, 0x0c0500, 0x0020, "SMBus Controller").driver("i801_smbus"),
        Pci::new((1, 0, 0), 0x060400, 0x0101, "PCI Bridge").behind((0, 0x1c, 0)),
        Pci::new(BMC, 0x030000, 0x0200, "BMC Graphics")
            .behind((1, 0, 0))
            .driver("ast"),
        Pci::new(SAS, 0x010700, 0x5e00, "SAS Controller")
            .behind((0x5d, 2, 0))
            .driver("mpt3sas"),
    ];

    for port in [(0x17, 2, 0), (0x3a, 2, 0), (0x5d, 2, 0)]
        .into_iter()
        .chain(NVME_PORTS)
    {
        functions.push(Pci::new(port, 0x060400, 0x0347, "PCIe Root Port").driver("pcieport"));
    }
    for port in NVME_PORTS {
        functions.push(
            Pci::new(nvme(port), 0x010802, 0x0b60, "NVMe SSD Controller")
                .behind(port)
                .driver("nvme"),
        );
    }
    for function in 0..2 {
        functions.push(
            Pci::new(
                (0x18, 0, function),
                0x020000,
                0x1572,
                "10G Ethernet Controller",
            )
            .behind((0x17, 2, 0))
            .driver("i40e"),
        );
    }
    for function in 0..4 {
        functions.push(
            Pci::new(
                (0x3b, 0, function),
                0x020000,
                0x1521,
                "Gigabit Ethernet Controller",
            )
            .behind((0x3a, 2, 0))
            .driver("igb"),
        );
    }

    functions.sort_by_key(|function| function.at);
    functions
}

const USB_DEVICES: [Usb; 5] = [
    Usb::root("usb1", 480.0, "2.00", 16),
    Usb::new("1-1", 480.0, "2.00", 0x09, Some("USB Hub")).ports(4),
    Usb::new(
        "1-1.1",
        480.0,
        "2.00",
        0x03,
        Some("Virtual Keyboard and Mouse"),
    ),
    Usb::new("1-1.2", 480.0, "2.00", 0x08, Some("Virtual Media")),
    Usb::root("usb2", 5000.0, "3.00", 10),
];

pub(super) fn spec() -> Spec {
    let mut drives: Vec<Disk> = NVME_PORTS
        .iter()
        .enumerate()
        .map(|(n, &port)| Disk {
            name: format!("nvme{n}n1"),
            model: Some("NVMe SSD 3.84TB"),
            sectors: NVME_SECTORS,
            on: Attached::Nvme(nvme(port), n as u32),
            rotational: false,
            partitions: whole(NVME_SECTORS, Some("xfs")),
        })
        .collect();

    drives.extend((0..8u8).map(|n| Disk {
        name: format!("sd{}", char::from(b'a' + n)),
        model: Some("SAS HDD 16TB"),
        sectors: HDD_SECTORS,
        on: Attached::Sas(SAS, u32::from(n)),
        rotational: true,
        partitions: whole(HDD_SECTORS, Some("xfs")),
    }));
    drives.extend([
        Disk {
            name: "sdi".into(),
            model: Some("SATA SSD 480GB"),
            sectors: SSD_SECTORS,
            on: Attached::Sata(SATA, 0),
            rotational: false,
            partitions: efi_and(SSD_SECTORS, "ext4"),
        },
        Disk {
            name: "sdj".into(),
            model: Some("SATA SSD 480GB"),
            sectors: SSD_SECTORS,
            on: Attached::Sata(SATA, 1),
            rotational: false,
            partitions: whole(SSD_SECTORS, None),
        },
    ]);

    // Each package's monitor: the package, and each of its cores.
    let mut chips: Vec<Chip> = (0..2)
        .map(|package: i64| {
            let on = if package == 0 {
                "platform/coretemp.0"
            } else {
                "platform/coretemp.1"
            };
            let chip = Chip::new(
                "coretemp",
                On::Device(on),
                Site::Processor(package as usize),
            )
            .temp(
                1,
                61_000 + 2000 * package,
                Some(&format!("Package id {package}")),
            );

            (0..32).fold(chip, |chip, core: i64| {
                chip.temp(
                    core as u32 + 2,
                    52_000 + (core * 1700) % 11_000 + 1000 * package,
                    Some(&format!("Core {core}")),
                )
            })
        })
        .collect();

    // A monitor on each module, eight on each of two of the SMBus's buses.
    chips.extend((0..16u16).map(|n| {
        Chip::new(
            "jc42",
            On::Module {
                smbus: SMBUS,
                bus: (n / 8) as u8,
                address: 0x18 + n % 8,
                generation: None,
            },
            Site::Module(n.into()),
        )
        .temp(1, 38_000 + 250 * i64::from(n), None)
    }));
    chips.extend((0..8).map(|n| {
        Chip::new("nvme", On::Drive(n), Site::Drive(n))
            .temp(1, 37_850 + 1000 * n as i64, Some("Composite"))
            .temp(2, 45_850 + 1000 * n as i64, Some("Sensor 1"))
    }));
    // The power meter, whose power is not read.
    chips.push(
        Chip::new(
            "power_meter",
            On::Device("LNXSYSTM:00/LNXSYBUS:00/ACPI000D:00"),
            Site::Board,
        )
        .other("power1", 412_000_000),
    );

    Spec {
        chassis: Chassis {
            kind: ChassisKind::Server,
            vendor: Some("Generic".into()),
            product: Some("2U Server".into()),
            board_vendor: Some("Generic".into()),
            board: Some("Server Board".into()),
            bios: Some("1.0".into()),
        },
        smbios: 23,
        cpu: Some(Processor {
            model: "Generic 32-Core Server Processor",
            packages: 2,
            kinds: vec![Kind {
                kind: None,
                cores: 32,
                threads: 2,
                max_mhz: Some(3500),
                base_mhz: Some(2000),
                caches: vec![
                    (1, CacheKind::Data, 48, 1),
                    (1, CacheKind::Instruction, 32, 1),
                    (2, CacheKind::Unified, 2048, 1),
                ],
            }],
            l3_kib: Some(48 << 10),
        }),
        memory_kib: Some(527_800_000),
        pci: functions(),
        usb: vec![(USB, USB_DEVICES.to_vec())],
        screens: vec![Screen {
            name: "VGA-1",
            card: 0,
            gpu: BMC,
            on: Shown::Open,
        }],
        drives,
        ports: [
            ("eno1", (0x3b, 0, 0), Some(1000)),
            ("eno2", (0x3b, 0, 1), None),
            ("eno3", (0x3b, 0, 2), None),
            ("eno4", (0x3b, 0, 3), None),
            ("ens1f0", (0x18, 0, 0), Some(10_000)),
            ("ens1f1", (0x18, 0, 1), Some(10_000)),
        ]
        .into_iter()
        .map(|(name, at, speed)| Port {
            name,
            link: Link::Ethernet,
            at,
            speed,
            up: speed.is_some(),
        })
        .collect(),
        chips,
    }
}

static SPEC: LazyLock<Spec> = LazyLock::new(spec);

pub(super) fn machine() -> Machine {
    SPEC.machine(|t| SPEC.snapshot(t))
}
