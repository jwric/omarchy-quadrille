//! The drives, their partitions and the filesystems on them, by type only:
//! where a filesystem is mounted is never read.
use std::collections::HashMap;
use std::fs::File;

use super::{PciAddress, Tree};

/// A drive.
#[derive(Debug, Clone, PartialEq)]
pub struct Drive {
    /// The kernel's name for it: `nvme0n1`, `sda`.
    pub name: String,
    pub model: Option<String>,
    pub bytes: u64,
    pub kind: DriveKind,
    pub rotational: bool,
    pub removable: bool,
    /// The PCI device it is reached through: its own controller for NVMe,
    /// the SATA or USB controller otherwise.
    pub pci: Option<PciAddress>,
    pub partitions: Vec<Partition>,
}

/// How a drive is attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveKind {
    Nvme,
    Sata,
    Usb,
    /// A memory card or soldered eMMC.
    Mmc,
    Other,
}

/// A partition of a drive.
#[derive(Debug, Clone, PartialEq)]
pub struct Partition {
    pub number: u32,
    /// Where it starts on the drive, in bytes.
    pub start: u64,
    pub bytes: u64,
    /// The type of the filesystem on it while it is mounted, or `swap`.
    pub filesystem: Option<String>,
    /// Whether the filesystem is reached through the device mapper: an
    /// encrypted volume, or logical volumes.
    pub mapped: bool,
}

/// A sector, as sysfs counts sizes and I/O.
const SECTOR: u64 = 512;

/// Every drive with a medium in it, and the file its I/O is counted in.
pub(super) fn drives(tree: &Tree) -> Vec<(Drive, Option<File>)> {
    let filesystems = filesystems(tree);

    tree.entries("sys/block")
        .into_iter()
        // Hardware has a device; loop, zram and the device mapper do not.
        .filter(|name| tree.exists(format!("sys/block/{name}/device")))
        // An eMMC chip's boot and replay-protected areas are parts of it,
        // not drives of their own.
        .filter(|name| !emmc_area(name))
        .filter_map(|name| {
            let at = |file: &str| format!("sys/block/{name}/{file}");
            let bytes = tree.number::<u64>(at("size"))? * SECTOR;

            if bytes == 0 {
                return None;
            }

            let kind = if name.starts_with("nvme") {
                DriveKind::Nvme
            } else if name.starts_with("mmcblk") {
                DriveKind::Mmc
            } else if tree.through_usb(at("")) {
                DriveKind::Usb
            } else if name.starts_with("sd") {
                DriveKind::Sata
            } else {
                DriveKind::Other
            };
            let partitions = tree
                .entries(at(""))
                .into_iter()
                .filter_map(|part| {
                    let at = |file: &str| format!("sys/block/{name}/{part}/{file}");
                    let holders = stacked(tree, &at("holders"));

                    Some(Partition {
                        number: tree.number(at("partition"))?,
                        start: tree.number::<u64>(at("start"))? * SECTOR,
                        bytes: tree.number::<u64>(at("size"))? * SECTOR,
                        filesystem: std::iter::once(&part)
                            .chain(&holders)
                            .find_map(|device| filesystems.get(device).cloned()),
                        mapped: !holders.is_empty(),
                    })
                })
                .collect();
            let drive = Drive {
                model: tree
                    .text(at("device/model"))
                    .or_else(|| tree.text(at("device/name"))),
                bytes,
                kind,
                rotational: tree.number::<u8>(at("queue/rotational")) == Some(1),
                removable: tree.number::<u8>(at("removable")) == Some(1),
                pci: tree.pci_path(at("")).last().copied(),
                partitions,
                name: name.clone(),
            };

            Some((drive, tree.open(at("stat"))))
        })
        .collect()
}

/// Whether a block device is an eMMC chip's boot area (`mmcblk0boot0`) or
/// its replay-protected one (`mmcblk0rpmb`).
fn emmc_area(name: &str) -> bool {
    name.strip_prefix("mmcblk").is_some_and(|rest| {
        let area = rest.trim_start_matches(|c: char| c.is_ascii_digit());

        area != rest
            && (area == "rpmb"
                || area
                    .strip_prefix("boot")
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())))
    })
}

/// The devices stacked on a partition, from its `holders`, nearest first:
/// an encrypted volume on it, then the logical volumes on that, and so on.
fn stacked(tree: &Tree, holders: &str) -> Vec<String> {
    let mut stacked = tree.entries(holders);
    let mut next = 0;

    // Each device once, so a tree that loops ends.
    while let Some(device) = stacked.get(next) {
        for above in tree.entries(format!("sys/block/{device}/holders")) {
            if !stacked.contains(&above) {
                stacked.push(above);
            }
        }

        next += 1;
    }

    stacked
}

