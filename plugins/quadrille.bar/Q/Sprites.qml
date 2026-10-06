pragma Singleton
import QtQuick

// 7 x 7 pixel icons, drawn the way quadrille draws its own (icon.rs): odd
// sides, so there is a centre pixel, and no taller than the capitals of the
// body face.
//
// Sizes. One pixel size to a surface: a sprite is drawn at the surface's virtual
// pixel, 1x, always. Different sizes of icon are different drawings, each made at
// its own size, with the detail those pixels allow (inner shapes, an outline, a
// mid tone) and of the same family as the 7 x 7 one:
//
//    7 x 7    beside a line of text, in a bar, in a row (the capitals are 8 high)
//   11 x 11   a row of two-line height, a badge
//   15 x 15   the head of a popup, two lines of text tall
//   21 x 21   a hero, three lines
//
// Odd sides, so there is a centre pixel. Name the others by size
// (`speaker`, `speaker15`), put them beside the small one, and never pass a
// `unit` to Sprite: it reports it (a mixel) and still draws it, for now.
//
// '#'  always lit
// '1'..'9'  lit when the sprite's `level` reaches that digit, otherwise dim:
//           signal bars, volume waves, a battery's charge cells
// '!'  lit in the sprite's `accent` colour (a mute cross, a charging bolt)
// 'o'  a mid tone, the sprite's `mid` (muted ink): the shading of a bigger icon
// ','  always dim, the sprite's `dim` (faint): a lit-off part that is never lit
// '.'  empty
QtObject {
  readonly property var wifi: [
    ".33333.",
    "3.....3",
    ".......",
    "..222..",
    ".2...2.",
    ".......",
    "...1..."
  ]
  // A wired link: a hub and its two ports.
  readonly property var ethernet: [
    "..###..",
    "..###..",
    "...#...",
    ".#####.",
    ".#...#.",
    "###.###",
    "###.###"
  ]
  readonly property var offline: [
    "!.....!",
    ".!...!.",
    "..!.!..",
    "...!...",
    "..!.!..",
    ".!...!.",
    "!.....!"
  ]
  readonly property var volume: [
    "...#..2",
    "..##.1.",
    "###.1.2",
    "###.1.2",
    "###.1.2",
    "..##.1.",
    "...#..2"
  ]
  readonly property var muted: [
    "...#...",
    "..##...",
    "###.!.!",
    "###..!.",
    "###.!.!",
    "..##...",
    "...#..."
  ]
  readonly property var microphone: [
    "..###..",
    "..###..",
    "..###..",
    "#.###.#",
    ".#####.",
    "...#...",
    "..###.."
  ]
  readonly property var bluetooth: [
    "...#...",
    ".#.##..",
    "..##.#.",
    "...##..",
    "..##.#.",
    ".#.##..",
    "...#..."
  ]
  // A horizontal cell: four columns of charge, `level` 1..4.
  readonly property var battery: [
    ".......",
    "######.",
    "#1234##",
    "#1234##",
    "#1234##",
    "######.",
    "......."
  ]
  readonly property var plug: [
    ".#...#.",
    ".#...#.",
    "#######",
    "#######",
    ".#####.",
    "...#...",
    "...#..."
  ]
  readonly property var cpu: [
    ".#.#.#.",
    "#######",
    "#.###.#",
    "#.###.#",
    "#.###.#",
    "#######",
    ".#.#.#."
  ]
  readonly property var clock: [
    "..###..",
    ".#.#.#.",
    "#..#..#",
    "#..####",
    "#.....#",
    ".#...#.",
    "..###.."
  ]
  readonly property var menu: [
    ".......",
    "#######",
    ".......",
    "#######",
    ".......",
    "#######",
    "......."
  ]
  readonly property var brightness: [
    "...#...",
    ".#...#.",
    "..###..",
    "#.###.#",
    "..###..",
    ".#...#.",
    "...#..."
  ]
  readonly property var bell: [
    "...#...",
    "..###..",
    ".#####.",
    ".#####.",
    ".#####.",
    "#######",
    "...#..."
  ]
  readonly property var warning: [
    "...#...",
    "..#.#..",
    "..#.#..",
    ".#.#.#.",
    ".#...#.",
    "#..#..#",
    "#######"
  ]
  readonly property var chevronLeft: [
    "....#..",
    "...##..",
    "..##...",
    ".##....",
    "..##...",
    "...##..",
    "....#.."
  ]
  readonly property var chevronRight: [
    "..#....",
    "..##...",
    "...##..",
    "....##.",
    "...##..",
    "..##...",
    "..#...."
  ]
  readonly property var chevronDown: [
    ".......",
    "##...##",
    ".##.##.",
    "..###..",
    "...#...",
    ".......",
    "......."
  ]
  readonly property var cross: [
    "#.....#",
    ".#...#.",
    "..#.#..",
    "...#...",
    "..#.#..",
    ".#...#.",
    "#.....#"
  ]
  readonly property var tick: [
    "......#",
    ".....##",
    "#...##.",
    "##.##..",
    ".###...",
    "..#....",
    "......."
  ]

  readonly property var microphoneMuted: [
    "!.###..",
    ".!###..",
    "..!##..",
    "#.#!#.#",
    ".###!#.",
    "...#.!.",
    "..###.!"
  ]
  readonly property var keyboard: [
    ".......",
    "#.#.#.#",
    ".......",
    "#.#.#.#",
    ".......",
    ".#####.",
    "......."
  ]
  readonly property var power: [
    "...#...",
    ".#.#.#.",
    "#..#..#",
    "#..#..#",
    "#.....#",
    ".#...#.",
    "..###.."
  ]
  readonly property var play: [
    ".#.....",
    ".##....",
    ".###...",
    ".####..",
    ".###...",
    ".##....",
    ".#....."
  ]
  readonly property var pause: [
    ".......",
    ".##.##.",
    ".##.##.",
    ".##.##.",
    ".##.##.",
    ".##.##.",
    "......."
  ]
  readonly property var next: [
    "#....#.",
    "##...#.",
    "###..#.",
    "####.#.",
    "###..#.",
    "##...#.",
    "#....#."
  ]
  readonly property var previous: [
    ".#....#",
    ".#...##",
    ".#..###",
    ".#.####",
    ".#..###",
    ".#...##",
    ".#....#"
  ]
  readonly property var lock: [
    "..###..",
    ".#...#.",
    ".#...#.",
    "#######",
    "###.###",
    "###.###",
    "#######"
  ]


  // ---- indicators: lit in a tone when the thing is on, faint when it is not.
  // A solid disc (the digit pixels are the fill, so an unlit one is a ring).
  readonly property var record: [
    "..###..",
    ".#111#.",
    "#11111#",
    "#11111#",
    "#11111#",
    ".#111#.",
    "..###.."
  ]
  readonly property var bellOff: [
    "!..#...",
    ".!##...",
    ".#!##..",
    ".##!#..",
    ".###!#.",
    "#####!#",
    "...#..!"
  ]
  readonly property var moon: [
    "..####.",
    ".###...",
    "###....",
    "###....",
    "###....",
    ".###..#",
    "..####."
  ]
  readonly property var coffee: [
    "..#.#..",
    ".#.#...",
    ".......",
    "#####..",
    "#####.#",
    "#####.#",
    ".###.#."
  ]
  readonly property var hourglass: [
    "#######",
    ".#####.",
    "..###..",
    "...#...",
    "..#.#..",
    ".#####.",
    "#######"
  ]
  // ---- bar widgets
  readonly property var update: [
    "...#...",
    "..###..",
    ".#####.",
    "...#...",
    "...#...",
    ".......",
    "#######"
  ]
  readonly property var robot: [
    "...#...",
    "#######",
    "#.###.#",
    "#######",
    "#.#.#.#",
    "#######",
    ".#...#."
  ]
  readonly property var note: [
    "...####",
    "...#..#",
    "...#..#",
    "...#...",
    ".###...",
    "####...",
    ".##...."
  ]
  readonly property var plus: [
    "...#...",
    "...#...",
    "...#...",
    "#######",
    "...#...",
    "...#...",
    "...#..."
  ]
  readonly property var minus: [
    ".......",
    ".......",
    ".......",
    "#######",
    ".......",
    ".......",
    "......."
  ]
  readonly property var pin: [
    "..###..",
    "..#.#..",
    "..###..",
    ".#####.",
    "...#...",
    "...#...",
    "...#..."
  ]
  readonly property var eye: [
    ".......",
    "..###..",
    ".#...#.",
    "#..#..#",
    ".#...#.",
    "..###..",
    "......."
  ]
  readonly property var eyeOff: [
    "!......",
    ".!###..",
    ".#!..#.",
    "#..!..#",
    ".#..!#.",
    "..###!.",
    "......!"
  ]
  readonly property var more: [
    ".......",
    ".......",
    ".......",
    "#.#.#.#",
    ".......",
    ".......",
    "......."
  ]
  // ---- weather: 9 x 7, a cloud with what it carries
  readonly property var cloud: [
    ".........",
    "...##....",
    "..####.##",
    ".########",
    "#########",
    ".#######.",
    "........."
  ]
  readonly property var cloudSun: [
    "..#......",
    ".###..##.",
    "####.####",
    ".#.######",
    "..#######",
    "..#######",
    "........."
  ]
  readonly property var cloudMoon: [
    "..##.....",
    ".##..##..",
    ".##.#####",
    "..#######",
    "..#######",
    "..#######",
    "........."
  ]
  readonly property var drizzle: [
    ".........",
    "...##....",
    "..####.##",
    ".########",
    "#########",
    "..#...#..",
    "........."
  ]
  readonly property var rain: [
    ".........",
    "...##....",
    "..####.##",
    ".########",
    "#########",
    ".#.#.#.#.",
    "#.#.#.#.."
  ]
  readonly property var snow: [
    ".........",
    "...##....",
    "..####.##",
    ".########",
    "#########",
    ".#...#...",
    "...#...#."
  ]
  readonly property var sleet: [
    ".........",
    "...##....",
    "..####.##",
    ".########",
    "#########",
    ".#.#...#.",
    "#.#..#.#."
  ]
  readonly property var thunder: [
    ".........",
    "...##....",
    "..####.##",
    ".########",
    "####!####",
    "..!!!....",
    "...!....."
  ]
  readonly property var fog: [
    ".........",
    ".#######.",
    ".........",
    "#########",
    ".........",
    ".#######.",
    "........."
  ]

  // The runs of lit pixels of `rows`, row by row, for a Repeater of
  // rectangles: [{ x, y, w, kind }] with kind 0 lit, 1 dim, 2 accent, 3 mid.
  function runs(rows, level) {
    var out = []
    for (var y = 0; y < rows.length; y++) {
      var row = rows[y]
      var x = 0
      while (x < row.length) {
        var kind = kindOf(row.charAt(x), level)
        if (kind < 0) { x++; continue }
        var start = x
        while (x < row.length && kindOf(row.charAt(x), level) === kind) x++
        out.push({ x: start, y: y, w: x - start, kind: kind })
      }
    }
    return out
  }

  function kindOf(ch, level) {
    if (ch === "#") return 0
    if (ch === "!") return 2
    if (ch === "o") return 3
    if (ch === ",") return 1
    if (ch >= "1" && ch <= "9") return (ch.charCodeAt(0) - 48) <= level ? 0 : 1
    return -1
  }

  // Sprite asks, when it finds itself drawn at a multiple of the pixel: true the
  // first time that size and factor are seen, so each is reported once.
  property var scaledSeen: ({})
  function noteScaled(rows, factor) {
    var key = width(rows) + "x" + height(rows) + "@" + factor.toFixed(2)
    if (scaledSeen[key]) return false
    scaledSeen[key] = true
    return true
  }

  function width(rows) { return rows.length > 0 ? rows[0].length : 0 }
  function height(rows) { return rows.length }
}
