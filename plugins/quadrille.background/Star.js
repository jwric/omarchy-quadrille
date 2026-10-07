.pragma library

// The Siemens star, as one sampling function shared by every view of it: the
// sheet's star (whole virtual pixels), DETAIL A and the construction drawing
// (the panel's own pixels). Angle boundaries are fixed-point determinants,
// never trigonometry at draw time, so a view of the star reproduces the star's
// pixels exactly.
var pairs = 32
var directions = []
for (var k = 1; k < 16; k++) directions.push({
  x: Math.round(Math.cos(k * Math.PI / 32) * 1000000),
  y: Math.round(Math.sin(k * Math.PI / 32) * 1000000)
})

// Is the point (x, y) (any fixed-point unit, y down) in a lit wedge?
function wedge(x, y) {
  var ax = Math.abs(x), ay = Math.abs(y), sector = 0
  for (var i = 0; i < directions.length; i++)
    if (ay * directions[i].x >= ax * directions[i].y) sector++
  return (sector + ((x < 0) !== (y < 0) ? 1 : 0)) % 2 === 0
}

// The star's role at virtual pixel (x, y) of the sheet, or null outside its
// disc: exactly the pixels the sheet draws. star: {cx, cy, diameter, high, low}.
function gridSampler(star, physical) {
  var qx = Math.round(physical.mmPerVpxX*10000), qy = Math.round(physical.mmPerVpxY*10000)
  var radius = Math.round(star.diameter/2*10000)
  return function(x, y) {
    var dx = (x-star.cx)*qx, dy = (y-star.cy)*qy
    return dx*dx+dy*dy <= radius*radius ? (wedge(dx, dy) ? star.high : star.low) : null
  }
}

// The star drawn on the panel's own pixels: the role of the panel pixel (i, j)
// panel pixels right of and below the panel pixel at the star's centre (the
// top-left panel pixel of the centre virtual pixel). Used where the panel's
// pixels are shown enlarged (DETAIL A, the construction drawing).
function panelSampler(star, physical) {
  var pxq = Math.round(physical.mmPerPixelX*10000000), pyq = Math.round(physical.mmPerPixelY*10000000)
  return function(i, j) { return wedge(i*pxq, j*pyq) ? star.high : star.low }
}

// Where the star stops resolving: the radius at which a wedge pair is two
// pixels wide, in pixels of the grid in question (32 / pi).
function limitRadius() { return pairs/Math.PI }
