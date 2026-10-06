.pragma library

// EDID's basic size is rounded to centimetres. Recover a square-pixel panel
// from its nearest nominal diagonal; explicit measured sizes stay explicit.
var panelDiagonals = [13.3, 13.5, 14, 15.6, 16, 17, 17.3, 21.5, 23.8, 24,
                      24.5, 25, 27, 28, 31.5, 32, 34, 34.1, 35, 38, 40, 42,
                      43, 49, 55];

function positive(value) {
    return typeof value === "number" && isFinite(value) && value > 0;
}

function nearest(value) {
    return value < 0 ? -Math.round(-value) : Math.round(value);
}

function diagonal(inches) {
    var candidate = panelDiagonals[0];
    for (var i = 1; i < panelDiagonals.length; ++i) {
        if (Math.abs(panelDiagonals[i] - inches) < Math.abs(candidate - inches))
            candidate = panelDiagonals[i];
    }
    return Math.abs(candidate - inches) / inches <= 0.03
            ? candidate : Math.round(inches * 10) / 10;
}

function resolve(input, overrides) {
    var widthPx = positive(input.width) ? input.width : 1;
    var heightPx = positive(input.height) ? input.height : 1;
    var scale = positive(input.scale) ? input.scale : 1;
    var pixelsPerVpx = Math.max(1, Math.round(2 * scale));
    var modelKey = (input.make || "") + " " + (input.model || "");
    var named = overrides && Object.prototype.hasOwnProperty.call(overrides, input.name)
            ? overrides[input.name] : null;
    var matched = overrides && Object.prototype.hasOwnProperty.call(overrides, modelKey)
            ? overrides[modelKey] : null;
    var override = named || matched || {};
    var widthMm, heightMm, diagonalMm;
    var source = "edid";

    if (positive(override.width_mm) && positive(override.height_mm)) {
        widthMm = override.width_mm;
        heightMm = override.height_mm;
        diagonalMm = Math.sqrt(widthMm * widthMm + heightMm * heightMm);
        source = "override";
    } else {
        if (positive(override.diagonal_inches)) {
            diagonalMm = override.diagonal_inches * 25.4;
            source = "override";
        } else if (positive(input.physicalWidth) && positive(input.physicalHeight)) {
            diagonalMm = diagonal(Math.sqrt(input.physicalWidth * input.physicalWidth
                                            + input.physicalHeight * input.physicalHeight) / 25.4) * 25.4;
        } else {
            diagonalMm = Math.sqrt(widthPx * widthPx + heightPx * heightPx) * 25.4 / 96;
            source = "estimated";
        }
        var pixelDiagonal = Math.sqrt(widthPx * widthPx + heightPx * heightPx);
        widthMm = diagonalMm * widthPx / pixelDiagonal;
        heightMm = diagonalMm * heightPx / pixelDiagonal;
    }

    if ((input.transform || 0) % 2 !== 0) {
        var swap = widthPx;
        widthPx = heightPx;
        heightPx = swap;
        swap = widthMm;
        widthMm = heightMm;
        heightMm = swap;
    }
    var mmPerPixelX = widthMm / widthPx;
    var mmPerPixelY = heightMm / heightPx;
    return { widthMm: widthMm, heightMm: heightMm, diagonalMm: diagonalMm,
             widthPx: widthPx, heightPx: heightPx, estimated: source === "estimated",
             source: source, pixelsPerVpx: pixelsPerVpx,
             mmPerPixelX: mmPerPixelX, mmPerPixelY: mmPerPixelY,
             pxPerMmX: 1 / mmPerPixelX, pxPerMmY: 1 / mmPerPixelY,
             mmPerVpxX: mmPerPixelX * pixelsPerVpx,
             mmPerVpxY: mmPerPixelY * pixelsPerVpx,
             mmPerLogicalPixelX: mmPerPixelX * scale,
             mmPerLogicalPixelY: mmPerPixelY * scale };
}

function snapMm(mm, pxPerMm, pixelsPerVpx) {
    return nearest(mm * pxPerMm / pixelsPerVpx) * pixelsPerVpx;
}

function uncomment(line) {
    var quote = "", escaped = false;
    for (var i = 0; i < line.length; ++i) {
        var c = line[i];
        if (escaped) { escaped = false; continue; }
        if (quote === '"' && c === "\\") { escaped = true; continue; }
        if (quote) { if (c === quote) quote = ""; }
        else if (c === '"' || c === "'") quote = c;
        else if (c === "#") return line.slice(0, i).trim();
    }
    return line.trim();
}

function tableName(text) {
    text = text.trim();
    if (text.indexOf("displays.") === 0) text = text.slice(9).trim();
    if (text[0] === '"') return JSON.parse(text);
    if (text[0] === "'" && text[text.length - 1] === "'"
            && text.slice(1, -1).indexOf("'") < 0) return text.slice(1, -1);
    if (/^[A-Za-z0-9_-]+$/.test(text)) return text;
    throw new Error("invalid display table name");
}

// The same deliberately small TOML subset as physical.rs: display tables and
// positive numeric dimensions. Reject mistakes rather than guess a calibration.
function parseOverrides(text) {
    var result = {}, current = null;
    var lines = text.split(/\r?\n/);
    for (var i = 0; i < lines.length; ++i) {
        var line = uncomment(lines[i]);
        if (!line) continue;
        if (line[0] === "[" && line[line.length - 1] === "]") {
            var name = tableName(line.slice(1, -1));
            if (Object.prototype.hasOwnProperty.call(result, name))
                throw new Error("duplicate display table: " + name);
            current = {};
            Object.defineProperty(result, name, {value: current, enumerable: true});
            continue;
        }
        var match = /^([a-z_]+)\s*=\s*(.+)$/.exec(line);
        if (!current || !match || ["diagonal_inches", "width_mm", "height_mm"].indexOf(match[1]) < 0)
            throw new Error("invalid display setting at line " + (i + 1));
        var number = match[2];
        if (!/^[+-]?[0-9]+(?:_[0-9]+)*(?:\.[0-9]+(?:_[0-9]+)*)?(?:[eE][+-]?[0-9]+(?:_[0-9]+)*)?$/.test(number))
            throw new Error("invalid display number at line " + (i + 1));
        var value = Number(number.replace(/_/g, ""));
        if (!positive(value) || Object.prototype.hasOwnProperty.call(current, match[1]))
            throw new Error("invalid or duplicate display dimension at line " + (i + 1));
        current[match[1]] = value;
    }
    for (var key in result) {
        var entry = result[key];
        var diagonalOnly = positive(entry.diagonal_inches) && Object.keys(entry).length === 1;
        var measured = positive(entry.width_mm) && positive(entry.height_mm) && Object.keys(entry).length === 2;
        if (!diagonalOnly && !measured) throw new Error("incomplete display dimensions: " + key);
    }
    return result;
}
