//! Seven-by-seven icons for the panels, in the toolkit's own manner: odd sides
//! so each has a centre, none taller than the capitals of the body face, and
//! drawn in whatever colour the text around them is.
//!
//! One pixel size to a surface: the virtual pixel. A sprite is drawn with
//! one lit pixel to one virtual pixel, always (`quadrille::widget::icon` and
//! `Pen::sprite` have no scale), so that an icon never has a mixed pixel, a
//! "mixel", next to the text. An icon that has to be bigger than the 7 of a
//! line of text, because it stands beside two or more lines or alone, is not
//! a 7 made bigger: it is a sprite of its own, drawn by hand at its own
//! native size, with the detail those pixels allow, of the same family as
//! these. The sizes are [`SIZES`]. The tests below keep to it: every sprite
//! has one of those sizes, none is a pixel-multiplied copy of a smaller one,
//! and nothing in this crate scales one.
use quadrille::draw::Sprite;

/// The sizes an icon may have, each drawn by hand at that size: 7 beside a
/// line of text, then 11, 15 and 21 for an icon that stands beside two, three
/// or four lines, or alone. All odd, so that each has a centre.
#[allow(dead_code)]
pub const SIZES: [i32; 4] = [7, 11, 15, 21];

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

    /// Whether `sprite` is a smaller one with every pixel made `k` by `k`,
    /// for some `k` of 2 or more: a scaled copy, not a drawing.
    fn is_a_scaled_copy(sprite: &Sprite) -> bool {
        (2..=sprite.width().min(sprite.height())).any(|k| {
            sprite.width() % k == 0
                && sprite.height() % k == 0
                && (0..sprite.height()).step_by(k as usize).all(|y| {
                    (0..sprite.width()).step_by(k as usize).all(|x| {
                        let lit = sprite.lit(x, y);

                        (0..k).all(|dy| (0..k).all(|dx| sprite.lit(x + dx, y + dy) == lit))
                    })
                })
        })
    }

    /// `sprite` with every pixel `k` by `k`: what must never be on screen.
    fn scaled(sprite: &Sprite, k: usize) -> Sprite {
        let mut rows = Vec::new();

        for y in 0..sprite.height() {
            let row: String = (0..sprite.width())
                .flat_map(|x| std::iter::repeat_n(if sprite.lit(x, y) { '#' } else { '.' }, k))
                .collect();

            for _ in 0..k {
                let row: &'static str = Box::leak(row.clone().into_boxed_str());

                rows.push(row);
            }
        }

        Sprite::new(Box::leak(rows.into_boxed_slice()))
    }

    #[test]
    fn every_icon_is_square_and_a_native_size() {
        for (name, icon) in ALL {
            assert_eq!(icon.width(), icon.height(), "{name} is not square");
            assert!(
                SIZES.contains(&icon.width()),
                "{name} is {} wide: icons are {SIZES:?}",
                icon.width()
            );
        }

        assert!(SIZES.iter().all(|size| size % 2 == 1), "sizes are odd");
        // The size beside a line of text is the odd one the capitals leave.
        let cap = i32::from(Face::BODY.cap());

        assert!(SIZES[0] <= cap && cap - SIZES[0] <= 1);
    }

    #[test]
    fn no_icon_is_a_scaled_copy_of_a_smaller_one() {
        for (name, icon) in ALL {
            assert!(!is_a_scaled_copy(&icon), "{name} is a scaled copy");
        }
    }

    #[test]
    fn a_scaled_copy_is_told_from_a_drawing() {
        // The check has to see what it is there to forbid: any icon at 2x, 3x
        // or 4x, and the doubled sizes a lazy larger icon would have.
        for (name, icon) in ALL {
            for k in 2..=4 {
                let copy = scaled(&icon, k);

                assert!(is_a_scaled_copy(&copy), "{name} at {k}x went unseen");
                assert_eq!(copy.width(), icon.width() * k as i32);
            }
        }

        // And a drawing of 11 by 11 that has detail of its own is not one.
        let drawn = Sprite::new(&[
            "....###....",
            "..##...##..",
            ".#.......#.",
            ".#..###..#.",
            "#..#...#..#",
            "#..#.#.#..#",
            "#..#...#..#",
            ".#..###..#.",
            ".#.......#.",
            "..##...##..",
            "....###....",
        ]);

        assert!(!is_a_scaled_copy(&drawn));
        assert!(SIZES.contains(&drawn.width()));
    }

    /// Every source file of this crate.
    fn sources(dir: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();

            if path.is_dir() {
                sources(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }

    #[test]
    fn no_icon_in_this_crate_is_drawn_bigger_than_it_is() {
        // `Pen::sprite` and `widget::icon` take no scale, so a scaled sprite
        // would need a routine of this crate's own: there is none, and this
        // is the place that says there must not be. The words are put
        // together here so that this file does not contain them.
        let words = [
            concat!("up", "scale"),
            concat!("magni", "fy"),
            concat!("enlar", "ge"),
            concat!("scal", "ed_sprite"),
            concat!("sprite_", "scale"),
            concat!("Sprite::", "scale"),
        ];

        let mut files = Vec::new();
        sources(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );

        assert!(files.len() > 10, "{files:?}");

        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();

            for word in words {
                assert!(
                    !text.contains(word),
                    "{} mentions {word}: icons are drawn at their own size, \
                     never scaled",
                    file.display()
                );
            }
        }
    }
}
