import QtQuick
import "Q"

// The weather popup's icons. The condition mapping stays the stock one (Model.js
// hands out weather-icons glyphs): a glyph goes in and a sprite comes out, as the
// bar's WeatherSkin does it. The conditions are drawn at their own 21 x 21, beside
// the temperature and the days of the forecast: one pixel size to a surface, so
// never the bar's 9 x 7 scaled up. '#' outline, 'o' fill (the mid tone), '!' the accent (sun,
// lightning: pass `accent: Role.caution`).
QtObject {
  readonly property var pin: [
    "..###..",
    ".#####.",
    ".##.##.",
    ".#####.",
    "..###..",
    "...#...",
    "...#..."
  ]

  function forGlyph(glyph) {
    var cp = glyph && glyph.length > 0 ? glyph.codePointAt(0) : 0
    switch (cp) {
      case 0xe30d: return sun21
      case 0xe32b: return moon21
      case 0xe302: return cloudSun21
      case 0xe32e: return cloudMoon21
      case 0xe313: case 0xe346: return fog21
      case 0xe308: case 0xe333: return drizzle21
      case 0xe30a: case 0xe327: case 0xe318: return rain21
      case 0xe3ad: return sleet21
      case 0xe31d: return thunder21
      case 0xe31a: return snow21
      default: return cloud21
    }
  }

  readonly property var sun21: [
    ".....................",
    ".....................",
    "..........#..........",
    "..........#..........",
    ".....................",
    ".....#...###...#.....",
    ".......##!!!##.......",
    "......#!!!!!!!#......",
    "......#!!!!!!!#......",
    ".....#!!!!!!!!!#.....",
    "..##.#!!!!!!!!!#.##..",
    ".....#!!!!!!!!!#.....",
    "......#!!!!!!!#......",
    "......#!!!!!!!#......",
    ".......##!!!##.......",
    ".....#...###...#.....",
    ".....................",
    "..........#..........",
    "..........#..........",
    ".....................",
    "....................."
  ]
  readonly property var moon21: [
    ".....................",
    ".....................",
    ".....................",
    ".........#...........",
    ".......##............",
    "......#o#............",
    ".....#o#.............",
    ".....#o#.............",
    "....#oo#.............",
    "....#oo#.............",
    "....#ooo#............",
    "....#ooo#............",
    "....#oooo#........#..",
    ".....#oooo#......#...",
    ".....#ooooo#######...",
    "......#ooooooooo#....",
    ".......##ooooo##.....",
    ".........#####.......",
    ".....................",
    ".....................",
    "....................."
  ]
  readonly property var cloud21: [
    ".....................",
    ".....................",
    ".....................",
    ".....................",
    ".......######........",
    "......#oooooo#.......",
    ".....#oooooooo#......",
    "....#oooooooooo##....",
    "....#oooooooooooo##..",
    "...#ooooooooooooooo#.",
    "..#oooooooooooooooo#.",
    ".#ooooooooooooooooo#.",
    ".#ooooooooooooooooo#.",
    ".#ooooooooooooooooo#.",
    "..#ooooooooooooooo#..",
    "...#ooooooooooooo#...",
    "....#############....",
    ".....................",
    ".....................",
    ".....................",
    "....................."
  ]
  readonly property var cloudSun21: [
    "......#..............",
    "......#..............",
    "..#.......#..........",
    ".....###.............",
    "....#!!!#............",
    "...#!!!!!#...........",
    "##.#!!!!!#.###.......",
    "...#!!!!######.......",
    "....#!!#oooooo#......",
    ".....###ooooooo#.....",
    "..#....#oooooooo###..",
    ".....##oooooooooooo#.",
    "....#oooooooooooooo#.",
    "....#oooooooooooooo#.",
    "....#oooooooooooooo#.",
    "....#ooooooooooooo#..",
    ".....##oooooooooo#...",
    ".......###########...",
    ".....................",
    ".....................",
    "....................."
  ]
  readonly property var cloudMoon21: [
    ".....................",
    ".....#...............",
    "...##................",
    "..#o#................",
    "..#o#................",
    ".#oo#................",
    ".#oo#................",
    ".#ooo#..######.......",
    "..#ooo##oooooo#......",
    "..##ooo#ooooooo#.....",
    "....####oooooooo###..",
    ".....##oooooooooooo#.",
    "....#oooooooooooooo#.",
    "....#oooooooooooooo#.",
    "....#oooooooooooooo#.",
    "....#ooooooooooooo#..",
    ".....##oooooooooo#...",
    ".......###########...",
    ".....................",
    ".....................",
    "....................."
  ]
  readonly property var fog21: [
    ".....................",
    "........#............",
    "......##o###.........",
    ".....#oooooo#........",
    ".....#oooooo#........",
    "....#oooooooo###.....",
    "...#oooooooooooo#....",
    "..#ooooooooooooo#....",
    "..#ooooooooooooo#....",
    "..#ooooooooooooo#....",
    "...##oooooooooo#.....",
    ".....##########......",
    ".....................",
    ".#################...",
    ".....................",
    ".....................",
    "....ooooooooooooooooo",
    ".....................",
    "..#############......",
    ".....................",
    "....................."
  ]
  readonly property var drizzle21: [
    ".....................",
    ".....................",
    "......######.........",
    ".....#oooooo#........",
    "....#oooooooo#.......",
    "....#ooooooooo###....",
    "....#oooooooooooo#...",
    "..##ooooooooooooo#...",
    ".#oooooooooooooooo#..",
    ".#oooooooooooooooo#..",
    ".#oooooooooooooooo#..",
    ".#ooooooooooooooo#...",
    "..##oooooooooooo#....",
    "....############.....",
    ".....................",
    ".....................",
    ".......#.......#.....",
    "......#.......#......",
    "...........#.........",
    "..........#..........",
    "....................."
  ]
  readonly property var rain21: [
    ".....................",
    ".....................",
    "......######.........",
    ".....#oooooo#........",
    "....#oooooooo#.......",
    "....#ooooooooo###....",
    "....#oooooooooooo#...",
    "..##ooooooooooooo#...",
    ".#oooooooooooooooo#..",
    ".#oooooooooooooooo#..",
    ".#oooooooooooooooo#..",
    ".#ooooooooooooooo#...",
    "..##oooooooooooo#....",
    "....############.....",
    ".....................",
    ".....................",
    "......#.....#.....#..",
    "......#.....#.....#..",
    ".....#..#..#..#..#...",
    "........#.....#......",
    ".......#.....#......."
  ]
  readonly property var sleet21: [
    ".....................",
    ".....................",
    "......######.........",
    ".....#oooooo#........",
    "....#oooooooo#.......",
    "....#ooooooooo###....",
    "....#oooooooooooo#...",
    "..##ooooooooooooo#...",
    ".#oooooooooooooooo#..",
    ".#oooooooooooooooo#..",
    ".#oooooooooooooooo#..",
    ".#ooooooooooooooo#...",
    "..##oooooooooooo#....",
    "....############.....",
    ".....................",
    ".....................",
    ".......#......#......",
    ".......#......#......",
    "......#...#..#....#..",
    ".....................",
    "....................."
  ]
  readonly property var thunder21: [
    ".....................",
    ".....................",
    "......######.........",
    ".....#oooooo#........",
    "....#oooooooo#.......",
    "....#ooooooooo###....",
    "....#oooooooooooo#...",
    "..##ooooooooooooo#...",
    ".#oooooooooooooooo#..",
    ".#oooooooooooooooo#..",
    ".#oooooooooooooooo#..",
    ".#ooooooooooooooo#...",
    "..##oooooooooooo#....",
    "....############.....",
    "............!!.......",
    "...........!!........",
    "..........!!.........",
    ".........!!!!........",
    "..........!!.........",
    ".........!!..........",
    "........!!..........."
  ]
  readonly property var snow21: [
    ".....................",
    ".....................",
    "......######.........",
    ".....#oooooo#........",
    "....#oooooooo#.......",
    "....#ooooooooo###....",
    "....#oooooooooooo#...",
    "..##ooooooooooooo#...",
    ".#oooooooooooooooo#..",
    ".#oooooooooooooooo#..",
    ".#oooooooooooooooo#..",
    ".#ooooooooooooooo#...",
    "..##oooooooooooo#....",
    "....############.....",
    ".....................",
    ".....................",
    "......#........#.....",
    ".....###......###....",
    "......#...#....#.....",
    ".........###.........",
    "..........#.........."
  ]
}
