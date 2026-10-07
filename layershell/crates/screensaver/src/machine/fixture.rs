//! A made-up laptop: what tests and committed images draw instead of the
//! machine they were made on. Its parts are generic, and its live values a
//! function of time, so a render of it is the same every time.
//!
//! The tests also build it as a sysfs tree, and read that back as the
//! fixture.
use std::f32::consts::TAU;

use quadrille_desktop::physical::{DisplayInput, Overrides, PhysicalSize};

use super::*;

const GENERIC: u16 = 0x0f0f;

/// The PCI devices: address, the bridge it is behind, class, device id,
/// name, class name and driver.
#[allow(clippy::type_complexity)]
const PCI: [(
    (u8, u8, u8),
    Option<(u8, u8, u8)>,
    u32,
    u16,
    &str,
    &str,
    Option<&str>,
); 11] = [
    (
        (0, 0, 0),
        None,
        0x060000,
        0x0001,
        "Host Bridge",
        "Host bridge",
        None,
    ),
    (
        (0, 2, 0),
        None,
        0x030000,
        0x0002,
        "Integrated Graphics",
        "VGA compatible controller",
        None,
    ),
    (
        (0, 6, 0),
        None,
        0x060400,
        0x0006,
        "PCIe Root Port",
        "PCI bridge",
        Some("pcieport"),
    ),
    (
        (0, 0x0d, 0),
        None,
        0x0c0330,
        0x000d,
        "USB 3.2 Controller",
        "USB controller",
        Some("xhci_hcd"),
    ),
    (
        (0, 0x14, 3),
        None,
        0x028000,
        0x0014,
        "Wi-Fi 6E Adapter",
        "Network controller",
        None,
    ),
    (
        (0, 0x1c, 0),
        None,
        0x060400,
        0x001c,
        "PCIe Root Port",
        "PCI bridge",
        Some("pcieport"),
    ),
    (
        (0, 0x1f, 0),
        None,
        0x060100,
        0x001f,
        "LPC Bridge",
        "ISA bridge",
        None,
    ),
    (
        (0, 0x1f, 3),
        None,
        0x040300,
        0x0020,
        "HD Audio Controller",
        "Audio device",
        Some("snd_hda_intel"),
    ),
    (
        (0, 0x1f, 4),
        None,
        0x0c0500,
        0x0021,
        "SMBus Controller",
        "SMBus",
        None,
    ),
    (
        (1, 0, 0),
        Some((0, 6, 0)),
        0x010802,
        0x0100,
        "NVMe SSD Controller",
        "Non-Volatile memory controller",
        Some("nvme"),
    ),
    (
        (2, 0, 0),
        Some((0, 0x1c, 0)),
        0x020000,
        0x0200,
        "Gigabit Ethernet Controller",
        "Ethernet controller",
        None,
    ),
];

const fn pci((bus, device, function): (u8, u8, u8)) -> PciAddress {
    PciAddress::new(0, bus, device, function)
}

const GPU: PciAddress = pci((0, 2, 0));
const NVME: PciAddress = pci((1, 0, 0));
const USB: PciAddress = pci((0, 0x0d, 0));
const WIRELESS: PciAddress = pci((0, 0x14, 3));
const ETHERNET: PciAddress = pci((2, 0, 0));

/// The USB devices: port, speed, version, class, ports, product,
/// removable.
#[allow(clippy::type_complexity)]
const USB_DEVICES: [(&str, f32, &str, u8, u32, Option<&str>, Option<bool>); 7] = [
    (
        "usb1",
        480.0,
        "2.00",
        0x09,
        12,
        Some("xHCI Host Controller"),
        None,
    ),
    (
        "1-3",
        12.0,
        "2.00",
        0x03,
        0,
        Some("USB Receiver"),
        Some(true),
    ),
    (
        "1-6",
        480.0,
        "2.00",
        0x0e,
        0,
        Some("Integrated Camera"),
        Some(false),
    ),
    ("1-10", 12.0, "2.00", 0xe0, 0, None, Some(false)),
    (
        "usb2",
        10000.0,
        "3.20",
        0x09,
        4,
        Some("xHCI Host Controller"),
        None,
    ),
    (
        "2-1",
        5000.0,
        "3.20",
        0x09,
        4,
        Some("USB 3.2 Hub"),
        Some(true),
    ),
    ("2-1.2", 5000.0, "3.20", 0x08, 0, Some("Card Reader"), None),
];

