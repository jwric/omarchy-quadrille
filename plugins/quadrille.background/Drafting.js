.pragma library
.import "Pen.js" as Pen
.import "Star.js" as Star
.import "Portrait.js" as Portrait
.import "Construction.js" as Construction
.import "Samples.js" as Samples
.import "Materials.js" as Materials
.import "Detail.js" as Detail

// APERTURE: a technical portrait of one display, drawn on its own virtual-pixel
// grid. The plan is an event-built integer raster, uploaded once by Canvas.Image.
// Every part is a module drawing whole virtual pixels in theme roles through one
// Pen, which also keeps the collision record (a label that does not fit is
// dropped, never cut).
//
// Reading order: the star (the main drawing, centred on the screen, its diameter
// dimensioned as a millimetre reference); then the few labels on it and its two
// views, DETAIL A (the grid-limit disc on the panel's own pixels, top right) and
// B (one grid pixel taken apart, top left); then the technical details: the
// monitor's number and its place in the desktop layout (top left), the samples
// of what the grid does to fine stripes (right), the elevation of the active
// area (bottom left), the theme's materials and one note on the physical scale
// (along the bottom). The parts are placed first; the star is then the largest
// whole 10 mm that clears them, and a part that leaves no room for a useful star
// is dropped whole.
var pairs = Star.pairs
var wedge = Star.wedge
// The star's two tones. "faint" is the sheet's choice; the others are kept so
// the rejected options can be rendered again (wallpaper.py --explore).
var starTones = ["edge","faint","muted"]
var compositions = ["aperture"]

function overlaps(a, b, gap) { return Pen.overlaps(a, b, gap) }

function canvasLength(vpx, dpr) {
  var numerator=Math.round(dpr*120),a=numerator,b=120
  while(b) {var next=a%b;a=b;b=next}
  var stride=numerator/a
  return Math.ceil(vpx/stride)*120/a
}

// The display's name as it is worth printing: a model that says what it is
// (not a hex product code), else the maker; the maker is not repeated.
function displayName(input) {
  var make=String(input.make||"").trim(), model=String(input.model||"").trim()
  if(model && !/^0x[0-9a-f]+$/i.test(model)) {
    var first=make.split(/\s+/)[0]
    return first && model.toLowerCase().indexOf(first.toLowerCase())<0 && (make+" "+model).length<=24 ? make+" "+model : model
  }
  return make
}

// The one line of metadata: name / connector / pixels / refresh. Parts are
// dropped from the front (the name first) rather than the line being cut.
function metadata(input, physical, width, span) {
  var hz=Number(input.refreshRate)
  var meta=[displayName(input),input.name||"OUTPUT",physical.widthPx+" × "+physical.heightPx,hz>0?Math.round(hz)+" Hz":""]
    .filter(function(s){return s!==""})
  for(var k=0;k<meta.length;k++) {var line=meta.slice(k).join(" / "); if(span(line)<=width) return line}
  return ""
}

function scaleNote(physical) {
  return physical.source==="override"?"PHYSICAL SCALE FROM MEASURED SIZE":
    physical.estimated?"PHYSICAL SCALE ASSUMES 96 PIXELS PER INCH":"PHYSICAL SCALE ESTIMATED FROM PANEL SIZE"
}
// The mark-error bound: half a virtual pixel, rounded up to 0.01 mm.
function tolerance(S) { return "MARKS WITHIN "+S.est+(Math.ceil(Math.max(S.mx,S.my)/2*100)/100).toFixed(2)+" mm" }

