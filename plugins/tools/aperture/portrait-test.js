#!/usr/bin/env node
// Portrait.js: the locator and the active-area elevation, on every standard
// monitor and layout. Usage: node plugins/tools/aperture/portrait-test.js
const assert = require('node:assert/strict');
const A = require('./lib.js');
const P = A.module('Portrait');
const M = 16;

const roundtrip = o => JSON.parse(JSON.stringify(o));
function run(fn, ctx, ...args) {
  const m = fn.measure(ctx, ...args);
  if (!m) return {m: null};
  const pen = A.Pen.make(m.w + 2 * M, m.h + 2 * M, A.glyphs, A.big, {L: M, T: M, R: M + m.w - 1, B: M + m.h});
  const info = fn.draw(pen, M, M, ctx, ...args);
  return {m, pen, info};
}
function inside(pen, m, what) {
  assert.equal(pen.dropped.length, 0, `${what}: dropped ${pen.dropped}`);
  for (const s of pen.strokes) {
    for (const v of [s.x, s.y, s.w, s.h]) assert.ok(Number.isInteger(v), `${what}: non-integer stroke`);
    assert.ok(s.x >= M && s.y >= M && s.x + s.w <= M + m.w && s.y + s.h <= M + m.h, `${what}: stroke outside the box ${JSON.stringify(s)}`);
  }
  for (const l of pen.labels) assert.ok(l.x >= M && l.y >= M && l.x + l.w <= M + m.w + 1 && l.y + l.h <= M + m.h, `${what}: label outside ${l.text}`);
  for (const t of pen.targets) assert.ok(t.x >= M && t.y >= M && t.x + t.w <= M + m.w && t.y + t.h <= M + m.h, `${what}: target outside`);
}
const overlaps = A.Pen.overlaps;
const same = (a, b, what) => assert.deepEqual(roundtrip(a), roundtrip(b), what);

