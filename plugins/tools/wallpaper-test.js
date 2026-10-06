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
const glyphs=lib('plugins/quadrille.bar/Q/Glyphs.js');
const big=lib('plugins/quadrille.bar/Q/GlyphsBig.js',{Glyphs:glyphs});
const vectors=JSON.parse(fs.readFileSync(path.join(root,'docs/physical-vectors.json')));
const laptop={...vectors.cases.find(v=>v.id==='laptop').input,make:'AU Optronics',model:'0x07B2',x:0,y:0};
const dell={...vectors.cases.find(v=>v.id==='ultrawide').input,model:'DELL U3417W',x:-952,y:-1440};
const layouts={real:[laptop,dell],vertical:[laptop,{...dell,x:0,y:-1440}],side:[laptop,{...dell,x:1536,y:0}],
  three:[laptop,dell,{...laptop,name:'DP-1',x:1536,y:0}],portrait:[{...laptop,transform:1}],single:[laptop],
  estimated:[{...laptop,name:'Virtual-1',physicalWidth:0,physicalHeight:0}]};
const matrix=[];
function check(input,outputs,composition,overrides={}) {
  const p=physical.resolve(input,overrides), monitors=outputs.map(input=>({input,physical:physical.resolve(input,overrides)}));
  const d=drafting.plan(input,p,monitors,glyphs,16,big,composition);
  for(const s of d.strokes) {
    for(const k of ['x','y','w','h']) assert(Number.isInteger(s[k]));
    assert(s.x>=0&&s.y>=0&&s.w>0&&s.h>0&&s.x+s.w<=d.width&&s.y+s.h<=d.height);
  }
  for(let i=0;i<d.labels.length;i++) {
    const a=d.labels[i];assert(a.x>=24&&a.y>=32&&a.x+a.w<=d.width-24&&a.y+a.h<=d.height-24);
    for(const b of d.labels.slice(0,i)) assert(!drafting.overlaps(a,b,2),`labels ${a.text}/${b.text}`);
    for(const b of d.bodies) assert(!drafting.overlaps(a,b),`label inside body: ${a.text} ${JSON.stringify(a)} ${JSON.stringify(b)}`);
  }
  for(let i=0;i<d.dimensions.length;i++) {
    const dim=d.dimensions[i];
    for(const b of d.bodies) assert(!drafting.overlaps(dim.label,b),`dimension inside body ${dim.output}`);
  }
  for(const m of d.xMarks) assert(Math.abs(m.vpx*p.pixelsPerVpx-m.mm*p.pxPerMmX)<=p.pixelsPerVpx/2+1e-9);
  for(let i=1;i<d.xMarks.length;i++) {
    const step=d.xMarks[i].vpx-d.xMarks[i-1].vpx, ideal=10/p.mmPerVpxX;
    assert([Math.floor(ideal),Math.ceil(ideal)].includes(step),'inconsistent ruler steps');
  }
  if(d.width>=533) {
    assert(d.labels.some(l=>l.text.includes('HOLD A RULER HERE')),`${input.name}/${composition}: missing ruler caption`);
    assert(d.labels.some(l=>l.text.startsWith('1 vpx = ')),`${input.name}/${composition}/${input.scale}/${input.transform}: missing quantisation`);
    assert(d.labels.some(l=>l.text==='DRAWN BY quadrille / REV 02'),`${input.name}/${composition}: missing revision`);
    for(let mm=0;mm<=d.ruler.length;mm+=d.ruler.labelStep) {
      const x=d.ruler.x+Math.round(mm/p.mmPerVpxX)-3;
      assert(d.labels.some(l=>l.text===(p.estimated?'~':'')+mm&&l.x===x&&l.y===d.ruler.y+14),
        `${input.name}/${composition}: inconsistent ruler label at ${mm}`);
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
    for(const composition of ['atlas','comparator','section']) {
      try {check(outputs[current],outputs,composition);} catch(e) {console.error(name,current,scale,composition);throw e;}
    }
    const drawing=check(outputs[current],outputs,'atlas');
    matrix.push({ruler:drawing.ruler,marks:drawing.xMarks,id:`${name}-${current}-${scale}`,name,scale,current,outputs});
  }
}
for(const v of vectors.cases) check(v.input,[v.input],'atlas',v.overrides);
const arr=drafting.layout([laptop,dell].map(input=>({input,physical:physical.resolve(input,{})})));
assert(Math.abs(arr[1].y+arr[1].monitor.physical.heightMm)<1e-9);
if(process.argv[2]==='--json') fs.writeFileSync(process.argv[3],JSON.stringify(matrix));
console.log(`wallpaper: ${matrix.length} layouts x 3 compositions; bounds, collisions, complete ruler captions and exact mark steps pass`);