function plan(input, physical, monitors, glyphs, topInset, big, composition, starTone) {
  starTone = starTones.indexOf(starTone)>=0 ? starTone : "faint"
  var ps=physical.pixelsPerVpx
  var w=Math.ceil(physical.widthPx/ps), h=Math.ceil(physical.heightPx/ps)
  var cx=Math.round(physical.widthPx/(2*ps)), cy=Math.round(physical.heightPx/(2*ps))
  var mx=physical.mmPerVpxX, my=physical.mmPerVpxY
  // The safe frame, then one gutter inside it on every side; the bar covers the top.
  var sx=Math.round(w*.02), sy=Math.round(h*.02), G=24
  var L=sx+G, R=w-sx-G, T=Math.max(sy,Math.max(16,Math.round(topInset||0)))+G, B=h-sy-G
  var pen=Pen.make(w,h,glyphs,big,{L:L,T:T,R:R,B:B})
  var current=Math.max(0,monitors.findIndex(function(m){return m.input.name===input.name}))
  var est=physical.estimated?"~":""
  var ctx={input:input,physical:physical,monitors:monitors,current:current,ps:ps,est:est,star:null}
  var S={w:w,h:h,cx:cx,cy:cy,L:L,T:T,R:R,B:B,ps:ps,mx:mx,my:my,est:est,pen:pen,ctx:ctx,
    input:input,physical:physical,monitors:monitors,current:current,tone:starTone,parts:{},boxes:{},omitted:[]}

  pen.outline(sx,sy,w-2*sx,h-2*sy,"edge")
  // The screen's corner pixels: a ring and a cross on each.
  for(var x of [0,Math.round(physical.widthPx/ps)])
    for(var y of [0,Math.round(physical.heightPx/ps)]) {pen.circle(x,y,10,10,"line");pen.cross(x,y,15,"accent")}

  compose(S)

  return {width:w,height:h,pixelWidth:physical.widthPx,pixelHeight:physical.heightPx,
    strokes:pen.strokes,labels:pen.labels,targets:pen.targets,leaders:pen.leaders,dropped:pen.dropped,
    omitted:S.omitted,star:S.star,parts:S.parts,boxes:S.boxes,
    gridRadius:Star.limitRadius()*mx,panelRadius:Star.limitRadius()*physical.mmPerPixelX,
    safe:{x:sx,y:sy,w:w-2*sx,h:h-2*sy},content:{x:L,y:T,w:R-L,h:B-T},
    ruler:S.ruler||null,xMarks:S.xMarks||[],composition:"aperture",plates:[]}
}

// ---- The parts' sizes -------------------------------------------------------

function cfg(S, extra) { return Object.assign({}, S.ctx, extra||{}) }
// A part sized before the star exists is measured against a provisional star:
// only the star's tones and a window's room depend on it.
function provisional(S, c) {
  var D=100, star={cx:S.cx,cy:S.cy,diameter:D,rx:Math.round(D/(2*S.mx)),ry:Math.round(D/(2*S.my)),high:S.tone,low:"ground"}
  return Object.assign({},S.ctx,c||{},{star:S.star||star})
}

// The identity: the monitor's number in the native large face, the locator of
// the desktop layout beside it, then the sheet's name knocked out and one line
// of metadata. Measured and drawn by the same function.
function identity(S, x, y, draw) {
  var pen=S.pen, n=S.current+1, numeral=(n<10?"0":"")+n
  var nw=pen.span(numeral,3), box={x:x,y:y,w:nw,h:36}, loc=null
  if(S.monitors.length>1) {
    var m=Portrait.measureLocator(S.ctx,72,36)
    if(m) {loc={x:x+nw+12,y:y+Math.max(0,Math.floor((36-m.h)/2)),w:m.w,h:m.h}; box.w=loc.x+m.w-x}
  }
  var nameW=pen.span("APERTURE")+4, my=y+36+12
  var room=Math.max(box.w,Math.min(S.R-x+1,Math.max(260,Math.floor((S.R-S.L)/2))))-nameW-8
  var line=metadata(S.input,S.physical,room,pen.span)
  box.w=Math.max(box.w,nameW+(line?8+pen.span(line):0)); box.h=my+12-y
  if(draw) {
    pen.text(numeral,x,y,"accent",3)
    if(loc) S.parts.locator=Portrait.drawLocator(pen,loc.x,loc.y,S.ctx,72,36)
    pen.inverse("APERTURE",x,my)
    if(line) pen.text(line,x+nameW+8,my,"muted")
    S.parts.identity={box:box,numeral:numeral,metadata:line}
  }
  return box
}

function footerLines(S, width) {
  var note=scaleNote(S.physical), lines=[tolerance(S)]
  if(S.pen.span(note)<=width) return lines.concat([note])
  var words=note.split(" "), cut=Math.ceil(words.length/2)
  return lines.concat([words.slice(0,cut).join(" "),words.slice(cut).join(" ")])
}
function footerSize(S, width) {
  var lines=footerLines(S,width), w=0
  for(var l of lines) w=Math.max(w,S.pen.span(l))
  return {w:w,h:18*lines.length-6,lines:lines}
}

// ---- The star ----------------------------------------------------------------

