import QtQuick

// 7 x 7 icons for the Bluetooth panel: the radio, the kinds of device BlueZ
// names (its `icon`, a freedesktop icon name), and the row actions. A plain
// object made by Panel.qml, not a singleton, so it reloads with the plugin.
// The alphabet is Sprites.qml's: '#' lit, '1'..'9' lit by level, '!' accent.
QtObject {
  // ---- the radio
  readonly property var bluetooth: [
    "...#...",
    ".#.##..",
    "..##.#.",
    "...##..",
    "..##.#.",
    ".#.##..",
    "...#..."
  ]
  // A device is connected: the rune between two dots.
  readonly property var bluetoothLinked: [
    "...#...",
    ".#.##..",
    "..##.#.",
    "#..##.#",
    "..##.#.",
    ".#.##..",
    "...#..."
  ]

  // The same rune drawn for the head of the popup, two lines tall: 15 x 15 at
  // the surface's own pixel (not the 7 x 7 doubled), 1-px strokes in '#' and
  // the rune's two triangles in the mid tone ('o').
  readonly property var bluetooth15: [
    "...............",
    ".......#.......",
    ".......##......",
    "...#...#o#.....",
    "....#..#oo#....",
    ".....#.#o#.....",
    "......###......",
    ".......#.......",
    "......###......",
    ".....#.#o#.....",
    "....#..#oo#....",
    "...#...#o#.....",
    ".......##......",
    ".......#.......",
    "..............."
  ]
  // ... and with a device connected: a dot either side, haloed in the mid tone.
  readonly property var bluetoothLinked15: [
    "...............",
    ".......#.......",
    ".......##......",
    "...#...#o#.....",
    "....#..#oo#....",
    ".....#.#o#.....",
    ".o....###....o.",
    "o#o....#....o#o",
    ".o....###....o.",
    ".....#.#o#.....",
    "....#..#oo#....",
    "...#...#o#.....",
    ".......##......",
    ".......#.......",
    "..............."
  ]

  // ---- kinds of device
  readonly property var headphones: [
    ".#####.",
    "#.....#",
    "#.....#",
    "##...##",
    "##...##",
    "##...##",
    "......."
  ]
  // Headphones with a microphone on a boom.
  readonly property var headset: [
    ".#####.",
    "#.....#",
    "#.....#",
    "##...##",
    "##...##",
    "##...##",
    ".###..."
  ]
  readonly property var speaker: [
    ".#####.",
    "#.....#",
    "#..#..#",
    "#.....#",
    "#.###.#",
    "#.###.#",
    ".#####."
  ]
  readonly property var mouse: [
    "..###..",
    ".##.##.",
    ".##.##.",
    ".#####.",
    ".#####.",
    ".#####.",
    "..###.."
  ]
  readonly property var keyboard: [
    ".......",
    "#######",
    "#.#.#.#",
    "##.#.##",
    "#.....#",
    "#######",
    "......."
  ]
  readonly property var phone: [
    ".#####.",
    ".#...#.",
    ".#...#.",
    ".#...#.",
    ".#####.",
    ".##.##.",
    ".#####."
  ]
  readonly property var gamepad: [
    ".......",
    ".#####.",
    "##.##.#",
    "#...###",
    "##.#.##",
    "#######",
    "##...##"
  ]
  // A smart watch: the strap, the case and its crown.
  readonly property var watch: [
    "..###..",
    ".#####.",
    ".#...#.",
    ".#.#.##",
    ".#...#.",
    ".#####.",
    "..###.."
  ]
  readonly property var computer: [
    ".......",
    ".#####.",
    ".#...#.",
    ".#...#.",
    ".#####.",
    "#######",
    "......."
  ]
  readonly property var display: [
    "#######",
    "#.....#",
    "#.....#",
    "#.....#",
    "#######",
    "..###..",
    ".#####."
  ]
  readonly property var camera: [
    ".......",
    ".##.###",
    "#######",
    "#.###.#",
    "#.#.#.#",
    "#.###.#",
    "#######"
  ]

  // ---- row actions
  // Connect: two links joined.
  readonly property var link: [
    ".......",
    ".......",
    "###.###",
    "#.###.#",
    "###.###",
    ".......",
    "......."
  ]
  // Disconnect: the two links pulled apart.
  readonly property var unlink: [
    ".......",
    ".......",
    "##...##",
    "#.#.#.#",
    "##...##",
    ".......",
    "......."
  ]
  // Pair a new device.
  readonly property var plus: [
    "...#...",
    "...#...",
    "...#...",
    "#######",
    "...#...",
    "...#...",
    "...#..."
  ]
  // Forget (unpair).
  readonly property var trash: [
    "..###..",
    "#######",
    ".#.#.#.",
    ".#.#.#.",
    ".#.#.#.",
    ".#.#.#.",
    ".#####."
  ]
}
