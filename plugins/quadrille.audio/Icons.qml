import QtQuick

// The audio popup's hero icons.
// Native sprites: one pixel size to a surface, so an icon that spans two lines of text
// is drawn at its own size, never a 7 x 7 scaled up (see "Sizes" in Q/Sprites.qml).
// '#' ink, 'o' the mid tone, '1'..'4' lit by `level`, '!' the accent.
QtObject {
  readonly property var speaker15: [
    "...............",
    "........#......",
    ".......##......",
    "......#o#...o..",
    ".....#oo#....o.",
    ".####ooo#.1..o.",
    ".#.o#ooo#..1..o",
    ".#oo#ooo#..1..o",
    ".#.o#ooo#..1..o",
    ".####ooo#.1..o.",
    ".....#oo#....o.",
    "......#o#...o..",
    ".......##......",
    "........#......",
    "..............."
  ]
  readonly property var speakerMuted15: [
    "...............",
    "........#......",
    ".......##......",
    "......#o#......",
    ".....#oo#......",
    ".####ooo#.!...!",
    ".#.o#ooo#..!.!.",
    ".#oo#ooo#...!..",
    ".#.o#ooo#..!.!.",
    ".####ooo#.!...!",
    ".....#oo#......",
    "......#o#......",
    ".......##......",
    "........#......",
    "..............."
  ]
  readonly property var headphones15: [
    "...............",
    "....#######....",
    "...#.......#...",
    "..#.........#..",
    "..#.........#..",
    ".#...........#.",
    ".#...........#.",
    ".####.....####.",
    ".#oo#.....#oo#.",
    ".#oo#.....#oo#.",
    ".#oo#.....#oo#.",
    ".#oo#.....#oo#.",
    ".####.....####.",
    "...............",
    "..............."
  ]
}