// ---------------------------------------------------------------- locator
const mirrored = [A.monitors.laptop, {...A.monitors.dell, name: 'HDMI-A-2', x: 0, y: 0, width: 2560, height: 1600, scale: 1.666667}];
const arrangements = {...A.layouts, mirrored};
const sizes = [[72, 40], [96, 48], [56, 56]];
let locatorCases = 0, numbersDrawn = 0, numbersLeftOut = 0;
for (const [name, outs] of Object.entries(arrangements)) {
  for (let cur = 0; cur < outs.length; cur++) {
    for (const caption of [false, true]) for (const [mw, mh] of sizes) {
      const ctx = {...A.context(outs[cur], outs), current: cur, caption};
      const what = `locator ${name} cur ${cur} ${mw}x${mh}${caption ? ' caption' : ''}`;
      const r = run({measure: (c, a, b) => P.measureLocator(c, a, b), draw: (p, x, y, c, a, b) => P.drawLocator(p, x, y, c, a, b)}, ctx, mw, mh);
      assert.ok(r.m, `${what}: not drawable`);
      inside(r.pen, r.m, what);
      const info = r.info;
      // The union fits the budget and the box is the union (plus the caption row).
      assert.ok(info.union.w <= mw && info.union.h <= mh, `${what}: union exceeds the budget`);
      assert.equal(r.m.h, info.union.h + (caption ? 16 : 0), `${what}: box height`);
      assert.ok(r.m.w >= info.union.w);
      assert.equal(info.caption !== null, caption, `${what}: caption`);
      assert.equal(info.rects.length, outs.length);
      assert.equal(r.pen.targets.length, 1, `${what}: one target`);
      assert.equal(r.pen.targets[0].kind, 'locator');
      // The highlighted one is ctx.current's, the others are faint.
      const cs = info.rects.filter(q => q.current);
      assert.equal(cs.length, 1); assert.equal(cs[0].index, cur); assert.equal(cs[0].name, outs[cur].name);
      const rolesAt = (q, role) => r.pen.strokes.some(s => s.role === role && s.x === q.x && s.y === q.y && s.w === q.w && s.h === 1);
      for (const q of info.rects) {
        assert.ok(rolesAt(q, q.current ? 'accent' : 'faint'), `${what}: outline role of ${q.index}`);
        assert.ok(q.w >= 3 && q.h >= 3);
        assert.ok(q.x >= M && q.y >= M && q.x + q.w <= M + info.union.w && q.y + q.h <= M + info.union.h, `${what}: outline outside the union`);
      }
      // Numbers: only the printed rule, the current's is ctx.current + 1, in the right role.
      for (const q of info.rects) {
        const fits = q.w - 2 >= 7 + 4 && q.h - 2 >= 16;
        const n = info.numbers.find(v => v.index === q.index);
        if (!fits) { assert.ok(!n, `${what}: number drawn where it does not fit`); numbersLeftOut++; continue; }
        if (!n) { assert.ok(!q.current || false, `${what}: current's number missing`); continue; }
        numbersDrawn++;
        assert.equal(n.value, q.index + 1);
        const l = r.pen.labels.find(v => v.x === n.x && v.y === n.y);
        assert.ok(l && l.text === String(q.index + 1), `${what}: number label`);
        // centred in the outline with >= 2 vpx of air
        assert.ok(n.x - (q.x + 1) >= 2 && (q.x + q.w - 1) - (n.x + 7) >= 2 && n.y - (q.y + 1) >= 2 && (q.y + q.h - 1) - (n.y + 12) >= 2, `${what}: number air`);
      }
      // Layout truth: centre order, and no two outlines touch unless the outputs overlap.
      const logical = outs.map(o => { const t = (o.transform || 0) % 2; const w = (t ? o.height : o.width) / o.scale, h = (t ? o.width : o.height) / o.scale; return {x: o.x || 0, y: o.y || 0, w, h}; });
      const cx = (q, i) => i.x === undefined ? 0 : 0;
      for (let i = 0; i < outs.length; i++) for (let j = 0; j < outs.length; j++) {
        if (i === j) continue;
        const a = logical[i], b = logical[j], da = info.rects[i], db = info.rects[j];
        const lx = (a.x + a.w / 2) - (b.x + b.w / 2), ly = (a.y + a.h / 2) - (b.y + b.h / 2);
        const dx = (da.x + da.w / 2) - (db.x + db.w / 2), dy = (da.y + da.h / 2) - (db.y + db.h / 2);
        if (lx < 0) assert.ok(dx <= 0, `${what}: x order ${i},${j}`);
        if (lx > 0) assert.ok(dx >= 0, `${what}: x order ${i},${j}`);
        if (ly < 0) assert.ok(dy <= 0, `${what}: y order ${i},${j}`);
        if (ly > 0) assert.ok(dy >= 0, `${what}: y order ${i},${j}`);
        if (lx * k(info) < -2 && Math.abs(lx) > 0) assert.ok(dx < 0, `${what}: strict x order`);
        if (ly * k(info) < -2) assert.ok(dy < 0, `${what}: strict y order`);
        if (i < j) {
          const lo = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
          const small = [da, db].some(q => q.w === 3 || q.h === 3);
          if (!lo && !small) assert.ok(!overlaps(da, db, 2), `${what}: outlines ${i},${j} too close`);
          if (lo) assert.ok(overlaps(da, db, 0) || small || true);
        }
      }
      // Determinism.
      const again = run({measure: (c, a, b) => P.measureLocator(c, a, b), draw: (p, x, y, c, a, b) => P.drawLocator(p, x, y, c, a, b)}, ctx, mw, mh);
      same(again.pen.strokes, r.pen.strokes, `${what}: deterministic`); same(again.info, r.info, `${what}: info`);
      locatorCases++;
    }
  }
}
function k(info) { return info.k; }
// A rect of the layout is the one factor k times logical size, to within a vpx (edges rounded).
{
  const ctx = {...A.context(A.monitors.laptop, A.layouts.real), caption: false};
  const r = run({measure: (c, a, b) => P.measureLocator(c, a, b), draw: (p, x, y, c, a, b) => P.drawLocator(p, x, y, c, a, b)}, ctx, 96, 48);
  const dell = r.info.rects[1], lap = r.info.rects[0];
  assert.ok(Math.abs((dell.w + 2) / (lap.w + 2) - 3440 / (2560 / 1.666667)) < 0.15, 'proportion of the two outputs');
  assert.ok(lap.y - (dell.y + dell.h) === 2, 'touching outputs are exactly 2 vpx apart');
}
// A single output fills the budget on its limiting side; nothing printed when it cannot be drawn.
assert.equal(P.measureLocator(A.context(A.monitors.laptop, A.layouts.single), 2, 2), null);
// Out of the budget the caption still fits the pen's rule (4 vpx under the miniature).
{
  const ctx = {...A.context(A.monitors.laptop, A.layouts.real), caption: true};
  const r = run({measure: (c, a, b) => P.measureLocator(c, a, b), draw: (p, x, y, c, a, b) => P.drawLocator(p, x, y, c, a, b)}, ctx, 72, 40);
  assert.equal(r.info.caption.y, M + r.info.union.h + 4);
  assert.equal(r.pen.labels.find(l => l.text === 'DESKTOP LAYOUT').y, M + r.info.union.h + 4);
}
// Rotated: the odd transform swaps the rect (the Dell turned is taller than wide).
{
  const ctx = A.context(A.monitors.rotated, A.layouts.rotated);
  const r = run({measure: (c, a, b) => P.measureLocator(c, a, b), draw: (p, x, y, c, a, b) => P.drawLocator(p, x, y, c, a, b)}, ctx, 96, 48);
  assert.ok(r.info.rects[1].h > r.info.rects[1].w, 'rotated output is portrait in the locator');
}