// The block the star needs at diameter D mm: its disc, then the reference under it.
function starBlock(S, D) {
  var rx=Math.round(D/(2*S.mx)), ry=Math.round(D/(2*S.my))
  return {rx:rx,ry:ry,x0:S.cx-rx,x1:S.cx-rx+Math.round(D/S.mx),bottom:S.cy+ry+(S.referenceBeside?29:46),
    label:S.referenceBeside?{x:S.cx-rx+Math.round(D/S.mx)+S.pen.span(S.est+D)+12,y:S.cy+ry+17,w:S.pen.span("ROUNDNESS  "+S.est+D+" mm REFERENCE"),h:12}:null}
}
// Does a box keep `gap` vpx clear of the star's disc and its reference?
function clearOfStar(S, D, b, gap) {
  var s=starBlock(S,D), r=Math.max(s.rx,s.ry)
  var nx=Math.max(b.x,Math.min(S.cx,b.x+b.w)), ny=Math.max(b.y,Math.min(S.cy,b.y+b.h))
  if(Math.hypot(nx-S.cx,ny-S.cy)<r+gap) return false
  // The reference block under the disc, its extension lines up its sides.
  var ref={x:s.x0-4,y:S.cy,w:s.x1-s.x0+9,h:s.bottom-S.cy}
  return !overlaps(b,ref,gap) && !(s.label && overlaps(b,s.label,gap))
}
// The largest whole 10 mm star (cap mm at most) whose disc and reference stay
// inside the content box and clear every placed part; 0 if none from 30 mm.
function largestStar(S, boxes, cap, gap, ok) {
  for(var D=cap;D>=30;D-=10) {
    var s=starBlock(S,D)
    if(S.cy-s.ry<S.T+4 || s.bottom>S.B || s.x0<S.L || s.x1>S.R || (s.label && s.label.x+s.label.w>S.R+1)) continue
    var clear=true
    for(var k in boxes) if(boxes[k] && !clearOfStar(S,D,boxes[k],gap)) {clear=false;break}
    if(clear && (!ok || ok(D))) return D
  }
  return 0
}

function starAt(S, D) {
  var rx=Math.round(D/(2*S.mx)), ry=Math.round(D/(2*S.my))
  var star={x:S.cx-rx,y:S.cy-ry,w:2*rx+1,h:2*ry+1,cx:S.cx,cy:S.cy,rx:rx,ry:ry,diameter:D,pairs:pairs,high:S.tone,low:"ground",kind:"star"}
  // The disc as row bands, so a label may sit in the corners of its box.
  for(var y0=star.y;y0<star.y+star.h;y0+=6) {
    var y1=Math.min(star.y+star.h,y0+6), half=0
    for(var yy=y0;yy<y1;yy++) {var dy=(yy-star.cy)/ry; if(Math.abs(dy)<=1) half=Math.max(half,Math.ceil(rx*Math.sqrt(1-dy*dy)))}
    S.pen.target({x:star.cx-half-1,y:y0,w:2*half+3,h:y1-y0,kind:"star"})
  }
  S.pen.raster(star,Star.gridSampler(star,S.physical))
  S.star=star; S.ctx.star=star
  return star
}
// The rim, and the grid-limit ring (the detail circle A): drawn after the
// leaders so both stay whole over them.
function starRings(S) {
  var st=S.star, g=Math.round(Star.limitRadius())
  S.pen.circle(st.cx,st.cy,st.rx,st.ry,"muted")
  S.pen.circle(st.cx,st.cy,g,g,"accent")
}
// The diameter as a dimension under the star, from its widest points, whose
// line is a millimetre scale: whole millimetres on the nearest virtual pixel.
// ROUNDNESS: the star should look round; its diameter is the reference.
function reference(S) {
  var pen=S.pen, st=S.star, D=st.diameter, x0=st.cx-st.rx, x1=x0+Math.round(D/S.mx), yd=st.cy+st.ry+12
  pen.vline(x0,st.cy+4,yd,"line"); pen.vline(x1,st.cy+4,yd,"line")
  pen.rect(x0,yd,x1-x0+1,1,"ink")
  var marks=[]
  for(var mm=0;mm<=D;mm++) {
    var at=x0+Math.round(mm/S.mx), tick=mm%10===0?7:mm%5===0?4:2
    pen.rect(at,yd-tick,1,tick,mm%10===0?"ink":"line")
    if(mm%10===0) marks.push({mm:mm,vpx:at-x0})
  }
  pen.target({x:x0,y:st.cy+st.ry+1,w:x1-x0+1,h:yd+1-st.cy-st.ry,kind:"reference"})
  var step=10
  while(step/S.mx<pen.span(S.est+D)+6) step*=2
  var numbers=[]
  for(var m of marks) if(m.mm%step===0 || m.mm===D) {
    var t=(m.mm>0?S.est:"")+m.mm, tx=x0+m.vpx-Math.floor((pen.span(String(m.mm))-1)/2)-(m.mm>0?S.est.length*6:0)
    if(pen.text(t,tx,yd+5,"muted")) numbers.push({mm:m.mm,text:t,x:tx})
  }
  var parts=[["ROUNDNESS","ink"],["  "+S.est+D+" mm REFERENCE","muted"]]
  var label=S.referenceBeside?pen.keyed(parts,x1+pen.span(S.est+D)+12,yd+5):pen.centred(parts,st.cx,yd+22)
  S.ruler={x:x0,y:yd,length:D,labelStep:step,numbers:numbers,label:label}; S.xMarks=marks
}

