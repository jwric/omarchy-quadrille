//! The network interfaces with hardware behind them, by kind: not by name,
//! which can carry the hardware address, and never by the network joined.
use std::fs::File;

use super::{PciAddress, Tree};

/// A network interface.
#[derive(Debug, Clone, PartialEq)]
pub struct Interface {
    pub link: Link,
    /// The kernel driver of its adapter.
    pub driver: Option<String>,
    /// The adapter on the PCI bus, or the USB controller it is plugged into.
    pub pci: Option<PciAddress>,
    /// Whether the adapter is on USB.
    pub usb: bool,
    /// The negotiated speed in Mbit/s, for a wired link that is up.
    pub speed: Option<u32>,
    /// Whether it was up when the machine was read.
    pub up: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    Ethernet,
    Wireless,
    Other,
}

/// ARPHRD_ETHER: Ethernet and Wi-Fi alike.
const ETHER: u32 = 1;

/// The interfaces with a device, and the files their bytes received and
/// sent are counted in.
pub(super) fn interfaces(tree: &Tree) -> Vec<(Interface, Option<(File, File)>)> {
    tree.entries("sys/class/net")
        .into_iter()
        // Loopback, bridges, tunnels and containers' have no device.
        .filter(|name| tree.exists(format!("sys/class/net/{name}/device")))
        .map(|name| {
            let at = |file: &str| format!("sys/class/net/{name}/{file}");
            let link = if tree.exists(at("wireless")) || tree.exists(at("phy80211")) {
                Link::Wireless
            } else if tree.number(at("type")) == Some(ETHER) {
                Link::Ethernet
            } else {
                Link::Other
            };
            let interface = Interface {
                link,
                driver: tree.link(at("device/driver")),
                pci: tree.pci_path(at("device")).last().copied(),
                usb: tree.through_usb(at("device")),
                speed: tree
                    .number::<i64>(at("speed"))
                    .and_then(|mbps| u32::try_from(mbps).ok())
                    .filter(|mbps| *mbps > 0),
                up: tree.text(at("operstate")).as_deref() == Some("up"),
            };
            let counters = tree
                .open(at("statistics/rx_bytes"))
                .zip(tree.open(at("statistics/tx_bytes")));

            (interface, counters)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::tests::laptop;
    use super::*;

    #[test]
    fn interfaces_are_known_by_kind_and_only_with_hardware() {
        let machine = laptop().read();
        let links: Vec<_> = machine.interfaces.iter().map(|i| i.link).collect();

        // The loopback and a bridge are left out.
        assert_eq!(links, [Link::Ethernet, Link::Wireless]);

        let wired = &machine.interfaces[0];
        assert_eq!(wired.speed, Some(1000));
        assert_eq!(wired.pci, Some(PciAddress::new(0, 2, 0, 0)));
        assert!(wired.up && !wired.usb);

        let wireless = &machine.interfaces[1];
        assert_eq!(wireless.speed, None);
        assert_eq!(wireless.pci, Some(PciAddress::new(0, 0, 0x14, 3)));
    }
}
