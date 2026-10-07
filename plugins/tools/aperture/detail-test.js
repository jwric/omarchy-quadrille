#!/usr/bin/env node
// Detail.js (DETAIL A, the native pixel detail): geometry and spec assertions on every
// standard monitor and layout, factors 6 and 3, every leader direction, the title off, and
// an anisotropic panel. Prints one summary line.
const assert=require('node:assert/strict'),A=require('./lib.js');
const mod=A.module('Detail');
const same=(a,b,msg)=>assert.equal(JSON.stringify(a),JSON.stringify(b),msg);
const M=16;

function run(input,outputs,cfg={},overrides={}) {
  const ctx={...A.context(input,outputs,overrides),...cfg};
  const m=mod.measure(ctx);
  assert(m&&m.w>0&&m.h>0,'measure');
  const pen=A.Pen.make(m.w+2*M,m.h+2*M,A.glyphs,A.big,{L:M,T:M,R:M+m.w-1,B:M+m.h});
  const info=mod.draw(pen,M,M,ctx);
  return {ctx,m,pen,info};
}
// The gap between two boxes (0 when they overlap), in the larger axis.
const gap=(a,b)=>Math.max(b.x-(a.x+a.w),a.x-(b.x+b.w),b.y-(a.y+a.h),a.y-(b.y+b.h));
const high=p=>p*p+2*p, low=p=>p*p-2*p;
// The last stroke wins, as in the paint.
function paint(pen) {
  const role=new Map();
  for(const s of pen.strokes) for(let y=s.y;y<s.y+s.h;y++) for(let x=s.x;x<s.x+s.w;x++) role.set(x+','+y,s.role);
  return role;
}

