pragma Singleton
import QtQuick

// 7 x 7 pixel icons for the popup panels (see Sprites.qml for the alphabet and
// for the bar's icons, Pictograms.qml for the overlays'). Kept apart so the three
// sets can grow without touching one another's file.
//
// '#' lit   '1'..'9' lit when the sprite's `level` reaches the digit   '!' accent
QtObject {
  // ---- power
  readonly property var bolt: [
    "....###",
    "...###.",
    "..###..",
    ".######",
    "..###..",
    ".###...",
    "###...."
  ]
  readonly property var leaf: [
    ".....##",
    "...####",
    "..#####",
    ".#####.",
    ".####..",
    "###....",
    "#......"
  ]
  readonly property var dial: [
    "..###..",
    ".#...#.",
    "#...#.#",
    "#..#..#",
    "#.....#",
    ".#...#.",
    "..###.."
  ]
  // ---- audio
  readonly property var headphones: [
    ".#####.",
    "#.....#",
    "#.....#",
    "##...##",
    "##...##",
    "##...##",
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
  readonly property var speaker: [
    ".#####.",
    "#.....#",
    "#..#..#",
    "#.....#",
    "#.###.#",
    "#.###.#",
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
}
