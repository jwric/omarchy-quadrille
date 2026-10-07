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

function canvasLength(vpx, dpr) {
  var numerator=Math.round(dpr*120),a=numerator,b=120
  while(b) {var next=a%b;a=b;b=next}
  var stride=numerator/a
  return Math.ceil(vpx/stride)*120/a
}

// The star's two tones. "faint" is the sheet's choice; the others are kept so
// the rejected options can be rendered again (wallpaper.py --explore).
var starTones = ["edge","faint","muted"]

function plan(input, physical, monitors, glyphs, topInset, big, composition, starTone) {
  composition = composition || "aperture"
  starTone = starTones.indexOf(starTone)>=0 ? starTone : "faint"
  var ps = physical.pixelsPerVpx
  var w = Math.ceil(physical.widthPx/ps), h = Math.ceil(physical.heightPx/ps)
  var cx = Math.round(physical.widthPx/(2*ps)), cy = Math.round(physical.heightPx/(2*ps))
  var mx = physical.mmPerVpxX, my = physical.mmPerVpxY
  var strokes = [], labels = [], targets = [], rings = [], bursts = [], xMarks = [], leaders = []
  var est = physical.estimated ? "~" : ""
  var dropped = []
  // The safe frame, then one gutter inside it on every side; the bar covers the top.
  var sx=Math.round(w*.02), sy=Math.round(h*.02), G=24
  var L=sx+G, R=w-sx-G, T=Math.max(sy,Math.max(16,Math.round(topInset||0)))+G, B=h-sy-G
  var compact = R-L<440 || B-T<360
  var portrait = h>w

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
  function span(value,n) {return glyphs.length(String(value))*6*(n||1)+1}
  // Text is a list of [words, role] parts on one line; a label is placed only
  // where it is wholly inside the content box and clear of everything else.
  function keyed(parts,x,y,n) {
    n=n||1
    var value=parts.map(function(p){return p[0]}).join("")
    var box={text:value,x:Math.round(x),y:Math.round(y),w:span(value,n),h:12*n}
    var free=box.x>=L && box.y>=T && box.x+box.w<=R+1 && box.y+box.h<=B
    for (var i=0;i<labels.length;i++) if(overlaps(box,labels[i],3)) free=false
    for (var i=0;i<targets.length;i++) if(overlaps(box,targets[i],3)) free=false
    for (var i=0;i<leaders.length;i++) if(overlaps(box,leaders[i],3)) free=false
    if(!free) {dropped.push(value); return false}
    labels.push(box)
    var at=0
    for (var p=0;p<parts.length;p++) {
      var runs=n===1?glyphs.runs(parts[p][0]):big.rects(parts[p][0],n)
      for(var i=0;i<runs.length;i++) rect(box.x+at*6*n+runs[i].x,box.y+runs[i].y,runs[i].w,runs[i].h||1,parts[p][1])
      at+=glyphs.length(parts[p][0])
    }
    return true
  }
  function text(value,x,y,role,n) {return keyed([[String(value),role||"ink"]],x,y,n)}
  function textRight(value,x,y,role,n) {return text(value,x-span(value,n)+1,y,role,n)}
  function keyedRight(parts,x,y) {return keyed(parts,x-span(parts.map(function(p){return p[0]}).join(""))+1,y)}
  function centred(parts,y) {return keyed(parts,cx-Math.floor(span(parts.map(function(p){return p[0]}).join(""))/2),y)}
  function cross(x,y,size,role) {rect(x-size,y,2*size+1,1,role);rect(x,y-size,1,2*size+1,role)}
  function raster(box, pixel) {
    for(var y=box.y;y<box.y+box.h;y++) {
      var start=box.x, last=pixel(start,y)
      for(var x=start+1;x<=box.x+box.w;x++) {
        var role=x<box.x+box.w?pixel(x,y):null
        if(role!==last) {if(last) rect(start,y,x-start,1,last);start=x;last=role}
      }
    }
  }
  // A one-pixel ring of radius r round a centre that may sit on a half pixel.
  function ring(x,y,r,role,clip) {
    var box=clip||{x:Math.floor(x-r)-1,y:Math.floor(y-r)-1,w:Math.ceil(2*r)+4,h:Math.ceil(2*r)+4}
    raster(box,function(px,py) {
      var d=Math.sqrt((px-x)*(px-x)+(py-y)*(py-y))
      return d<=r+0.5 && d>r-0.5 ? role : null
    })
  }
  // A one-pixel ring the way a pixel artist draws it: the pixels within half a
  // pixel of the radius, so the sides run flat and no lone pixel juts out at
  // the four compass points. Integer arithmetic when the pixels are square.
  function circle(x,y,rx,ry,role) {
    rx=Math.max(1,Math.round(rx));ry=Math.max(1,Math.round(ry))
    raster({x:x-rx-1,y:y-ry-1,w:2*rx+3,h:2*ry+3},function(px,py) {
      var dx=px-x,dy=py-y
      if(rx===ry) {var d=dx*dx+dy*dy; return d>rx*rx-rx && d<=rx*rx+rx ? role : null}
      var e=Math.sqrt(dx*dx/(rx*rx)+dy*dy/(ry*ry))-1, m=Math.min(rx,ry)
      return e*m>-0.5 && e*m<=0.5 ? role : null
    })
  }
  // Leaders run only horizontally, vertically or at 45 degrees, one pixel a step.
  function leader(x0,y0,x1,y1,role) {
    var n=Math.max(Math.abs(x1-x0),Math.abs(y1-y0)),dx=Math.sign(x1-x0),dy=Math.sign(y1-y0)
    for(var i=0;i<=n;i++) rect(x0+dx*i,y0+dy*i,1,1,role)
    // Collision boxes cover exactly the leader's own pixels, four steps a box.
    for(var i=0;i<=n;i+=4) {
      var k=Math.min(4,n-i), xa=x0+dx*i, ya=y0+dy*i, xb=xa+dx*k, yb=ya+dy*k
      leaders.push({x:Math.min(xa,xb),y:Math.min(ya,yb),w:Math.abs(xb-xa)+1,h:Math.abs(yb-ya)+1})
    }
  }

  outline(sx,sy,w-2*sx,h-2*sy,"edge")
  for(var x of [0,Math.round(physical.widthPx/ps)])
    for(var y of [0,Math.round(physical.heightPx/ps)]) {circle(x,y,10,10,"line");cross(x,y,15,"accent")}

  // Callsign, top left.
  var model=(String(input.make||"")+" "+String(input.model||"")).trim()
  var hz=Number(input.refreshRate)
  // An output that reports no make or model closes up its line.
  var callsign=[["QUADRILLE / "+(input.name||"OUTPUT"),"accent"],[model,"muted"],
    [physical.widthPx+" × "+physical.heightPx+(hz>0?" / "+Math.round(hz)+" Hz":""),"ink"],
    [est+physical.widthMm.toFixed(1)+" × "+est+physical.heightMm.toFixed(1)+" mm","ink"]]
    .filter(function(line){return line[0]!==""})
  for(var i=0;i<callsign.length;i++) text(callsign[i][0],L,T+18*i,callsign[i][1])

  // The bottom band: palette, then the ruler (left) and the title block (right).
  var paletteRows=(R-L)/12<52?2:1
  var rulerHead=B-46, rulerY=B-26
  var paletteY=rulerHead-24-(paletteRows===2?80:36)

  // The star: the largest whole 10 mm that leaves room for three lines above it.
  var diameter=Math.max(10,Math.floor(Math.min(160,physical.widthMm*.44,
    2*(paletteY-16-cy)*my,2*(cy-T-70)*my,2*(cx-L-136)*mx)/10)*10)
  var rx=Math.round(diameter/(2*mx)),ry=Math.round(diameter/(2*my))
  var star={x:cx-rx,y:cy-ry,w:2*rx+1,h:2*ry+1,cx:cx,cy:cy,rx:rx,ry:ry,
    diameter:diameter,pairs:pairs,high:starTone,low:"ground"}
  targets.push(star)
  var qx=Math.round(mx*10000),qy=Math.round(my*10000),radius=Math.round(diameter/2*10000)
  raster(star,function(x,y) {
    var dx=(x-cx)*qx,dy=(y-cy)*qy
    return dx*dx+dy*dy<=radius*radius?(wedge(dx,dy)?star.high:star.low):null
  })
  for(var radiusMm of [diameter*.4,diameter*.25])
    rings.push({mm:radiusMm,rx:Math.round(radiusMm/mx),ry:Math.round(radiusMm/my),frequency:pairs/(2*Math.PI*radiusMm)})
  var panelRadius=pairs*physical.mmPerPixelX/Math.PI
  var gridRadius=pairs*mx/Math.PI
  var ringA=Math.round(gridRadius/mx)

  // Flanks: true stripes left; the grid's limit and past it right.
  var periods=[12,6,3,2,1.5,1]
  var fw=Math.min(144,cx-rx-L-40), barTop=cy-52
  var flanks=!compact && fw>=72
  var leftRoom=cx-rx-16-L, rightRoom=R-(cx+rx+16)
  var alias=est+(1/(3*mx)).toFixed(2)
  var longRemarks=[" / FINEST THE GRID CAN DRAW"," / TOO FINE: SHOWS AS "+alias," / TOO FINE: COMES OUT SOLID"]
  var shortRemarks=[" GRID'S FINEST"," SHOWS AS "+alias," COMES OUT SOLID"]
  var remarks=longRemarks
  for(var i=0;i<3;i++) if(span(est+"0.00"+longRemarks[i])>rightRoom) remarks=shortRemarks
  var pairHead=span("PAIR = ONE LIGHT + ONE DARK STRIPE")<=leftRoom?["PAIR = ONE LIGHT + ONE DARK STRIPE"]:["PAIR = ONE LIGHT","+ ONE DARK STRIPE"]
  var flankTop=barTop-4-18*pairHead.length, flankBottom=barTop+2*44+18+24+12
  // The slanted edge prefers the space under the callsign; the flanks move
  // down to make room for it when the palette allows.
  var edgeBottom=T+126
  if(flanks && flankTop<edgeBottom+8) {
    var push=edgeBottom+8-flankTop
    if(flankBottom+push<=paletteY-16) {barTop+=push;flankTop+=push;flankBottom+=push}
  }

  // DETAIL A: the middle of the star drawn on the panel's own pixel grid, each
  // panel pixel a block of whole virtual pixels. 43 panel pixels across where
  // the space above the right flank allows; ring A is drawn in it when it fits.
  var detail=null
  if(!compact) {
    var block=Math.max(1,Math.round(6/ps)), below=63
    var room=(flanks?Math.min(barTop,flankTop+22)-12:paletteY-16)-below-T
    var count=Math.min(43,Math.floor(room/block)), count=count-(count%2===0?1:0)
    if(count>=21) {
      var half=(count-1)/2, size=count*block
      detail={x:R-size,y:T,w:size,h:size,block:block,count:count,factor:block*ps,
        pitchX:physical.mmPerPixelX,pitchY:physical.mmPerPixelY,sourceX:cx*ps-half,sourceY:cy*ps-half,kind:"detail"}
      targets.push(detail)
      var pxq=Math.round(physical.mmPerPixelX*10000000),pyq=Math.round(physical.mmPerPixelY*10000000)
      raster(detail,function(x,y) {
        var i=Math.floor((x-detail.x)/block)-half,j=Math.floor((y-detail.y)/block)-half
        return wedge(i*pxq,j*pyq)?star.high:star.low
      })
      outline(detail.x-1,detail.y-1,size+2,size+2,"edge")
      var ccx=detail.x+half*block+(block-1)/2, ccy=detail.y+half*block+(block-1)/2
      detail.panelRing=pairs/Math.PI*block
      ring(ccx,ccy,detail.panelRing,"caution",detail)
      detail.gridRing=gridRadius/physical.mmPerPixelX*block
      if(detail.gridRing<=half*block+(block-1)/2-1) ring(ccx,ccy,detail.gridRing,"accent",detail)
      else detail.gridRing=null
      // The leader leaves ring A at 45 degrees, along a wedge boundary, then
      // turns along a shoulder that carries the detail's title.
      var a=Math.round(ringA/Math.SQRT2)+1
      var shoulderY=Math.max(T+42,detail.y+Math.floor(size/2))
      var elbowX=cx+cy-shoulderY
      var title="DETAIL A  "+detail.factor+":1", subtitle="THE CENTRE ON THE PANEL'S OWN PIXELS"
      if(elbowX+40<=detail.x) {
        leader(cx+a,cy-a,elbowX,shoulderY,"muted")
        leader(elbowX,shoulderY,detail.x-2,shoulderY,"muted")
        detail.leader={x0:cx+a,y0:cy-a,x1:elbowX,y1:shoulderY,x2:detail.x-2}
        if(big) text("A",elbowX+5,shoulderY+5,"accent",2)
        textRight(title,detail.x-10,shoulderY-36,"accent")
        textRight(subtitle,detail.x-10,shoulderY-18,"muted")
      } else {
        // No room for a shoulder: the drafting convention of a short leader to
        // the letter beside the view, and the detail titled where it stands.
        // The leader runs out past the corner of the star's box, and the letter
        // sits beyond its end, clear of the star and the leader like any label.
        var e=Math.max(rx,ry)+4
        leader(cx+a,cy-a,cx+e,cy-e,"muted")
        detail.leader={x0:cx+a,y0:cy-a,x1:cx+e,y1:cy-e}
        if(big) text("A",cx+e+4,cy-e-28,"accent",2)
        textRight(title,detail.x-10,detail.y,"accent")
        textRight(subtitle,detail.x-10,detail.y+18,"muted")
      }
      // A scale of panel pixels under the inset: a tick at every pixel, a long
      // one every ten; then its length and the pixel pitch, in words.
      var scaleY=detail.y+size+4
      rect(detail.x,scaleY,size,1,"muted")
      for(var i=0;i<count;i++) rect(detail.x+i*block,scaleY+1,1,i%10===0?4:2,"muted")
      rect(detail.x+size-1,scaleY+1,1,4,"muted")
      var rowY=scaleY+11
      textRight("1 PIXEL = "+est+physical.mmPerPixelX.toFixed(3)+" mm / "+est+physical.pxPerMmX.toFixed(1)+" PIXELS PER mm",R,rowY,"ink")
      textRight(count+" PIXELS ACROSS = "+est+(count*physical.mmPerPixelX).toFixed(1)+" mm",R,rowY+18,"muted")
      keyedRight([["SMALL RING","caution"],[", "+est+panelRadius.toFixed(1)+" mm OUT: THE PANEL'S LIMIT","muted"]],R,rowY+36)
    }
  }

  // The star's rings and outline go over the leader, so the rim stays whole.
  for(var r of rings) circle(cx,cy,r.rx,r.ry,"line")
  circle(cx,cy,rx,ry,"accent")
  circle(cx,cy,ringA,ringA,"accent")

  function burst(x,y,bw,bh,index) {
    var n=periods[index],f=1/(n*mx)
    var box={x:x,y:y,w:bw,h:bh,period:{n:n,d:1},frequency:f,aliased:n<2,vertical:false}
    targets.push(box);bursts.push(box)
    raster(box,function(px,py) {return Math.floor(2*(px-x)/n)%2===0?"ink":"void"})
    return box
  }
  if(flanks) {
    for(var i=0;i<pairHead.length;i++) text(pairHead[i],L,flankTop+18*i,"muted")
    for(var i=0;i<3;i++) {
      var b=burst(L,barTop+i*44,fw,18,i)
      text(est+b.frequency.toFixed(2)+" PAIRS PER mm",L,b.y+24,"muted")
      b=burst(R-fw,barTop+i*44,fw,18,i+3)
      textRight(est+b.frequency.toFixed(2)+remarks[i],R,b.y+24,i===0?"muted":"caution")
    }
  }
  // Every role as a swatch with a one-pixel edge outline, its name under it.
  var roleNames=["void","ground","raised","hover","edge","faint","muted","ink","accent","live","caution","alarm"]
  var swatches=[],cols=12/paletteRows
  if(!compact) for(var i=0;i<roleNames.length;i++) {
    var col=i%cols,row=Math.floor(i/cols),x=L+Math.floor(col*(R-L)/cols)
    var end=L+Math.floor((col+1)*(R-L)/cols),y=paletteY+44*row
    var swatch={x:x,y:y,w:end-x-6,h:18,role:roleNames[i]}
    swatches.push(swatch)
    rect(x,y,swatch.w,18,swatch.role)
    outline(x,y,swatch.w,18,"edge")
    text(swatch.role,x,y+24,"muted")
  }

  // Title block, bottom right: the screen's number, where the size came from,
  // and how closely the marks follow it.
  var ident=Math.max(0,monitors.findIndex(function(m){return m.input.name===input.name}))+1
  var numeral=(ident<10?"0":"")+ident
  if(big) textRight(numeral,R,B-36,"accent",3)
  var tx=R-span(numeral,3)-15
  var source=physical.source==="edid"?"SIZE INFERRED FROM THE SCREEN'S "+(input.physicalWidth/10)+" × "+(input.physicalHeight/10)+" cm":
    physical.estimated?"SIZE UNKNOWN: ~ VALUES ASSUME 96 PIXELS PER INCH":"SIZE FROM YOUR MEASUREMENTS"
  var error=Math.ceil(Math.max(mx,my)/2*100)/100
  textRight("APERTURE TEST CARD / SCREEN "+ident+" OF "+Math.max(1,monitors.length),tx,B-48,"muted")
  textRight(source,tx,B-30,"muted")
  textRight("MARKS WITHIN "+est+error.toFixed(2)+" mm / FRAME 2% IN",tx,B-12,"muted")

  // The ruler, bottom left: whole millimetres on the nearest virtual pixel.
  var rulerX=L+3
  // As long as fits before the title block, 100 mm at most.
  var titleLeft=R
  for(var l of labels) if(l.y>=B-48) titleLeft=Math.min(titleLeft,l.x)
  var length=Math.max(10,Math.min(100,Math.floor((titleLeft-40-rulerX)*mx/10)*10))
  var step=100
  for(var s of [10,20,50,100]) if(s/mx>=span(est+length)+5) {step=s;break}
  for(var mm=0;mm<=length;mm++) {
    var at=rulerX+Math.round(mm/mx),tick=mm%10===0?10:mm%5===0?6:3
    rect(at,rulerY,1,tick,mm%10===0?"ink":"line")
    if(mm%10===0) {xMarks.push({mm:mm,vpx:at-rulerX});if(mm%step===0) text((mm>0?est:"")+mm,at-3,rulerY+14,"muted")}
  }
  rect(rulerX,rulerY,Math.round(length/mx)+1,1,"ink")
  text("HOLD A RULER HERE / "+est+length+" mm",L,rulerHead,"ink")

  // The star's captions, placed last so the leader keeps them clear.
  var captions=[[["OUTER RING "+est+rings[0].frequency.toFixed(2)+", INNER "+est+rings[1].frequency.toFixed(2)+" PAIRS PER mm","muted"]],
    [["RING A","accent"],[", "+est+gridRadius.toFixed(1)+" mm OUT: THE GRID'S LIMIT","muted"]],
    [[est+diameter+" mm ACROSS: SHOULD LOOK ROUND","ink"]]]
  var widest=0
  for(var c of captions) widest=Math.max(widest,span(c.map(function(p){return p[0]}).join("")))
  // The block is centred over the star unless the 45 degree leader needs room:
  // then it moves left just enough (the leader passes x = cx+cy-y).
  var axis=cx
  if(detail && detail.leader) axis=Math.min(cx,cx+ry+14-4-Math.ceil(widest/2))
  for(var i=0;i<3;i++) {
    var value=captions[i].map(function(p){return p[0]}).join("")
    keyed(captions[i],axis-Math.floor(span(value)/2),cy-ry-62+18*i)
  }

  // The slanted edge: a 1 in 12 slope whose steps are the grid, with the one
  // definition of a virtual pixel. It takes the first place that is clear:
  // under the callsign, beside it in the top band, or under the left flank.
  var legend=["SLANTED EDGE, 1 IN 12","GRID STEP = 1 VIRTUAL PIXEL","= "+ps+" \u00d7 "+ps+" PANEL PIXELS = "+est+mx.toFixed(2)+" mm"]
  var callsignW=0
  for(var l of labels) if(l.x===L && l.y>=T && l.y<=T+54) callsignW=Math.max(callsignW,l.w)
  var spots=[{x:L,y:T+82},{x:L+callsignW+32,y:T+2},{x:L,y:flankBottom+28}]
  var edge=null
  for(var k=0;!compact && !edge && k<spots.length;k++) {
    var cand={x:spots[k].x,y:spots[k].y,w:48,h:32,kind:"slanted"}
    var boxes=[{x:cand.x,y:cand.y-2,w:60+span(legend[2]),h:48}]
    var ok=cand.x>=L && cand.y-2>=T && boxes[0].x+boxes[0].w<=R+1 && cand.y+46<=B
    for(var t of targets.concat(labels,leaders,swatches)) if(overlaps(boxes[0],t,8)) ok=false
    if(ok) edge=cand
  }
  if(edge) {
    targets.push(edge)
    raster(edge,function(x,y){return 12*(x-edge.x-24)>=y-edge.y-16?"ink":"void"})
    for(var i=0;i<3;i++) text(legend[i],edge.x+60,edge.y-2+18*i,"muted")
  } else if(!compact) dropped.push("slanted edge")

  return {width:w,height:h,pixelWidth:physical.widthPx,pixelHeight:physical.heightPx,
    strokes:strokes,labels:labels,plates:[],targets:targets,star:star,rings:rings,bursts:bursts,
    detail:detail,swatches:swatches,leaders:leaders,panelRadius:panelRadius,gridRadius:gridRadius,dropped:dropped,
    safe:{x:sx,y:sy,w:w-2*sx,h:h-2*sy},content:{x:L,y:T,w:R-L,h:B-T},xMarks:xMarks,
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