function check(label,input,outputs,cfg,overrides) {
  const {ctx,m,pen,info}=run(input,outputs,cfg,overrides);
  const ps=ctx.ps, b=info.block, F=info.factor;
  const where=`${label} ps${ps}`;
  const role=paint(pen);
  const roleAt=(x,y)=>role.get(x+','+y);

  // Whole pixels, inside the measured box, nothing dropped.
  for(const s of pen.strokes) {
    assert([s.x,s.y,s.w,s.h].every(Number.isInteger),where+' integer strokes');
    assert(s.w>0&&s.h>0,where+' empty stroke');
    assert(s.x>=M&&s.y>=M&&s.x+s.w<=M+m.w&&s.y+s.h<=M+m.h,where+' stroke outside the measured box '+JSON.stringify(s)+JSON.stringify(m));
  }
  same(pen.dropped,[],where+' dropped');
  assert(info.texts.every(t=>t.drawn),where+' text not drawn');

  // The factor, the block, the radii.
  assert.equal(b,Math.max(1,Math.round((cfg&&cfg.factor||6)/ps)),where+' block');
  assert.equal(F,b*ps,where+' F');
  const limit=A.Star.limitRadius();
  assert(Math.abs(info.R-limit*ps)<1e-9,where+' R');
  assert(Math.abs(info.rimRadius-limit*F)<=0.5+1e-9,where+' rim radius '+info.rimRadius+' vs '+limit*F);
  assert(Math.abs(info.panelRadius-limit*b)<=0.5+1e-9,where+' panel radius');
  assert.equal(info.rimRadius*2,info.rim2); assert.equal(info.panelRadius*2,info.pan2);

  // One target: the disc's box, whole.
  assert.equal(pen.targets.length,1,where+' targets');
  const target=pen.targets[0];
  same(target,{x:info.disc.x,y:info.disc.y,w:info.disc.w,h:info.disc.h,kind:'detail'},where+' target');
  const cx=info.centre.x, cy=info.centre.y;
  assert(Number.isInteger(cx*2)&&Number.isInteger(cy*2),where+' centre on a half pixel');
  assert.equal(cx,target.x+(target.w-1)/2,where+' centre x'); assert.equal(cy,target.y+(target.h-1)/2,where+' centre y');
  assert.equal(cx%1===0.5,b%2===0,where+' centre parity');
  assert.equal(info.origin.x+(b-1)/2,cx); assert.equal(info.origin.y+(b-1)/2,cy);
  // Doubled offset from the disc's centre of a vpx.
  const U=x=>2*(x-cx), V=y=>2*(y-cy), D=(x,y)=>U(x)**2+V(y)**2;

  // The rings: exactly the pixels within half a pixel of the radius, 8-fold symmetric.
  const accent=[], caution=[];
  for(let y=target.y;y<target.y+target.h;y++) for(let x=target.x;x<target.x+target.w;x++) {
    const r=roleAt(x,y), d=D(x,y);
    if(d>low(info.rim2)&&d<=high(info.rim2)) assert.equal(r,'accent',where+` rim pixel ${x},${y}`);
    else assert.notEqual(r,'accent',where+` stray accent ${x},${y}`);
    if(info.panelRing&&d>low(info.pan2)&&d<=high(info.pan2)) assert.equal(r,'caution',where+` panel ring pixel ${x},${y}`);
    else assert.notEqual(r,'caution',where+` stray caution ${x},${y}`);
    if(r==='accent') accent.push([x,y]); if(r==='caution') caution.push([x,y]);
  }
  assert.equal(info.panelRing,info.rim2-info.pan2>=6,where+' panel ring rule'); assert.equal(info.panelRing,ps>1,where+' panel ring only apart from the rim');
  for(const [name,set] of [['rim',accent],['panel limit ring',caution]]) {
    if(name!=='rim'&&!info.panelRing) { assert.equal(set.length,0,where+' caution drawn on top of the rim'); continue }
    assert(set.length>8,where+' '+name+' empty');
    const keys=new Set(set.map(p=>p[0]+','+p[1]));
    for(const [x,y] of set) {
      const u=U(x), v=V(y);
      for(const [a,c] of [[u,-v],[-u,v],[-u,-v],[v,u],[v,-u],[-v,u],[-v,-u]])
        assert(keys.has((cx+a/2)+','+(cy+c/2)),where+` ${name} not symmetric at ${x},${y}`);
    }
    // Compass points: the extreme run is flat (no single-pixel nub).
    const top=Math.min(...set.map(p=>p[1])), run_=set.filter(p=>p[1]===top).length;
    assert(run_>=3,where+` ${name} nub at the top (${run_})`);
    const left=Math.min(...set.map(p=>p[0])); assert(set.filter(p=>p[0]===left).length>=3,where+` ${name} nub at the left`);
  }
  // The rim's extent: the disc's box is exactly the rim's.
  assert.equal(Math.min(...accent.map(p=>p[0])),target.x); assert.equal(Math.max(...accent.map(p=>p[0])),target.x+target.w-1);
  assert.equal(Math.min(...accent.map(p=>p[1])),target.y); assert.equal(Math.max(...accent.map(p=>p[1])),target.y+target.h-1);

  // The leaders' pixels (to be skipped where they cover a block).
  const leaderPx=new Set();
  const dist=(x,y)=>Math.sqrt(D(x,y))/2;
  const gridDir=ctx.gridLeader&&ctx.gridLeader!==(ctx.panelLeader||'se')?ctx.gridLeader:(ctx.gridLeader&&ctx.gridLeader===(ctx.panelLeader||'se')?['sw','nw','ne','se'].find(k=>k!==ctx.gridLeader):(ctx.panelLeader==='sw'?'nw':'sw'));
  assert.equal(info.gridLeader,gridDir,where+' grid leader direction');
  assert.notEqual(info.gridLeader,info.panelLeader);
  if(!info.panelRing) { assert.equal(info.leader,null); assert(!info.texts.some(t=>t.text==='PANEL LIMIT'),where+' PANEL LIMIT without a ring'); assert.equal(pen.leaders.length>0,true); }
  const dirs={se:[1,1],sw:[-1,1],ne:[1,-1],nw:[-1,-1]};
  const hxs={};
  function leaderCheck(L,dir,radius,minShoulder,text) {
    const dx=Math.sign(L.x1-L.x0), dy=Math.sign(L.y1-L.y0);
    assert.equal(Math.abs(L.x1-L.x0),Math.abs(L.y1-L.y0),where+` ${text} leader is 45 degrees`);
    for(let i=0;i<=Math.abs(L.x1-L.x0);i++) leaderPx.add((L.x0+dx*i)+','+(L.y0+dy*i));
    const hx=Math.sign(L.x2-L.x1); assert(hx!==0,where+' shoulder'); assert.equal(hx,dx,where+' shoulder runs on from the diagonal');
    for(let x=L.x1;x!==L.x2+hx;x+=hx) leaderPx.add(x+','+L.y1);
    assert(Math.abs(L.x2-L.x1)>=minShoulder,where+` ${text} shoulder length `+Math.abs(L.x2-L.x1));
    assert.deepEqual([dx,dy],dirs[dir],where+` ${text} direction`);
    // It runs along the 45 degree wedge boundary through the top-left corner of block (0,0).
    for(let i=0;i<=Math.abs(L.x1-L.x0);i++) {
      const X=L.x0+dx*i-info.origin.x, Y=L.y0+dy*i-info.origin.y;
      if(dx*dy>0) assert.equal(X,Y,where+' leader off the diagonal boundary');
      else assert.equal(X+Y,-1,where+' leader off the anti-diagonal boundary');
    }
    // Starts at the ring's radius; the elbow 4 outside the rim.
    assert(Math.abs(dist(L.x0,L.y0)-radius)<=1.5,where+` ${text} leader start `+dist(L.x0,L.y0)+' vs '+radius);
    const e=dist(L.x1,L.y1)-info.rimRadius;
    assert(e>=4&&e<=4+1.5+0.5,where+' elbow '+e+' outside the rim');
    hxs[text]=hx;
  }
  leaderCheck(info.rimLeader,gridDir,info.rimRadius,12,'GRID LIMIT');
  if(info.panelRing) leaderCheck(info.leader,ctx.panelLeader||'se',info.panelRadius,10,'PANEL LIMIT');
  for(const k of leaderPx) { const [x,y]=k.split(',').map(Number), d=D(x,y);
    const covered=(d>low(info.rim2)&&d<=high(info.rim2))||(info.panelRing&&d>low(info.pan2)&&d<=high(info.pan2));
    if(!covered) assert.equal(roleAt(x,y),'muted',where+' leader pixel '+k); }

  // Every block is its panel pixel's role: the shared sampler, pixel by pixel.
  const tone=A.Star.panelSampler(ctx.star,ctx.physical);
  let seen=0;
  for(let y=target.y;y<target.y+target.h;y++) for(let x=target.x;x<target.x+target.w;x++) {
    const d=D(x,y), r=roleAt(x,y);
    if(d>low(info.rim2)) continue;
    const i=Math.floor((x-info.origin.x)/b), j=Math.floor((y-info.origin.y)/b);
    if(leaderPx.has(x+','+y)) continue;
    if(info.panelRing&&d>low(info.pan2)&&d<=high(info.pan2)) continue;
    assert.equal(r,tone(i,j),where+` block ${i},${j} at ${x},${y}`); seen++;
  }
  assert(seen>0.9*Math.PI*(info.rimRadius-1)**2,where+' blocks fill the disc: '+seen);
  // No block pixel outside the rim; the star's roles appear nowhere else.
  for(const [k,r] of role) if(r===ctx.star.high||r===ctx.star.low) {
    const [x,y]=k.split(',').map(Number);
    assert(x>=target.x&&x<target.x+target.w&&y>=target.y&&y<target.y+target.h,where+' block pixel outside the disc box');
    assert(D(x,y)<=low(info.rim2),where+' block pixel outside the rim '+k);
  }
  // No gap between the blocks and the rim wider than 1 vpx along any row or column.
  let widest=0;
  for(const axis of [0,1]) for(let a=0;a<target.w;a++) for(const sign of [1,-1]) {
    const line=a_=>axis?[target.x+a,a_]:[a_,target.y+a];
    const centre=axis?cy:cx, base=axis?target.y:target.x;
    // walk from the centre outward to the first rim pixel, then count the void pixels before it
    let t=sign>0?Math.ceil(centre):Math.floor(centre), end=sign>0?base+target.w-1:base;
    let found=null;
    for(;sign>0?t<=end:t>=end;t+=sign) { const [x,y]=line(t); if(roleAt(x,y)==='accent') {found=t;break} }
    if(found===null) continue;
    let g=0; for(let k=found-sign;sign>0?k>=Math.ceil(centre):k<=Math.floor(centre);k-=sign) { const [x,y]=line(k); if(roleAt(x,y)===undefined) g++; else break }
    widest=Math.max(widest,g);
  }
  assert(widest<=1,where+' gap between blocks and rim '+widest);

  // Text: as specified.
  const byText=Object.fromEntries(info.texts.map(t=>[t.text,t]));
  const est=ctx.est, px=ctx.physical.mmPerPixelX, py=ctx.physical.mmPerPixelY;
  let pitch='PIXEL PITCH '+est+px.toFixed(3);
  if(Math.abs(px-py)>0.0005) pitch+=' × '+est+py.toFixed(3);
  pitch+=' mm';
  assert.equal(info.pitch,pitch,where+' pitch'); assert(byText[pitch],where+' pitch text'); assert.equal(byText[pitch].role,'muted');
  assert.equal(info.pitchX,px); assert.equal(info.pitchY,py);
  if(ctx.physical.estimated) assert(pitch.includes('~'),where+' estimated pitch not marked'); else assert(!pitch.includes('~'));
  assert.equal(pitch.includes('×'),Math.abs(px-py)>0.0005);
  const titled=cfgTitle(cfg);
  if(titled) {
    assert(byText['A']&&byText['A'].role==='accent'&&byText['A'].h===24,where+' A');
    const t=info.texts.find(t=>t.text.startsWith('NATIVE PIXEL DETAIL')); assert(t,where+' title');
    assert.equal(t.text,'NATIVE PIXEL DETAIL  '+F+':1',where+' title text'); assert.equal(t.role,'ink');
    // The 1x capitals are bottom-aligned with the A's cap height (rows 4..19 of its 24).
    assert.equal(t.y+9,byText['A'].y+19,where+' title baseline');
    assert.equal(t.x+1-(byText['A'].x+12),6,where+' 6 vpx of air between the A\'s ink and the N');
    assert(byText['A'].y+byText['A'].h+4<=target.y,where+' title row clear of the disc');
    assert.equal(info.title.y,byText['A'].y); assert(info.title.w>=t.x+t.w-info.title.x);
  } else { assert(!byText['A']&&!info.title,where+' title off'); }
  // Under the disc, left aligned with it, 4 below its box.
  assert.equal(byText[pitch].x,target.x); assert.equal(info.pitchAbove,ctx.pitch==='above');
  if(ctx.pitch==='above') {
    assert.equal(byText[pitch].y,titled?info.title.y+24+4:M,where+' pitch row under the title row');
    assert.equal(target.y,byText[pitch].y+12+4,where+' disc 4 under the pitch row');
    assert(pen.labels.every(l=>l.y<target.y+target.h),where+' something under the disc');
    assert.equal(m.h,Math.max(...pen.labels.map(l=>l.y+l.h),target.y+target.h)-M,where+' box ends at the disc or a label');
  } else assert.equal(byText[pitch].y,target.y+target.h+4);
  // Labels and leaders and the target: clear of each other.
  for(const l of pen.labels) {
    assert(gap(l,target)>=4,where+' label too close to the disc: '+l.text);
    for(const k of pen.leaders) assert(gap(l,k)>=4,where+' label too close to the leader: '+l.text);
  }
  for(let i=0;i<pen.labels.length;i++) for(let j=i+1;j<pen.labels.length;j++) assert(gap(pen.labels[i],pen.labels[j])>=3,where+' labels touch');
  // Facts printed are facts: the label sits at the end of the shoulder, 4 clear, vertically on it.
  for(const [text,Lx] of [['GRID LIMIT',info.rimLeader],['PANEL LIMIT',info.panelRing?info.leader:null]]) {
    if(!Lx) continue;
    const pl=byText[text]; assert(pl&&pl.role==='ink',where+' '+text);
    assert.equal(pl.y+6,Lx.y1,where+' '+text+' vertically on the shoulder');
    if(hxs[text]>0) assert.equal(pl.x-(Lx.x2+1),4); else assert.equal(Lx.x2-(pl.x+pl.w),4);
  }
  // The anchor for the sheet's own leader: this disc, wherever it is put.
  const an=mod.anchor(ctx,M,M); assert.equal(an.cx,cx); assert.equal(an.cy,cy); assert.equal(an.rim,info.rimRadius);
  const an2=mod.anchor(ctx,M+7,M-3); assert.equal(an2.cx,cx+7); assert.equal(an2.cy,cy-3);
  // measure() is the box that draw() fills.
  same(info.box,{x:M,y:M,w:m.w,h:m.h},where+' info.box is the measured box at the draw position');
  const at2=run(input,outputs,cfg,overrides); same(at2.info.box,{x:M,y:M,w:at2.m.w,h:at2.m.h});
  // Deterministic.
  const again=run(input,outputs,cfg,overrides);
  same(again.pen.strokes,pen.strokes,where+' strokes differ between runs'); same(again.info,info,where+' info differs between runs');
  return {info,m,widest};
}
function cfgTitle(cfg) { return !(cfg&&cfg.title===false); }

