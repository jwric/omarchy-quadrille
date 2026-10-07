.pragma library

// The drawing is an event-built integer raster, uploaded once by Canvas.Image.
// Angle boundaries use fixed-point determinants, never shader trig or blending.
var pairs = 32
var directions = []
for (var k = 1; k < 16; k++) directions.push({
  x: Math.round(Math.cos(k * Math.PI / 32) * 1000000),
  y: Math.round(Math.sin(k * Math.PI / 32) * 1000000)
})

function wedge(x, y) {
  var ax = Math.abs(x), ay = Math.abs(y), sector = 0
  for (var i = 0; i < directions.length; i++)
    if (ay * directions[i].x >= ax * directions[i].y) sector++
  return (sector + ((x < 0) !== (y < 0) ? 1 : 0)) % 2 === 0
}

function overlaps(a, b, gap) {
  gap = gap || 0
  return a.x < b.x+b.w+gap && b.x < a.x+a.w+gap &&
    a.y < b.y+b.h+gap && b.y < a.y+a.h+gap
}

// A Canvas rounds its logical image size. Pad to the reduced fractional-scale
// numerator so its backing texture has whole virtual pixels even at 200/120.
function canvasLength(vpx, dpr) {
  var numerator=Math.round(dpr*120),a=numerator,b=120
  while(b) {var next=a%b;a=b;b=next}
  var stride=numerator/a
  return Math.ceil(vpx/stride)*120/a
}

