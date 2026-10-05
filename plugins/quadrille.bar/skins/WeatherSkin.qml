import QtQuick
import "../Q"
import "../Q/Glyphs.js" as Glyphs

// omarchy.weather: a sprite for the condition and the temperature in text. Both
// come from the panel the stock widget loads beside its button (`label` is the
// weather-icons glyph it would have drawn, `reportTempNum` and `tempUnit` the
// reading), so the condition mapping stays the stock one: a glyph in, a sprite out.
Skin {
  id: root

  readonly property var panel: deep("reportTempNum")
  readonly property string glyph: panel ? String(panel.label || "") : ""
  readonly property string temp: panel && panel.reportTempNum !== "" ? panel.reportTempNum + String(panel.tempUnit || "") : ""

  // weather-icons codepoints (the ones Model.js hands out) to a sprite
  readonly property var sprite: {
    var cp = glyph.length > 0 ? glyph.codePointAt(0) : 0
    switch (cp) {
      case 0xe30d: return Sprites.brightness
      case 0xe32b: return Sprites.moon
      case 0xe302: return Sprites.cloudSun
      case 0xe32e: return Sprites.cloudMoon
      case 0xe313: case 0xe346: return Sprites.fog
      case 0xe308: case 0xe333: return Sprites.drizzle
      case 0xe30a: case 0xe327: case 0xe318: return Sprites.rain
      case 0xe3ad: return Sprites.sleet
      case 0xe31d: return Sprites.thunder
      case 0xe31a: return Sprites.snow
      default: return Sprites.cloud
    }
  }
  readonly property int spriteW: Sprites.width(sprite)

  visible: glyph !== ""
  implicitWidth: glyph === "" ? 0 : (spriteW + (temp !== "" ? 2 + Glyphs.length(temp) * 6 : 0) + 2 * pad) * g.unit

  Sprite {
    id: icon
    x: root.pad * root.g.unit
    y: root.g.centre(root.height, height)
    rows: root.sprite
    color: Role.ink
    dim: Role.faint
    accent: Role.caution
  }
  PixelText {
    visible: root.temp !== ""
    x: icon.x + icon.width + root.g.px(2)
    y: root.g.centre(root.height, height)
    text: root.temp
    ink: Role.ink
  }
}
