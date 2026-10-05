pragma Singleton
import QtQuick

// More 7 x 7 pixel icons, for the overlays (see Sprites.qml for the alphabet and
// for the icons of the bar). Kept apart so the bar's set and this one can grow
// without touching each other's file.
//
// '#' lit   '1'..'9' lit when the sprite's `level` reaches the digit   '!' accent
QtObject {
  // A touchpad: the pad, and the two buttons under it.
  readonly property var touchpad: [
    "#######",
    "#.....#",
    "#.....#",
    "#.....#",
    "#######",
    "#..#..#",
    "#######"
  ]
  // A touch screen, and the point that is touched.
  readonly property var touchscreen: [
    "#######",
    "#.....#",
    "#..!..#",
    "#.!!!.#",
    "#..!..#",
    "#######",
    "..###.."
  ]
  readonly property var download: [
    "...#...",
    "...#...",
    ".#.#.#.",
    "..###..",
    "...#...",
    ".......",
    "#######"
  ]
  // A magnifier: nothing found, or something to search for.
  readonly property var search: [
    ".###...",
    "#...#..",
    "#...#..",
    "#...#..",
    ".###...",
    ".....#.",
    "......#"
  ]
  readonly property var clipboard: [
    "..###..",
    "#.###.#",
    "#.....#",
    "#.###.#",
    "#.....#",
    "#.###.#",
    "#######"
  ]
  readonly property var smile: [
    "..###..",
    ".#...#.",
    "#.#.#.#",
    "#.....#",
    "#.#.#.#",
    ".#.#.#.",
    "..###.."
  ]
  readonly property var image: [
    "#######",
    "#...#.#",
    "#.....#",
    "#..#..#",
    "#.###.#",
    "###.###",
    "#######"
  ]
  readonly property var folder: [
    "###....",
    "#######",
    "#.....#",
    "#.....#",
    "#.....#",
    "#.....#",
    "#######"
  ]
  readonly property var file: [
    "#####..",
    "#...##.",
    "#...###",
    "#.....#",
    "#.....#",
    "#.....#",
    "#######"
  ]
  readonly property var text: [
    "#######",
    ".......",
    "#####..",
    ".......",
    "#######",
    ".......",
    "####..."
  ]
  // A window with a title bar: an application, when we do not know which.
  readonly property var app: [
    "#######",
    "#.#...#",
    "#######",
    "#.....#",
    "#.....#",
    "#.....#",
    "#######"
  ]
  readonly property var key: [
    ".###...",
    "#...#..",
    "#...#..",
    ".###...",
    "..#....",
    "..##...",
    "..#.#.."
  ]
  readonly property var shield: [
    "#######",
    "#..#..#",
    "#..#..#",
    "#.###.#",
    ".#...#.",
    "..#.#..",
    "...#..."
  ]
  readonly property var trash: [
    "..###..",
    "#######",
    ".#.#.#.",
    ".#.#.#.",
    ".#.#.#.",
    ".#.#.#.",
    ".#####."
  ]
  readonly property var sun: [
    "...#...",
    ".#...#.",
    "..###..",
    "#.###.#",
    "..###..",
    ".#...#.",
    "...#..."
  ]
}
