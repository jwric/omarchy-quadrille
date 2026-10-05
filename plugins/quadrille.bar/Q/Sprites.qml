pragma Singleton
import QtQuick

// 7 x 7 pixel icons, drawn the way quadrille draws its own (icon.rs): odd
// sides, so there is a centre pixel, and no taller than the capitals of the
// body face.
//
// '#'  always lit
// '1'..'9'  lit when the sprite's `level` reaches that digit, otherwise dim:
//           signal bars, volume waves, a battery's charge cells
// '!'  lit in the sprite's `accent` colour (a mute cross, a charging bolt)
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
    "...#...",
    ".#.#.#.",
    "..###..",
    "...#...",
    "#######",
    "#.....#"
  ]
  readonly property var robot: [
    "...#...",
    "...#...",
    ".#####.",
    "##.#.##",
    ".#####.",
    ".#.#.#.",
    "......."
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
    "#...#....",
    ".#.##..#.",
    "..####.##",
    ".########",
    "#########",
    ".#######.",
    "........."
  ]
  readonly property var cloudMoon: [
    ".##......",
    "##.##....",
    "##.####.#",
    ".########",
    "#########",
    ".#######.",
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
  // rectangles: [{ x, y, w, kind }] with kind 0 lit, 1 dim, 2 accent.
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
    if (ch >= "1" && ch <= "9") return (ch.charCodeAt(0) - 48) <= level ? 0 : 1
    return -1
  }

  function width(rows) { return rows.length > 0 ? rows[0].length : 0 }
  function height(rows) { return rows.length }
}
