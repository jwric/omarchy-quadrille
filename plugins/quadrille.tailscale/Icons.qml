import QtQuick

// 7 x 7 sprites for the Tailscale panel and its bar face (see Q/Sprites.qml for
// the alphabet; `plus` and `warning` come from there). A plain object, not a
// singleton: Panel.qml and TailscaleIcon.qml each make one, so the plugin
// reloads with a rescan and touches no shared file.
//
// '#' lit   '1'..'9' lit when the sprite's `level` reaches the digit, else dim
// '!' accent   '.' empty
QtObject {
  // ---- the Tailscale mark: a 3 x 3 grid of dots whose lit ones make a T on its
  // head, here joined into one stroke so it reads at bar size. Drawn at level 0,
  // so the digits are the faded dots.
  readonly property var mark: [
    "1..1..1",
    ".......",
    ".......",
    "#######",
    "...#...",
    "...#...",
    "1..#..1"
  ]
  // Off: the whole mark faded.
  readonly property var markOff: [
    "1..1..1",
    ".......",
    ".......",
    "1111111",
    "...1...",
    "...1...",
    "1..1..1"
  ]
  // Needs login: the mark with a badge in the accent at its corner.
  readonly property var markLogin: [
    "1..1..1",
    ".......",
    ".......",
    "#######",
    "...#...",
    "...#.!!",
    "1..#.!!"
  ]

  // ---- the same mark for the two-line hero, drawn at its own size (15 x 15)
  // rather than the small one doubled: the nine dots as they are in the logo,
  // square, three pixels across with three between, the faded ones digits.
  readonly property var markLarge: [
    "111...111...111",
    "111...111...111",
    "111...111...111",
    "...............",
    "...............",
    "...............",
    "###...###...###",
    "###...###...###",
    "###...###...###",
    "...............",
    "...............",
    "...............",
    "111...###...111",
    "111...###...111",
    "111...###...111"
  ]
  readonly property var markLargeOff: [
    "111...111...111",
    "111...111...111",
    "111...111...111",
    "...............",
    "...............",
    "...............",
    "111...111...111",
    "111...111...111",
    "111...111...111",
    "...............",
    "...............",
    "...............",
    "111...111...111",
    "111...111...111",
    "111...111...111"
  ]
  // Needs login: the corner dot is the badge, in the accent.
  readonly property var markLargeLogin: [
    "111...111...111",
    "111...111...111",
    "111...111...111",
    "...............",
    "...............",
    "...............",
    "###...###...###",
    "###...###...###",
    "###...###...###",
    "...............",
    "...............",
    "...............",
    "111...###...!!!",
    "111...###...!!!",
    "111...###...!!!"
  ]

  // ---- what each machine runs (Model.osIcon's glyphs, one sprite each)
  readonly property var linux: [
    "..##...",
    ".#.##..",
    ".####..",
    "##..##.",
    "#....#.",
    "##..##.",
    ".##.##."
  ]
  readonly property var apple: [
    "....#..",
    "...#...",
    ".#####.",
    "######.",
    "#####..",
    "######.",
    ".##.#.."
  ]
  readonly property var windows: [
    "###.###",
    "###.###",
    "###.###",
    ".......",
    "###.###",
    "###.###",
    "###.###"
  ]
  readonly property var android: [
    ".#...#.",
    "..###..",
    ".#####.",
    "##.#.##",
    "#######",
    ".......",
    "......."
  ]
  // A globe: Mullvad's exit nodes, which are places in the world.
  readonly property var globe: [
    "..###..",
    ".#.#.#.",
    "#..#..#",
    "#######",
    "#..#..#",
    ".#.#.#.",
    "..###.."
  ]
  // A computer: any other system.
  readonly property var computer: [
    "#######",
    "#.....#",
    "#.....#",
    "#######",
    ".......",
    "#######",
    "#.###.#"
  ]

  // ---- rows and actions
  // Authorize the operator: a shield, half of it filled.
  readonly property var shieldHalf: [
    "#######",
    "#..####",
    "#..####",
    "#..####",
    ".#.###.",
    "..###..",
    "...#..."
  ]
  // A connection (an account on a tailnet).
  readonly property var person: [
    "..###..",
    ".#####.",
    ".#####.",
    "..###..",
    ".......",
    ".#####.",
    "#######"
  ]
  // An exit node: the way out of the tailnet.
  readonly property var exit: [
    "###....",
    "#......",
    "#....#.",
    "#.#####",
    "#....#.",
    "#......",
    "###...."
  ]
  // Send files: a paper plane.
  readonly property var send: [
    "#......",
    "###....",
    "#.###..",
    "#...###",
    "#.###..",
    "###....",
    "#......"
  ]
  readonly property var copy: [
    "####...",
    "#..#...",
    "#.#####",
    "#.#...#",
    "###...#",
    "..#...#",
    "..#####"
  ]
}
