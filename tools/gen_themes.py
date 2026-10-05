#!/usr/bin/env python3
"""Generate the Omarchy themes from quadrille's palettes.

quadrille's palettes are read straight from its `theme.rs`, so a colour
changed there changes here at the next run. One directory is written per
palette under `themes/`, holding what Omarchy needs to switch to it:

    colors.toml    the 16-colour terminal palette and the surface roles
    shell.toml     the shell's surfaces, flat and in whole virtual pixels
    hyprland.lua   window borders and gaps in whole virtual pixels, no motion
    icons.theme    the GNOME icon theme to pair with it
    backgrounds/   a dithered graticule, drawn pixel for pixel
    preview.png    what the theme switcher shows

Everything else (btop, neovim, chromium, kitty, ...) Omarchy fills from its
own templates and the colours above.

    tools/gen_themes.py [--quadrille PATH] [--unit N]

`--unit` is the number of logical pixels in a virtual pixel; quadrille's
`PixelScaleMode::Auto(2)` makes it 2.
"""

import argparse
import re
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_THEME_RS = ROOT.parent / "graticule" / "crates" / "quadrille" / "src" / "theme.rs"
SHELL_TEMPLATE = Path("/usr/share/omarchy/default/themed/shell.toml.tpl")

# ---------------------------------------------------------------- palettes

ROLES = (
    "void ground raised hover edge ink muted faint line accent on_accent "
    "highlight live caution alarm"
).split()


def parse_palettes(theme_rs: Path) -> dict[str, dict[str, tuple[int, int, int]]]:
    """Read every `Palette::NAME` constant out of theme.rs."""
    source = theme_rs.read_text()
    palettes = {}
    pattern = re.compile(r"pub const (\w+): Self = Self \{(.*?)\};", re.S)
    for name, body in pattern.findall(source):
        fields = re.findall(r"(\w+): rgb\(0x(\w+), 0x(\w+), 0x(\w+)\)", body)
        if {f[0] for f in fields} >= set(ROLES):
            palettes[name.lower()] = {
                f[0]: (int(f[1], 16), int(f[2], 16), int(f[3], 16)) for f in fields
            }
    if not palettes:
        sys.exit(f"no palettes found in {theme_rs}")
    return palettes


def hexc(c):
    return "#%02x%02x%02x" % c


def mix(a, b, t):
    """`a` toward `b` by `t`, rounded to 8 bits like quadrille's `theme::mix`."""
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def luminance(c):
    return (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]) / 255


WHITE = (255, 255, 255)
BLACK = (0, 0, 0)

# What a terminal needs that a role palette does not name: the six hues of
# ANSI and a brighter ink for the cursor and bold text. Each is derived from
# the roles, so a tube stays a tube; it is not a rainbow bolted on.
SPECS = {
    "terminal": dict(
        icons="Yaru-wartybrown",
        selection=lambda r: r["edge"],
        bright=lambda r: mix(r["ink"], WHITE, 0.4),
        blue=lambda r: mix(r["highlight"], r["muted"], 0.5),
        cyan=lambda r: r["highlight"],
        magenta=lambda r: mix(r["alarm"], r["ink"], 0.55),
    ),
    "paper": dict(
        icons="Yaru-wartybrown",
        selection=lambda r: r["hover"],
        bright=lambda r: mix(r["ink"], BLACK, 0.45),
        blue=lambda r: mix(r["muted"], r["live"], 0.4),
        cyan=lambda r: mix(r["live"], r["ink"], 0.35),
        magenta=lambda r: mix(r["alarm"], r["ink"], 0.45),
    ),
    "phosphor": dict(
        icons="Yaru-sage",
        selection=lambda r: r["highlight"],
        bright=lambda r: r["accent"],
        blue=lambda r: r["muted"],
        cyan=lambda r: mix(r["ink"], r["accent"], 0.5),
        magenta=lambda r: mix(r["alarm"], r["ink"], 0.5),
    ),
    "amber": dict(
        icons="Yaru-wartybrown",
        selection=lambda r: r["highlight"],
        bright=lambda r: r["accent"],
        blue=lambda r: r["muted"],
        cyan=lambda r: mix(r["ink"], r["accent"], 0.5),
        magenta=lambda r: mix(r["alarm"], r["ink"], 0.5),
    ),
    "lcd": dict(
        icons="Yaru-sage",
        selection=lambda r: r["highlight"],
        bright=lambda r: r["ink"],
        blue=lambda r: r["muted"],
        cyan=lambda r: mix(r["ink"], r["faint"], 0.5),
        magenta=lambda r: mix(r["alarm"], r["ink"], 0.4),
    ),
}


