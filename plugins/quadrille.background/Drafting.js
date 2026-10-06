.pragma library

// Integer virtual-pixel strokes. The Canvas and the headless checks consume the
// same plan; the font contributes its native bitmap runs, never scaled glyphs.
function mark(mm, pixels, pxPerMm, phys) {
  return Math.round((pixels / 2 + mm * pxPerMm) / phys)
}

function layout(monitors) {
  if (!monitors.length) return []
  var left = monitors.slice()
  left.sort(function(a, b) {
    return Math.hypot(a.input.x || 0, a.input.y || 0) - Math.hypot(b.input.x || 0, b.input.y || 0)
  })
  var first = left.shift()
  var placed = [{ monitor: first, x: 0, y: 0 }]
  function gap(a, b, axis) {
    var pos = axis === "x" ? "x" : "y"
    var size = axis === "x" ? "widthPx" : "heightPx"
    var ap = a.input[pos] || 0, bp = b.input[pos] || 0
    var aw = a.physical[size] / (a.input.scale || 1)
    var bw = b.physical[size] / (b.input.scale || 1)
    return Math.max(0, bp - ap - aw, ap - bp - bw)
  }
  function coordinate(a, b, origin, axis) {
    var size = axis === "x" ? "widthPx" : "heightPx"
    var mm = axis === "x" ? "widthMm" : "heightMm"
    var ratio = axis === "x" ? "mmPerLogicalPixelX" : "mmPerLogicalPixelY"
    var ap = a.input[axis] || 0, bp = b.input[axis] || 0
    var aw = a.physical[size] / (a.input.scale || 1)
    var bw = b.physical[size] / (b.input.scale || 1)
    if (bp >= ap + aw - 0.01) return origin + a.physical[mm] + (bp - ap - aw) * b.physical[ratio]
    if (bp + bw <= ap + 0.01) return origin - b.physical[mm] - (ap - bp - bw) * b.physical[ratio]
    return origin + (bp - ap) * b.physical[ratio]
  }
  while (left.length) {
    var best = null
    for (var i = 0; i < left.length; i++) {
      for (var j = 0; j < placed.length; j++) {
        var a = placed[j].monitor, b = left[i]
        var distance = Math.hypot(gap(a, b, "x"), gap(a, b, "y"))
        if (!best || distance < best.distance) best = { i: i, j: j, distance: distance }
      }
    }
    var near = placed[best.j], next = left.splice(best.i, 1)[0]
    placed.push({ monitor: next,
      x: coordinate(near.monitor, next, near.x, "x"),
      y: coordinate(near.monitor, next, near.y, "y") })
  }
  return placed
}

function overlaps(a, b, gap) {
  gap = gap || 0
  return a.x < b.x + b.w + gap && b.x < a.x + a.w + gap &&
    a.y < b.y + b.h + gap && b.y < a.y + a.h + gap
}

