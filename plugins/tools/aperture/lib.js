// Shared by the Aperture tools: load the sheet's QML JavaScript libraries in node,
// and the monitors and layouts every test uses.
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const root=path.resolve(__dirname,'../../..');
// A QML JavaScript library in its own realm; `.import "X.js" as Name` lines load
// X.js (beside it) as the global Name, as the QML engine would.
function lib(file,globals={}) {
  const src=fs.readFileSync(path.join(root,file),'utf8');
  for(const m of src.matchAll(/^\.import\s+"([^"]+\.js)"\s+as\s+(\w+)/gm))
    if(!(m[2] in globals)) globals[m[2]]=lib(path.join(path.dirname(file),m[1]));
  const ctx=vm.createContext(globals);
  vm.runInContext(src.replace(/^\.(pragma|import).*$/gm,''),ctx,{filename:file});return ctx;
}
const BG='plugins/quadrille.background';
const glyphs=lib('plugins/quadrille.bar/Q/Glyphs.js');
const big=lib('plugins/quadrille.bar/Q/GlyphsBig.js',{Glyphs:glyphs});
const physical=lib(BG+'/Physical.js'), Pen=lib(BG+'/Pen.js'), Star=lib(BG+'/Star.js');
const module_=name=>lib(`${BG}/${name}.js`);
// The user's two outputs as Hyprland reports them, and variants for the case matrix.
const laptop={name:'eDP-2',make:'AU Optronics',model:'0x07B2',width:2560,height:1600,physicalWidth:340,physicalHeight:220,
  transform:0,scale:1.666667,x:0,y:0,refreshRate:240.01401};
const dell={name:'HDMI-A-1',make:'Dell Inc.',model:'DELL U3417W',width:3440,height:1440,physicalWidth:800,physicalHeight:330,
  transform:0,scale:1,x:-952,y:-1440,refreshRate:59.973};
const monitors={laptop, dell,
  hd:{...laptop,name:'HDMI-A-2',make:'',model:'',width:1920,height:1080,physicalWidth:530,physicalHeight:300,scale:1,refreshRate:60},
  qhd:{...laptop,name:'DP-3',make:'',model:'',width:2560,height:1440,physicalWidth:600,physicalHeight:340,scale:1.5,refreshRate:144},
  hidpi:{...laptop,scale:2},                       // 4 physical pixels a virtual pixel
  lowdpi:{...laptop,name:'DP-4',width:1280,height:800,scale:0.5,physicalWidth:0,physicalHeight:0}, // 1 a virtual pixel, estimated
  portrait:{...laptop,transform:1},
  rotated:{...dell,transform:1,x:-1440,y:-400},
  estimated:{...laptop,name:'Virtual-1',make:'',model:'',physicalWidth:0,physicalHeight:0}};
// Layouts: lists of outputs in compositor order (logical positions as Hyprland gives them).
const layouts={
  real:[laptop,dell], side:[laptop,{...dell,x:1536,y:0}], vertical:[laptop,{...dell,x:0,y:-1440}],
  three:[laptop,{...dell,y:-1440},{...laptop,name:'DP-1',x:1536,y:0}],
  rotated:[laptop,{...dell,transform:1,x:-1440,y:-400}], single:[laptop],
};
// The standard context an element is drawn with: this output, every output, and
// the star as the sheet would place it (centre at 0, 0 unless given).
function context(input,outputs,overrides={},star={}) {
  const p=physical.resolve(input,overrides);
  const list=outputs.map(m=>({input:m,physical:physical.resolve(m,overrides)}));
  const current=Math.max(0,outputs.findIndex(m=>m.name===input.name));
  const diameter=star.diameter||100;
  const s={cx:0,cy:0,diameter,rx:Math.round(diameter/(2*p.mmPerVpxX)),ry:Math.round(diameter/(2*p.mmPerVpxY)),high:'faint',low:'ground',...star};
  return {input,physical:p,monitors:list,current,ps:p.pixelsPerVpx,est:p.estimated?'~':'',star:s};
}
module.exports={root,lib,glyphs,big,physical,Pen,Star,module:module_,monitors,layouts,context};
