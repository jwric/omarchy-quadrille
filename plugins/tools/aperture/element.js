#!/usr/bin/env node
// element.js MODULE OUT.json SPEC.json
// Draws one Aperture element alone on a void sheet, inside a margin, and checks that it
// stays in its measured box (every stroke, label and target) and drops nothing.
// SPEC: {"monitor":"laptop"|..., "layout":"real"|..., "cfg":{...}, "margin":16}
// Prints the measured size and the draw() result; writes strokes for element.py.
const fs=require('node:fs'),A=require('./lib.js');
const [name,out,specFile]=process.argv.slice(2);
const spec=JSON.parse(fs.readFileSync(specFile,'utf8'));
const mod=A.module(name), input=A.monitors[spec.monitor||'laptop'];
const outputs=(A.layouts[spec.layout||'single']||[input]).map(m=>m.name===input.name?input:m);
if(!outputs.some(m=>m.name===input.name)) outputs.unshift(input);
const ctx={...A.context(input,outputs,{},spec.star||{}),...(spec.cfg||{})};
const m=mod.measure(ctx), M=spec.margin||16;
const box={L:M,T:M,R:M+m.w-1,B:M+m.h};
const pen=A.Pen.make(m.w+2*M,m.h+2*M,A.glyphs,A.big,box);
const result=mod.draw(pen,M,M,ctx);
const bad=pen.strokes.filter(s=>s.x<M||s.y<M||s.x+s.w>M+m.w||s.y+s.h>M+m.h);
console.log(JSON.stringify({measure:m,ps:ctx.ps,dropped:pen.dropped,outside:bad.length,result},null,0));
fs.writeFileSync(out,JSON.stringify([{id:name,ps:ctx.ps,width:pen.w,height:pen.h,pixelWidth:pen.w*ctx.ps,pixelHeight:pen.h*ctx.ps,
  strokes:pen.strokes.map(s=>[s.x,s.y,s.w,s.h,s.role]),labels:pen.labels,targets:pen.targets}]));
if(bad.length||pen.dropped.length) process.exitCode=1;