let cases=0, widest=0, blocks=0;
const layoutNames=Object.keys(A.layouts);
for(const [name,input] of Object.entries(A.monitors)) for(const factor of [6,3]) {
  for(const lname of layoutNames) {
    const outputs=(A.layouts[lname]||[input]).map(o=>o.name===input.name?input:o);
    if(!outputs.some(o=>o.name===input.name)) outputs.unshift(input);
    // The element does not depend on the layout: only the first layout is checked in full.
    const r=check(`${name}/${lname}/f${factor}`,input,outputs,{factor});
    widest=Math.max(widest,r.widest); blocks+=r.info.blocks; cases++;
    break;
  }
}
for(const name of ['laptop','dell']) for(const dir of ['se','sw','ne','nw']) for(const factor of [6,3]) for(const grid of [undefined,'se','sw','ne','nw']) {
  const r=check(`${name}/${dir}+${grid}/f${factor}`,A.monitors[name],A.layouts.real,{factor,panelLeader:dir,gridLeader:grid}); cases++; widest=Math.max(widest,r.widest);
}
// Same drawing in every layout: the layouts only differ in other monitors.
for(const lname of layoutNames) { const r=check(`laptop/${lname}`,A.monitors.laptop,A.layouts[lname]); cases++; widest=Math.max(widest,r.widest); }
for(const name of ['laptop','dell','hidpi','lowdpi']) for(const title of [true,false]) for(const dir of ['se','sw']) {
  check(`${name} pitch above ${title}`,A.monitors[name],A.layouts.real,{pitch:'above',title,panelLeader:dir,gridLeader:'nw'}); cases++;
}
check('title off',A.monitors.laptop,A.layouts.real,{title:false}); cases++;
check('title off nw',A.monitors.dell,A.layouts.real,{title:false,panelLeader:'nw'}); cases++;
// A panel whose pixels are not square: both pitches printed.
const measured={'eDP-2':{width_mm:340,height_mm:220}};
const aniso=check('anisotropic',A.monitors.laptop,A.layouts.real,{},measured); cases++;
assert(aniso.info.pitch.includes('×'),'anisotropic pitch not printed twice: '+aniso.info.pitch);
// Defaults: factor 6, leader se, title on.
const d=run(A.monitors.dell,A.layouts.real).info; assert.equal(d.factor,6); assert.equal(d.panelLeader,'se'); assert(d.title);
// ps 4 prints 8:1.
const h=run(A.monitors.hidpi,A.layouts.single).info; assert(h.texts.some(t=>t.text==='NATIVE PIXEL DETAIL  8:1'));
// No star: nothing to draw.
assert.equal(mod.measure({...A.context(A.monitors.laptop,A.layouts.single),star:null}),null);

console.log(`detail-test ok: ${cases} cases, widest block-to-rim gap ${widest} vpx, laptop disc ${run(A.monitors.laptop,A.layouts.real).info.disc.w} vpx, dell ${run(A.monitors.dell,A.layouts.real).info.disc.w} vpx`);
