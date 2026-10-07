#!/usr/bin/env node
// Geometry and fact assertions for the real Aperture plan (a technical portrait of
// each output); --json OUT writes the render matrix wallpaper.py renders and checks.
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const root=path.resolve(__dirname,'../..');
// A QML JavaScript library in its own realm; `.import "X.js" as Name` lines load
// X.js (beside it) as the global Name, as the QML engine would.
function lib(file,globals={}) {
  const src=fs.readFileSync(path.join(root,file),'utf8');
  for(const m of src.matchAll(/^\.import\s+"([^"]+\.js)"\s+as\s+(\w+)/gm))
    if(!(m[2] in globals)) globals[m[2]]=lib(path.join(path.dirname(file),m[1]));
  const ctx=vm.createContext(globals);
  vm.runInContext(src.replace(/^\.(pragma|import).*$/gm,''),ctx,{filename:file});return ctx;
}
const physical=lib('plugins/quadrille.background/Physical.js');
const drafting=lib('plugins/quadrille.background/Drafting.js');
const Star=lib('plugins/quadrille.background/Star.js');
for(const dpr of [1,1.25,1.5,200/120,2]) for(const size of [250,534,854,1720]) {
  const length=drafting.canvasLength(size,dpr),px=length*dpr;
  assert(Number.isInteger(length));assert(Math.abs(px-Math.round(px))<1e-9);assert(px>=size);
}
const glyphs=lib('plugins/quadrille.bar/Q/Glyphs.js');
const big=lib('plugins/quadrille.bar/Q/GlyphsBig.js',{Glyphs:glyphs});
const vectors=JSON.parse(fs.readFileSync(path.join(root,'docs/physical-vectors.json')));
const laptop={...vectors.cases.find(v=>v.id==='laptop').input,make:'AU Optronics',model:'0x07B2',x:0,y:0,refreshRate:240.01401};
const dell={...vectors.cases.find(v=>v.id==='ultrawide').input,model:'DELL U3417W',x:-952,y:-1440,refreshRate:59.973};
const layouts={real:[laptop,dell],vertical:[laptop,{...dell,x:0,y:-1440}],side:[laptop,{...dell,x:1536,y:0}],
  three:[laptop,dell,{...laptop,name:'DP-1',x:1536,y:0}],portrait:[{...laptop,transform:1}],single:[laptop],
  estimated:[{...laptop,name:'Virtual-1',physicalWidth:0,physicalHeight:0}],
  rotated:[laptop,{...dell,transform:1,x:-1440,y:-400}]};