// Three deliberately different sheet arrangements share one physical drawing.
// Only "atlas" is used by the desktop; the others remain renderable for review.
function plan(input, physical, monitors, glyphs, topInset, big, composition) {
  var ps = physical.pixelsPerVpx
  var w = Math.floor(physical.widthPx / ps), h = Math.floor(physical.heightPx / ps)
  var strokes = [], labels = [], plates = [], bodies = [], dimensions = []
  var est = physical.estimated ? "~" : ""
  var top = Math.max(16, Math.round(topInset || 0)) + 16
  composition = composition || "atlas"
  function rect(x, y, width, height, role) {
    var s = {x: Math.round(x), y: Math.round(y), w: Math.round(width), h: Math.round(height), role: role}
    if (s.w > 0 && s.h > 0 && s.x >= 0 && s.y >= 0 && s.x+s.w <= w && s.y+s.h <= h) strokes.push(s)
  }
  function outline(x,y,width,height,role) {
    rect(x,y,width,1,role); rect(x,y+height-1,width,1,role)
    rect(x,y,1,height,role); rect(x+width-1,y,1,height,role)
  }
  function text(value,x,y,role,n) {
    value = String(value); n = n || 1
    var width = glyphs.length(value)*6*n+n, height = 12*n
    var box = {text:value, x:Math.round(x), y:Math.round(y), w:width, h:height, room:width}
    if (box.x < 24 || box.y < top || box.x+width > w-24 || box.y+height > h-24) return false
    for (var i=0;i<labels.length;i++) if (overlaps(box,labels[i],2)) return false
    labels.push(box)
    var runs = n===1 ? glyphs.runs(value) : big.rects(value,n)
    for (var i=0;i<runs.length;i++) rect(box.x+runs[i].x,box.y+runs[i].y,runs[i].w,runs[i].h||1,role||"ink")
    return true
  }
  function cross(x,y,role) { rect(x-2,y,5,1,role); rect(x,y-2,1,5,role) }
  // Registration only: no wallpaper-wide lattice or fictitious physical bezel.
  outline(12,top-8,w-24,h-top-4,"edge")
  for (var x=50;x<w-30;x+=100) rect(x,h-15,1,7,"edge")
  for (var y=top+50;y<h-30;y+=100) rect(9,y,7,1,"edge")
  text("QUADRILLE / LIVE DIMENSIONING",32,top,"ink")
  var index = Math.max(0,monitors.findIndex(function(m){return m.input.name===input.name}))+1
  text((index<10?"0":"")+index+" / "+monitors.length,w-92,top,"accent")
  text("ACTIVE AREA / COMPOSITOR ELEVATION",32,top+18,"muted")

  var narrow = w<740
  var rulerY = h-91, rulerX = 40
  var heroX = narrow ? w-180 : w-260, heroY = top+98
  var figureX = 48, figureY = top+92
  var budgetW = narrow ? w-150 : Math.floor(w*.44)
  var budgetH = narrow ? h-370 : h-290
  if (composition==="comparator") {
    figureX=Math.floor(w*.25); figureY=top+115; budgetW=Math.floor(w*.48)
    heroX=40; heroY=top+60; rulerY=top+62; rulerX=w-Math.round(100/physical.mmPerVpxX)-50
  } else if (composition==="section") {
    figureX=Math.floor(w*.48); figureY=top+128; budgetW=Math.floor(w*.36)
    heroX=48; heroY=top+110; rulerY=h-91
  }
  if (narrow) { heroX=w-180; heroY=top+45; figureX=48; figureY=top+225; budgetW=w-150; budgetH=Math.max(55,h-460); rulerX=40; rulerY=h-91 }
  if (physical.estimated) text("~",heroX-12,heroY+16,"accent")
  text(physical.widthMm.toFixed(1),heroX,heroY,"accent",big?3:1)
  text("mm / ACTIVE WIDTH",heroX,heroY+40,"ink")
  text(est+physical.heightMm.toFixed(1)+" mm HIGH",heroX,heroY+60,"muted")
  text(input.name||"OUTPUT",heroX,heroY+86,"ink")
  var model=(String(input.make||"")+" "+String(input.model||"")).trim()
  var modelLines=glyphs.wrap(model,Math.floor((w-24-heroX)/6),2)
  for (var i=0;i<modelLines.length;i++) text(modelLines[i],heroX,heroY+104+i*14,"muted")

  budgetH=Math.min(budgetH,rulerY-figureY-112-14*monitors.length)
  var placed=layout(monitors), minX=Infinity,minY=Infinity,maxX=-Infinity,maxY=-Infinity
  for(var i=0;i<placed.length;i++) {
    var a=placed[i], p=a.monitor.physical
    minX=Math.min(minX,a.x); minY=Math.min(minY,a.y)
    maxX=Math.max(maxX,a.x+p.widthMm); maxY=Math.max(maxY,a.y+p.heightMm)
  }
  var mmPerVpx=Math.max(1,Math.ceil((maxX-minX)/Math.max(40,budgetW)),Math.ceil((maxY-minY)/Math.max(50,budgetH)))
  var dw=Math.round((maxX-minX)/mmPerVpx), dh=Math.round((maxY-minY)/mmPerVpx)
  for(var i=0;i<placed.length;i++) {
    var a=placed[i], p=a.monitor.physical, here=a.monitor.input.name===input.name
    var b={x:figureX+Math.round((a.x-minX)/mmPerVpx),y:figureY+Math.round((a.y-minY)/mmPerVpx),
      w:Math.max(2,Math.round(p.widthMm/mmPerVpx)),h:Math.max(2,Math.round(p.heightMm/mmPerVpx)),here:here,name:a.monitor.input.name}
    bodies.push(b)
    outline(b.x,b.y,b.w,b.h,here?"accent":"line")
    // A sparse 50 mm registration patch belongs to the current panel only.
    if(here) for(var my=50;my<p.heightMm-20;my+=50) for(var mx=50;mx<p.widthMm-20;mx+=50)
      cross(b.x+Math.round(mx/mmPerVpx),b.y+Math.round(my/mmPerVpx),"edge")
  }
  // Dimensions may occupy an exposed side only. The keyed schedule below is
  // always complete, including layouts where another panel blocks both sides.
  var occupied=bodies.slice()
  for(var i=0;i<bodies.length;i++) {
    var b=bodies[i], p=placed[i].monitor.physical
    for(var axis of ["w","h"]) {
      var value=(p.estimated?"~":"")+(axis==="w"?p.widthMm:p.heightMm).toFixed(1)+" mm"
      var tw=glyphs.length(value)*6+1, accepted=false
      for(var side=0;side<2&&!accepted;side++) {
        var lines=[], label
        if(axis==="w") {
          var at=side===0?b.y-12:b.y+b.h+12
          label={x:b.x+Math.floor((b.w-tw)/2),y:side===0?at-15:at+4,w:tw,h:12}
          lines=[{x:b.x,y:at,w:b.w,h:1},{x:b.x,y:Math.min(at-3,b.y-2),w:1,h:Math.abs(at-b.y)+6},
            {x:b.x+b.w-1,y:Math.min(at-3,b.y-2),w:1,h:Math.abs(at-b.y)+6}]
          if(side===1) { lines[1].y=b.y+b.h+2; lines[2].y=lines[1].y; lines[1].h=14; lines[2].h=14 }
        } else {
          var at=side===0?b.x+b.w+12:b.x-12
          label={x:side===0?at+5:at-tw-5,y:b.y+Math.floor((b.h-12)/2),w:tw,h:12}
          lines=[{x:at,y:b.y,w:1,h:b.h},{x:side===0?b.x+b.w+2:at-3,y:b.y,w:14,h:1},
            {x:side===0?b.x+b.w+2:at-3,y:b.y+b.h-1,w:14,h:1}]
        }
        var all=lines.concat([label]), free=true
        for(var c=0;c<all.length;c++) {
          var box=all[c]
          if(box.x<26||box.y<top+40||box.x+box.w>w-26||box.y+box.h>h-150) free=false
          for(var o=0;o<occupied.length;o++) if(overlaps(box,occupied[o],1)) free=false
          for(var o=0;o<labels.length;o++) if(overlaps(box,labels[o],3)) free=false
        }
        if(!free) continue
        for(var l=0;l<lines.length;l++) rect(lines[l].x,lines[l].y,lines[l].w,lines[l].h,"line")
        if(axis==="w") for(var t=-3;t<=3;t++) {rect(b.x+t,at+t,1,1,"line");rect(b.x+b.w-1+t,at+t,1,1,"line")}
        else for(var t=-3;t<=3;t++) {rect(at+t,b.y+t,1,1,"line");rect(at+t,b.y+b.h-1+t,1,1,"line")}
        if(text(value,label.x,label.y,"ink")) dimensions.push({label:label,lines:lines,axis:axis,output:b.name})
        occupied=occupied.concat(all); accepted=true
      }
    }
  }
  var legendY=figureY+dh+38
  text("1:"+(mmPerVpx/physical.mmPerVpxX).toFixed(1)+" / ACTIVE RECTANGLES",figureX,legendY,"muted")
  for(var i=0;i<placed.length;i++) {
    var p=placed[i].monitor.physical, name=placed[i].monitor.input.name
    text((name===input.name?"> ":"  ")+name+"  "+(p.estimated?"~":"")+p.widthMm.toFixed(1)+" x "+p.heightMm.toFixed(1)+" mm",figureX,legendY+18+i*14,"muted")
  }
  text("Offsets follow compositor",figureX,legendY+22+placed.length*14,"muted")

  // True-size reference: absolute mm locations rounded individually, including
  // the origin. The annotation states raster error, never calibration accuracy.
  var length=Math.min(100,Math.floor((w-80)*physical.mmPerVpxX/10)*10)
  var labelStep=100
  for(var step of [10,20,50,100]) {
    if(step/physical.mmPerVpxX >= glyphs.length(est+Math.max(0,length-step))*6+3) { labelStep=step; break }
  }
  var xMarks=[], yMarks=[]
  for(var mm=0;mm<=length;mm++) {
    var at=rulerX+Math.round(mm/physical.mmPerVpxX)
    var tick=mm%10===0?10:mm%5===0?6:3
    rect(at,rulerY,1,tick,mm%10===0?"ink":"line")
    if(mm%10===0) {
      xMarks.push({mm:mm,vpx:at-rulerX})
      if(mm%labelStep===0) text(est+mm,at-3,rulerY+14,"muted")
    }
  }
  var span=Math.round(length/physical.mmPerVpxX)
  rect(rulerX,rulerY,span+1,1,"ink")
  var caption="HOLD A RULER HERE / "+est+length+" mm / 1:1"
  text(caption,Math.min(rulerX,w-26-glyphs.length(caption)*6-1),rulerY-22,"ink")
  var noteX=narrow?w-218:(composition==="comparator"?w-318:Math.max(rulerX+span+36,w-318)), noteY=narrow?h-40:h-102
  text("1 vpx = "+est+physical.mmPerVpxX.toFixed(2)+" mm",noteX,noteY,"muted")
  if(!narrow) {
    text("MARK ERROR <= "+(Math.ceil(physical.mmPerVpxX/2*100)/100).toFixed(2)+" mm",noteX,noteY+16,"muted")
    text("EDID "+(input.physicalWidth||0)+" x "+(input.physicalHeight||0)+" mm / "+physical.source,noteX,noteY+38,"muted")
    text(physical.widthPx+" x "+physical.heightPx+" / "+ps+" px PER vpx",noteX,noteY+54,"muted")
  }
  text("DRAWN BY quadrille / REV 02",32,h-40,"muted")
  return {width:w,height:h,pixelWidth:physical.widthPx,pixelHeight:physical.heightPx,
    strokes:strokes,labels:labels,plates:plates,bodies:bodies,dimensions:dimensions,xMarks:xMarks,yMarks:yMarks,
    ruler:{x:rulerX,y:rulerY,length:length,labelStep:labelStep},composition:composition}
}

function paint(ctx, drawing, phys, colors, background, dpr) {
  ctx.reset(); ctx.scale(1/dpr,1/dpr)
  ctx.clearRect(0,0,drawing.pixelWidth,drawing.pixelHeight)
  if(background) {ctx.fillStyle=colors.void;ctx.fillRect(0,0,drawing.pixelWidth,drawing.pixelHeight)}
  var last=""
  for(var i=0;i<drawing.strokes.length;i++) {
    var s=drawing.strokes[i]
    if(last!==s.role) {ctx.fillStyle=colors[s.role];last=s.role}
    ctx.fillRect(s.x*phys,s.y*phys,s.w*phys,s.h*phys)
  }
}
