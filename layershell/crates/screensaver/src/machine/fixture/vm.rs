//! A made-up virtual machine, with almost nothing: four virtual CPUs (each
//! a package of one core, as a hypervisor gives them by default), memory
//! and no module, a virtual display that gives no EDID, a virtio disk and
//! network adapter, and no hardware monitor or USB.
use std::sync::LazyLock;

use super::*;

const GPU: At = (0, 1, 0);
const NETWORK: At = (1, 0, 0);
const DISK: At = (2, 0, 0);

const PCI: [Pci; 9] = [
    Pci::new((0, 0, 0), 0x060000, 0x0001, "Host Bridge"),
    Pci::new(GPU, 0x030000, 0x1050, "Virtual GPU").driver("virtio-pci"),
    Pci::new((0, 2, 0), 0x060400, 0x000c, "PCIe Root Port").driver("pcieport"),
    Pci::new((0, 2, 1), 0x060400, 0x000c, "PCIe Root Port").driver("pcieport"),
    Pci::new((0, 0x1f, 0), 0x060100, 0x2918, "LPC Bridge").driver("lpc_ich"),
    Pci::new((0, 0x1f, 2), 0x010601, 0x2922, "SATA Controller").driver("ahci"),
    Pci::new((0, 0x1f, 3), 0x0c0500, 0x2930, "SMBus Controller").driver("i801_smbus"),
    Pci::new(NETWORK, 0x020000, 0x1041, "Virtual Network Device")
        .behind((0, 2, 0))
        .driver("virtio-pci"),
    Pci::new(DISK, 0x010000, 0x1042, "Virtual Block Device")
        .behind((0, 2, 1))
        .driver("virtio-pci"),
];

const DISK_SECTORS: u64 = 41_943_040;

pub(super) fn spec() -> Spec {
    Spec {
        chassis: Chassis {
            kind: ChassisKind::Other,
            vendor: Some("Generic".into()),
            product: Some("Virtual Machine".into()),
            board_vendor: None,
            board: None,
            bios: Some("1.0".into()),
        },
        smbios: 1,
        cpu: Some(Processor {
            model: "Generic Virtual CPU",
            packages: 4,
            kinds: vec![Kind {
                kind: None,
                cores: 1,
                threads: 1,
                max_mhz: None,
                base_mhz: None,
                caches: vec![
                    (1, CacheKind::Data, 32, 1),
                    (1, CacheKind::Instruction, 32, 1),
                    (2, CacheKind::Unified, 4096, 1),
                ],
            }],
            l3_kib: Some(16 << 10),
        }),
        memory_kib: Some(4_015_000),
        pci: PCI.to_vec(),
        usb: Vec::new(),
        screens: vec![Screen {
            name: "Virtual-1",
            card: 0,
            gpu: GPU,
            on: Shown::Modes(1280, 800),
        }],
        drives: vec![Disk {
            name: "vda".into(),
            model: None,
            sectors: DISK_SECTORS,
            on: Attached::Virtio(DISK, 2),
            rotational: true,
            partitions: efi_and(DISK_SECTORS, "ext4"),
        }],
        ports: vec![Port {
            name: "enp1s0",
            link: Link::Ethernet,
            at: NETWORK,
            speed: None,
            up: true,
        }],
        chips: Vec::new(),
    }
}

static SPEC: LazyLock<Spec> = LazyLock::new(spec);

pub(super) fn machine() -> Machine {
    SPEC.machine(|t| SPEC.snapshot(t))
}