const portrait1080={...laptop,width:1920,height:1080,scale:1,transform:3};
const roleOrder=['void','ground','raised','hover','edge','line','faint','muted','ink','accent','live','caution','alarm'];
const oldPhrases=['HOLD A RULER HERE','SHOULD LOOK ROUND','THE CENTRE ON','GRID STEP =','PAIR = ONE LIGHT','TOO FINE','SLANTED EDGE','SIZE INFERRED'];
const matrix=[];
let facts=0;
// The plan comes from another vm realm: compare by value through JSON.
const same=(a,b,msg)=>assert.equal(JSON.stringify(a),JSON.stringify(b),msg);
const plain=v=>JSON.parse(JSON.stringify(v));
// A fact that fails is recorded (see guard) and the case goes on, so one run reports everything wrong.
const soft=new Set();
const ok=(cond,msg)=>{facts++;if(!cond) soft.add(msg||'a fact failed')};
const eq=(a,b,msg)=>{facts++;if(a!==b) soft.add(`${msg||'unequal'}: ${JSON.stringify(a)} !== ${JSON.stringify(b)}`)};
const within=(box,c)=>box.x>=c.x&&box.y>=c.y&&box.x+box.w<=c.x+c.w+1&&box.y+box.h<=c.y+c.h;
const inside=(a,b)=>a.x>=b.x&&a.y>=b.y&&a.x+a.w<=b.x+b.w&&a.y+a.h<=b.y+b.h;
const overlaps=(a,b,gap)=>drafting.overlaps(a,b,gap);
const tones=d=>({high:d.star.high,low:d.star.low});
function planOf(input,outputs,overrides,tone) {
  const p=physical.resolve(input,overrides), monitors=outputs.map(input=>({input,physical:physical.resolve(input,overrides)}));
  return {p,monitors,d:drafting.plan(input,p,monitors,glyphs,16,big,'aperture',tone)};
}
// Every leader is a list of points joined by horizontal, vertical or 45 degree segments.
function segments(pts,tag) {
  ok(Array.isArray(pts)&&pts.length>=2,`${tag}: leader needs two points`);
  for(let i=1;i<pts.length;i++) {
    const dx=pts[i][0]-pts[i-1][0],dy=pts[i][1]-pts[i-1][1];
    ok(Number.isInteger(pts[i][0])&&Number.isInteger(pts[i][1])&&Number.isInteger(pts[i-1][0])&&Number.isInteger(pts[i-1][1]),`${tag}: leader point is not whole: ${JSON.stringify(pts)}`);
    ok(dx===0||dy===0||Math.abs(dx)===Math.abs(dy),`${tag}: leader segment ${i} is not H, V or 45 degrees (${dx},${dy})`);
  }
}
// Pixels a label of one element may be close to: its own target (the locator's
// numbers sit inside it, the elevation's dimensions hug it).
function ownTarget(d,label,target) {
  const loc=d.parts.locator, ev=d.parts.elevation;
  if(loc&&target.kind==='locator'&&loc.numbers.some(n=>n.x===label.x&&n.y===label.y&&String(n.value)===label.text)) return true;
  if(ev&&target.kind==='elevation'&&ev.labels.some(l=>l.x===label.x&&l.y===label.y&&l.text===label.text)) return true;
  return false;
}
// The scale note for a size's source, written independently of Drafting.scaleNote.
function noteFor(p) {
  return p.source==='override'?'PHYSICAL SCALE FROM MEASURED SIZE':p.source==='estimated'?'PHYSICAL SCALE ASSUMES 96 PIXELS PER INCH':'PHYSICAL SCALE ESTIMATED FROM PANEL SIZE';
}