def colors_toml(name, r):
    spec = SPECS[name]
    dark = luminance(r["ground"]) < luminance(r["ink"])
    c = {
        "accent": r["accent"],
        "selection": spec["selection"](r),
        "muted": r["muted"],
        "background": r["void"],
        "dark_background": r["void"],
        "darker_background": mix(r["void"], BLACK if dark else WHITE, 0.4),
        "lighter_background": r["ground"],
        "foreground": r["ink"],
        "dark_foreground": r["muted"],
        "light_foreground": mix(r["ink"], WHITE if dark else BLACK, 0.25),
        "bright_foreground": spec["bright"](r),
        "red": r["alarm"],
        "yellow": r["caution"],
        "orange": r["accent"],
        "green": r["live"],
        "cyan": spec["cyan"](r),
        "blue": spec["blue"](r),
        "magenta": spec["magenta"](r),
        "brown": r["line"],
        "bright_red": r["alarm"],
        "bright_yellow": r["caution"],
        "bright_green": r["live"],
        "bright_cyan": spec["cyan"](r),
        "bright_blue": spec["blue"](r),
        "bright_magenta": spec["magenta"](r),
    }
    lines = [f'mode = "{"dark" if dark else "light"}"', ""]
    groups = [
        ["accent", "selection", "muted"],
        ["background", "dark_background", "darker_background", "lighter_background"],
        ["foreground", "dark_foreground", "light_foreground", "bright_foreground"],
        ["red", "yellow", "orange", "green", "cyan", "blue", "magenta", "brown"],
        [k for k in c if k.startswith("bright_") and k != "bright_foreground"],
    ]
    for group in groups:
        lines += [f'{k} = "{hexc(c[k])}"' for k in group] + [""]
    return "\n".join(lines)


# ------------------------------------------------------------------- shell


