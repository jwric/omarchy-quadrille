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
const matrix=[];
function check(input,outputs,composition,overrides={},repeat=false) {
  const p=physical.resolve(input,overrides), monitors=outputs.map(input=>({input,physical:physical.resolve(input,overrides)}));
  const d=drafting.plan(input,p,monitors,glyphs,16,big,composition);
  for(const s of d.strokes) {
    for(const k of ['x','y','w','h']) assert(Number.isInteger(s[k]));
    assert(s.x>=0&&s.y>=0&&s.w>0&&s.h>0&&s.x+s.w<=d.width&&s.y+s.h<=d.height);
  }
  for(let i=0;i<d.labels.length;i++) {
    const a=d.labels[i];assert(a.x>=24&&a.y>=32&&a.x+a.w<=d.width-24&&a.y+a.h<=d.height-16);
    for(const b of d.labels.slice(0,i)) assert(!drafting.overlaps(a,b,3),`labels ${a.text}/${b.text}`);
    for(const b of d.targets) assert(!drafting.overlaps(a,b,3),`label inside target: ${a.text}`);
  }
  for(let i=0;i<d.targets.length;i++) {
    const a=d.targets[i];
    assert(a.x>=0&&a.y>=0&&a.x+a.w<=d.width&&a.y+a.h<=d.height);
    for(const b of d.targets.slice(0,i)) assert(!drafting.overlaps(a,b,3),'targets collide');
    for(const b of d.plates) assert(!drafting.overlaps(a,b,3),'plate/target collide');
  }
  for(const p of d.plates) for(const l of d.labels) {
    if(!['void','ground','raised','hover','edge','faint','muted','ink','accent','live','caution','alarm'].includes(l.text))
      assert(!drafting.overlaps(p,l,3),`plate/label ${l.text}`);
  }
  assert(Math.abs(2*d.star.rx*p.mmPerVpxX-d.star.diameter)<=p.mmPerVpxX+1e-9);
  assert(Math.abs(2*d.star.ry*p.mmPerVpxY-d.star.diameter)<=p.mmPerVpxY+1e-9);
  assert(Math.abs(d.panelRadius*p.pxPerMmX-32/Math.PI)<1e-9);
  assert(Math.abs(d.gridRadius/p.mmPerVpxX-32/Math.PI)<1e-9);
  // Repeated change events must produce exactly the same binary wedges and runs.
  if(repeat) assert.equal(JSON.stringify(d),JSON.stringify(drafting.plan(input,p,monitors,glyphs,16,big,composition)));
  for(const m of d.xMarks) assert(Math.abs(m.vpx*p.pixelsPerVpx-m.mm*p.pxPerMmX)<=p.pixelsPerVpx/2+1e-9);
  for(let i=1;i<d.xMarks.length;i++) {
    const step=d.xMarks[i].vpx-d.xMarks[i-1].vpx, ideal=10/p.mmPerVpxX;
    assert([Math.floor(ideal),Math.ceil(ideal)].includes(step),'inconsistent ruler steps');
  }
  if(d.width>=533) {
    for(const fragment of ['HOLD A RULER HERE','1 vpx = ','EDID ','ALIAS INSIDE','ASPECT CHECK','SAFE FRAME'])
      assert(d.labels.some(l=>l.text.includes(fragment)),`${input.name}/${composition}: missing ${fragment}; dropped ${d.dropped}`);
    for(let mm=0;mm<=d.ruler.length;mm+=d.ruler.labelStep) {
      const x=d.ruler.x+Math.round(mm/p.mmPerVpxX)-3;
      assert(d.labels.some(l=>l.text===(p.estimated?'~':'')+mm&&l.x===x&&l.y===d.ruler.y+14),`ruler label at ${mm}`);
    }
  }
  return d;
}
for(const scale of [1,1.666667]) for(const [name,list] of Object.entries(layouts)) {
  for(let current=0;current<list.length;current++) {
    const outputs=list.map(m=>({...m})); outputs[current].scale=scale;
    if(['real','vertical','three'].includes(name)) outputs[1].y=-outputs[1].height/outputs[1].scale;
    if(name==='side') outputs[1].x=outputs[0].width/outputs[0].scale;
    if(name==='three') outputs[2].x=outputs[0].width/outputs[0].scale;
    const drawings={};let drawing;
    for(const composition of ['aperture','broadcast','bench']) {
      try {
        const d=check(outputs[current],outputs,composition,{},name==='real');
        if(composition==='aperture') drawing=d;
        drawings[composition]={star:d.star,rings:d.rings,bursts:d.bursts,
          edges:d.targets.filter(t=>t.kind==='slanted')};
      } catch(e) {console.error(name,current,scale,composition);throw e;}
    }
    matrix.push({ruler:drawing.ruler,marks:drawing.xMarks,drawings,panelRadius:drawing.panelRadius,gridRadius:drawing.gridRadius,physical:physical.resolve(outputs[current],{}),id:`${name}-${current}-${scale}`,name,scale,current,outputs});
  }
}
for(const v of vectors.cases) check(v.input,[v.input],'aperture',v.overrides);
if(process.argv[2]==='--json') fs.writeFileSync(process.argv[3],JSON.stringify(matrix));
console.log(`wallpaper: ${matrix.length} layouts x 3 compositions; bounds, collisions, true diameters, both Nyquist radii, repeatability and exact mark steps pass`);
