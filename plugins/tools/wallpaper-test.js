#!/usr/bin/env node
// The actual painter's plan, using the same physical cases as Rust and QML.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const repo = path.resolve(__dirname, '../..');
function library(file) {
  const context = vm.createContext({});
  vm.runInContext(fs.readFileSync(path.join(repo, file), 'utf8').replace(/^\.pragma library\s*$/m, ''), context);
  return context;
}
const physical = library('plugins/quadrille.background/Physical.js');
const drafting = library('plugins/quadrille.background/Drafting.js');
const glyphs = library('plugins/quadrille.bar/Q/Glyphs.js');
const vectors = JSON.parse(fs.readFileSync(path.join(repo, 'docs/physical-vectors.json'), 'utf8'));
const metrics = {};
for (const vector of vectors.cases) {
  const p = physical.resolve(vector.input, vector.overrides);
  const drawing = drafting.plan(vector.input, p, [{ input: vector.input, physical: p }], glyphs, 16);
  assert.equal(drawing.width, Math.floor(p.widthPx / p.pixelsPerVpx));
  assert.equal(drawing.height, Math.floor(p.heightPx / p.pixelsPerVpx));
  for (const stroke of drawing.strokes) {
    for (const key of ['x', 'y', 'w', 'h']) assert(Number.isInteger(stroke[key]), `${vector.id}: ${key}`);
    assert(stroke.w > 0 && stroke.h > 0);
    assert(stroke.x >= 0 && stroke.y >= 0 && (stroke.x + stroke.w) * p.pixelsPerVpx <= p.widthPx &&
      (stroke.y + stroke.h) * p.pixelsPerVpx <= p.heightPx, `${vector.id}: clipped virtual pixel stroke`);
  }
  for (const label of drawing.labels) assert(glyphs.length(label.text) * 6 + 1 <= label.room, `${vector.id}: cut label`);
  for (const [marks, pixels, density] of [[drawing.xMarks, p.widthPx, p.pxPerMmX], [drawing.yMarks, p.heightPx, p.pxPerMmY]]) {
    for (const mark of marks) {
      const error = Math.abs(mark.vpx * p.pixelsPerVpx - (pixels / 2 + mark.mm * density));
      assert(error <= p.pixelsPerVpx / 2 + 1e-9, `${vector.id}: ruler error ${error}`);
    }
    const ideal = 10 * density / p.pixelsPerVpx;
    for (let i = 1; i < marks.length; i++) assert([Math.floor(ideal), Math.ceil(ideal)].includes(marks[i].vpx - marks[i - 1].vpx));
  }
  if (p.estimated) assert(drawing.labels.some(label => label.text.startsWith('1 vpx = ~')));
  assert(drawing.labels.every(label => !/inches|\bPPI\b/.test(label.text)));
  if (['laptop', 'ultrawide'].includes(vector.id)) {
    const intervals = drawing.xMarks.slice(1).map((mark, i) => (mark.vpx - drawing.xMarks[i].vpx) * p.pixelsPerVpx);
    const errors = drawing.xMarks.map(mark => Math.abs(mark.vpx * p.pixelsPerVpx - (p.widthPx / 2 + mark.mm * p.pxPerMmX)));
    metrics[vector.id] = { idealPxPer10Mm: 10 * p.pxPerMmX, intervalsPx: [...new Set(intervals)].sort((a, b) => a - b),
      maxAbsoluteMarkErrorPx: Math.max(...errors), maxIntervalErrorPx: Math.max(...intervals.map(i => Math.abs(i - 10 * p.pxPerMmX))),
      pixelsPerVpx: p.pixelsPerVpx, xMarks: drawing.xMarks, yMarks: drawing.yMarks };
  }
}
const laptop = vectors.cases.find(v => v.id === 'laptop');
const ultrawide = vectors.cases.find(v => v.id === 'ultrawide');
const monitors = [{ input: { ...laptop.input, x: 0, y: 0 }, physical: physical.resolve(laptop.input, {}) },
  { input: { ...ultrawide.input, x: -952, y: -1440 }, physical: physical.resolve(ultrawide.input, {}) }];
const arrangement = drafting.layout(monitors);
assert.equal(arrangement[0].monitor.input.name, 'eDP-2');
assert(Math.abs(arrangement[1].y + arrangement[1].monitor.physical.heightMm) < 1e-9, 'shared compositor edge');
const flipped = drafting.layout([monitors[0], { input: { ...ultrawide.input, x: 1536, y: 0 }, physical: monitors[1].physical }]);
assert(Math.abs(flipped[1].x - monitors[0].physical.widthMm) < 0.001, 'different-DPI horizontal adjacency');
if (process.argv[2]) fs.writeFileSync(process.argv[2], JSON.stringify(metrics, null, 2));
console.log(`wallpaper: ${vectors.cases.length} physical cases, virtual-grid strokes, dropped labels, rulers and mixed-DPI adjacency pass`);
for (const [name, value] of Object.entries(metrics)) console.log(`${name}: ${value.idealPxPer10Mm.toFixed(6)} px/10 mm; intervals ${value.intervalsPx.join('/')}; max mark error ${value.maxAbsoluteMarkErrorPx.toFixed(6)} px`);
