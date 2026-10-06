import QtQuick

// Sprites of the display panel (see Q/Sprites.qml for the alphabet). A
// plain object, instantiated by Panel.qml, so it reloads with the plugin and no
// shared singleton is touched.
//
// '#' lit   '1'..'9' lit when the sprite's `level` reaches the digit   '!' accent
QtObject {
  // One screen on a stand: a row of the display list (the same drawing as the
  // audio panel's HDMI output).
  readonly property var display: [
    "#######",
    "#.....#",
    "#.....#",
    "#.....#",
    "#######",
    "..###..",
    ".#####."
  ]
  // The hero's icons: two lines tall, so drawn at 15 x 15 in the same hand
  // (outline lit, the glass in the second tone: '2' with level 1 and dim muted),
  // never a 7 x 7 scaled up. One screen on a stand (the stock panel's 󰍹)...
  readonly property var heroDisplay: [
    "###############",
    "#.............#",
    "#.22222222222.#",
    "#.22222222222.#",
    "#.22222222222.#",
    "#.22222222222.#",
    "#.22222222222.#",
    "#.22222222222.#",
    "#.22222222222.#",
    "#.............#",
    "###############",
    "###############",
    "......###......",
    "......###......",
    "...#########..."
  ]
  // ...and two, one behind the other with a pixel of air between (󰍺).
  readonly property var heroDisplays: [
    ".....##########",
    ".....#........#",
    ".....#.222222.#",
    "............2.#",
    "###########.2.#",
    "#.........#.2.#",
    "#.2222222.#...#",
    "#.2222222.#.###",
    "#.2222222.#.###",
    "#.2222222.#....",
    "#.........#....",
    "###########....",
    "###########....",
    "....###........",
    "..#######......"
  ]
  // A display that is on.
  readonly property var tick: [
    "......#",
    ".....##",
    "#...##.",
    "##.##..",
    ".###...",
    "..#....",
    "......."
  ]
}