/// The type of filesystem on each block device that has one mounted, by
/// the kernel's name for the device, and `swap` on those swapped to. Only
/// the device and the type are read from each line.
fn filesystems(tree: &Tree) -> HashMap<String, String> {
    let mounts = tree.text("proc/mounts").unwrap_or_default();
    let swaps = tree.text("proc/swaps").unwrap_or_default();
    let mounted = mounts.lines().filter_map(|line| {
        let mut fields = line.split_whitespace();
        let device = fields.next()?;
        let kind = fields.nth(1)?;
        Some((device, kind))
    });
    let swapped = swaps
        .lines()
        .skip(1)
        .filter_map(|line| Some((line.split_whitespace().next()?, "swap")));
    let mut filesystems = HashMap::new();

    for (device, kind) in mounted.chain(swapped) {
        let Some(path) = device.strip_prefix("/dev/") else {
            continue;
        };
        // `/dev/mapper/root` is a link to `../dm-0`.
        let name = tree
            .link(format!("dev/{path}"))
            .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path).to_owned());

        filesystems.entry(name).or_insert_with(|| kind.to_owned());
    }

    filesystems
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Fake, laptop};
    use super::*;

    #[test]
    fn a_drive_has_its_partitions_and_their_filesystems() {
        let machine = laptop().read();
        let [drive] = machine.drives.as_slice() else {
            panic!("One drive: {:?}", machine.drives);
        };

        assert_eq!(drive.name, "nvme0n1");
        assert_eq!(drive.model.as_deref(), Some("NVMe SSD 1TB"));
        assert_eq!(drive.kind, DriveKind::Nvme);
        assert_eq!(drive.bytes, 1_953_525_168 * 512);
        assert_eq!(drive.pci, Some(PciAddress::new(0, 1, 0, 0)));
        assert!(!drive.rotational && !drive.removable);

        let boot = &drive.partitions[0];
        assert_eq!((boot.number, boot.start, boot.bytes), (1, 1 << 20, 1 << 30));
        assert_eq!(boot.filesystem.as_deref(), Some("vfat"));
        assert!(!boot.mapped);

        // Encrypted: btrfs is mounted from the mapped device over it.
        let system = &drive.partitions[1];
        assert_eq!(system.filesystem.as_deref(), Some("btrfs"));
        assert!(system.mapped);
    }

    #[test]
    fn virtual_and_empty_block_devices_are_not_drives() {
        let fake = Fake::new();
        fake.file("sys/devices/virtual/block/zram0/size", "1000")
            .link("sys/block/zram0", "../devices/virtual/block/zram0")
            .file(
                "sys/devices/pci0000:00/0000:00:14.0/usb1/1-2/host0/block/sda/size",
                "0",
            )
            .dir("sys/devices/pci0000:00/0000:00:14.0/usb1/1-2/host0/block/sda/device")
            .link(
                "sys/block/sda",
                "../devices/pci0000:00/0000:00:14.0/usb1/1-2/host0/block/sda",
            );

        assert!(fake.read().drives.is_empty());
    }

    /// Logical volumes on an encrypted partition: the filesystem is two
    /// devices up from the partition.
    #[test]
    fn a_filesystem_on_volumes_on_an_encrypted_partition_is_found() {
        let fake = laptop();
        fake.link("sys/devices/virtual/block/dm-0/holders/dm-1", "../../dm-1")
            .file("sys/devices/virtual/block/dm-1/size", "1000000\n")
            .link("sys/block/dm-1", "../devices/virtual/block/dm-1")
            .link("dev/mapper/vg-root", "../dm-1")
            .file(
                "proc/mounts",
                "/dev/mapper/vg-root / ext4 rw,relatime 0 0\n",
            );

        let system = &fake.read().drives[0].partitions[1];

        assert_eq!(system.filesystem.as_deref(), Some("ext4"));
        assert!(system.mapped);
    }

    /// An eMMC chip is one drive: its boot and replay-protected areas,
    /// which the kernel lists beside it, are not drives.
    #[test]
    fn an_emmc_chips_boot_areas_are_not_drives() {
        let fake = Fake::new();
        let host = "sys/devices/pci0000:00/0000:00:1a.0/mmc_host/mmc0/mmc0:0001/block";

        for (name, sectors) in [
            ("mmcblk0", "122142720"),
            ("mmcblk0boot0", "8192"),
            ("mmcblk0boot1", "8192"),
            ("mmcblk0rpmb", "8192"),
        ] {
            let dir = format!("{host}/{name}");
            fake.file(&format!("{dir}/size"), sectors)
                .dir(&format!("{dir}/device"))
                .link(&format!("sys/block/{name}"), &format!("../{}", &dir[4..]));
        }

        let names: Vec<String> = fake.read().drives.into_iter().map(|d| d.name).collect();

        assert_eq!(names, ["mmcblk0"]);
        assert!(!emmc_area("mmcblk0") && !emmc_area("mmcblk10p1") && !emmc_area("mmcblkboot"));
    }

    #[test]
    fn a_drive_on_usb_is_a_usb_drive() {
        let fake = Fake::new();
        let dir = "sys/devices/pci0000:00/0000:00:14.0/usb1/1-2/host0/block/sda";
        fake.file(&format!("{dir}/size"), "2048")
            .file(&format!("{dir}/removable"), "1")
            .file(&format!("{dir}/device/model"), "Flash Drive")
            .link("sys/block/sda", &format!("../{}", &dir[4..]));
        let drive = &fake.read().drives[0];

        assert_eq!(drive.kind, DriveKind::Usb);
        assert!(drive.removable);
        assert_eq!(drive.pci, Some(PciAddress::new(0, 0, 0x14, 0)));
    }
}
