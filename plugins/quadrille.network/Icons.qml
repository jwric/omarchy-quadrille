import QtQuick

// 7 x 7 sprites the network panel needs and the kit does not have (see
// Q/Sprites.qml for the alphabet; the wifi fan, the wired socket, the cross and
// the lock come from there). A plain object, instantiated by Panel.qml, so it
// reloads with the plugin and touches no shared singleton.
//
// '#' lit   '1'..'9' lit when the sprite's `level` reaches the digit   '!' accent
QtObject {
  // ---- The hero's link, at the size of its two lines of text (15 x 15 on the
  // same grid as the text: never a 7 x 7 drawn at 2 vpx a pixel). The same
  // family as the bar's 7 x 7 wifi / ethernet / offline.

  // The fan: the dot, then the arcs, lit by `level` 1..3 like the 7 x 7 one.
  readonly property var wifiHero: [
    "...............",
    "....3333333....",
    "..333.....333..",
    ".33.........33.",
    "3.............3",
    "...............",
    "......222......",
    "....2222222....",
    "...22.....22...",
    "..2.........2..",
    "...............",
    "......111......",
    ".....11111.....",
    "......111......",
    "..............."
  ]
  // The hub and its two ports, outlined in ink and filled in the second tone
  // (draw at `level` 1 with `dim` muted).
  readonly property var ethernetHero: [
    "...............",
    ".....#####.....",
    ".....#222#.....",
    ".....#222#.....",
    ".....#####.....",
    ".......#.......",
    ".......#.......",
    "..###########..",
    "..#.........#..",
    "#####.....#####",
    "#222#.....#222#",
    "#222#.....#222#",
    "#####.....#####",
    "...............",
    "..............."
  ]
  // No link: the cross, in the accent tone.
  readonly property var offlineHero: [
    "...............",
    ".!...........!.",
    "..!.........!..",
    "...!.......!...",
    "....!.....!....",
    ".....!...!.....",
    "......!.!......",
    ".......!.......",
    "......!.!......",
    ".....!...!.....",
    "....!.....!....",
    "...!.......!...",
    "..!.........!..",
    ".!...........!.",
    "..............."
  ]

  // ---- 7 x 7, for the legend of a button.

  // Share the network as a QR code: three finder squares and a few data bits.
  readonly property var qr: [
    "###.###",
    "#.#.#.#",
    "###.###",
    ".......",
    "###.#.#",
    "#.#.##.",
    "###.#.#"
  ]
  // Run a speed test: a dial open at the foot, its needle past the middle.
  readonly property var gauge: [
    ".......",
    "..###..",
    ".#...#.",
    "#...#.#",
    "#..#..#",
    "#.....#",
    "##...##"
  ]
  // Forget a saved network.
  readonly property var forget: [
    "..###..",
    "#######",
    ".#.#.#.",
    ".#.#.#.",
    ".#.#.#.",
    ".#.#.#.",
    ".#####."
  ]
}