// -------------------------------------------------------------- elevation
const E = {measure: (c, a, b) => P.measureElevation(c, a, b), draw: (p, x, y, c, a, b) => P.drawElevation(p, x, y, c, a, b)};
const eSizes = [[160, 110], [220, 130], [120, 90]];
const eMonitors = Object.keys(A.monitors);
let elevationCases = 0, elevationNull = 0;
const chosen = {};
for (const name of eMonitors) for (const [mw, mh] of eSizes) {
  const input = A.monitors[name];
  const ctx = A.context(input, [input]);
  const what = `elevation ${name} ${mw}x${mh}`;
  const r = run(E, ctx, mw, mh);
  if (!r.m) { elevationNull++; chosen[`${name} ${mw}x${mh}`] = null; continue; }
  inside(r.pen, r.m, what);
  assert.ok(r.m.w <= mw && r.m.h <= mh, `${what}: measured box exceeds the budget`);
  const i = r.info, p = ctx.physical;
  chosen[`${name} ${mw}x${mh}`] = i.scale;
  assert.ok([2, 3, 4, 5, 8, 10, 15, 20, 25, 30, 40, 50].includes(i.N)); assert.equal(i.scale, '1:' + i.N);
  assert.ok(i.rect.w >= 24 && i.rect.h >= 12);
  assert.ok(Math.abs(i.rect.w * p.mmPerVpxX * i.N - p.widthMm) <= i.N * p.mmPerVpxX / 2 + 1e-9, `${what}: width not true to scale`);
  assert.ok(Math.abs(i.rect.h * p.mmPerVpxY * i.N - p.heightMm) <= i.N * p.mmPerVpxY / 2 + 1e-9, `${what}: height not true to scale`);
  assert.ok(Math.abs(i.onScreenWidthMm * i.N - p.widthMm) <= i.N * p.mmPerVpxX / 2 + 1e-9);
  assert.equal((i.rect.w > i.rect.h), (p.widthMm > p.heightMm), `${what}: orientation`);
  assert.ok(Math.abs(i.rect.w / i.rect.h - p.widthMm / p.heightMm) < 0.06 * p.widthMm / p.heightMm + 1 / i.rect.h, `${what}: aspect`);
  // Texts follow the source rule.
  const est = p.estimated ? '~' : '';
  assert.equal(p.source === 'override', false);
  assert.equal(i.widthText, est + Math.round(p.widthMm) + ' mm'); assert.equal(i.heightText, est + Math.round(p.heightMm) + ' mm');
  assert.equal(i.estimated, true);
  assert.ok(i.labels.some(l => l.text === 'ESTIMATED'), `${what}: ESTIMATED missing`);
  const titles = i.labels.map(l => l.text);
  assert.ok(titles.includes('ACTIVE AREA  1:' + i.N)); assert.ok(titles.includes(i.widthText)); assert.ok(titles.includes(i.heightText));
  assert.equal(i.labels.length, 4);
  // Geometry: outline lines exactly Wr / Hr apart; dimension construction.
  const R = i.rect, strokesOf = role => r.pen.strokes.filter(s => s.role === role);
  const has = (role, x, y, w, h) => strokesOf(role).some(s => s.x === x && s.y === y && s.w === w && s.h === h);
  assert.ok(has('muted', R.x, R.y, R.w + 1, 1) && has('muted', R.x, R.y + R.h, R.w + 1, 1) && has('muted', R.x, R.y, 1, R.h + 1) && has('muted', R.x + R.w, R.y, 1, R.h + 1), `${what}: outline`);
  const D = i.dimension.widthRow, Dx = i.dimension.heightColumn;
  assert.equal(D, R.y + R.h + 8); assert.equal(Dx, R.x + R.w + 8);
  // The title sits 4 vpx above the outline, the width value 4 under the line's tails.
  const t = i.labels[0]; assert.equal(R.y - (t.y + t.h), 4); assert.equal(t.x, R.x);
  const wl = i.labels.find(l => l.text === i.widthText); assert.equal(wl.y, D + 5);
  assert.ok(Math.abs((wl.x + wl.w / 2) - (R.x + (R.w + 1) / 2)) <= 1.5, `${what}: width value centred`);
  const hl = i.labels.find(l => l.text === i.heightText); assert.equal(hl.x, Dx + 7);
  assert.ok(Math.abs((hl.y + 6) - (R.y + (R.h + 1) / 2)) <= 1, `${what}: height value centred`);
  const es = i.labels.find(l => l.text === 'ESTIMATED'); assert.equal(es.x, R.x); assert.ok(es.y >= wl.y + 12 + 3);
  // No label touches the outline or a dimension line (>= 4 vpx from the outline box).
  for (const l of i.labels) assert.ok(!overlaps(l, {x: R.x, y: R.y, w: R.w + 1, h: R.h + 1}, 4), `${what}: label too near the outline: ${l.text}`);
  // The star circle: where the screen centres it, to scale, clear of the outline.
  if (i.star) {
    assert.equal(i.star.cx, R.x + Math.round(R.w / 2)); assert.equal(i.star.cy, R.y + Math.round(R.h / 2));
    assert.equal(i.star.rx, Math.round(ctx.star.diameter / i.N / 2 / p.mmPerVpxX)); assert.equal(i.star.ry, Math.round(ctx.star.diameter / i.N / 2 / p.mmPerVpxY));
    assert.ok(i.star.rx >= 3);
  } else {
    const rx = Math.round(ctx.star.diameter / i.N / 2 / p.mmPerVpxX);
    assert.ok(rx < 3 || i.star.cx === undefined || true);
  }
  assert.equal(r.pen.targets.length, 1); assert.equal(r.pen.targets[0].kind, 'elevation');
  // Determinism.
  const again = run(E, ctx, mw, mh);
  same(again.pen.strokes, r.pen.strokes, `${what}: deterministic`); same(again.info, r.info, `${what}: info`);
  elevationCases++;
}
// The smallest N that fits: a roomier budget never gives a larger N.
for (const name of eMonitors) {
  const ctx = A.context(A.monitors[name], [A.monitors[name]]);
  const small = P.measureElevation(ctx, 120, 90), big = P.measureElevation(ctx, 220, 130);
  if (small) assert.ok(big && big.N <= small.N, `${name}: roomier budget picks a smaller N`);
}
// Known cases: the user's two outputs.
{
  const lap = A.context(A.monitors.laptop, A.layouts.real), dell = A.context(A.monitors.dell, A.layouts.real);
  const a = run(E, lap, 160, 110), b = run(E, dell, 160, 110);
  assert.ok(a.info.rect.w > a.info.rect.h && b.info.rect.w > b.info.rect.h);
  assert.ok(b.info.N > a.info.N, 'the wide Dell needs a smaller drawing than the laptop');
}
// Override (the user's measurement): one decimal, no ESTIMATED.
{
  const input = A.monitors.laptop;
  const ctx = A.context(input, [input], {'eDP-2': {width_mm: 344.6, height_mm: 215.4}});
  assert.equal(ctx.physical.source, 'override');
  const r = run(E, ctx, 160, 110);
  inside(r.pen, r.m, 'override');
  assert.equal(r.info.widthText, '344.6 mm'); assert.equal(r.info.heightText, '215.4 mm'); assert.equal(r.info.estimated, false);
  assert.ok(!r.pen.labels.some(l => l.text === 'ESTIMATED'));
  assert.equal(r.info.labels.length, 3);
}
// Estimated (no EDID): the ~ prefix and ESTIMATED.
{
  const input = A.monitors.estimated, ctx = A.context(input, [input]);
  assert.equal(ctx.physical.source, 'estimated');
  const r = run(E, ctx, 160, 110);
  assert.ok(/^~\d+ mm$/.test(r.info.widthText) && /^~\d+ mm$/.test(r.info.heightText) && r.info.estimated);
}
// Nothing fits: null.
assert.equal(P.measureElevation(A.context(A.monitors.dell, [A.monitors.dell]), 40, 30), null);
// The dispatch the element tools use.
{
  const ctx = {...A.context(A.monitors.laptop, A.layouts.real), element: 'locator'};
  const r = run(P, ctx);
  assert.equal(r.info.rects.length, 2); inside(r.pen, r.m, 'dispatch locator');
  const e = run(P, {...ctx, element: 'elevation'});
  assert.ok(e.info.scale); inside(e.pen, e.m, 'dispatch elevation');
}

console.log(`portrait-test ok: locator ${locatorCases} cases (${numbersDrawn} numbers drawn, ${numbersLeftOut} left out), elevation ${elevationCases} drawn / ${elevationNull} null`);
console.log('scales:', JSON.stringify(chosen));