def shell_toml(r, unit):
    """The shell's surfaces: flat ground, hairline edges, an inverse block for
    what is chosen, sizes in whole virtual pixels."""
    template = SHELL_TEMPLATE.read_text()
    c = {k: hexc(v) for k, v in r.items()}

    # The template speaks in Omarchy's theme variables. Resolve them to
    # literals so the file stands on its own as the theme's shell.toml.
    variables = {
        "background": c["ground"],
        "foreground": c["ink"],
        "accent": c["accent"],
        "red": c["alarm"],
    }

    def resolve(match):
        expr = match.group(1).split()
        if expr[0] == "shell_gradient":
            return variables.get(expr[2], c["edge"])
        if expr[0] == "mix":
            t = int(expr[3].rstrip("%")) / 100
            return hexc(mix(r["ground"], r["ink"], 1 - t) if expr[1] == "foreground" else r["ink"])
        return variables[expr[0]]

    text = re.sub(r"\{\{\s*(.*?)\s*\}\}", resolve, template)

    px = lambda n: n * unit  # noqa: E731  virtual pixels to logical pixels
    body = 11 * unit  # Departure Mono's native size

    overrides = {
        ("bar", "background"): c["ground"],
        ("bar", "text"): c["ink"],
        ("bar", "scale-with-font"): "false",
        ("bar", "size-horizontal"): px(16),
        ("bar", "size-vertical"): px(16),
        ("hyprland", "active-border"): c["accent"],
        ("hyprland", "active-border-foreground"): c["edge"],
        # Flat controls: a hairline, no tint; the chosen one is a block of accent.
        ("controls", "normal-fill-alpha"): "0.0",
        ("controls", "normal-border"): c["edge"],
        ("controls", "normal-border-width"): px(1),
        ("controls", "normal-border-alpha"): "1.0",
        ("controls", "hover-cursor-color"): c["ink"],
        ("controls", "hover-cursor-fill-alpha"): "0.12",
        ("controls", "hover-cursor-border"): c["muted"],
        ("controls", "hover-cursor-border-alpha"): "1.0",
        ("controls", "focus-border"): c["accent"],
        ("controls", "focus-border-alpha"): "1.0",
        ("controls", "selected-color"): c["accent"],
        ("controls", "selected-fill-alpha"): "0.2",
        ("font", "base-size"): body,
        ("popups", "background"): c["ground"],
        ("popups", "border"): c["edge"],
        ("popups", "border-width"): px(1),
        ("tooltip", "background"): c["raised"],
        ("tooltip", "border"): c["edge"],
        ("notifications", "background"): c["ground"],
        ("notifications", "border"): c["edge"],
        ("notifications", "border-width"): px(1),
        ("launcher", "background-alpha"): "1.0",
        ("launcher", "border"): c["edge"],
        ("launcher", "selected-background"): c["accent"],
        ("launcher", "selected-background-alpha"): "1.0",
        ("launcher", "selected-text"): c["on_accent"],
        ("launcher", "selected-border"): c["accent"],
        ("launcher", "selected-border-alpha"): "1.0",
        ("menu", "border"): c["edge"],
        ("menu", "selected-background"): c["accent"],
        ("menu", "selected-background-alpha"): "1.0",
        ("menu", "selected-text"): c["on_accent"],
        ("menu", "selected-border"): c["accent"],
        ("menu", "selected-border-alpha"): "1.0",
        ("polkit", "background"): c["ground"],
        ("polkit", "border"): c["edge"],
        ("polkit", "border-error"): c["alarm"],
        ("lock", "background"): c["ground"],
        ("lock", "background-alpha"): "1.0",
        ("lock", "border"): c["edge"],
        ("lock", "border-active"): c["accent"],
        ("lock", "border-error"): c["alarm"],
        ("image-picker", "selected-border"): c["accent"],
    }

    out, section = [], None
    for line in text.splitlines():
        header = re.match(r"\[([\w-]+)\]", line)
        if header:
            section = header.group(1)
        key = re.match(r"(?:#\s*)?([\w-]+)(\s*)=", line)
        if key and (section, key.group(1)) in overrides:
            value = overrides.pop((section, key.group(1)))
            value = f'"{value}"' if isinstance(value, str) and value.startswith("#") else value
            line = f"{key.group(1)}{key.group(2)}= {value}"
        out.append(line)

    # Font sizes the type scale would otherwise derive: Departure Mono has
    # native sizes only, so pin every token to one of them.
    pinned = {
        "caption": body, "body-small": body, "body": body, "subtitle": body,
        "title": body, "heading": 2 * body, "display": 2 * body,
        "display-large": 3 * body, "icon-small": body, "icon": body, "icon-large": 2 * body,
    }
    text = "\n".join(out)
    for key, value in pinned.items():
        text = re.sub(rf"(?m)^#\s*{key}\s*=.*$", f"{key:<13} = {value}", text)

    leftover = {k: v for k, v in overrides.items()}
    if leftover:
        print(f"  note: template has no {sorted(leftover)}", file=sys.stderr)

    # The shell's own tokens name a handful of surfaces; quadrille names fifteen
    # roles. The Q kit of plugins/quadrille.bar reads them from here, so a theme
    # switch carries the whole palette (Color.shellValues keeps unknown tables).
    roles = [
        "[quadrille]",
        "# quadrille's palette, stated role by role for the plugins in plugins/.",
        "# unit is the number of logical pixels in a virtual pixel.",
        f"unit = {unit}",
    ] + [f'{role:<9} = "{c[role]}"' for role in ROLES]
    return text.rstrip("\n") + "\n\n" + "\n".join(roles) + "\n"


def hyprland_lua(r, unit):
    accent = "rgb(%02x%02x%02x)" % r["accent"]
    edge = "rgb(%02x%02x%02x)" % r["edge"]
    return f"""-- Generated by tools/gen_themes.py: borders and gaps in whole virtual pixels
-- ({unit} logical pixels each), square corners, and no motion.
local active_border_color = "{accent}"
local inactive_border_color = "{edge}"

hl.config({{
  general = {{
    border_size = {unit},
    gaps_in = {2 * unit},
    gaps_out = {4 * unit},

    col = {{
      active_border = active_border_color,
      inactive_border = inactive_border_color,
    }},
  }},

  group = {{
    col = {{
      border_active = active_border_color,
      border_inactive = inactive_border_color,
    }},
  }},

  decoration = {{
    rounding = 0,
    shadow = {{ enabled = false }},
    blur = {{ enabled = false }},
  }},

  animations = {{
    enabled = false,
  }},
}})
"""


