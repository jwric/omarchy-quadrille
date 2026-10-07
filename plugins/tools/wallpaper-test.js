#!/usr/bin/env node
// Geometry assertions for the real drawing plan; --json emits the render matrix.
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const root=path.resolve(__dirname,'../..');
function lib(file,globals={}) {
  const ctx=vm.createContext(globals);
  vm.runInContext(fs.readFileSync(path.join(root,file),'utf8').replace(/^\.(pragma|import).*$/gm,''),ctx);return ctx;
}
const physical=lib('plugins/quadrille.background/Physical.js');
const drafting=lib('plugins/quadrille.background/Drafting.js');
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
  estimated:[{...laptop,name:'Virtual-1',physicalWidth:0,physicalHeight:0}]};
const roleNames=['void','ground','raised','hover','edge','faint','muted','ink','accent','live','caution','alarm'];
const matrix=[];
// The plan comes from another vm realm: compare by value through JSON.
const same=(a,b,msg)=>assert.equal(JSON.stringify(a),JSON.stringify(b),msg);
const within=(box,c)=>box.x>=c.x&&box.y>=c.y&&box.x+box.w<=c.x+c.w+1&&box.y+box.h<=c.y+c.h;
function tones(d) {return {high:d.star.high,low:d.star.low};}
function summary(d) {
  return {star:d.star,rings:d.rings,bursts:d.bursts,edges:d.targets.filter(t=>t.kind==='slanted'),detail:d.detail,swatches:d.swatches};
}
function planOf(input,outputs,overrides,tone) {
  const p=physical.resolve(input,overrides), monitors=outputs.map(input=>({input,physical:physical.resolve(input,overrides)}));
  return {p,monitors,d:drafting.plan(input,p,monitors,glyphs,16,big,'aperture',tone)};
}
function check(input,outputs,overrides={},repeat=false,tone='faint') {
  const {p,monitors,d}=planOf(input,outputs,overrides,tone), c=d.content, tag=`${input.name} ${d.pixelWidth}x${d.pixelHeight}`;
  for(const s of d.strokes) {
    for(const k of ['x','y','w','h']) assert(Number.isInteger(s[k]));
    assert(s.x>=0&&s.y>=0&&s.w>0&&s.h>0&&s.x+s.w<=d.width&&s.y+s.h<=d.height);
  }
  same(d.plates,[],'plates are gone');
  // Every label lies in the content box, clear of the others, of every target and of the leader.
  for(let i=0;i<d.labels.length;i++) {
    const a=d.labels[i];assert(within(a,c),`${tag}: label outside the content box: ${a.text}`);
    for(const b of d.labels.slice(0,i)) assert(!drafting.overlaps(a,b,3),`${tag}: labels ${a.text}/${b.text}`);
    for(const b of d.targets) assert(!drafting.overlaps(a,b,3),`${tag}: label inside target: ${a.text}`);
    for(const b of d.leaders) assert(!drafting.overlaps(a,b,3),`${tag}: label on the leader: ${a.text}`);
  }
  for(let i=0;i<d.targets.length;i++) {
    const a=d.targets[i];
    assert(a.x>=0&&a.y>=0&&a.x+a.w<=d.width&&a.y+a.h<=d.height);
    for(const b of d.targets.slice(0,i)) assert(!drafting.overlaps(a,b,3),`${tag}: targets collide`);
  }
  assert(Math.abs(2*d.star.rx*p.mmPerVpxX-d.star.diameter)<=p.mmPerVpxX+1e-9);
  assert(Math.abs(2*d.star.ry*p.mmPerVpxY-d.star.diameter)<=p.mmPerVpxY+1e-9);
  assert(Math.abs(d.panelRadius*p.pxPerMmX-32/Math.PI)<1e-9);
  assert(Math.abs(d.gridRadius/p.mmPerVpxX-32/Math.PI)<1e-9);
  // A round pixel gives a round star: the same number of virtual pixels each way.
  if(Math.abs(p.mmPerPixelX-p.mmPerPixelY)<1e-12) assert.equal(d.star.rx,d.star.ry,`${tag}: star is not round`);
  assert.equal(d.star.high,tone);assert.equal(d.star.low,'ground');
  // Repeated change events must produce exactly the same binary wedges and runs.
  if(repeat) assert.equal(JSON.stringify(d),JSON.stringify(drafting.plan(input,p,monitors,glyphs,16,big,'aperture',tone)));
  for(const m of d.xMarks) assert(Math.abs(m.vpx*p.pixelsPerVpx-m.mm*p.pxPerMmX)<=p.pixelsPerVpx/2+1e-9);
  for(let i=1;i<d.xMarks.length;i++) {
    const step=d.xMarks[i].vpx-d.xMarks[i-1].vpx, ideal=10/p.mmPerVpxX;
    assert([Math.floor(ideal),Math.ceil(ideal)].includes(step),'inconsistent ruler steps');
  }
  if(d.bursts.length) {
    // Pushed left, right, left, right: the left flank 12, 6, 3 and the right 2, 1.5, 1.
    same(d.bursts.map(b=>b.period.n),[12,2,6,1.5,3,1],`${tag}: burst periods`);
    for(let i=0;i<6;i++) assert.equal(d.bursts[i].x<d.star.cx,i%2===0,`${tag}: burst ${i} on the wrong flank`);
    for(const b of d.bursts) {assert.equal(b.period.d,1);assert.equal(b.aliased,b.period.n<2);}
  }
  const full=c.w>=440&&c.h>=360;
  if(!full) assert.equal(d.detail,null,`${tag}: compact sheet has a detail`);
  if(full) {
    same(d.dropped,[],`${tag}: dropped ${JSON.stringify(d.dropped)}`);
    for(const fragment of ['HOLD A RULER HERE','GRID STEP = 1 VIRTUAL PIXEL','PANEL PIXELS = ','SHOULD LOOK ROUND','RING A','DETAIL A',
      "THE CENTRE ON THE PANEL'S OWN PIXELS",'PIXELS PER mm','PIXELS ACROSS','SMALL RING','PAIR = ONE LIGHT','FRAME 2% IN','MARKS WITHIN','SIZE '])
      assert(d.labels.some(l=>l.text.includes(fragment)),`${tag}: missing ${fragment}; dropped ${JSON.stringify(d.dropped)}`);
    for(let mm=0;mm<=d.ruler.length;mm+=d.ruler.labelStep) {
      const x=d.ruler.x+Math.round(mm/p.mmPerVpxX)-3, text=(mm>0&&p.estimated?'~':'')+mm;
      assert(d.labels.some(l=>l.text===text&&l.x===x&&l.y===d.ruler.y+14),`${tag}: ruler label at ${mm}`);
    }
    // DETAIL A: the centre of the star on the panel's own pixels.
    const a=d.detail;assert(a,`${tag}: no detail`);
    assert.equal(a.kind,'detail');
    assert(Number.isInteger(a.block)&&Number.isInteger(a.factor));
    assert.equal(a.factor,a.block*p.pixelsPerVpx);
    assert(a.count%2===1&&a.count>=21&&a.count<=43,`${tag}: detail count ${a.count}`);
    assert.equal(a.w,a.count*a.block);assert.equal(a.h,a.count*a.block);
    assert.equal(a.sourceX+(a.count-1)/2,d.star.cx*p.pixelsPerVpx);
    assert.equal(a.sourceY+(a.count-1)/2,d.star.cy*p.pixelsPerVpx);
    assert.equal(a.pitchX,p.mmPerPixelX);
    assert.equal(a.pitchY,p.mmPerPixelY);
    assert(a.x>=c.x&&a.y>=c.y&&a.x+a.w<=c.x+c.w&&a.y+a.h<=c.y+c.h,`${tag}: detail outside the content box`);
    assert(Math.abs(a.panelRing-32/Math.PI*a.block)<1e-9);
    assert(a.gridRing===null||Math.abs(a.gridRing-d.gridRadius/p.mmPerPixelX*a.block)<1e-9);
    const l=a.leader, ringA=Math.round(d.gridRadius/p.mmPerVpxX);
    assert(l,`${tag}: no leader`);
    assert(l.x1-l.x0>0&&l.x1-l.x0===l.y0-l.y1,`${tag}: leader is not a 45 degree diagonal`);
    assert.equal(l.x0-d.star.cx,d.star.cy-l.y0,`${tag}: leader does not leave along the diagonal`);
    const start=Math.hypot(l.x0-d.star.cx,l.y0-d.star.cy);
    assert(start>ringA&&start<=ringA+3,`${tag}: leader starts ${start} from the centre, ring A is ${ringA}`);
    if(l.x2!==undefined) assert(l.x2>l.x1&&l.x2<a.x,`${tag}: leader shoulder`);
    assert(d.leaders.length>0);
    // The palette: twelve roles in order, each named under its swatch.
    assert.equal(d.swatches.length,12);
    d.swatches.forEach((sw,i)=>{
      assert.equal(sw.role,roleNames[i]);
      assert(d.labels.some(l=>l.text===sw.role&&l.x===sw.x&&l.y===sw.y+24),`${tag}: swatch label ${sw.role}`);
      assert(within(sw,c),`${tag}: swatch outside the content box: ${sw.role}`);
      for(const other of d.swatches.slice(0,i)) assert(!drafting.overlaps(sw,other,0),`${tag}: swatches ${sw.role}/${other.role}`);
      for(const l of d.labels) assert(!drafting.overlaps(sw,l,3)||l.text===sw.role&&l.x===sw.x&&l.y===sw.y+24||(l.text!==sw.role&&false),`${tag}: swatch/label ${sw.role}/${l.text}`);
      for(const t of d.targets) assert(!drafting.overlaps(sw,t,3),`${tag}: swatch/target ${sw.role}`);
      for(const t of d.leaders) assert(!drafting.overlaps(sw,t,3),`${tag}: swatch/leader ${sw.role}`);
    });
  }
  return d;
}
// Only the two tones may differ between plans: every box, label and stroke stays where it was.
function sameGeometry(a,b,tag) {
  const bare=d=>JSON.stringify({labels:d.labels,targets:d.targets.map(t=>t===d.star?{...t,high:null}:t),swatches:d.swatches,
    detail:d.detail,leaders:d.leaders,content:d.content,ruler:d.ruler,strokes:d.strokes.map(s=>[s.x,s.y,s.w,s.h])});
  assert.equal(bare(a),bare(b),`${tag}: a star tone moved something`);
}
for(const scale of [1,1.666667]) for(const [name,list] of Object.entries(layouts)) {
  for(let current=0;current<list.length;current++) {
    const outputs=list.map(m=>({...m})); outputs[current].scale=scale;
    if(['real','vertical','three'].includes(name)) outputs[1].y=-outputs[1].height/outputs[1].scale;
    if(name==='side') outputs[1].x=outputs[0].width/outputs[0].scale;
    if(name==='three') outputs[2].x=outputs[0].width/outputs[0].scale;
    let d;
    try {d=check(outputs[current],outputs,{},name==='real');}
    catch(e) {console.error(name,current,scale);throw e;}
    const entry={ruler:d.ruler,marks:d.xMarks,drawings:{aperture:summary(d)},tones:tones(d),panelRadius:d.panelRadius,gridRadius:d.gridRadius,
      physical:physical.resolve(outputs[current],{}),id:`${name}-${current}-${scale}`,name,scale,current,outputs};
    if(name==='real') {
      entry.toneVariants={faint:tones(d)};
      for(const tone of ['edge','muted']) {
        const t=check(outputs[current],outputs,{},false,tone);
        assert.equal(t.star.high,tone);assert.equal(t.star.low,'ground');
        sameGeometry(d,t,`${entry.id}/${tone}`);
        entry.toneVariants[tone]=tones(t);
      }
      // Anything that is not a tone falls back to faint.
      const {p,monitors}=planOf(outputs[current],outputs,{},'faint');
      for(const bad of [undefined,'ink','',null]) assert.equal(drafting.plan(outputs[current],p,monitors,glyphs,16,big,'aperture',bad).star.high,'faint');
    }
    matrix.push(entry);
  }
}
for(const [name,input] of Object.entries({hd:{...laptop,width:1920,height:1080,scale:1},qhd:{...laptop,width:2560,height:1440,scale:1.5}})) {
  const d=check(input,[input],{},true);
  matrix.push({ruler:d.ruler,marks:d.xMarks,drawings:{aperture:summary(d)},tones:tones(d),panelRadius:d.panelRadius,gridRadius:d.gridRadius,
    physical:physical.resolve(input,{}),id:`${name}-0-${input.scale}`,name,scale:input.scale,current:0,outputs:[input]});
}
for(const v of vectors.cases) check(v.input,[v.input],v.overrides);
if(process.argv[2]==='--json') fs.writeFileSync(process.argv[3],JSON.stringify(matrix));
console.log(`wallpaper: ${matrix.length} layout/output/scale cases; bounds, collisions, nothing dropped, detail A, palette, star tones, true diameters, both Nyquist radii, repeatability and exact mark steps pass`);