// DETAIL A's leader leaves the grid-limit ring at 45 degrees along the wedge
// boundary toward the view, out past the star's rim; then level to the view's
// rim when the view is beside its row, or up to the view's rim when the view is
// above. Returns null when no such route exists.
function detailRoutes(S, D, a) {
  var r=Math.round(D/(2*S.mx)), sx=a.cx>=S.cx?1:-1, g=Math.round(Star.limitRadius()/Math.SQRT2)+1
  var ay=Math.round(a.cy), t=S.cy-ay, routes=[]
  var rimX=sx>0?Math.floor(a.cx-a.rim)-2:Math.ceil(a.cx+a.rim)+2, start=[S.cx+sx*g,S.cy-g]
  // At 45 degrees to the view's row, then level to its rim.
  if(t>r/Math.SQRT2+8 && sx*(rimX-(S.cx+sx*t))>=10) routes.push([start,[S.cx+sx*t,ay],[rimX,ay]])
  // 45 degrees until under the view, as near its centre as leaves room to turn,
  // then straight up to its rim (never at the sides, where its own labels leave).
  for(var u=Math.round(sx*(a.cx-S.cx));u>r/Math.SQRT2+8;u--) {
    var xu=S.cx+sx*u, du=xu-a.cx
    if(Math.abs(du)>a.rim*0.5) break
    var rimY=Math.ceil(a.cy+Math.sqrt(a.rim*a.rim-du*du))+2
    if(S.cy-u>=rimY+4) {routes.push([start,[xu,S.cy-u],[xu,rimY]]); break}
  }
  // Out of the star at 45 degrees, up a column beside the view, then level to its rim.
  var te=sx*(rimX-S.cx)-10, xe=S.cx+sx*te, ye=S.cy-te
  if(te>=r/Math.SQRT2+6 && ye>ay+8) routes.push([start,[xe,ye],[xe,ay],[rimX,ay]])
  var t1=Math.ceil(r/Math.SQRT2)+10, x1=S.cx+sx*t1, y1=S.cy-t1, dx=x1-a.cx
  if(y1>a.cy+a.rim && Math.abs(dx)<=a.rim*0.7)
    routes.push([start,[x1,y1],[x1,Math.ceil(a.cy+Math.sqrt(a.rim*a.rim-dx*dx))+2]])
  return routes
}
function detailRoute(S, D, a) { var r=detailRoutes(S,D,a); return r.length?r[0]:null }
// Is a leader route clear (by 3 vpx) of every label already placed?
function routeClear(S, pts) {
  for(var i=1;i<pts.length;i++) {
    var x0=pts[i-1][0], y0=pts[i-1][1], x1=pts[i][0], y1=pts[i][1]
    var n=Math.max(Math.abs(x1-x0),Math.abs(y1-y0)), dx=Math.sign(x1-x0), dy=Math.sign(y1-y0)
    for(var k=0;k<=n;k++) {
      var b={x:x0+dx*k,y:y0+dy*k,w:1,h:1}
      for(var l of S.pen.labels) if(overlaps(b,l,3)) return false
    }
  }
  return true
}
function polyLeader(S, pts) {
  for(var i=1;i<pts.length;i++) if(pts[i][0]!==pts[i-1][0] || pts[i][1]!==pts[i-1][1]) S.pen.leader(pts[i-1][0],pts[i-1][1],pts[i][0],pts[i][1],"line")
  return pts
}
// B's leader: from its window marker on the star to B's coarse view, arriving
// level from the side the view faces: 45 degrees to the view's row then level
// when there is room, else 45 degrees to a column beside the view, up it, then in.
function windowRoute(mk, anchor, side) {
  var x0=side>0?mk.x-1:mk.x+mk.w, y0=mk.y+Math.floor(mk.h/2), xc=anchor.x+side*6
  var dy=anchor.y-y0, sy=Math.sign(dy)
  if(-side*(x0-xc)<=0 || side*(x0-xc)>=Math.abs(dy)) {
    var t=Math.abs(dy), x1=x0-side*t
    if(side*(x1-xc)>=0) return [[x0,y0],[x1,anchor.y],[anchor.x,anchor.y]]
  }
  // To the column xc first (45 degrees, toward the view's row), then along it.
  var top=mk.y-1, bottom=mk.y+mk.h, xs=mk.x+Math.floor(mk.w/2)
  var ys=sy<0?top:bottom, k=Math.abs(xc-xs), x1=xc, y1=ys+sy*k
  if(sy*(anchor.y-y1)<0) return null
  return [[xs,ys],[x1,y1],[x1,anchor.y],[anchor.x,anchor.y]]
}