# -------------------------------------------------------------- wallpaper

BAYER4 = (
    (0, 8, 2, 10),
    (12, 4, 14, 6),
    (3, 11, 1, 9),
    (15, 7, 13, 5),
)


def png(width, height, rows):
    """A minimal RGB PNG writer: `rows` yields `width` (r, g, b) tuples."""
    raw = bytearray()
    for row in rows:
        raw.append(0)
        for pixel in row:
            raw += bytes(pixel)

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def graticule(r, w, h):
    """A dithered glow with a graticule over it, in virtual pixels."""
    cx, cy = w // 2, h // 2
    minor, major = 40, 200
    grid = mix(r["void"], r["line"], 0.35)
    tick = mix(r["void"], r["line"], 0.8)
    glow = r["ground"]
    rows = []
    for y in range(h):
        row = []
        for x in range(w):
            # A radial level, 0 at the rim and 1 at the centre, thresholded by
            # a Bayer matrix so the tone is dither, never a blend.
            d = (((x - cx) / (w * 0.55)) ** 2 + ((y - cy) / (h * 0.62)) ** 2) ** 0.5
            level = max(0.0, 1.0 - d)
            lit = level * 16 > BAYER4[y % 4][x % 4] + 0.5
            pixel = glow if lit else r["void"]
            dx, dy = x - cx, y - cy
            if dx % minor == 0 and dy % 2 == 0 or dy % minor == 0 and dx % 2 == 0:
                pixel = grid
            if (dx % major == 0 and abs(dy % minor) <= 2) or (dy % major == 0 and abs(dx % minor) <= 2):
                pixel = tick
            if (dx == 0 and abs(dy) < 14 and abs(dy) > 3) or (dy == 0 and abs(dx) < 14 and abs(dx) > 3):
                pixel = r["accent"]
            row.append(pixel)
        rows.append(row)
    # Corner brackets, the way quadrille marks a focus.
    m, a = 24, 16
    for sx, x0 in ((1, m), (-1, w - 1 - m)):
        for sy, y0 in ((1, m), (-1, h - 1 - m)):
            for i in range(a):
                rows[y0][x0 + sx * i] = r["edge"]
                rows[y0 + sy * i][x0] = r["edge"]
    return rows


def upscale(rows, factor):
    out = []
    for row in rows:
        wide = [p for p in row for _ in range(factor)]
        out += [wide] * factor
    return out


def write_theme(out_dir: Path, name, r, unit, size):
    (out_dir / "backgrounds").mkdir(parents=True, exist_ok=True)
    (out_dir / "colors.toml").write_text(colors_toml(name, r))
    (out_dir / "shell.toml").write_text(shell_toml(r, unit))
    (out_dir / "hyprland.lua").write_text(hyprland_lua(r, unit))
    (out_dir / "icons.theme").write_text(SPECS[name]["icons"])

    w, h = size[0] // unit, size[1] // unit
    rows = graticule(r, w, h)
    (out_dir / "backgrounds" / "1-graticule.png").write_bytes(
        png(w * unit, h * unit, upscale(rows, unit))
    )
    # The switcher's preview: the same picture, a quarter of the size.
    (out_dir / "preview.png").write_bytes(png(w // 2, h // 2, [row[::2] for row in rows[::2]]))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--quadrille", type=Path, default=DEFAULT_THEME_RS)
    parser.add_argument("--unit", type=int, default=2)
    parser.add_argument("--size", default="3440x1440", help="wallpaper size in logical pixels")
    args = parser.parse_args()

    size = tuple(int(n) for n in args.size.split("x"))
    palettes = parse_palettes(args.quadrille)
    for name, roles in palettes.items():
        if name not in SPECS:
            print(f"skipping {name}: no terminal spec", file=sys.stderr)
            continue
        print(f"quadrille-{name}")
        write_theme(ROOT / "themes" / f"quadrille-{name}", name, roles, args.unit, size)


if __name__ == "__main__":
    main()
