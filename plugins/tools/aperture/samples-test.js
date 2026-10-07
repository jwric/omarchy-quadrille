#!/usr/bin/env node
// Samples.js: PATTERN -> RESULT. Every standard monitor and layout, every config.
const assert0 = require('node:assert/strict');
// Objects come from the QML library's own realm: compare by value, not prototype.
const assert = Object.assign((...a) => assert0(...a), assert0, {deepEqual: (a, b, m) => assert0.equal(JSON.stringify(a), JSON.stringify(b), m)});
const A = require('./lib.js');
const mod = A.module('Samples');
const cfgs = [{}, {compact: true}, {actual: true}, {compact: true, actual: true}, {cells: 10}];
const gap = (a, b, g) => a.x < b.x + b.w + g && b.x < a.x + a.w + g && a.y < b.y + b.h + g && b.y < a.y + a.h + g;

// Independent sample of the grating: n = num/den, shifted by phase, at integer t.
const sampleAt = (num, den, phase, t) => Math.floor(2 * den * (t + phase) / num) % 2 === 0 ? 'ink' : 'void';
const CASES = {'2': [2, 1, 0], '4/3': [4, 3, 3], '1': [1, 1, 0]};

function run(monitor, layout, cfg) {
  const input = A.monitors[monitor];
  const outputs = (A.layouts[layout] || [input]).map(m => m.name === input.name ? input : m);
  if (!outputs.some(m => m.name === input.name)) outputs.unshift(input);
  const ctx = {...A.context(input, outputs), ...cfg};
  const m = mod.measure(ctx);
  assert.ok(m && m.w > 0 && m.h > 0, 'measure');
  const M = 16, box = {L: M, T: M, R: M + m.w - 1, B: M + m.h};
  const pen = A.Pen.make(m.w + 2 * M, m.h + 2 * M, A.glyphs, A.big, box);
  const info = mod.draw(pen, M, M, ctx);
  const where = `${monitor}/${layout}/${JSON.stringify(cfg)}`;
  const fail = s => { throw new Error(`${where}: ${s}`); };

  // Strokes: whole vpx, inside the measured box; nothing dropped.
  for (const s of pen.strokes) {
    for (const v of [s.x, s.y, s.w, s.h]) if (!Number.isInteger(v)) fail('non-integer stroke ' + JSON.stringify(s));
    if (s.x < M || s.y < M || s.x + s.w > M + m.w || s.y + s.h > M + m.h) fail('stroke outside box ' + JSON.stringify(s));
  }
  for (const l of pen.labels) if (l.x < M || l.y < M || l.x + l.w > M + m.w + 1 || l.y + l.h > M + m.h) fail('label outside box ' + l.text);
  assert.deepEqual(pen.dropped, [], where + ' dropped');
  // Targets apart by 4, labels clear of targets and leaders by 4, of each other by 3.
  for (let i = 0; i < pen.targets.length; i++) for (let j = i + 1; j < pen.targets.length; j++)
    if (gap(pen.targets[i], pen.targets[j], 4)) fail('targets too close ' + JSON.stringify([pen.targets[i], pen.targets[j]]));
  for (const l of pen.labels) {
    for (const t of pen.targets) if (gap(l, t, 4)) fail(`label ${l.text} too close to target ${t.kind}`);
    for (const t of pen.leaders) if (gap(l, t, 4)) fail(`label ${l.text} too close to a leader`);
  }
  for (let i = 0; i < pen.labels.length; i++) for (let j = i + 1; j < pen.labels.length; j++)
    if (gap(pen.labels[i], pen.labels[j], 3)) fail('labels too close');
  assert.deepEqual(info.labels.map(l => [l.text, l.x, l.y, l.w, l.h]).sort(), pen.labels.map(l => [l.text, l.x, l.y, l.w, l.h]).sort(), 'info.labels is the drawn labels');
  const texts = pen.labels.map(l => l.text);
  for (const t of ['PAIRS', 'PER mm', 'PATTERN', 'RESULT 6:1', '1 PAIR', 'RESOLVED', 'FALSE DETAIL', 'ALIASED', 'DETAIL LOST'])
    if (!texts.includes(t)) fail('missing label ' + t);
  assert.equal(texts.filter(t => /6:1/.test(t)).length, 1, '6:1 once');
  assert.equal(texts.filter(t => t === '1:1').length, cfg.actual ? 1 : 0, '1:1 label');
  assert.equal(texts.filter(t => t === '1 PAIR').length, 1);

  // The painted sheet, to compare what is on it with what info says.
  const paint = new Map();
  for (const s of pen.strokes) for (let y = s.y; y < s.y + s.h; y++) for (let x = s.x; x < s.x + s.w; x++) paint.set(x + ',' + y, s.role);
  const at = (x, y) => paint.get(x + ',' + y) || null;

  const cells = info.cells, E = info.factor;
  assert.equal(E, 6);
  assert.equal(cells, cfg.cells || (cfg.compact ? 6 : 8));
  const mm = ctx.physical.mmPerVpxX;
  assert.deepEqual(info.cases.map(c => c.n), ['2', '4/3', '1']);
  assert.equal(info.numbers.length, 3);
  for (const c of info.cases) {
    const [num, den, phase] = CASES[c.n];
    assert.equal(c.phase, phase);
    // Frequency: 1 / (n mmPerVpxX), 2 decimals, est prefix.
    const f = den / (num * mm);
    assert.equal(c.text, ctx.est + f.toFixed(2), where + ' frequency text');
    assert.ok(Math.abs(c.frequency - f) < 1e-9);
    // Pattern: 6 x cells wide, 18 tall; every enlarged column is the grating at X/6.
    assert.equal(c.pattern.w, cells * E); assert.equal(c.pattern.h, 18); assert.equal(c.pattern.roles.length, cells * E);
    for (let X = 0; X < c.pattern.w; X++) {
      const want = Math.floor(2 * den * (X + E * phase) / (E * num)) % 2 === 0 ? 'ink' : 'void';
      assert.equal(c.pattern.roles[X], want, `${where} n=${c.n} pattern column ${X}`);
      for (const dy of [0, 17]) assert.equal(at(c.pattern.x + X, c.pattern.y + dy), want, `painted pattern ${c.n} ${X}`);
    }
    // Stripe widths inside the view (not the two window ends): 6, 4, 3.
    const runs = []; let start = 0;
    for (let X = 1; X <= c.pattern.w; X++) if (X === c.pattern.w || c.pattern.roles[X] !== c.pattern.roles[start]) { runs.push(X - start); start = X; }
    const inner = runs.slice(1, -1), width = {'2': 6, '4/3': 4, '1': 3}[c.n];
    assert.ok(inner.length >= 2 && inner.every(w => w === width), `${where} n=${c.n} stripe widths ${runs}`);
    // Ticks: 2 vpx `line` at c * 6, 1 vpx under the view.
    assert.deepEqual(c.ticks, Array.from({length: cells}, (_, k) => c.pattern.x + k * E));
    for (const tx of c.ticks) {
      assert.equal(at(tx, c.pattern.y + 17), c.pattern.roles[tx - c.pattern.x]);
      assert.equal(at(tx, c.pattern.y + 18), 'edge');       // the frame's bottom row
      assert.equal(at(tx, c.pattern.y + 19), null);         // one vpx clear of the frame
      assert.equal(at(tx, c.pattern.y + 20), 'line'); assert.equal(at(tx, c.pattern.y + 21), 'line');
      assert.equal(at(tx, c.pattern.y + 22), null);
    }
    // Result: the sheet's raster. Compare with an independent point sample at integer vpx.
    assert.equal(c.result.w, cells * E); assert.equal(c.result.h, 18); assert.equal(c.result.cells.length, cells);
    for (let k = 0; k < cells; k++) {
      assert.equal(c.result.cells[k], sampleAt(num, den, phase, k), `${where} n=${c.n} result cell ${k}`);
      for (let dx = 0; dx < E; dx++) for (const dy of [0, 17])
        assert.equal(at(c.result.x + k * E + dx, c.result.y + dy), c.result.cells[k], `painted result ${c.n} ${k}`);
    }
    if (c.n === '2') c.result.cells.forEach((r, k) => k && assert.notEqual(r, c.result.cells[k - 1], 'n=2 alternates'));
    if (c.n === '4/3') c.result.cells.forEach((r, k) => {
      assert.equal(r, c.result.cells[k % 4], 'period 4');
      assert.equal(c.result.cells[0], c.result.cells[1]); assert.equal(c.result.cells[2], c.result.cells[3]);
      assert.notEqual(c.result.cells[1], c.result.cells[2]);
    });
    if (c.n === '1') assert.ok(c.result.cells.every(r => r === c.result.cells[0]), 'n=1 flat');
    // Arrow: 10 shaft + head, 4 or more clear of both views, one vpx `line`.
    assert.ok(c.arrow.x0 - (c.pattern.frame.x + c.pattern.frame.w) >= 4 && c.result.frame.x - (c.arrow.x1 + 1) >= 4, 'arrow air from the frames');
    for (let ax = c.arrow.x0; ax <= c.arrow.x0 + 9; ax++) assert.equal(at(ax, c.arrow.y), 'line');
    for (let k = 0; k < 3; k++) for (let d = -k; d <= k; d++) assert.equal(at(c.arrow.x1 - k, c.arrow.y + d), 'line');
    assert.equal(c.arrow.y, c.pattern.y + 8);
    // Actual 1:1.
    if (cfg.actual) {
      assert.ok(c.actual && c.actual.w === cells && c.actual.h === 18);
      assert.equal(c.actual.x - (c.result.x + c.result.w), 7);
      for (let k = 0; k < cells; k++) for (let dy = 0; dy < 18; dy++) assert.equal(at(c.actual.x + k, c.actual.y + dy), c.result.cells[k]);
    } else assert.equal(c.actual, null);
    // The frequency is printed vertically centred on the view (box 12 tall in 18).
    const lab = pen.labels.find(l => l.text === c.text && l.y === c.pattern.y + 3);
    assert.ok(lab, 'number label centred');
    const name = pen.labels.find(l => l.text === c.label);
    assert.equal(name.y, c.pattern.y + 3);
    const right = cfg.actual ? c.actual.x + c.actual.w : c.result.x + c.result.w;
    assert.equal(name.x - right, 8);
    if (c.technical) { const t = pen.labels.find(l => l.text === c.technical); assert.ok(t && t.x === name.x && t.y > name.y); }
  }
  // Numbers right-aligned in one column, headings over them.
  const nl = info.cases.map(c => pen.labels.find(l => l.text === c.text));
  assert.ok(nl.every(l => l.x + l.w === nl[0].x + nl[0].w), 'numbers right-aligned');
  for (const h of ['PAIRS', 'PER mm']) assert.equal(pen.labels.find(l => l.text === h).x + pen.labels.find(l => l.text === h).w, nl[0].x + nl[0].w);
  // Frames: a one vpx `edge` outline one vpx outside every view, which is the registered target
  // (the pattern's also takes in its ticks); nothing inside it but the view itself.
  for (const c of info.cases) for (const [v, kind] of [[c.pattern, 'samples-pattern'], [c.result, 'samples-result'], [c.actual, 'samples-actual']]) {
    if (!v) continue;
    const f = v.frame;
    assert.deepEqual(f, {x: v.x - 1, y: v.y - 1, w: v.w + 2, h: v.h + 2}, 'frame is one vpx outside');
    for (let i = 0; i < f.w; i++) for (const yy of [f.y, f.y + f.h - 1]) assert.equal(at(f.x + i, yy), 'edge', `${where} frame row ${kind}`);
    for (let j = 0; j < f.h; j++) for (const xx of [f.x, f.x + f.w - 1]) assert.equal(at(xx, f.y + j), 'edge', `${where} frame column ${kind}`);
    for (let j = 0; j < v.h; j++) for (let i = 0; i < v.w; i++) assert.notEqual(at(v.x + i, v.y + j), 'edge', 'the view holds no frame colour');
    const t = pen.targets.find(t => t.kind === kind && t.x === f.x && t.y === f.y);
    assert.ok(t && t.w === f.w && t.h >= f.h, `${where} target is the framed box ${kind}`);
    if (kind !== 'samples-pattern') assert.equal(t.h, f.h);
  }
  assert.equal(pen.targets.length, info.cases.length * (cfg.actual ? 3 : 2), 'one target a view');
  // The bracket spans exactly one light and one dark stripe of the first pattern.
  const b = info.bracket, p0 = info.cases[0].pattern;
  assert.equal(b.x1 - b.x0 + 1, 12); assert.equal(b.x0, p0.x);
  assert.equal(b.y, p0.y - 5);
  const seg = p0.roles.slice(b.x0 - p0.x, b.x1 - p0.x + 1);
  assert.deepEqual(seg, [...Array(6).fill('ink'), ...Array(6).fill('void')], 'bracket spans one pair');
  for (let bx = b.x0; bx <= b.x1; bx++) assert.equal(at(bx, b.y), 'line');
  for (const ex of [b.x0, b.x1]) { assert.equal(at(ex, b.y + 1), 'line'); assert.equal(at(ex, b.y + 2), null); }
  assert.equal(p0.frame.y - (b.y + 1) - 1, 2, 'two vpx of air above the frame');
  const pl = pen.labels.find(l => l.text === '1 PAIR'); assert.ok(pl.x > b.x1 + 4);
  // The labels' roles are right: numbers and headings muted, case names ink, ALIASED muted.
  const roleOf = (l) => { const s = pen.strokes.find(s => s.x >= l.x && s.x < l.x + l.w && s.y >= l.y && s.y < l.y + l.h); return s && s.role; };
  for (const t of ['PAIRS', 'PER mm', 'PATTERN', 'RESULT 6:1', '1 PAIR', 'ALIASED', ...info.numbers]) assert.equal(roleOf(pen.labels.find(l => l.text === t)), 'muted', t);
  for (const t of ['RESOLVED', 'FALSE DETAIL', 'DETAIL LOST']) assert.equal(roleOf(pen.labels.find(l => l.text === t)), 'ink', t);
  return {m, info, pen, ctx};
}

