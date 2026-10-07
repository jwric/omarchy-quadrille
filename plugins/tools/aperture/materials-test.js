#!/usr/bin/env node
// Materials.js: the tone and texture strip. Every standard monitor, row and stack,
// compact or not, plus group subsets: integer strokes inside the measured box,
// nothing dropped, targets and labels clear, determinism, and the spec's numbers.
const assert=require('node:assert/strict'),A=require('./lib.js');
const mod=A.module('Materials');
const ORDER=[['surfaces',['void','ground','raised','hover']],['inks',['edge','line','faint','muted','ink']],
  ['signals',['accent','live','caution','alarm']]];
const TITLES={surfaces:'SURFACES',inks:'INKS',signals:'SIGNALS'};
const SIZE={surfaces:[36,28,24,24],inks:[24,18,24,24],signals:[10,8,10,8]};
const NEXT={void:'ground',ground:'raised',raised:'hover',hover:'edge'};
const overlap=(a,b,g)=>a.x<b.x+b.w+g&&b.x<a.x+a.w+g&&a.y<b.y+b.h+g&&b.y<a.y+a.h+g;
let cases=0;
function run(monName,layoutName,cfg){
  const input=A.monitors[monName], outputs=A.layouts[layoutName].map(m=>m.name===input.name?input:m);
  if(!outputs.some(m=>m.name===input.name)) outputs.unshift(input);
  const ctx={...A.context(input,outputs),...cfg}, tag=`${monName}/${layoutName}/${JSON.stringify(cfg)}`;
  const m=mod.measure(ctx); assert(m,tag+': measure null');
  const M=16, box={L:M,T:M,R:M+m.w-1,B:M+m.h};
  const make=()=>{const pen=A.Pen.make(m.w+2*M,m.h+2*M,A.glyphs,A.big,box);return [pen,mod.draw(pen,M,M,ctx)]};
  const [pen,info0]=make(), [pen2,info2]=make(), info=JSON.parse(JSON.stringify(info0));
  assert.equal(JSON.stringify(pen.strokes),JSON.stringify(pen2.strokes),tag+': strokes not deterministic');
  assert.equal(JSON.stringify(info),JSON.stringify(JSON.parse(JSON.stringify(info2))),tag+': info not deterministic');
  assert.equal(pen.dropped.length,0,tag+': dropped '+pen.dropped);
  for(const s of pen.strokes){
    for(const k of ['x','y','w','h']) assert(Number.isInteger(s[k]),tag+': non-integer stroke');
    assert(s.x>=M&&s.y>=M&&s.x+s.w<=M+m.w&&s.y+s.h<=M+m.h,tag+': stroke outside measured box '+JSON.stringify(s));
  }
  // targets and labels
  for(let i=0;i<pen.targets.length;i++){
    for(let j=i+1;j<pen.targets.length;j++) assert(!overlap(pen.targets[i],pen.targets[j],3),tag+': targets closer than 4');
    for(const l of pen.labels) assert(!overlap(pen.targets[i],l,3),tag+': label closer than 4 to a target');
  }
  for(let i=0;i<pen.labels.length;i++) for(let j=i+1;j<pen.labels.length;j++) assert(!overlap(pen.labels[i],pen.labels[j],3),tag+': labels collide');
  // roles
  const want=(cfg.groups||ORDER.map(g=>g[0])).map(g=>ORDER.find(o=>o[0]===g.toLowerCase()));
  const roles=info.specimens.map(s=>s.role);
  assert.deepEqual(roles,want.flatMap(g=>g[1]),tag+': roles/order');
  assert.deepEqual(info.groups.map(g=>g.name),want.map(g=>TITLES[g[0]]),tag+': group names');
  for(const s of info.specimens){
    const [w,wc,hn,hc]=SIZE[s.group], ew=cfg.compact?wc:w, h=cfg.compact?hc:hn;
    assert.equal(s.w,ew,tag+': width '+s.role); assert.equal(s.h,h,tag+': height '+s.role);
    if(s.group==='signals'){ assert.equal(s.texture,null); assert.equal(s.flat.h,h) }
    else {
      assert.deepEqual(s.flat,{x:s.x,y:s.y,w:ew,h:12}); assert.equal(s.texture.h,12); assert.equal(s.texture.y,s.y+12);
    }
    if(s.group==='surfaces'){
      assert.deepEqual(s.frame,{x:s.x-1,y:s.y-1,w:ew+2,h:h+2}); assert.equal(s.texture.kind,'ramp');
      assert.equal(s.texture.to,NEXT[s.role]); assert.deepEqual(s.texture.levels,[4,8,12]);
    }
    if(s.group==='inks') assert.equal(s.texture.from,'void');
    // label: under the specimen, 4 clear of its frame slot, muted
    assert.equal(s.label.y,s.y+s.h+1+4,tag+': label gap'); assert(s.label.drawn);
    assert(pen.labels.some(l=>l.text===s.role&&l.x===s.label.x&&l.y===s.label.y),tag+': label not in pen');
    const tl=pen.strokes.filter(t=>t.role==='muted'&&t.y>=s.label.y&&t.y<s.label.y+12&&t.x>=s.label.x&&t.x<s.label.x+s.label.w);
    assert(tl.length,tag+': label not muted '+s.role);
  }
  // groups: names ink 4 above the frame slot; pitch = max(outer width, span)+8; 20 between groups
  for(const g of info.groups) assert(pen.labels.some(l=>l.text===g.name&&l.x===g.x&&l.y===g.y),tag+': group name not in pen');
  const by={}; info.specimens.forEach(s=>(by[s.group]=by[s.group]||[]).push(s));
  for(const k in by){
    const list=by[k], gi=info.groups.find(g=>g.name===TITLES[k]), frame=k==='surfaces'?1:0;
    assert.equal(list[0].x-frame,gi.x,tag+': first specimen left of its group');
    for(let i=1;i<list.length;i++){
      const outer=list[i-1].w+2*frame, pitch=Math.max(outer,pen.span(list[i-1].role))+8;
      assert.equal(list[i].x-list[i-1].x,pitch,tag+': pitch '+k);
    }
    assert(Math.min(...list.map(s=>s.y))-1-(gi.y+12)>=4,tag+': name too near the specimens');
  }
  if(cfg.layout==='stack'){
    for(let i=1;i<info.groups.length;i++){
      const prevBottom=Math.max(...info.specimens.filter(s=>s.group===Object.keys(by)[i-1]).map(s=>s.label.y+12));
      assert.equal(info.groups[i].y-prevBottom,20,tag+': stack gap');
      assert.equal(info.groups[i].x,info.groups[0].x);
    }
  } else {
    for(let i=1;i<info.groups.length;i++){
      const prev=Object.keys(by)[i-1], right=Math.max(...by[prev].map(s=>Math.max(s.label.x+s.label.w-1,s.x+s.w+(prev==='surfaces'?1:0))));
      assert(info.groups[i].x-right>=20-1,tag+': row group gap '+(info.groups[i].x-right));
    }
    assert(new Set(info.groups.map(g=>g.y)).size===1);
  }
  assert.equal(info.w,m.w); assert.equal(info.h,m.h);
  cases++;
  return {m,info};
}
const mons=Object.keys(A.monitors), summary={};
for(const mon of mons) for(const lay of Object.keys(A.layouts)) for(const layout of ['row','stack']) for(const compact of [false,true])
  { const r=run(mon,lay,{layout,compact}); summary[layout+(compact?'-c':'')]=[r.m.w,r.m.h] }
for(const groups of [['surfaces'],['inks'],['signals'],['inks','signals'],['SURFACES','Inks']])
  for(const layout of ['row','stack']) run('laptop','real',{layout,groups:groups.map(g=>g.toLowerCase()===g?g:g)});
run('laptop','real',{base:false}); run('dell','real',{base:false,compact:true});
// the strip is the same for every monitor (it depends on nothing physical)
assert.equal(mod.measure({layout:'row'}).w,mod.measure({layout:'row',compact:false}).w);
assert.equal(mod.measure({groups:[]}),null);
// numbers from the spec: full strip widths and heights
assert.deepEqual(summary.row,[544,58]); assert.deepEqual(summary.stack,[176,200]);
console.log(`materials-test ok: ${cases} cases, row ${summary.row.join('x')}, stack ${summary.stack.join('x')}, compact row ${summary['row-c'].join('x')}`);
