#!/usr/bin/env node
// Construction.js (B, the exploded pixel construction): geometry and the spec's numeric facts.
const assert=require('node:assert/strict');
const A=require('./lib.js');
const C=A.module('Construction');
const json=v=>JSON.stringify(v);
const M=16;
let cases=0, shrunk=[], ties=0, tieCases=0, nulls=[];
function run(monitor, layout, diameter, cfg, star0={}) {
  const input=A.monitors[monitor];
  const outputs=(A.layouts[layout]||[input]).map(m=>m.name===input.name?input:m);
  if(!outputs.some(m=>m.name===input.name)) outputs.unshift(input);
  const ctx={...A.context(input,outputs,{},{diameter,...star0}),...cfg};
  const tag=`${monitor}/${layout} d${diameter} ${json(cfg)}`;
  const win=C.windowOf(ctx);
  if(!win) { assert.equal(C.measure(ctx),null,tag+': measure null with no window'); nulls.push(tag); return; }
  cases++;
  const m=C.measure(ctx), box={L:M,T:M,R:M+m.w-1,B:M+m.h};
  const draw=()=>{
    const pen=A.Pen.make(m.w+2*M,m.h+2*M,A.glyphs,A.big,box);
    const info=C.draw(pen,M,M,ctx); return {pen,info};
  };
  const {pen,info}=draw();
  const again=draw();
  assert.equal(json(pen.strokes),json(again.pen.strokes),tag+': deterministic strokes');
  assert.equal(json(info),json(again.info),tag+': deterministic info');
  // integer strokes inside the measured box, nothing dropped
  for(const s of pen.strokes) {
    for(const k of ['x','y','w','h']) assert(Number.isInteger(s[k]),tag+': integer stroke');
    assert(s.w>0&&s.h>0);
    assert(s.x>=M&&s.y>=M&&s.x+s.w<=M+m.w&&s.y+s.h<=M+m.h,tag+': stroke outside the measured box '+json(s));
  }
  assert.equal(pen.dropped.length,0,tag+': dropped');
  assert(info.labels.every(l=>l.drawn),tag+': a label not drawn');
  // targets and labels clear (>= 4 vpx)
  const gap=(a,b)=>Math.max(a.x-(b.x+b.w),b.x-(a.x+a.w),a.y-(b.y+b.h),b.y-(a.y+a.h));
  assert.equal(pen.targets.length,3,tag+': three drawings');
  for(let i=0;i<pen.targets.length;i++) {
    for(let j=0;j<i;j++) assert(gap(pen.targets[i],pen.targets[j])>=4,tag+': targets too close');
    for(const l of pen.labels) assert(gap(pen.targets[i],l)>=4,tag+': label too close to a target: '+l.text);
  }
  for(let i=0;i<pen.labels.length;i++) for(let j=0;j<i;j++) assert(gap(pen.labels[i],pen.labels[j])>=4,tag+': labels too close');
  for(const t of pen.targets) assert(t.x>=M&&t.y>=M&&t.x+t.w<=M+m.w&&t.y+t.h<=M+m.h,tag+': target outside the box');
  // the figure's numbers
  const ps=ctx.ps, b=Math.max(1,Math.round(6/ps)), F=b*ps;
  assert.equal(info.factor,F); assert.equal(info.block,b);
  assert.equal(F,ps===4?8:6,tag+': factor');
  const texts=info.labels.map(l=>l.text);
  assert(texts.includes('B')&&texts.includes(F+':1')&&texts.includes('PANEL PIXELS 1:1'),tag+': labels '+json(texts));
  assert(texts.includes('DRAWING GRID ×'+ps),tag+': the × and ps in the label');
  assert.equal(info.cells,win.cells);
  if(win.cells!==win.asked) shrunk.push(`${tag}: ${win.asked}->${win.cells}`);
  assert.equal(info.coarse.size,win.cells*F); assert.equal(info.fine.size,win.cells*F);
  assert.equal(info.coarse.x,info.fine.x); assert.equal(info.fine.y-info.coarse.y,win.cells*F+12,tag+': views 12 apart');
  assert.equal(info.coarse.y-M,24+6,tag+': views start 6 under the title row');
  const mirror=!!cfg.mirror, sz=info.coarse.size;
  assert.equal(info.mirror,mirror);
  // the anchor: 2 vpx outside the coarse frame, away from the labels, on its middle row
  const frameL=info.coarse.x-1, frameR=info.coarse.x+sz;
  assert.equal(info.viewAnchor.x,mirror?frameR+2:frameL-2,tag+': anchor x');
  assert.equal(info.viewAnchor.y,info.coarse.y+sz/2,tag+': anchor y');
  const lab=t=>info.labels.find(l=>l.text===t);
  if(mirror) {
    assert.equal(M+m.w,frameR+1,tag+': views end the figure');
    for(const t of [info.labels[2],info.labels[3]]) assert.equal((info.tile.x-2*info.tile.s)-(t.x+t.w),10,tag+': labels 10 from the iso, '+t.text);
    assert(info.labels[3].x+info.labels[3].w<info.tile.x,tag+': labels left');
    assert.equal(lab('B').x+(lab('B').w+8+lab(info.factor+':1').w),M+m.w,tag+': title ends on the views');
  } else {
    assert.equal(info.coarse.x,M+1); assert.equal(lab('B').x,M);
    for(const t of [info.labels[2],info.labels[3]]) assert.equal(t.x-(info.tile.x+2*info.tile.s),10,tag+': labels 10 from the iso');
  }
  // labels centred on their row's view
  assert.equal(info.labels[2].y+6,info.coarse.y+sz/2); assert.equal(info.labels[3].y+6,info.fine.y+sz/2);
  assert.equal(lab('B').y+20,lab(info.factor+':1').y+10,tag+': one baseline');
  assert.equal(info.tile.s%ps,0); assert.equal(info.tile.s,ps*Math.max(1,Math.round((cfg.compact?9:12)/ps)),tag+': tile side');
  assert.equal(info.panel.n,ps); assert.equal(info.panel.s,info.tile.s);
  assert.equal(info.tile.x,info.panel.x);
  const gapIso=mirror?(info.coarse.x-1)-(info.tile.x+2*info.tile.s):(info.tile.x-2*info.tile.s)-(info.coarse.x+sz+1);
  assert(gapIso>=16&&gapIso<=20,tag+': iso gap '+gapIso);
  // the window: exactly one boundary, between the grid-limit ring and the rim
  const star=ctx.star, mx=ctx.physical.mmPerVpxX, my=ctx.physical.mmPerVpxY;
  assert.equal(info.window.w,win.cells); assert.equal(info.window.x,win.x);
  assert.equal(info.boundary,cfg.boundary===undefined?36:cfg.boundary);
  const ox=win.x-star.cx, oy=win.y-star.cy, n=win.cells;
  const found=[]; for(let k=0;k<64;k++) {
    // brute force: the samples of the window on either side of the line k pi/32 (mm space)
    let neg=0,pos=0;
    for(let j=0;j<n;j++) for(let i=0;i<n;i++) {
      const X=(ox+i)*mx, Y=(oy+j)*my, side=X*Math.sin(k*Math.PI/32)-Y*Math.cos(k*Math.PI/32), dot=X*Math.cos(k*Math.PI/32)+Y*Math.sin(k*Math.PI/32);
      if(dot<=0) continue; if(side<-1e-12) neg++; else if(side>1e-12) pos++;
    }
    if(neg&&pos) found.push(k);
  }
  assert.equal(json(found),json([info.boundary]),tag+': boundaries through the window samples '+json(found));
  let near=Infinity,far=0;
  for(const [dx,dy] of [[-1,-1],[n,-1],[-1,n],[n,n]]) far=Math.max(far,Math.hypot((ox+dx)*mx,(oy+dy)*my));
  for(let j=0;j<n;j++) for(let i=0;i<n;i++) near=Math.min(near,Math.hypot((ox+i)*mx,(oy+j)*my));
  const limit=A.Star.limitRadius()*mx;
  assert(far<star.diameter/2,tag+': window past the rim '+far);
  assert(near>limit,tag+': window inside the grid-limit ring');
  // the window centre is on the boundary (within a pixel)
  const cxm=(ox+n/2)*mx, cym=(oy+n/2)*my, t=info.boundary*Math.PI/32;
  assert(Math.abs(cxm*Math.sin(t)-cym*Math.cos(t))<=Math.max(mx,my)*1.0,tag+': window off the boundary');
  // coarse = gridSampler, fine = panelSampler, coarse = fine at each vpx's first panel pixel
  const grid=A.Star.gridSampler(star,ctx.physical), panel=A.Star.panelSampler(star,ctx.physical);
  let tie=0;
  for(let j=0;j<n;j++) for(let i=0;i<n;i++) {
    const g=grid(win.x+i,win.y+j); assert(g,tag+': window pixel outside the disc');
    assert.equal(info.coarse.roles[j][i],g,tag+': coarse role');
    const f=info.fine.roles[j*ps][i*ps];
    if(f!==g) tie++;
  }
  for(let j=0;j<n*ps;j++) for(let i=0;i<n*ps;i++)
    assert.equal(info.fine.roles[j][i],panel((win.x-star.cx)*ps+i,(win.y-star.cy)*ps+j),tag+': fine role');
  if(tie) { ties+=tie; tieCases++ }
  assert(tie<=2,tag+': the samplers disagree on '+tie+' pixels');
  // the strokes paint exactly the views' blocks (rebuild the raster from the strokes)
  const grid2=new Map();
  for(const s of pen.strokes) for(let y=s.y;y<s.y+s.h;y++) for(let x=s.x;x<s.x+s.w;x++) grid2.set(x+','+y,s.role);
  for(let j=0;j<n;j++) for(let i=0;i<n;i++) for(let dy=0;dy<F;dy++) for(let dx=0;dx<F;dx++)
    assert.equal(grid2.get((info.coarse.x+i*F+dx)+','+(info.coarse.y+j*F+dy)),info.coarse.roles[j][i],tag+': coarse paint');
  for(let j=0;j<n*ps;j++) for(let i=0;i<n*ps;i++) for(let dy=0;dy<b;dy++) for(let dx=0;dx<b;dx++)
    assert.equal(grid2.get((info.fine.x+i*b+dx)+','+(info.fine.y+j*b+dy)),info.fine.roles[j][i],tag+': fine paint');
  // the marker
  const mk=C.markWindow(A.Pen.make(400,400,A.glyphs,A.big,{L:0,T:0,R:399,B:399}),{...ctx,star:{...star,cx:star.cx+200,cy:star.cy+200}});
  const sh=C.windowOf({...ctx,star:{...star,cx:star.cx+200,cy:star.cy+200}});
  assert.equal(json(mk),json({x:sh.x-1,y:sh.y-1,w:n+2,h:n+2}),tag+': marker');
  assert.equal(sh.x-200,win.x,tag+': window follows the star');
}
const cfgs=[{},{compact:true},{cells:6},{boundary:40},{at:0.7},{mirror:true},{mirror:true,compact:true}];
for(const monitor of Object.keys(A.monitors)) for(const diameter of [60,90,100,160,180]) for(const cfg of cfgs)
  run(monitor,'single',diameter,cfg);