/// The displays: connector, name, mode, refresh, image size in mm, year.
#[allow(clippy::type_complexity)]
const DISPLAYS: [(&str, Option<&str>, (u32, u32), u32, (u32, u32), u16); 2] = [
    ("eDP-1", None, (2560, 1600), 120, (302, 189), 2025),
    (
        "DP-1",
        Some("27 Monitor"),
        (3840, 2160),
        60,
        (597, 336),
        2024,
    ),
];

/// The drive, in 512-byte sectors: its size, and each partition's start
/// and size.
const DRIVE_SECTORS: u64 = 1_953_525_168;
const PARTITIONS: [(u64, u64); 2] = [(2048, 2_097_152), (2_099_200, 1_951_425_934)];

const MEMORY_KIB: u64 = 32_509_360;

pub(super) fn machine() -> Machine {
    Machine {
        chassis: Chassis {
            kind: ChassisKind::Laptop,
            vendor: Some("Generic".into()),
            product: Some("Laptop 14".into()),
            board_vendor: Some("Generic".into()),
            board: Some("Mainboard".into()),
            bios: Some("1.0".into()),
        },
        cpu: Some(Cpu {
            model: Some("Generic 16-Core Processor".into()),
            packages: 1,
            cores: 16,
            threads: 24,
            kinds: vec![
                Cores {
                    kind: CoreKind::Performance,
                    cores: 8,
                    threads: 16,
                    max_mhz: Some(5000),
                },
                Cores {
                    kind: CoreKind::Efficient,
                    cores: 8,
                    threads: 8,
                    max_mhz: Some(3800),
                },
            ],
            caches: [
                (1, CacheKind::Data, 32 << 10, 8),
                (1, CacheKind::Data, 48 << 10, 8),
                (1, CacheKind::Instruction, 32 << 10, 8),
                (1, CacheKind::Instruction, 64 << 10, 8),
                (2, CacheKind::Unified, 2 << 20, 8),
                (2, CacheKind::Unified, 4 << 20, 2),
                (3, CacheKind::Unified, 24 << 20, 1),
            ]
            .into_iter()
            .map(|(level, kind, bytes, instances)| Cache {
                level,
                kind,
                bytes,
                instances,
            })
            .collect(),
            base_mhz: Some(2200),
            max_mhz: Some(5000),
        }),
        memory: Some(Memory {
            bytes: Some(MEMORY_KIB * 1024),
            modules: vec![
                Module {
                    generation: Some("DDR5".into()),
                };
                2
            ],
        }),
        drives: vec![Drive {
            name: "nvme0n1".into(),
            model: Some("NVMe SSD 1TB".into()),
            bytes: DRIVE_SECTORS * 512,
            kind: DriveKind::Nvme,
            rotational: false,
            removable: false,
            pci: Some(NVME),
            partitions: PARTITIONS
                .iter()
                .zip([("vfat", false), ("btrfs", true)])
                .enumerate()
                .map(
                    |(index, (&(start, size), (filesystem, mapped)))| Partition {
                        number: index as u32 + 1,
                        start: start * 512,
                        bytes: size * 512,
                        filesystem: Some(filesystem.into()),
                        mapped,
                    },
                )
                .collect(),
        }],
        pci: PCI
            .iter()
            .map(
                |&(address, parent, class, device, name, class_name, driver)| PciDevice {
                    address: pci(address),
                    parent: parent.map(pci),
                    class,
                    vendor: GENERIC,
                    device,
                    vendor_name: Some("Generic".into()),
                    name: Some(name.into()),
                    class_name: Some(class_name.into()),
                    driver: driver.map(Into::into),
                },
            )
            .collect(),
        usb: USB_DEVICES
            .iter()
            .map(
                |&(port, speed, version, class, ports, product, removable)| UsbDevice {
                    port: port.into(),
                    parent: buses::parent(port),
                    controller: Some(USB),
                    speed: Some(speed),
                    version: Some(version.into()),
                    class,
                    ports,
                    product: product.map(Into::into),
                    maker: None,
                    removable,
                },
            )
            .collect(),
        connectors: DISPLAYS
            .iter()
            .map(|&(name, model, pixels, refresh, mm, year)| Connector {
                name: name.into(),
                kind: if name.starts_with("eDP") {
                    ConnectorKind::Internal
                } else {
                    ConnectorKind::DisplayPort
                },
                gpu: Some(GPU),
                panel: Some(Panel {
                    maker: Some("GEN".into()),
                    name: model.map(Into::into),
                    pixels,
                    refresh: Some(refresh as f32),
                    year: Some(year),
                    size: PhysicalSize::resolve(
                        &DisplayInput {
                            name: name.into(),
                            make: "GEN".into(),
                            model: model.unwrap_or_default().into(),
                            width: pixels.0,
                            height: pixels.1,
                            physical_width: f64::from(mm.0),
                            physical_height: f64::from(mm.1),
                            transform: 0,
                            scale: 1.0,
                        },
                        &Overrides::default(),
                    ),
                }),
            })
            .chain([Connector {
                name: "HDMI-A-1".into(),
                kind: ConnectorKind::Hdmi,
                gpu: Some(GPU),
                panel: None,
            }])
            .collect(),
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
        interfaces: vec![
            Interface {
                link: Link::Ethernet,
                driver: None,
                pci: Some(ETHERNET),
                usb: false,
                speed: Some(1000),
                up: true,
            },
            Interface {
                link: Link::Wireless,
                driver: None,
                pci: Some(WIRELESS),
                usb: false,
                speed: None,
                up: true,
            },
        ],
        sensors: [
            (SensorKind::Temperature, "acpitz", None, Site::Board),
            (
                SensorKind::Temperature,
                "cpu_thermal",
                Some("Package"),
                Site::Processor,
            ),
            (
                SensorKind::Temperature,
                "nvme",
                Some("Composite"),
                Site::Device(NVME),
            ),
            (SensorKind::Temperature, "spd5118", None, Site::Module(0)),
            (SensorKind::Temperature, "spd5118", None, Site::Module(1)),
            (SensorKind::Fan, "ec", Some("CPU Fan"), Site::Board),
            (SensorKind::Fan, "ec", Some("System Fan"), Site::Board),
        ]
        .into_iter()
        .map(|(kind, chip, label, site)| Sensor {
            kind,
            chip: chip.into(),
            label: label.map(Into::into),
            site,
        })
        .collect(),
        sampler: Sampler::made(sample),
    }
}