function check(input,outputs,overrides={},repeat=false,tone='faint',opts={}) {
  const {p,monitors,d}=planOf(input,outputs,overrides,tone), c=d.content, tag=`${input.name} ${d.pixelWidth}x${d.pixelHeight}`;
  const ps=p.pixelsPerVpx, est=p.estimated?'~':'', current=Math.max(0,monitors.findIndex(m=>m.input.name===input.name));
  const tall=d.height>d.width, parts=d.parts, texts=d.labels.map(l=>l.text);
  const label=(text,msg)=>{const l=d.labels.find(l=>l.text===text);ok(l,`${tag}: missing label ${msg||text}; labels ${JSON.stringify(texts)}`);return l};

  // A sheet too small for a portrait (the physical vectors include a 100 mm panel) keeps
  // the raster and the star's facts; nothing else is required of it.
  const lite=opts.small&&(c.w<440||c.h<360);
  // ---- the raster and the collision record
  for(const s of d.strokes) {
    for(const k of ['x','y','w','h']) assert(Number.isInteger(s[k]));
    assert(s.x>=0&&s.y>=0&&s.w>0&&s.h>0&&s.x+s.w<=d.width&&s.y+s.h<=d.height,`${tag}: stroke outside the sheet`);
  }
  same(d.plates,[],'plates are gone');
  if(!lite) for(let i=0;i<d.labels.length;i++) {
    const a=d.labels[i];ok(within(a,c),`${tag}: label outside the content box: ${a.text}`);
    for(const b of d.labels.slice(0,i)) ok(!overlaps(a,b,3),`${tag}: labels ${a.text} / ${b.text}`);
    for(const b of d.targets) ok(ownTarget(d,a,b)||!overlaps(a,b,3),`${tag}: label ${a.text} within 3 of a ${b.kind} target`);
    const hit=d.leaders.find(b=>overlaps(a,b,3));ok(!hit,`${tag}: label ${a.text} ${JSON.stringify(a)} is within 3 of a leader ${JSON.stringify(hit)}; detailLeader ${JSON.stringify(parts.detailLeader)}`);
  }
  if(!lite) for(let i=0;i<d.targets.length;i++) {
    const a=d.targets[i];
    ok(a.x>=0&&a.y>=0&&a.x+a.w<=d.width&&a.y+a.h<=d.height,`${tag}: ${a.kind} target outside the sheet`);
    for(const b of d.targets.slice(0,i)) {
      if(a.kind==='star'&&b.kind==='star') continue;
      // The reference is the star's own dimension: it begins on the row under the disc.
      if([a.kind,b.kind].sort().join()==='reference,star') {ok(!overlaps(a,b,0),`${tag}: the reference overlaps the star`);continue}
      ok(!overlaps(a,b,3),`${tag}: ${a.kind} and ${b.kind} targets are within 3 of each other`);
    }
  }
  for(const k of ['x','y','w','h']) for(const l of d.labels) assert(Number.isInteger(l[k]));
  // Omissions: nothing on a landscape sheet, at most the materials on a tall one.
  if(!opts.small) {
    if(tall) ok(d.omitted.every(o=>o==='materials'),`${tag}: a tall sheet omitted ${JSON.stringify(d.omitted)}`);
    else same(d.omitted,[],`${tag}: a landscape sheet omitted ${JSON.stringify(d.omitted)}`);
  }
  if(!lite) same(d.dropped,[],`${tag}: dropped ${JSON.stringify(d.dropped)}`);
  const has=n=>!d.omitted.includes(n);
  if(!opts.small) for(const n of ['identity','detail','construction','samples','elevation','footer'].concat(has('materials')?['materials']:[])) ok(parts[n],`${tag}: no ${n}`);
  // Each part's own box lies in the box planned for it, the boxes keep apart, and the star clears them.
  const names=Object.keys(d.boxes);
  if(!lite) for(const n of names) {
    const b=d.boxes[n];ok(inside(b,{x:c.x,y:c.y,w:c.w+1,h:c.h}),`${tag}: ${n} box outside the content box`);
    for(const m of names.slice(0,names.indexOf(n))) ok(!overlaps(b,d.boxes[m],15),`${tag}: boxes ${n}/${m} closer than 16`);
    const r=Math.max(d.star.rx,d.star.ry), nx=Math.max(b.x,Math.min(d.star.cx,b.x+b.w)), ny=Math.max(b.y,Math.min(d.star.cy,b.y+b.h));
    ok(Math.hypot(nx-d.star.cx,ny-d.star.cy)>=r+16,`${tag}: the star is within 16 of the ${n} box`);
  }
  for(const [n,info] of [['construction',parts.construction],['elevation',parts.elevation]])
    if(info) ok(inside(info.box,d.boxes[n]),`${tag}: ${n} drew outside its box`);
  // DETAIL A reports its working frame's origin as `box`, not the element's corner: its drawn pieces are held to the box instead.
  if(parts.detail) {
    const db=d.boxes.detail, pieces=[parts.detail.disc,...parts.detail.texts];
    for(const k of ['leader','rimLeader']) {const l=parts.detail[k];if(l) pieces.push({x:Math.min(l.x0,l.x1,l.x2),y:Math.min(l.y0,l.y1),w:Math.max(l.x0,l.x1,l.x2)-Math.min(l.x0,l.x1,l.x2)+1,h:Math.abs(l.y1-l.y0)+1})}
    for(const q of pieces) ok(inside(q,db),`${tag}: DETAIL A drew ${JSON.stringify(q)} outside its box ${JSON.stringify(db)}`);
  }

  // ---- the star
  const D=d.star.diameter;
  eq(D%10,0,`${tag}: star diameter ${D} is not a multiple of 10 mm`);
  if(!opts.small) ok(D>=60,`${tag}: star diameter ${D} under 60 mm`);
  ok(Math.abs(2*d.star.rx*p.mmPerVpxX-D)<=p.mmPerVpxX+1e-9,`${tag}: star width`);
  ok(Math.abs(2*d.star.ry*p.mmPerVpxY-D)<=p.mmPerVpxY+1e-9,`${tag}: star height`);
  ok(Math.abs(d.panelRadius*p.pxPerMmX-32/Math.PI)<1e-9,`${tag}: panel radius`);
  ok(Math.abs(d.gridRadius/p.mmPerVpxX-32/Math.PI)<1e-9,`${tag}: grid radius`);
  // A round pixel gives a round star: the same number of virtual pixels each way.
  if(Math.abs(p.mmPerPixelX-p.mmPerPixelY)<1e-12) eq(d.star.rx,d.star.ry,`${tag}: star is not round`);
  eq(d.star.high,tone,`${tag}: star tone`);eq(d.star.low,'ground',`${tag}: star low`);
  if(opts.id==='real-0-1.666667') eq(D,100,`${tag}: the laptop's star`);
  if(opts.id==='real-1-1') eq(D,180,`${tag}: the Dell's star`);
  // Repeated change events must produce exactly the same plan.
  if(repeat) assert.equal(JSON.stringify(d),JSON.stringify(drafting.plan(input,p,monitors,glyphs,16,big,'aperture',tone)),`${tag}: not repeatable`);
  if(lite) return d;

  // ---- the reference: a millimetre scale under the star
  const R=d.ruler;
  eq(R.length,D,`${tag}: ruler length`);eq(R.x,d.star.cx-d.star.rx,`${tag}: ruler origin`);eq(R.y,d.star.cy+d.star.ry+12,`${tag}: ruler row`);
  eq(d.xMarks.length,D/10+1,`${tag}: number of 10 mm marks`);
  d.xMarks.forEach((m,i)=>{
    eq(m.mm,10*i,`${tag}: mark ${i}`);
    ok(Math.abs(m.mm*p.pxPerMmX/ps-m.vpx)<=0.5+1e-9,`${tag}: the ${m.mm} mm mark is ${m.vpx} vpx from the origin, ${m.mm*p.pxPerMmX/ps} true`);
  });
  for(let i=1;i<d.xMarks.length;i++) {
    const step=d.xMarks[i].vpx-d.xMarks[i-1].vpx, ideal=10/p.mmPerVpxX;
    ok([Math.floor(ideal),Math.ceil(ideal)].includes(step),`${tag}: inconsistent ruler steps`);
  }
  ok(R.labelStep%10===0&&R.labelStep/p.mmPerVpxX>=glyphs.length(est+D)*6+1+6,`${tag}: label step ${R.labelStep} too tight`);
  for(let mm=0;mm<=D;mm+=10) if(mm%R.labelStep===0||mm===D) {
    const m=d.xMarks[mm/10], text=(mm>0?est:'')+mm, x=R.x+m.vpx-3*String(mm).length-(mm>0?est.length*6:0);
    ok(d.labels.some(l=>l.text===text&&l.x===x&&l.y===R.y+5),`${tag}: no number label ${text} at ${x},${R.y+5}`);
    ok(R.numbers.some(n=>n.mm===mm&&n.text===text&&n.x===x),`${tag}: ruler.numbers lacks ${mm}`);
  }
  label(`ROUNDNESS  ${est}${D} mm REFERENCE`);

  // ---- identity
  const id=parts.identity, numeral=String(current+1).padStart(2,'0');
  eq(id.numeral,numeral,`${tag}: numeral`);label(numeral,'the numeral');label('APERTURE');
  ok(id.metadata&&id.metadata.includes(input.name),`${tag}: metadata lacks the connector: ${id.metadata}`);
  ok(id.metadata.includes(`${p.widthPx} × ${p.heightPx}`),`${tag}: metadata lacks the size: ${id.metadata}`);
  if(input.refreshRate>0) ok(id.metadata.includes(`${Math.round(input.refreshRate)} Hz`),`${tag}: metadata lacks the refresh rate: ${id.metadata}`);
  else ok(!id.metadata.includes('Hz'),`${tag}: a refresh rate that is not known`);
  label(id.metadata,'the metadata line');
  if(monitors.length>1) {
    const loc=parts.locator;ok(loc,`${tag}: no locator`);
    eq(loc.rects.length,monitors.length,`${tag}: locator rects`);
    loc.rects.forEach((r,i)=>{
      eq(r.index,i,`${tag}: locator order`);eq(r.name,monitors[i].input.name,`${tag}: locator name`);eq(r.current,i===current,`${tag}: locator highlight ${i}`);
    });
    eq(loc.rects.filter(r=>r.current).length,1);
    // The rectangles keep the layout's order, left to right and top to bottom.
    for(let i=0;i<monitors.length;i++) for(let j=0;j<monitors.length;j++) {
      const a=monitors[i].input,b=monitors[j].input;
      if(a.x<b.x) ok(loc.rects[i].x<=loc.rects[j].x,`${tag}: locator x order ${i}/${j}`);
      if(a.y<b.y) ok(loc.rects[i].y<=loc.rects[j].y,`${tag}: locator y order ${i}/${j}`);
    }
  } else ok(!parts.locator,`${tag}: a locator for one output`);

  // ---- DETAIL A
  const a=parts.detail;
  if(a) {
    eq(a.factor,a.block*ps,`${tag}: detail factor`);ok(Number.isInteger(a.block));
    const route=parts.detailLeader;ok(route,`${tag}: DETAIL A has no leader`);
    segments(route,`${tag}: DETAIL A`);
    const first=route[0],last=route[route.length-1];
    const start=Math.hypot(first[0]-d.star.cx,first[1]-d.star.cy);
    ok(start>=32/Math.PI&&start<=32/Math.PI+3,`${tag}: DETAIL A's leader starts ${start} from the star's centre`);
    const end=Math.hypot(last[0]-a.centre.x,last[1]-a.centre.y);
    ok(end>a.rimRadius&&end<=a.rimRadius+3,`${tag}: DETAIL A's leader ends ${end-a.rimRadius} from the rim`);
    ok(Math.abs(a.rimRadius-Star.limitRadius()*ps*a.block)<=1,`${tag}: the rim is the grid limit`);
    eq(a.pitchX,p.mmPerPixelX);eq(a.pitchY,p.mmPerPixelY);
  }

  // ---- B
  const b=parts.construction;
  if(b) {
    const w=b.window, grid=Star.gridSampler(d.star,p), r=D/2;
    ok(b.cells===w.w&&b.cells===w.h,`${tag}: B window size`);
    const nx=Math.max(w.x,Math.min(d.star.cx,w.x+w.w-1)), ny=Math.max(w.y,Math.min(d.star.cy,w.y+w.h-1));
    ok(Math.hypot((nx-d.star.cx)*p.mmPerVpxX,(ny-d.star.cy)*p.mmPerVpxY)>d.gridRadius,`${tag}: B's window reaches the grid-limit ring`);
    let far=0;for(const x of [w.x,w.x+w.w-1]) for(const y of [w.y,w.y+w.h-1]) far=Math.max(far,Math.hypot((x-d.star.cx)*p.mmPerVpxX,(y-d.star.cy)*p.mmPerVpxY));
    ok(far<r,`${tag}: B's window leaves the star's disc`);
    for(let j=0;j<w.h;j++) for(let i=0;i<w.w;i++) {
      const s=grid(w.x+i,w.y+j);ok(s!==null,`${tag}: B's window pixel is off the star`);
      eq(b.coarse.roles[j][i],s,`${tag}: B's coarse view differs from the star at ${w.x+i},${w.y+j}`);
    }
    eq(b.factor,b.block*ps,`${tag}: B factor`);
    label('DRAWING GRID ×'+ps);label('PANEL PIXELS 1:1');label(b.factor+':1','the factor of B');
    const wl=parts.windowLeader;ok(wl,`${tag}: B has no window leader`);
    segments(wl,`${tag}: B`);
    same(wl[wl.length-1],[b.viewAnchor.x,b.viewAnchor.y],`${tag}: B's leader does not end on the view anchor`);
    const mk=parts.window;ok(mk&&mk.w===w.w+2&&mk.x===w.x-1,`${tag}: window marker`);
    ok(Math.max(Math.abs(wl[0][0]-Math.max(mk.x,Math.min(wl[0][0],mk.x+mk.w-1))),Math.abs(wl[0][1]-Math.max(mk.y,Math.min(wl[0][1],mk.y+mk.h-1))))<=1,`${tag}: B's leader does not start at its marker`);
  }

  // ---- samples
  const s=parts.samples;
  if(s) {
    const want=[2,4/3,1].map(n=>est+(1/(n*p.mmPerVpxX)).toFixed(2));
    same(s.cases.map(k=>k.text),want,`${tag}: sample frequencies`);same(s.numbers,want,`${tag}: sample numbers`);
    for(const t of want) label(t,'frequency '+t);
    for(const t of ['RESOLVED','FALSE DETAIL','ALIASED','DETAIL LOST']) label(t);
  }

  // ---- elevation
  const e=parts.elevation;
  if(e) {
    ok(Math.abs(e.rect.w*p.mmPerVpxX*e.N-p.widthMm)<=e.N*p.mmPerVpxX/2+1e-9,`${tag}: elevation width not to scale 1:${e.N}`);
    ok(Math.abs(e.rect.h*p.mmPerVpxY*e.N-p.heightMm)<=e.N*p.mmPerVpxY/2+1e-9,`${tag}: elevation height not to scale 1:${e.N}`);
    eq(e.scale,'1:'+e.N);label(`ACTIVE AREA  1:${e.N}`);
    const sizeText=mm=>p.source==='override'?mm.toFixed(1)+' mm':est+Math.round(mm)+' mm';
    label(sizeText(p.widthMm));label(sizeText(p.heightMm));
    eq(texts.includes('ESTIMATED'),p.source!=='override',`${tag}: ESTIMATED only on an inferred size`);
    eq(e.estimated,p.source!=='override');
  }

  // ---- materials
  const m=parts.materials;
  if(m) {
    same(m.specimens.map(x=>x.role),roleOrder,`${tag}: the materials`);
    same(m.specimens.map(x=>x.group),['surfaces','surfaces','surfaces','surfaces','inks','inks','inks','inks','inks','signals','signals','signals','signals']);
  }

  // ---- footer
  const f=parts.footer, tol='MARKS WITHIN '+est+(Math.ceil(Math.max(p.mmPerVpxX,p.mmPerVpxY)/2*100)/100).toFixed(2)+' mm';
  eq(f.lines[0],tol,`${tag}: the tolerance line`);
  eq(f.lines.slice(1).join(' '),noteFor(p),`${tag}: the scale note`);
  eq(noteFor(p),drafting.scaleNote(p),`${tag}: scaleNote`);
  ok(f.lines.length<=3);for(const l of f.lines) label(l);

  // ---- the old sheet is gone
  for(const phrase of oldPhrases) ok(!d.labels.some(l=>l.text.includes(phrase)),`${tag}: the old phrase ${phrase} is back`);
  return d;
}
// Only the star's tones may differ between plans: every box, label and stroke stays where
// it was, and the star's own views (DETAIL A, B, the tile) change tone only.
function sameGeometry(a,b,tag) {
  const bare=d=>JSON.stringify({labels:d.labels,targets:d.targets,leaders:d.leaders,content:d.content,ruler:d.ruler,xMarks:d.xMarks,
    omitted:d.omitted,boxes:d.boxes,star:{...d.star,high:null},strokes:d.strokes.map(s=>[s.x,s.y,s.w,s.h]),
    parts:d.parts},(k,v)=>['roles','tones','top','fill','high'].includes(k)?undefined:v);
  assert.equal(bare(a),bare(b),`${tag}: a star tone moved something`);
}
function entry(id,name,scale,current,outputs,d,p) {
  const keep=plain({width:d.width,height:d.height,pixelWidth:d.pixelWidth,pixelHeight:d.pixelHeight,star:d.star,parts:d.parts,ruler:d.ruler,
    xMarks:d.xMarks,omitted:d.omitted,gridRadius:d.gridRadius,panelRadius:d.panelRadius,content:d.content});
  return {id,name,scale,current,outputs,physical:p,tones:tones(d),plan:keep};
}
// A failing case is recorded and the rest still run, so one report shows every failure.
const failures=[];
function guard(id,fn) {
  soft.clear();let result;
  try {result=fn()} catch(e) {soft.add(String(e.stack||e.message).split('\n').slice(0,3).join(' | '))}
  for(const m of soft) failures.push(`${id}: ${m}`);
  return soft.size?null:result;
}
for(const scale of [1,1.666667]) for(const [name,list] of Object.entries(layouts)) {
  for(let current=0;current<list.length;current++) {
    const outputs=list.map(m=>({...m})); outputs[current].scale=scale;
    if(['real','vertical','three'].includes(name)) outputs[1].y=-outputs[1].height/outputs[1].scale;
    if(name==='side') outputs[1].x=outputs[0].width/outputs[0].scale;
    if(name==='three') outputs[2].x=outputs[0].width/outputs[0].scale;
    const id=`${name}-${current}-${scale}`;
    const d=guard(id,()=>check(outputs[current],outputs,{},name==='real',undefined,{id}));
    if(!d) {matrix.push(entry(id,name,scale,current,outputs,planOf(outputs[current],outputs,{},'faint').d,physical.resolve(outputs[current],{})));continue}
    const e=entry(id,name,scale,current,outputs,d,physical.resolve(outputs[current],{}));
    if(name==='real') guard(id+'/tones',()=>{
      e.toneVariants={faint:tones(d)};
      for(const tone of ['edge','muted']) {
        const t=check(outputs[current],outputs,{},false,tone,{id});
        eq(t.star.high,tone);eq(t.star.low,'ground');
        sameGeometry(d,t,`${id}/${tone}`);
        e.toneVariants[tone]=tones(t);
      }
      // Anything that is not a tone falls back to faint.
      const {p,monitors}=planOf(outputs[current],outputs,{},'faint');
      for(const bad of [undefined,'ink','',null]) eq(drafting.plan(outputs[current],p,monitors,glyphs,16,big,'aperture',bad).star.high,'faint');
    });
    matrix.push(e);
  }
}
for(const [name,input] of Object.entries({hd:{...laptop,width:1920,height:1080,scale:1},qhd:{...laptop,width:2560,height:1440,scale:1.5},portrait1080})) {
  const id=`${name}-0-${input.scale}`, d=guard(id,()=>check(input,[input],{},true,undefined,{id}))||planOf(input,[input],{},'faint').d;
  matrix.push(entry(id,name,input.scale,0,[input],d,physical.resolve(input,{})));
}
// The physical vectors: every size source (EDID, estimated, diagonal and measured
// overrides), rotations, even a tiny panel; nothing is dropped, whatever is omitted.
for(const v of vectors.cases) guard('vector '+v.id,()=>check(v.input,[v.input],v.overrides,false,undefined,{small:true}));
if(process.argv[2]==='--json') fs.writeFileSync(process.argv[3],JSON.stringify(matrix));
if(failures.length) {console.error(failures.length+' FAILED:\n'+failures.join('\n'));process.exit(1)}
console.log(`wallpaper: ${matrix.length} layout/output/scale cases plus ${vectors.cases.length} physical vectors; ${facts} facts: bounds, collisions, nothing dropped or (landscape) omitted, star 100/180 mm and true diameters, millimetre reference, identity and locator, DETAIL A, B (window = the star's pixels), sample frequencies, elevation scale, materials, footer, old phrases gone, repeatability and star tones pass`);