let n = 0;
for (const monitor of Object.keys(A.monitors)) for (const layout of Object.keys(A.layouts)) for (const cfg of cfgs) {
  const a = run(monitor, layout, cfg), b = run(monitor, layout, cfg);
  assert.deepEqual(a.pen.strokes, b.pen.strokes, 'determinism');
  assert.deepEqual(a.info, b.info, 'determinism (info)');
  assert.deepEqual(JSON.parse(JSON.stringify(a.info)), a.info, 'info is plain JSON');
  n++;
}
// The estimated outputs carry the ~ prefix on every number; measured ones do not.
for (const [name, est] of [['estimated', true], ['lowdpi', true], ['laptop', false], ['dell', false]]) {
  const r = run(name, 'single', {});
  assert.ok(r.info.numbers.every(s => s.startsWith('~') === est), name + ' est prefix ' + r.info.numbers);
}
// An output with no usable size cannot be drawn.
assert.equal(mod.measure({...A.context(A.monitors.laptop, [A.monitors.laptop]), physical: {mmPerVpxX: 0}}), null);
const lap = run('laptop', 'real', {actual: true}), dl = run('dell', 'real', {});
console.log(`samples-test: ok, ${n} cases x2 (monitors x layouts x configs); laptop ${lap.info.numbers.join(' ')} mm^-1 (${lap.m.w}x${lap.m.h} with 1:1), dell ${dl.info.numbers.join(' ')} (${dl.m.w}x${dl.m.h})`);