/// The fixture's values at `t`: a processor that warms and cools over
/// about forty seconds with the fans following it, a battery running down,
/// and bursts of I/O.
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

/// Builds the fixture as a sysfs tree in `fake`, with the files of
/// identifiers a real one has beside what is read.
#[cfg(test)]
pub(super) fn tree(fake: &super::tests::Fake) {
    use super::displays::tests::edid;

    let dmi = "sys/class/dmi/id";
    fake.file(&format!("{dmi}/chassis_type"), "10\n")
        .file(&format!("{dmi}/sys_vendor"), "Generic\n")
        .file(&format!("{dmi}/product_name"), "Laptop 14\n")
        .file(&format!("{dmi}/board_vendor"), "Generic\n")
        .file(&format!("{dmi}/board_name"), "Mainboard\n")
        .file(&format!("{dmi}/bios_version"), "1.0\n");

    // Eight performance cores of two threads, eight efficient ones of one
    // in two clusters of four.
    let system = "sys/devices/system/cpu";
    let mut cpuinfo = String::new();

    for cpu in 0..24u32 {
        let at = format!("{system}/cpu{cpu}");
        let performance = cpu < 16;
        let (core, sibling) = if performance {
            (cpu / 2, format!("{}-{}", cpu / 2 * 2, cpu / 2 * 2 + 1))
        } else {
            (cpu - 8, cpu.to_string())
        };
        let cluster = if cpu < 20 { "16-19" } else { "20-23" };
        let caches = if performance {
            [
                ("1", "Data", "48K", sibling.as_str()),
                ("1", "Instruction", "32K", &sibling),
                ("2", "Unified", "2048K", &sibling),
            ]
        } else {
            [
                ("1", "Data", "32K", sibling.as_str()),
                ("1", "Instruction", "64K", &sibling),
                ("2", "Unified", "4096K", cluster),
            ]
        };

        fake.file(&format!("{at}/topology/physical_package_id"), "0\n")
            .file(&format!("{at}/topology/core_id"), format!("{core}\n"))
            .file(
                &format!("{at}/cpufreq/cpuinfo_max_freq"),
                if performance {
                    "5000000\n"
                } else {
                    "3800000\n"
                },
            )
            .file(
                &format!("{at}/cpufreq/base_frequency"),
                if performance {
                    "2200000\n"
                } else {
                    "1600000\n"
                },
            );

        for (index, (level, kind, size, shared)) in caches
            .into_iter()
            .chain([("3", "Unified", "24576K", "0-23")])
            .enumerate()
        {
            let at = format!("{at}/cache/index{index}");
            fake.file(&format!("{at}/level"), level)
                .file(&format!("{at}/type"), kind)
                .file(&format!("{at}/size"), size)
                .file(&format!("{at}/shared_cpu_list"), shared);
        }

        cpuinfo += &format!("processor\t: {cpu}\nmodel name\t: Generic 16-Core Processor\n\n");
    }

    fake.file("proc/cpuinfo", cpuinfo)
        .file("sys/devices/cpu_core/cpus", "0-15\n")
        .file("sys/devices/cpu_atom/cpus", "16-23\n")
        .dir(&format!("{system}/cpufreq"))
        .file(
            "proc/meminfo",
            format!("MemTotal:       {MEMORY_KIB} kB\nMemFree:         1000 kB\n"),
        );

    // The PCI devices, each at its place under the root complex.
    let mut ids = String::from("# The fixture's pci.ids\n0f0f  Generic\n");
    let mut ids_seen = Vec::new();
    let place = |address: (u8, u8, u8)| {
        let (_, parent, ..) = PCI.iter().find(|row| row.0 == address).unwrap();
        match parent {
            Some(parent) => format!("pci0000:00/{}/{}", pci(*parent), pci(address)),
            None => format!("pci0000:00/{}", pci(address)),
        }
    };

    for &(address, _, class, device, name, _, driver) in &PCI {
        let at = format!("sys/devices/{}", place(address));
        fake.file(&format!("{at}/class"), format!("0x{class:06x}\n"))
            .file(&format!("{at}/vendor"), format!("0x{GENERIC:04x}\n"))
            .file(&format!("{at}/device"), format!("0x{device:04x}\n"))
            .link(
                &format!("sys/bus/pci/devices/{}", pci(address)),
                &format!("../../../devices/{}", place(address)),
            );

        if let Some(driver) = driver {
            fake.link(
                &format!("{at}/driver"),
                &format!("../../../bus/pci/drivers/{driver}"),
            );
        }
        if !ids_seen.contains(&device) {
            ids += &format!("\t{device:04x}  {name}\n\t\t0f0f 0001  {name} (subsystem)\n");
            ids_seen.push(device);
        }
    }

    ids += "0f10  Someone Else\n\t0001  Not This One\n\
        C 01  Mass storage controller\n\t08  Non-Volatile memory controller\n\t\t02  NVM Express\n\
        C 02  Network controller\n\t00  Ethernet controller\n\t80  Network controller\n\
        C 03  Display controller\n\t00  VGA compatible controller\n\
        C 04  Multimedia controller\n\t03  Audio device\n\
        C 06  Bridge\n\t00  Host bridge\n\t01  ISA bridge\n\t04  PCI bridge\n\
        C 0c  Serial bus controller\n\t03  USB controller\n\t05  SMBus\n";
    fake.file("usr/share/hwdata/pci.ids", ids);

    // The USB tree, on the controller at 00:0d.0.
    let controller = format!("sys/devices/{}", place((0, 0x0d, 0)));

    for &(port, speed, version, class, ports, product, removable) in &USB_DEVICES {
        let mut path = Vec::new();
        let mut step = Some(port.to_owned());
        while let Some(port) = step {
            step = buses::parent(&port);
            path.insert(0, port);
        }
        let at = format!("{controller}/{}", path.join("/"));
        let own_class = if matches!(class, 0x09 | 0xe0) {
            class
        } else {
            0
        };

        fake.file(&format!("{at}/speed"), format!("{speed}\n"))
            .file(&format!("{at}/version"), format!(" {version}\n"))
            .file(&format!("{at}/bDeviceClass"), format!("{own_class:02x}\n"))
            .file(&format!("{at}/maxchild"), format!("{ports}\n"))
            .file(
                &format!("{at}/removable"),
                match removable {
                    Some(true) => "removable\n",
                    Some(false) => "fixed\n",
                    None => "unknown\n",
                },
            )
            .link(
                &format!("sys/bus/usb/devices/{port}"),
                &format!("../../../{}", &at[4..]),
            );

        if let Some(product) = product {
            fake.file(&format!("{at}/product"), format!("{product}\n"));
        }
        if own_class == 0 {
            fake.file(
                &format!("{at}/{port}:1.0/bInterfaceClass"),
                format!("{class:02x}\n"),
            )
            .link(
                &format!("sys/bus/usb/devices/{port}:1.0"),
                &format!("../../../{}/{port}:1.0", &at[4..]),
            );
        }
    }

    fake.file(
        &format!("{controller}/usb1/1-3/serial"),
        "SERIAL-USB-0006\n",
    )
    .file(
        &format!("{controller}/usb1/manufacturer"),
        "Linux 6.0.0-host xhci-hcd\n",
    )
    .file(
        &format!("{controller}/usb2/manufacturer"),
        "Linux 6.0.0-host xhci-hcd\n",
    );

    // The displays on the integrated graphics.
    let card = format!("sys/devices/{}/drm/card0", place((0, 2, 0)));
    fake.link("sys/class/drm/card0", &format!("../../{}", &card[4..]))
        .file("sys/class/drm/version", "drm 1.1.0\n");

    for &(name, model, (width, height), refresh, mm, year) in &DISPLAYS {
        let at = format!("{card}/card0-{name}");
        fake.file(&format!("{at}/status"), "connected\n")
            .file(
                &format!("{at}/modes"),
                format!("{width}x{height}\n1920x1080\n"),
            )
            .file(
                &format!("{at}/edid"),
                edid("GEN", model, (width, height, refresh), mm, year),
            )
            .link(
                &format!("sys/class/drm/card0-{name}"),
                &format!("../../{}", &at[4..]),
            );
    }

    fake.file(&format!("{card}/card0-HDMI-A-1/status"), "disconnected\n")
        .file(&format!("{card}/card0-HDMI-A-1/modes"), "")
        .file(&format!("{card}/card0-HDMI-A-1/edid"), "")
        .link(
            "sys/class/drm/card0-HDMI-A-1",
            &format!("../../{}/card0-HDMI-A-1", &card[4..]),
        );

    // The drive: a boot partition, and an encrypted one with btrfs on it.
    let nvme = format!("sys/devices/{}/nvme/nvme0", place((1, 0, 0)));
    let disk = format!("{nvme}/nvme0n1");
    fake.file(
        &format!("{nvme}/model"),
        "NVMe SSD 1TB                            \n",
    )
    .file(&format!("{disk}/size"), format!("{DRIVE_SECTORS}\n"))
    .file(&format!("{disk}/removable"), "0\n")
    .file(&format!("{disk}/queue/rotational"), "0\n")
    .file(
        &format!("{disk}/stat"),
        "1000 0 2048 0 500 0 2000 0 0 0 0\n",
    )
    .link(&format!("{disk}/device"), "../../nvme0")
    .link("sys/block/nvme0n1", &format!("../{}", &disk[4..]));

    for (index, (start, size)) in PARTITIONS.iter().enumerate() {
        let at = format!("{disk}/nvme0n1p{}", index + 1);
        fake.file(&format!("{at}/partition"), format!("{}\n", index + 1))
            .file(&format!("{at}/start"), format!("{start}\n"))
            .file(&format!("{at}/size"), format!("{size}\n"));
    }

    fake.link(&format!("{disk}/nvme0n1p2/holders/dm-0"), "../../../../../../../../../virtual/block/dm-0")
        .file("sys/devices/virtual/block/dm-0/size", "1951393166\n")
        .file("sys/devices/virtual/block/dm-0/dm/name", "root\n")
        .link("sys/block/dm-0", "../devices/virtual/block/dm-0")
        .file("sys/devices/virtual/block/zram0/size", "16777216\n")
        .link("sys/block/zram0", "../devices/virtual/block/zram0")
        .link("dev/mapper/root", "../dm-0")
        .file(
            "proc/mounts",
            "/dev/mapper/root / btrfs rw,relatime 0 0\n\
             /dev/nvme0n1p1 /boot vfat rw,relatime 0 0\n\
             /dev/mapper/root /home btrfs rw,relatime 0 0\n\
             tmpfs /tmp tmpfs rw 0 0\n\
             proc /proc proc rw 0 0\n",
        )
        .file(
            "proc/swaps",
            "Filename\t\t\t\tType\t\tSize\t\tUsed\t\tPriority\n/dev/zram0  partition\t8388604\t0\t100\n",
        );

    // The batteries and chargers: the machine's, and a mouse's.
    let supply = "sys/class/power_supply";
    fake.file(&format!("{supply}/BAT0/type"), "Battery\n")
        .file(&format!("{supply}/BAT0/technology"), "Li-poly\n")
        .file(&format!("{supply}/BAT0/energy_full_design"), "60000000\n")
        .file(&format!("{supply}/BAT0/energy_full"), "54600000\n")
        .file(&format!("{supply}/BAT0/cycle_count"), "212\n")
        .file(&format!("{supply}/BAT0/capacity"), "80\n")
        .file(&format!("{supply}/BAT0/status"), "Discharging\n")
        .file(&format!("{supply}/BAT0/power_now"), "9500000\n")
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

    // The network: ethernet and Wi-Fi, the loopback and a bridge.
    for (name, address, wireless) in [("eth0", (2, 0, 0), false), ("wlan0", (0, 0x14, 3), true)] {
        let at = format!("sys/devices/{}/net/{name}", place(address));
        fake.file(&format!("{at}/type"), "1\n")
            .file(&format!("{at}/operstate"), "up\n")
            .file(&format!("{at}/statistics/rx_bytes"), "1000000\n")
            .file(&format!("{at}/statistics/tx_bytes"), "200000\n")
            .link(&format!("{at}/device"), "../..")
            .link(
                &format!("sys/class/net/{name}"),
                &format!("../../{}", &at[4..]),
            );

        if wireless {
            fake.dir(&format!("{at}/wireless"));
        } else {
            fake.file(&format!("{at}/speed"), "1000\n");
        }
    }

    for (name, kind) in [("lo", "772"), ("docker0", "1")] {
        fake.file(
            &format!("sys/devices/virtual/net/{name}/type"),
            format!("{kind}\n"),
        )
        .file(
            &format!("sys/devices/virtual/net/{name}/operstate"),
            "unknown\n",
        )
        .link(
            &format!("sys/class/net/{name}"),
            &format!("../../devices/virtual/net/{name}"),
        );
    }

    // The hardware monitors, the battery's among them.
    let hwmon =
        |n: u32, name: &str, device: Option<&str>, channels: &[(&str, &str, Option<&str>)]| {
            let at = format!("sys/class/hwmon/hwmon{n}");
            fake.file(&format!("{at}/name"), format!("{name}\n"));
            if let Some(device) = device {
                fake.link(&format!("{at}/device"), &format!("../../../{device}"));
            }
            for (channel, value, label) in channels {
                fake.file(&format!("{at}/{channel}_input"), format!("{value}\n"));
                if let Some(label) = label {
                    fake.file(&format!("{at}/{channel}_label"), format!("{label}\n"));
                }
            }
        };
    let smbus = format!("devices/{}/i2c-0", place((0, 0x1f, 4)));

    fake.dir("sys/devices/platform/cpu_thermal")
        .dir(&format!("sys/{smbus}/0-0050"))
        .dir(&format!("sys/{smbus}/0-0051"))
        .dir(&format!("sys/devices/{}/PNP0C09:00", place((0, 0x1f, 0))));
    hwmon(0, "acpitz", None, &[("temp1", "44000", None)]);
    hwmon(
        1,
        "cpu_thermal",
        Some("devices/platform/cpu_thermal"),
        &[("temp1", "52000", Some("Package"))],
    );
    hwmon(
        2,
        "nvme",
        Some(&nvme[4..]),
        &[("temp1", "41850", Some("Composite"))],
    );
    hwmon(
        3,
        "spd5118",
        Some(&format!("{smbus}/0-0050")),
        &[("temp1", "45500", None)],
    );
    hwmon(
        4,
        "spd5118",
        Some(&format!("{smbus}/0-0051")),
        &[("temp1", "46250", None)],
    );
    hwmon(
        5,
        "ec",
        Some(&format!("devices/{}/PNP0C09:00", place((0, 0x1f, 0)))),
        &[
            ("fan1", "2200", Some("CPU Fan")),
            ("fan2", "1870", Some("System Fan")),
        ],
    );
    hwmon(
        6,
        "BAT0",
        Some("class/power_supply/BAT0"),
        &[("in0", "16200", None), ("curr1", "500", None)],
    );

    // What must never be read.
    fake.file(&format!("{dmi}/product_serial"), "SERIAL-DMI-0001\n")
        .file(&format!("{dmi}/product_uuid"), "UUID-0000-1111\n")
        .file(&format!("{nvme}/serial"), "SERIAL-NVME-0002\n")
        .file(&format!("{nvme}/subsysnqn"), "NQN-SERIAL-0004\n")
        .file(&format!("{disk}/eui"), "EUI-0003\n")
        .file(&format!("{disk}/wwid"), "EUI-0003\n")
        .file(
            "sys/devices/virtual/block/dm-0/dm/uuid",
            "CRYPT-LUKS2-UUID-0000-1111\n",
        )
        .file(&format!("{supply}/BAT0/serial_number"), "SERIAL-BAT-0005\n");
}

