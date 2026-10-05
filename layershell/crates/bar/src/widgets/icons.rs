//! Seven-by-seven icons for the panels, in the toolkit's own manner: odd sides
//! so each has a centre, none taller than the capitals of the body face, and
//! drawn in whatever colour the text around them is.
use quadrille::draw::Sprite;

/// A speaker with sound.
pub const SPEAKER: Sprite = Sprite::new(&[
    "...#...", "..##.#.", "###..#.", "###..#.", "###..#.", "..##.#.", "...#...",
]);

/// A speaker crossed out.
pub const SPEAKER_MUTED: Sprite = Sprite::new(&[
    "...#...", "..##...", "###.#.#", "###..#.", "###.#.#", "..##...", "...#...",
]);

/// Headphones.
pub const HEADPHONES: Sprite = Sprite::new(&[
    "..###..", ".#...#.", "#.....#", "#.....#", "###.###", "###.###", ".#...#.",
]);

/// A microphone.
pub const MIC: Sprite = Sprite::new(&[
    "..###..", "..###..", "#.###.#", "#.###.#", ".#####.", "...#...", "..###..",
]);

/// A microphone crossed out.
pub const MIC_MUTED: Sprite = Sprite::new(&[
    "#.###..", ".####..", "#.###.#", "#.#.#.#", ".#####.", "...#.#.", "..###.#",
]);

/// Wi-Fi: three arcs and a dot.
pub const WIFI: Sprite = Sprite::new(&[
    ".#####.", "#.....#", "..###..", ".#...#.", "...#...", ".......", "...#...",
]);

/// Wi-Fi, off.
pub const WIFI_OFF: Sprite = Sprite::new(&[
    "#.###..", "#.#...#", "..#.#..", ".#...#.", "...#.#.", "......#", "...#..#",
]);

/// A padlock, locked.
pub const LOCK: Sprite = Sprite::new(&[
    "..###..", ".#...#.", ".#...#.", "#######", "###.###", "###.###", "#######",
]);

/// A network socket.
pub const ETHERNET: Sprite = Sprite::new(&[
    "#######", "#.#.#.#", "#.#.#.#", "#.....#", "#######", "..###..", "..#.#..",
]);

/// A shield, for a VPN.
pub const SHIELD: Sprite = Sprite::new(&[
    "#######", "#.....#", "#..#..#", "#..#..#", ".#...#.", "..#.#..", "...#...",
]);

/// The Bluetooth rune.
pub const BLUETOOTH: Sprite = Sprite::new(&[
    "...#...", "...##..", ".#.#.#.", "..###..", ".#.#.#.", "...##..", "...#...",
]);

/// A battery.
pub const BATTERY: Sprite = Sprite::new(&[
    "..###..", "#######", "#.....#", "#.....#", "#.....#", "#.....#", "#######",
]);

/// A plug, for mains power.
pub const PLUG: Sprite = Sprite::new(&[
    ".#...#.", ".#...#.", "#######", "#.....#", ".#...#.", "..#.#..", "...#...",
]);

/// The power symbol.
pub const POWER: Sprite = Sprite::new(&[
    "...#...", ".#.#.#.", "#..#..#", "#.....#", "#.....#", ".#...#.", "..###..",
]);

/// A crescent, for suspend.
pub const MOON: Sprite = Sprite::new(&[
    "..###..", ".#.....", "#......", "#......", "#.....#", ".#...#.", "..###..",
]);

/// An arrow turning round.
pub const REBOOT: Sprite = Sprite::new(&[
    "..####.", ".#...#.", "#.....#", "#...###", "#....#.", ".#..#..", "..##...",
]);

/// A door with an arrow out.
pub const LOGOUT: Sprite = Sprite::new(&[
    "###....", "#.#.#..", "#.#..#.", "#.#####", "#.#..#.", "#.#.#..", "###....",
]);

/// A keyboard.
pub const KEYBOARD: Sprite = Sprite::new(&[
    ".......", "#######", "#.#.#.#", "#######", "#.###.#", "#######", ".......",
]);

/// A mouse.
pub const MOUSE: Sprite = Sprite::new(&[
    "..###..", ".#.#.#.", ".#.#.#.", ".#####.", ".#...#.", ".#...#.", "..###..",
]);

/// A phone.
pub const PHONE: Sprite = Sprite::new(&[
    ".#####.", ".#...#.", ".#...#.", ".#...#.", ".#...#.", ".#.#.#.", ".#####.",
]);

/// A leaf, for saving power.
pub const LEAF: Sprite = Sprite::new(&[
    "....###", "...####", "..#####", ".####.#", "#####..", ".#.....", "#......",
]);

/// Scales, for balanced.
pub const SCALES: Sprite = Sprite::new(&[
    "...#...", "#######", "...#...", ".#.#.#.", "#..#..#", "...#...", ".#####.",
]);

/// A bolt, for performance.
pub const BOLT: Sprite = Sprite::new(&[
    "...##..", "..##...", ".##....", "#######", "....##.", "...##..", "..##...",
]);

/// Every icon, with its name, for a specimen and for the tests.
#[allow(dead_code)]
pub const ALL: [(&str, Sprite); 23] = [
    ("SPEAKER", SPEAKER),
    ("SPEAKER_MUTED", SPEAKER_MUTED),
    ("HEADPHONES", HEADPHONES),
    ("MIC", MIC),
    ("MIC_MUTED", MIC_MUTED),
    ("WIFI", WIFI),
    ("WIFI_OFF", WIFI_OFF),
    ("LOCK", LOCK),
    ("ETHERNET", ETHERNET),
    ("SHIELD", SHIELD),
    ("BLUETOOTH", BLUETOOTH),
    ("BATTERY", BATTERY),
    ("PLUG", PLUG),
    ("POWER", POWER),
    ("MOON", MOON),
    ("REBOOT", REBOOT),
    ("LOGOUT", LOGOUT),
    ("KEYBOARD", KEYBOARD),
    ("MOUSE", MOUSE),
    ("PHONE", PHONE),
    ("LEAF", LEAF),
    ("SCALES", SCALES),
    ("BOLT", BOLT),
];

#[cfg(test)]
mod tests {
    use super::*;
    use quadrille::Face;

    #[test]
    fn every_icon_has_a_centre_and_fits_the_capitals() {
        for (name, icon) in ALL {
            assert_eq!(icon.width(), 7, "{name} is not seven wide");
            assert_eq!(icon.height() % 2, 1, "{name} has an even height");
            assert!(
                icon.height() <= i32::from(Face::BODY.cap()),
                "{name} is taller than a capital"
            );
        }
    }
}
