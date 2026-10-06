import QtQuick

// The clock popup's icons.
// Native sprites: one pixel size to a surface, so an icon that spans two lines of text
// is drawn at its own size, never a 7 x 7 scaled up (see "Sizes" in Q/Sprites.qml).
// '#' ink, 'o' the mid tone, '1'..'4' lit by `level`, '!' the accent.
QtObject {
  readonly property var calendar15: [
    "...............",
    "..##.......##..",
    "..##.......##..",
    ".#############.",
    ".#############.",
    ".#...........#.",
    ".#...........#.",
    ".#.o.o.o.o.o.#.",
    ".#...........#.",
    ".#.o.o.o.!!o.#.",
    ".#.......!!..#.",
    ".#.o.o.o.o.o.#.",
    ".#...........#.",
    ".#############.",
    "..............."
  ]
}