#[cfg(test)]
mod tests {
    use super::super::tests::laptop;
    use super::*;

    /// The fixture is what reading its tree gives: every part of the
    /// inventory read from a whole machine.
    #[test]
    fn the_fixture_is_its_tree_read() {
        let read = format!("{:#?}", laptop().read());
        let fixture = format!("{:#?}", Machine::fixture());

        for (line, (read, fixture)) in read.lines().zip(fixture.lines()).enumerate() {
            assert_eq!(read, fixture, "line {}", line + 1);
        }
        assert_eq!(read.lines().count(), fixture.lines().count());
    }

    #[test]
    fn the_fixtures_values_are_a_function_of_time_for_each_of_its_parts() {
        let machine = Machine::fixture();

        for t in [0.0, 0.37, 5.0, 61.3, 1234.5] {
            let snapshot = machine.sample(t);

            assert_eq!(snapshot, machine.sample(t));
            assert_eq!(snapshot.sensors.len(), machine.sensors.len());
            assert_eq!(snapshot.batteries.len(), machine.batteries.len());
            assert_eq!(snapshot.chargers.len(), machine.chargers.len());
            assert_eq!(snapshot.drives.len(), machine.drives.len());
            assert_eq!(snapshot.interfaces.len(), machine.interfaces.len());

            for (index, value) in snapshot.sensors.iter().enumerate() {
                let value = value.expect("Every sensor reads");
                let range = match machine.sensors[index].kind {
                    SensorKind::Fan => 500.0..4000.0,
                    _ => 25.0..90.0,
                };
                assert!(range.contains(&value), "sensor {index} at {t}: {value}");
            }

            let charge = snapshot.batteries[0].fraction.unwrap();
            assert!((0.05..=0.8).contains(&charge));
        }

        assert_ne!(machine.sample(0.0), machine.sample(10.0));
    }

    #[test]
    fn the_fixture_has_what_the_sheets_draw() {
        let machine = Machine::fixture();

        assert_eq!(machine.chassis.kind, ChassisKind::Laptop);
        assert_eq!(machine.displays().count(), 2);
        assert_eq!(machine.fans().count(), 2);
        assert_eq!(machine.drives[0].kind, DriveKind::Nvme);

        let links: Vec<_> = machine.interfaces.iter().map(|i| i.link).collect();
        assert_eq!(links, [Link::Ethernet, Link::Wireless]);

        // Every device a part hangs from is on the bus.
        for address in machine
            .drives
            .iter()
            .filter_map(|d| d.pci)
            .chain(machine.interfaces.iter().filter_map(|i| i.pci))
            .chain(machine.connectors.iter().filter_map(|c| c.gpu))
            .chain(machine.usb.iter().filter_map(|u| u.controller))
            .chain(machine.pci.iter().filter_map(|d| d.parent))
        {
            assert!(machine.pci_device(address).is_some(), "{address}");
        }
    }
}
