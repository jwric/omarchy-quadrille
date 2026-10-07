//! The machine's own batteries and chargers; a mouse's or a headset's
//! battery is not the machine's, and is left out.
use std::fs::File;

use super::Tree;
use super::sensors::reread;

/// A battery: what it was made to hold and what it holds now.
#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    /// `Li-ion`, `Li-poly`.
    pub chemistry: Option<String>,
    pub maker: Option<String>,
    /// What it held new, in watt-hours.
    pub design_wh: Option<f32>,
    /// What it holds full now.
    pub full_wh: Option<f32>,
    /// Charge cycles, where the firmware counts them (some say 0 always).
    pub cycles: Option<u32>,
}

impl Battery {
    /// What it holds full as a share of what it held new.
    pub fn health(&self) -> Option<f32> {
        Some(self.full_wh? / self.design_wh?).filter(|health| health.is_finite())
    }
}

/// A charger.
#[derive(Debug, Clone, PartialEq)]
pub struct Charger {
    pub kind: ChargerKind,
    /// The most it can deliver, where it says.
    pub watts: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargerKind {
    /// A barrel or mains adapter.
    Mains,
    /// USB power delivery.
    Usb,
}

/// A battery's state at a moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Charge {
    /// How full it is, from 0 to 1.
    pub fraction: Option<f32>,
    pub state: Option<ChargeState>,
    /// The power flowing in or out of it.
    pub watts: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeState {
    Charging,
    Discharging,
    Full,
    /// Plugged in, and neither charging nor full.
    Idle,
}

/// The files a battery's state is read from.
pub(super) struct Gauge {
    capacity: Option<File>,
    status: Option<File>,
    power: Option<File>,
    current: Option<File>,
    voltage: Option<File>,
}

impl Gauge {
    pub(super) fn read(&self) -> Charge {
        let number = |file: &Option<File>| reread(file.as_ref()?)?.parse::<f64>().ok();
        let watts = number(&self.power)
            .map(|uw| uw / 1e6)
            .or_else(|| Some(number(&self.current)? * number(&self.voltage)? / 1e12));

        Charge {
            fraction: number(&self.capacity)
                .map(|percent| (percent / 100.0).clamp(0.0, 1.0) as f32),
            state: match self.status.as_ref().and_then(reread).as_deref() {
                Some("Charging") => Some(ChargeState::Charging),
                Some("Discharging") => Some(ChargeState::Discharging),
                Some("Full") => Some(ChargeState::Full),
                Some("Not charging") => Some(ChargeState::Idle),
                _ => None,
            },
            watts: watts.map(|watts| watts.abs() as f32),
        }
    }
}

/// The supplies that power the machine, with their types.
fn supplies(tree: &Tree) -> impl Iterator<Item = (String, String)> + '_ {
    tree.entries("sys/class/power_supply")
        .into_iter()
        .filter_map(|name| {
            let at = |file: &str| format!("sys/class/power_supply/{name}/{file}");
            // A peripheral's says so; the machine's own say nothing, or
            // `System`.
            if tree.text(at("scope")).as_deref() == Some("Device") {
                return None;
            }

            Some((tree.text(at("type"))?, name))
        })
}

pub(super) fn batteries(tree: &Tree) -> Vec<(Battery, Gauge)> {
    supplies(tree)
        .filter(|(kind, _)| kind == "Battery")
        .map(|(_, name)| {
            let at = |file: &str| format!("sys/class/power_supply/{name}/{file}");
            let number = |file: &str| tree.number::<f64>(at(file));
            // In µWh, or µAh at the design voltage in µV.
            let wh = |energy: &str, charge: &str| {
                number(energy)
                    .map(|uwh| uwh / 1e6)
                    .or_else(|| Some(number(charge)? * number("voltage_min_design")? / 1e12))
                    .map(|wh| wh as f32)
            };
            let battery = Battery {
                chemistry: tree.text(at("technology")),
                maker: tree.text(at("manufacturer")),
                design_wh: wh("energy_full_design", "charge_full_design"),
                full_wh: wh("energy_full", "charge_full"),
                cycles: tree.number(at("cycle_count")),
            };
            let gauge = Gauge {
                capacity: tree.open(at("capacity")),
                status: tree.open(at("status")),
                power: tree.open(at("power_now")),
                current: tree.open(at("current_now")),
                voltage: tree.open(at("voltage_now")),
            };

            (battery, gauge)
        })
        .collect()
}

/// The chargers, and the files that say whether they are plugged in.
pub(super) fn chargers(tree: &Tree) -> Vec<(Charger, Option<File>)> {
    supplies(tree)
        .filter_map(|(kind, name)| {
            let at = |file: &str| format!("sys/class/power_supply/{name}/{file}");
            let kind = match kind.as_str() {
                "Mains" => ChargerKind::Mains,
                "USB" => ChargerKind::Usb,
                _ => return None,
            };
            let watts = tree
                .number::<f64>(at("voltage_max"))
                .zip(tree.number::<f64>(at("current_max")))
                .map(|(uv, ua)| (uv * ua / 1e12) as f32)
                .filter(|watts| *watts > 0.0);

            Some((Charger { kind, watts }, tree.open(at("online"))))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Fake, laptop};
    use super::*;

    #[test]
    fn the_battery_and_charger_are_the_machines_own() {
        let machine = laptop().read();
        let [battery] = machine.batteries.as_slice() else {
            panic!("One battery, not the mouse's: {:?}", machine.batteries);
        };

        assert_eq!(battery.chemistry.as_deref(), Some("Li-poly"));
        assert_eq!(battery.design_wh, Some(60.0));
        assert_eq!(battery.full_wh, Some(54.6));
        assert_eq!(battery.cycles, Some(212));
        assert!((battery.health().unwrap() - 0.91).abs() < 1e-6);

        assert_eq!(
            machine.chargers,
            [Charger {
                kind: ChargerKind::Usb,
                watts: Some(65.0)
            }]
        );

        let snapshot = machine.sample(0.0);
        assert_eq!(snapshot.chargers, [Some(false)]);
        assert_eq!(
            snapshot.batteries,
            [Charge {
                fraction: Some(0.8),
                state: Some(ChargeState::Discharging),
                watts: Some(9.5)
            }]
        );
    }

    #[test]
    fn a_battery_that_counts_charge_is_measured_at_its_design_voltage() {
        let fake = Fake::new();
        let at = "sys/class/power_supply/BAT1";
        fake.file(&format!("{at}/type"), "Battery\n")
            .file(&format!("{at}/charge_full_design"), "6250000\n")
            .file(&format!("{at}/charge_full"), "4717000\n")
            .file(&format!("{at}/voltage_min_design"), "15200000\n")
            .file(&format!("{at}/current_now"), "1000000\n")
            .file(&format!("{at}/voltage_now"), "16000000\n")
            .file(&format!("{at}/status"), "Not charging\n");
        let machine = fake.read();

        assert_eq!(machine.batteries[0].design_wh, Some(95.0));
        assert!((machine.batteries[0].full_wh.unwrap() - 71.698).abs() < 0.001);

        let charge = &machine.sample(0.0).batteries[0];
        assert_eq!(charge.watts, Some(16.0));
        assert_eq!(charge.state, Some(ChargeState::Idle));
        assert_eq!(charge.fraction, None);
    }
}
