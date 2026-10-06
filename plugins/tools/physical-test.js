#!/usr/bin/env node
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const root = path.resolve(__dirname, "../..");
const physical = vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(root, "plugins/quadrille.background/Physical.js"), "utf8")
    .replace(/^\.pragma library\s*\n/, ""), physical);
const vectors = JSON.parse(fs.readFileSync(path.join(root, "docs/physical-vectors.json"), "utf8"));

for (const vector of vectors.cases) {
    const actual = physical.resolve(vector.input, vector.overrides);
    for (const [key, expected] of Object.entries(vector.expected)) {
        if (typeof expected === "number")
            assert.ok(Math.abs(actual[key] - expected) <= 1e-9,
                `${vector.id}.${key}: ${actual[key]} != ${expected}`);
        else assert.equal(actual[key], expected, `${vector.id}.${key}`);
    }
    for (let mm = -100; mm <= 100; ++mm) {
        const px = physical.snapMm(mm, actual.pxPerMmX, actual.pixelsPerVpx);
        assert.equal(px % actual.pixelsPerVpx === 0, true, `${vector.id}: mark off grid`);
        assert.ok(Math.abs(px - mm * actual.pxPerMmX) <= actual.pixelsPerVpx / 2 + 1e-9,
            `${vector.id}: mark error exceeds half a virtual pixel`);
    }
}
for (const vector of vectors.marks)
    assert.equal(physical.snapMm(vector.mm, vector.pxPerMm, vector.pixelsPerVpx),
        vector.expected, vector.id);
for (const vector of vectors.overrideParsers)
    assert.deepEqual(JSON.parse(JSON.stringify(physical.parseOverrides(vector.text))),
        vector.expected, vector.id);
for (const text of vectors.invalidOverrides)
    assert.throws(() => physical.parseOverrides(text), undefined, text);

console.log(`physical: ${vectors.cases.length} models, ${vectors.marks.length} marks, `
    + `${vectors.overrideParsers.length + vectors.invalidOverrides.length} override files passed`);