function plan(input, physical, monitors, glyphs, topInset, big, composition) {
  composition = composition || "aperture"
  var ps = physical.pixelsPerVpx
  var w = Math.ceil(physical.widthPx/ps), h = Math.ceil(physical.heightPx/ps)
  var cx = Math.round(physical.widthPx/(2*ps)), cy = Math.round(physical.heightPx/(2*ps))
  var mx = physical.mmPerVpxX, my = physical.mmPerVpxY
  var strokes = [], labels = [], plates = [], targets = [], rings = [], bursts = [], xMarks = []
  var est = physical.estimated ? "~" : ""
  var compact = w<533 || h<480
  var top = Math.max(16, Math.round(topInset || 0))+16
  var dropped = []
  function rect(x,y,width,height,role) {
    x = Math.round(x); y = Math.round(y); width = Math.round(width); height = Math.round(height)
    var right = Math.min(w,x+width), bottom = Math.min(h,y+height)
    x = Math.max(0,x); y = Math.max(0,y)
    if (right>x && bottom>y) strokes.push({x:x,y:y,w:right-x,h:bottom-y,role:role})
  }
  function outline(x,y,width,height,role) {
    rect(x,y,width,1,role); rect(x,y+height-1,width,1,role)
    rect(x,y,1,height,role); rect(x+width-1,y,1,height,role)
  }
  function text(value,x,y,role,n) {
    value=String(value); n=n||1
    var box={text:value,x:Math.round(x),y:Math.round(y),w:glyphs.length(value)*6*n+1,h:12*n}
    var free=box.x>=24 && box.y>=top && box.x+box.w<=w-24 && box.y+box.h<=h-16
    for (var i=0;i<labels.length;i++) if(overlaps(box,labels[i],3)) free=false
    for (var i=0;i<targets.length;i++) if(overlaps(box,targets[i],3)) free=false
    if(!free) {dropped.push(value); return false}
    labels.push(box)
    var runs=n===1?glyphs.runs(value):big.rects(value,n)
    for(var i=0;i<runs.length;i++) rect(box.x+runs[i].x,box.y+runs[i].y,runs[i].w,runs[i].h||1,role||"ink")
    return true
  }
  function centred(value,y,role) {text(value,cx-Math.floor((glyphs.length(value)*6+1)/2),y,role)}
  function cross(x,y,size,role) {rect(x-size,y,2*size+1,1,role);rect(x,y-size,1,2*size+1,role)}
  // Runs merge adjacent pixels of one role. No bitmap contains intermediate tones.
  function raster(box, pixel) {
    for(var y=box.y;y<box.y+box.h;y++) {
      var start=box.x, last=pixel(start,y)
      for(var x=start+1;x<=box.x+box.w;x++) {
        var role=x<box.x+box.w?pixel(x,y):null
        if(role!==last) {if(last) rect(start,y,x-start,1,last);start=x;last=role}
      }
    }
  }
  function circle(x,y,rx,ry,role) {
    rx=Math.max(1,Math.round(rx));ry=Math.max(1,Math.round(ry))
    raster({x:x-rx,y:y-ry,w:2*rx+1,h:2*ry+1},function(px,py) {
      var dx=px-x,dy=py-y
      var outer=dx*dx*ry*ry+dy*dy*rx*rx<=rx*rx*ry*ry
      var ix=Math.max(0,rx-1),iy=Math.max(0,ry-1)
      var inner=ix>0&&iy>0&&dx*dx*iy*iy+dy*dy*ix*ix<ix*ix*iy*iy
      return outer&&!inner?role:null
    })
  }

  // The corner targets touch the true active-area corners, including partial
  // last virtual pixels. The inset frame states its actual percentage.
  var safe={x:Math.round(w*.02),y:Math.round(h*.02),w:w-2*Math.round(w*.02),h:h-2*Math.round(h*.02)}
  outline(safe.x,safe.y,safe.w,safe.h,"edge")
  for(var x of [0,Math.round(physical.widthPx/ps)])
    for(var y of [0,Math.round(physical.heightPx/ps)]) {circle(x,y,10,10,"line");cross(x,y,15,"accent")}
  var model=(String(input.make||"")+" "+String(input.model||"")).trim()
  text("QUADRILLE / "+(input.name||"OUTPUT"),40,top,"accent")
  text(model,40,top+18,"muted")
  var hz=Number(input.refreshRate)
  text(physical.widthPx+" x "+physical.heightPx+(hz>0?" / "+Math.round(hz)+" Hz":""),40,top+36,"ink")
  var ident=Math.max(0,monitors.findIndex(function(m){return m.input.name===input.name}))+1
  if(big && w>660) text((ident<10?"0":"")+ident,w-88,top,"accent",2)
  text("TEST CHART / "+composition.toUpperCase(),w-280,top+38,"muted")
  text("SAFE FRAME / 2%",w-280,top,"muted")
  text("SIEMENS / 32 PAIRS",w-280,top+18,"muted")

  var diameter=Math.max(10,Math.floor(Math.min(160,physical.widthMm*.4,physical.heightMm*.5,
    2*(h-138-cy)*my,2*(cy-top-66)*my)/10)*10)
  if(composition==="broadcast") diameter=Math.max(10,Math.floor(diameter*.55/10)*10)
  if(composition==="bench") diameter=Math.max(10,Math.floor(diameter*.8/10)*10)
  var rx=Math.round(diameter/(2*mx)),ry=Math.round(diameter/(2*my))
  var star={x:cx-rx,y:cy-ry,w:2*rx+1,h:2*ry+1,cx:cx,cy:cy,rx:rx,ry:ry,diameter:diameter,pairs:pairs}
  targets.push(star)
  var qx=Math.round(mx*10000),qy=Math.round(my*10000),radius=Math.round(diameter/2*10000)
  raster(star,function(x,y) {
    var dx=(x-cx)*qx,dy=(y-cy)*qy
    return dx*dx+dy*dy<=radius*radius?(wedge(dx,dy)?"ink":"void"):null
  })
  circle(cx,cy,rx,ry,"accent")
  for(var radiusMm of [diameter*.4,diameter*.25]) {
    var r={mm:radiusMm,rx:Math.round(radiusMm/mx),ry:Math.round(radiusMm/my),frequency:pairs/(2*Math.PI*radiusMm)}
    rings.push(r);circle(cx,cy,r.rx,r.ry,"line")
  }
  var panelRadius=pairs*physical.mmPerPixelX/Math.PI
  var gridRadius=pairs*mx/Math.PI
  circle(cx,cy,panelRadius/mx,panelRadius/my,"caution")
  circle(cx,cy,gridRadius/mx,gridRadius/my,"accent")
  cross(cx,cy,3,"accent")
  // Frequency annotations have reserved lanes above/below the target, never
  // over the pattern. Inner rings are distinguished by role in the legend.
  centred(est+diameter+" mm / ASPECT CHECK",cy+ry+8,"ink")
  for(var i=0;i<rings.length;i++)
    centred("R "+est+rings[i].mm+" mm / "+est+rings[i].frequency.toFixed(2)+" lp/mm",cy-ry-38+i*16,"muted")

  var periods=[{n:12,d:1},{n:6,d:1},{n:3,d:1},{n:2,d:1},{n:1,d:1},{n:2,d:ps}]
  function burst(x,y,bw,bh,index,vertical) {
    var period=periods[index], f=period.d/(period.n*(vertical?my:mx))
    var box={x:x,y:y,w:bw,h:bh,period:period,frequency:f,aliased:index>=4,vertical:!!vertical}
    targets.push(box);bursts.push(box)
    raster(box,function(px,py) {
      var at=vertical?py-y:px-x
      return Math.floor(2*at*period.d/period.n)%2===0?"ink":"void"
    })
    text(est+f.toFixed(2)+(index===3?" GRID":index===5?" PANEL":index===4?" ALIAS":" lp/mm"),x,y+bh+6,index>=4?"caution":"muted")
  }
  var flankW=Math.min(144,Math.floor((cx-rx-84))), left=48,right=w-48-flankW
  if(!compact && composition==="aperture") {
    text("BURST / lp/mm",left,cy-105,"muted")
    text("LIMITS / lp/mm",right,cy-105,"muted")
    for(var i=0;i<3;i++) {burst(left,cy-78+i*48,flankW,22,i,false);burst(right,cy-78+i*48,flankW,22,i+3,false)}
    text("LAST TWO ALIASED",right,cy+70,"caution")
  } else if(!compact && composition==="bench") {
    // Six three-bar groups in each wing: the orthogonal test is visible at once.
    var cellW=Math.max(24,Math.floor((cx-rx-104)/2)),cellH=30
    for(var wing=0;wing<2;wing++) for(var i=0;i<6;i++) {
      var x=wing===0?48+(i%2)*(cellW+16):w-48-2*cellW-16+(i%2)*(cellW+16)
      var y=cy-68+Math.floor(i/2)*56
      burst(x,y,cellW,cellH,i,i%2===1)
      outline(x-3,y-3,cellW+6,cellH+6,"edge")
    }
  } else if(!compact) {
    // A station-style bar field dominates; the centred star is a smaller seal.
    // A pair of Bayer ramps provides a second, strictly binary tonal language.
    var bayer=[0,8,2,10,12,4,14,6,3,11,1,9,15,7,13,5]
    var rampW=Math.max(32,Math.floor((cx-rx-104)/8)*8)
    for(var wing=0;wing<2;wing++) {
      var ramp={x:wing===0?48:w-48-rampW,y:cy-26,w:rampW,h:52}
      targets.push(ramp)
      raster(ramp,function(x,y) {
        var level=Math.floor((x-ramp.x)*16/ramp.w)
        return bayer[(y%4)*4+x%4]<level?"ink":"void"
      })
      text("BAYER / 0..15 OF 16",ramp.x,cy+34,"muted")
    }
  }

  var roleNames=["void","ground","raised","hover","edge","faint","muted","ink","accent","live","caution","alarm"]
  var paletteY=composition==="broadcast"?top+68:h-112
  var paletteH=composition==="broadcast"?Math.max(18,cy-ry-paletteY-78):18
  var palette={x:40,y:paletteY,w:w-80,h:paletteH+18,kind:"palette"}
  if(!compact) plates.push(palette)
  for(var i=0;!compact && i<roleNames.length;i++) {
    var x=40+Math.floor(i*(w-80)/12),end=40+Math.floor((i+1)*(w-80)/12)
    rect(x,paletteY,end-x-2,paletteH,roleNames[i])
    text(roleNames[i],x,paletteY+paletteH+6,"muted")
  }
  // The slant is exactly 1:12, atan(1/12)=4.76 degrees. Its stair steps are the test.
  // It occupies the lower central lane only when that lane is clear.
  var edge={x:composition==="broadcast"?cx-24:left,
    y:composition==="broadcast"?h-122:cy+(composition==="bench"?100:70),w:48,h:32,kind:"slanted"}
  var edgeFree=!compact
  for(var i=0;i<targets.length;i++) if(overlaps(edge,targets[i],18)) edgeFree=false
  for(var i=0;i<labels.length;i++) if(overlaps(edge,labels[i],3)) edgeFree=false
  for(var i=0;i<plates.length;i++) if(overlaps(edge,plates[i],3)) edgeFree=false
  if(edgeFree) {
    targets.push(edge)
    raster(edge,function(x,y){return 12*(x-edge.x-24)>=y-edge.y-16?"ink":"void"})
    text("EDGE 1:12",edge.x+58,edge.y+10,"muted")
  }

  var rulerX=40,rulerY=h-50
  var length=Math.min(100,Math.floor((w-370)*mx/10)*10)
  length=Math.max(10,length)
  var step=100
  for(var s of [10,20,50,100]) if(s/mx>=glyphs.length(est+length)*6+6) {step=s;break}
  for(var mm=0;mm<=length;mm++) {
    var at=rulerX+Math.round(mm/mx),tick=mm%10===0?10:mm%5===0?6:3
    rect(at,rulerY,1,tick,mm%10===0?"ink":"line")
    if(mm%10===0) {xMarks.push({mm:mm,vpx:at-rulerX});if(mm%step===0) text(est+mm,at-3,rulerY+14,"muted")}
  }
  rect(rulerX,rulerY,Math.round(length/mx)+1,1,"ink")
  text("1:1 / "+est+length+" mm / HOLD A RULER HERE",rulerX,rulerY-20,"ink")
  var noteX=Math.max(rulerX+Math.round(length/mx)+32,w-358,
    rulerX+glyphs.length("1:1 / "+est+length+" mm / HOLD A RULER HERE")*6+17)
  text("EDID "+(input.physicalWidth||0)+" x "+(input.physicalHeight||0)+" -> "+est+physical.widthMm.toFixed(1)+" x "+est+physical.heightMm.toFixed(1)+" mm",noteX,h-70,"muted")
  text((physical.source==="edid"?"INFERRED":physical.source.toUpperCase())+" / 1 vpx = "+est+mx.toFixed(2)+" mm / +/- "+est+(Math.ceil(mx/2*100)/100).toFixed(2),noteX,h-52,"muted")
  text("R GRID "+est+gridRadius.toFixed(1)+" / PANEL "+est+panelRadius.toFixed(1)+" mm / ALIAS INSIDE",noteX,h-34,"muted")
  return {width:w,height:h,pixelWidth:physical.widthPx,pixelHeight:physical.heightPx,
    strokes:strokes,labels:labels,plates:plates,targets:targets,star:star,rings:rings,bursts:bursts,
    panelRadius:panelRadius,gridRadius:gridRadius,dropped:dropped,safe:safe,xMarks:xMarks,
    ruler:{x:rulerX,y:rulerY,length:length,labelStep:step},composition:composition}
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