for(const layout of Object.keys(A.layouts)) for(const cfg of [cfgs[0],cfgs[1],cfgs[5]]) run('dell',layout,100,cfg), run('laptop',layout,100,cfg);
// the star away from the origin, as on a sheet
for(const monitor of ['laptop','dell']) run(monitor,'real',100,{},{cx:640,cy:400});
// the standard sheet cases must not shrink the window
const standard=shrunk.filter(s=>/^(laptop|dell)\/(single|real) d(90|100|160|180) \{\}/.test(s));
assert.deepEqual(standard,[],'the standard cases keep their 8 cells');
// markWindow draws nothing but a 1 vpx ink outline
{
  const input=A.monitors.laptop, ctx=A.context(input,[input],{},{diameter:100});
  const pen=A.Pen.make(300,300,A.glyphs,A.big,{L:0,T:0,R:299,B:299});
  const mk=C.markWindow(pen,{...ctx,star:{...ctx.star,cx:150,cy:150}});
  const count=pen.strokes.reduce((a,s)=>a+s.w*s.h,0);
  assert.equal(count,4*mk.w); assert(pen.strokes.every(s=>s.role==='ink'));
  assert.equal(pen.targets.length,0);
}
// a star too small for any window draws nothing
{
  const input=A.monitors.hidpi, ctx={...A.context(input,[input],{},{diameter:10})};
  assert.equal(C.windowOf(ctx),null); assert.equal(C.measure(ctx),null);
  const pen=A.Pen.make(300,300,A.glyphs,A.big,{L:0,T:0,R:299,B:299});
  assert.equal(C.draw(pen,0,0,ctx),null); assert.equal(pen.strokes.length,0);
}
console.log(`construction-test: ok, ${cases} cases (${nulls.length} with no window), ${shrunk.length} shrunk to a smaller window, `+
  `${ties} tie pixels in ${tieCases} cases`);
if(shrunk.length) console.log('  shrunk: '+shrunk.slice(0,40).join('; ')+(shrunk.length>40?'; ...':''));
if(nulls.length) console.log('  no window: '+nulls.join('; '));
