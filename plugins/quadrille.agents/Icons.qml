import QtQuick

// The agents popup's own icons (alphabet: see ../quadrille.bar/Q/Sprites.qml): the
// provider marks, which the stock panel draws from assets/<id>.svg as antialiased
// pictures, redrawn as sprites in the theme's roles. 7 x 7 for a line of text (the
// tabs), 15 x 15 for the two-line hero.
QtObject {
  // Claude: the starburst, its rays joined at the centre.
  readonly property var claude: [
    "...#...",
    ".#.#.#.",
    "..###..",
    "#######",
    "..###..",
    ".#.#.#.",
    "...#..."
  ]
  // Codex: the cloud with a prompt (> _) knocked out of it.
  readonly property var codex: [
    ".#####.",
    "#######",
    "#.#####",
    "##.####",
    "#.##..#",
    "#######",
    ".#####."
  ]
  // Fireworks: eight rays standing off a centre spark.
  readonly property var fireworks: [
    "#..#..#",
    ".#.#.#.",
    ".......",
    "##.#.##",
    ".......",
    ".#.#.#.",
    "#..#..#"
  ]

  // ---- 15 x 15, for the hero (two lines tall): the same marks drawn at that
  // size, not the small ones scaled. '#' ink, '2' muted (Sprite level 1, dim muted).
  // Claude: twelve rays, long and short, joined in a solid core.
  readonly property var claude15: [
    ".......#.......",
    "....#..#..#....",
    ".....#.#.#.....",
    ".....#.#.#.....",
    ".#....###....#.",
    "..##..###..##..",
    "....#######....",
    "###############",
    "....#######....",
    "..##..###..##..",
    ".#....###....#.",
    ".....#.#.#.....",
    ".....#.#.#.....",
    "....#..#..#....",
    ".......#......."
  ]
  // Codex: the cloud, outlined in ink and filled muted, its prompt (> _) in ink.
  readonly property var codex15: [
    "....###.###....",
    "..##222#222##..",
    ".#22222222222#.",
    ".#22222222222#.",
    "#222222222222#.",
    "#22#222222222##",
    "#222#222222222#",
    "#2222#22222222#",
    "#222#222222222#",
    "#22#222####222#",
    ".#22222222222#.",
    ".#22222222222#.",
    "..##2222222##..",
    "....##222##....",
    "......###......"
  ]
  // Fireworks: eight rays off a centre spark, their inner ends muted.
  readonly property var fireworks15: [
    ".......#.......",
    ".#.....#.....#.",
    "..#....#....#..",
    "...#...#...#...",
    "....2..2..2....",
    "...............",
    ".......#.......",
    "####2.###.2####",
    ".......#.......",
    "...............",
    "....2..2..2....",
    "...#...#...#...",
    "..#....#....#..",
    ".#.....#.....#.",
    ".......#......."
  ]
  // An agent without a mark: the bar's robot, its face muted.
  readonly property var robot15: [
    ".......#.......",
    "......###......",
    ".......#.......",
    "..###########..",
    ".#22222222222#.",
    ".#22###2###22#.",
    "##22###2###22##",
    "##22222222222##",
    ".#22222222222#.",
    ".#222#####222#.",
    ".#22222222222#.",
    "..###########..",
    "....#.....#....",
    "...###...###...",
    "..............."
  ]
}
