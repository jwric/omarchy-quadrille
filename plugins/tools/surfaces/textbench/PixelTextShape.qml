import QtQuick
import QtQuick.Shapes
import "quadrille.bar/Q"
import "Runs.js" as Big

// Variant 2: one Shape with one path of all the rectangles, in virtual pixels, scaled by the unit.
Item {
  id: root
  readonly property var g: Px.of(root)
  property string text: ""
  property color ink: Role.ink
  width: Array.from(text).length * g.cellW
  height: g.line
  Shape {
    scale: root.g.unit
    transformOrigin: Item.TopLeft
    preferredRendererType: Shape.GeometryRenderer
    ShapePath {
      fillColor: root.ink
      strokeColor: "transparent"
      strokeWidth: 0
      fillRule: ShapePath.WindingFill
      PathSvg { path: Big.svgPath(root.text) }
    }
  }
}