// ---- The composition -----------------------------------------------------------

function compose(S) {
  var pen=S.pen, wide=S.R-S.L>=1200, tall=S.h>S.w
  var W=S.R-S.L+1
  // Every part's box first, in sheet order; a part that does not fit is omitted.
  var boxes={}
  function add(name, b) {
    if(!b) {S.omitted.push(name); return null}
    if(b.x<S.L || b.y<S.T || b.x+b.w>S.R+1 || b.y+b.h>S.B) {S.omitted.push(name); return null}
    for(var k in boxes) if(boxes[k] && overlaps(b,boxes[k],16)) {S.omitted.push(name); return null}
    boxes[name]=b; return b
  }
  var idb=identity(S,S.L,S.T,false)
  add("identity",idb)
  var cA=tall?{gridLeader:"nw",panelLeader:"sw",pitch:"above"}:{}, det=Detail.measure(provisional(S,cA))
  var cB={compact:!wide,mirror:true,boundary:36,at:0.8}, con=Construction.measure(provisional(S,cB))
  // The samples: on a wide sheet eight cells and the 1:1 patch, else six.
  var cS=null, smp=null
  for(var c of (wide?[{actual:true},{}]:[{compact:true}])) {
    var m=Samples.measure(cfg(S,c)); if(m && m.w<=(tall?W:W/2-24)) {cS=c; smp=m; break}
  }
  // The materials, bottom left; the footer, bottom right, in the room beside them.
  var cM=null, mat=null, minFoot=footerSize(S,0).w
  for(var c of (wide?[{}, {compact:true}]:[{compact:true}])) {
    var m=Materials.measure(cfg(S,c))
    if(m && m.w<=W-minFoot-24) {cM=c; mat=m; break}
  }
  if(tall) {
    // A tall sheet: B and DETAIL A side by side under the identity, the star,
    // then the samples and the elevation above the footer.
    var row=idb.y+idb.h+24
    var side=con&&det&&con.w+24+det.w<=W
    var detB=det&&add("detail",{x:S.R-det.w+1,y:row,w:det.w,h:det.h})
    var conB=con&&add("construction",{x:S.L,y:side||!detB?row:detB.y+detB.h+24,w:con.w,h:con.h})
    var foot=footerSize(S,W)
    var footB=add("footer",{x:S.R-foot.w+1,y:S.B-foot.h,w:foot.w,h:foot.h})
    var low=footB?footB.y-24:S.B
    var smpB=smp&&add("samples",{x:S.R-smp.w+1,y:low-smp.h,w:smp.w,h:smp.h})
    var ev=Portrait.measureElevation(S.ctx,Math.max(100,W-(smpB?smpB.w+24:0)),Math.min(150,S.B-(smpB?smpB.y:low)))
    if(ev) add("elevation",{x:S.L,y:S.B-ev.h,w:ev.w,h:ev.h}); else S.omitted.push("elevation")
    add("materials",null)
  } else {
    var detB=det&&add("detail",{x:S.R-det.w+1,y:S.T,w:det.w,h:det.h})
    var matB=mat?add("materials",{x:S.L,y:S.B-mat.h,w:mat.w,h:mat.h}):add("materials",null)
    var foot=footerSize(S,matB?W-matB.w-24:W)
    add("footer",{x:S.R-foot.w+1,y:S.B-foot.h,w:foot.w,h:foot.h})
    // B under the identity, its views facing the star; the samples under
    // DETAIL A; the elevation in the room between B and the materials.
    var conB=con&&add("construction",{x:S.L,y:idb.y+idb.h+24,w:con.w,h:con.h})
    var smpB=smp&&add("samples",{x:S.R-smp.w+1,y:detB?detB.y+detB.h+24:S.T,w:smp.w,h:smp.h})
    var ey=(conB?conB.y+conB.h:idb.y+idb.h)+24, eb=(matB?matB.y:S.B)-24, ew=Math.max(160,conB?conB.w:0,wide?300:0)
    var ev=eb-ey>=60?Portrait.measureElevation(S.ctx,Math.min(ew,Math.floor(W/2)-24),Math.min(eb-ey,150)):null
    if(ev) add("elevation",{x:S.L,y:eb-ev.h,w:ev.w,h:ev.h}); else S.omitted.push("elevation")
  }

  // The star: the largest that clears every part and leaves DETAIL A a route;
  // when that is under 60 mm a part makes way, the least needed first.
  var anchorA=null
  function leaderOk(D) { return !anchorA || detailRoute(S,D,anchorA)!==null }
  function best() {
    anchorA=boxes.detail?Detail.anchor(provisional(S,cA),boxes.detail.x,boxes.detail.y):null
    S.referenceBeside=false
    var D=largestStar(S,boxes,cap,16,leaderOk)
    if(!tall) {S.referenceBeside=true; var Db=largestStar(S,boxes,cap,16,leaderOk); if(Db>D) D=Db; else S.referenceBeside=false}
    return D
  }
  var cap=wide?180:120, D=best()
  var makeWay=["materials","elevation","samples","construction","detail"]
  while(D<60 && makeWay.length) {
    var k=makeWay.shift()
    if(boxes[k]) {delete boxes[k]; S.omitted.push(k); D=best()}
  }
  if(!D) D=30
  S.boxes=boxes
  starAt(S,D)

  // Draw every part in its box.
  identity(S,S.L,S.T,true)
  if(boxes.detail) S.parts.detail=Detail.draw(pen,boxes.detail.x,boxes.detail.y,cfg(S,cA))
  if(boxes.construction) S.parts.construction=Construction.draw(pen,boxes.construction.x,boxes.construction.y,cfg(S,cB))
  if(boxes.samples) S.parts.samples=Samples.draw(pen,boxes.samples.x,boxes.samples.y,cfg(S,cS))
  if(boxes.elevation) S.parts.elevation=Portrait.drawElevation(pen,boxes.elevation.x,boxes.elevation.y,S.ctx,boxes.elevation.w,boxes.elevation.h)
  if(boxes.materials) S.parts.materials=Materials.draw(pen,boxes.materials.x,boxes.materials.y,cfg(S,cM))
  if(boxes.footer) {
    S.parts.footer={lines:foot.lines}
    for(var i=0;i<foot.lines.length;i++) pen.textRight(foot.lines[i],S.R,boxes.footer.y+18*i,"muted")
  }
  // Leaders, then the star's rings over them, then the reference.
  if(boxes.detail) {
    var routes=detailRoutes(S,D,Detail.anchor(cfg(S,cA),boxes.detail.x,boxes.detail.y))
    var route=routes.filter(function(r){return routeClear(S,r)})[0]||routes[0]
    if(route) S.parts.detailLeader=polyLeader(S,route)
  }
  if(boxes.construction && S.parts.construction) {
    var mk=Construction.markWindow(pen,cfg(S,cB))
    var wr=mk?windowRoute(mk,S.parts.construction.viewAnchor,cB.mirror?1:-1):null
    if(mk) S.parts.window=mk
    if(wr) S.parts.windowLeader=polyLeader(S,wr)
  }
  starRings(S)
  reference(S)
}

function paint(ctx,drawing,phys,colors,background,dpr) {
  ctx.reset();ctx.scale(1/dpr,1/dpr)
  ctx.clearRect(0,0,drawing.pixelWidth,drawing.pixelHeight)
  if(background) {ctx.fillStyle=colors.void;ctx.fillRect(0,0,drawing.pixelWidth,drawing.pixelHeight)}
  var last=""
  for(var i=0;i<drawing.strokes.length;i++) {
    var s=drawing.strokes[i]
    if(last!==s.role) {ctx.fillStyle=colors[s.role];last=s.role}
    ctx.fillRect(s.x*phys,s.y*phys,s.w*phys,s.h*phys)
  }
}
