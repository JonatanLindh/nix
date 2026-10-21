"""Build the "Glass" Kvantum theme from KvArcDark, matching the Quickshell look.

usage: kvantum-glass.py KVARCDARK_DIR OUT_DIR SETTINGS.json

SETTINGS.json maps kvconfig sections to the keys to override ({"%General": {...}}).

Arc's opaque greys become translucent white over the (translucent) window, and
its blue accent becomes Glass's near-white: solid where something is "on"
(checkboxes, radios, slider fill) and a faint wash for selected rows, pressed
buttons and the like, so their light text stays readable.
"""

import configparser
import json
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

WHITE = "#f4f7f6"  # Theme.white
INK = "#0b1d20"  # Theme.ink
GLASS = "#0e1a1e"  # Theme.glass, without its alpha

# Arc colour -> (new colour, alpha multiplier)
SURFACES = {
    "#383c4a": (GLASS, 0.80),  # menus, docks
    "#404552": ("#ffffff", 0.06),  # frames, tab bodies, active tab
    "#444a58": ("#ffffff", 0.08),  # button/combo at rest (Theme.fill)
    "#505666": ("#ffffff", 0.12),  # hovered button, line edits (Theme.fillHover)
    "#3c404e": ("#ffffff", 0.10),  # hovered tab
    "#343844": ("#ffffff", 0.06),  # floating tabs
    "#4d5367": ("#ffffff", 0.10),
    "#22252e": ("#ffffff", 0.08),  # frame edges (Theme.divider)
    "#2d303b": ("#ffffff", 0.14),  # slider groove (Theme.track)
    "#2f343f": ("#ffffff", 0.08),  # menubar edge
    "#4693e6": ("#ffffff", 0.35),  # focus ring (Theme.glassBorder, stronger)
    "#111217": (INK, 1.0),  # check marks / radio dots, drawn on the accent
    "#363c48": (INK, 1.0),
}

# Per-element overrides: the slider handle is solid white, like the shell's sliders.
OVERRIDES = {
    re.compile(r"^slidercursor"): {
        "#444a58": (WHITE, 1.0),
        "#505666": (WHITE, 1.0),
        "#262933": (INK, 0.35),
    },
}

ACCENT = "#5294e2"
# Elements where the accent is a solid "on" state; everywhere else it's a wash.
SOLID = re.compile(r"^(checkbox|radio|slider-toggled|dial-handle|ellipse)")
ACCENT_SOLID = (WHITE, 1.0)
ACCENT_WASH = ("#ffffff", 0.18)  # Theme.fillActive

PROPS = {"fill": "fill-opacity", "stroke": "stroke-opacity", "stop-color": "stop-opacity"}


def norm(c):
    c = c.strip().lower()
    if re.fullmatch(r"#[0-9a-f]{3}", c):
        c = "#" + "".join(ch * 2 for ch in c[1:])
    return c


def lookup(colour, owner):
    c = norm(colour)
    for pattern, table in OVERRIDES.items():
        if pattern.match(owner or "") and c in table:
            return table[c]
    if c == ACCENT:
        return ACCENT_SOLID if SOLID.match(owner or "") else ACCENT_WASH
    return SURFACES.get(c)


def recolor(props, owner):
    """props: dict of presentation properties (from style= or attributes)."""
    for prop, opacity in PROPS.items():
        if prop in props and (m := lookup(props[prop], owner)):
            colour, alpha = m
            props[prop] = colour
            props[opacity] = f"{float(props.get(opacity, 1)) * alpha:.3f}"


def walk(el, owner, gradients, stops=False):
    i = el.get("id")
    # Kvantum looks elements up by id; Inkscape's auto ids (rect123) aren't element names.
    if i and not re.fullmatch(r"[a-zA-Z]+\d+(-\d+)?", i):
        owner = i

    # Gradients are shared and referenced from elsewhere; recolour their stops
    # on behalf of the first element that uses them.
    for attr in ("fill", "stroke"):
        ref = re.search(r"url\(#([^)]+)\)", el.get("style", "") + el.get(attr, ""))
        if ref:
            gradients.setdefault(ref.group(1), owner)

    # Stops are handled in main(), on behalf of the gradient's user.
    if el.tag.endswith("stop") and not stops:
        return

    style = el.get("style")
    if style:
        props = dict(
            (k.strip(), v.strip()) for k, v in (d.split(":", 1) for d in style.split(";") if ":" in d)
        )
        recolor(props, owner)
        el.set("style", ";".join(f"{k}:{v}" for k, v in props.items()))

    attrs = {k: el.get(k) for k in [*PROPS, *PROPS.values()] if el.get(k) is not None}
    if attrs:
        recolor(attrs, owner)
        for k, v in attrs.items():
            el.set(k, v)

    for child in el:
        walk(child, owner, gradients, stops)


def build_svg(src, dst):
    for prefix, uri in {
        "": "http://www.w3.org/2000/svg",
        "xlink": "http://www.w3.org/1999/xlink",
        "inkscape": "http://www.inkscape.org/namespaces/inkscape",
        "sodipodi": "http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd",
        "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
        "cc": "http://creativecommons.org/ns#",
        "dc": "http://purl.org/dc/elements/1.1/",
    }.items():
        ET.register_namespace(prefix, uri)

    tree = ET.parse(src)
    gradients = {}
    walk(tree.getroot(), None, gradients)
    # Second pass for gradient stops, owned by whoever references them
    # (following Inkscape's xlink:href chains to the gradient holding the stops).
    by_id = {el.get("id"): el for el in tree.getroot().iter() if el.get("id")}
    href = "{http://www.w3.org/1999/xlink}href"
    done = set()
    for gid, owner in gradients.items():
        g = by_id.get(gid)
        while g is not None and g.get(href) and not any(c.tag.endswith("stop") for c in g):
            g = by_id.get(g.get(href).lstrip("#"))
        if g is None or g.get("id") in done:
            continue
        done.add(g.get("id"))
        for stop in g:
            walk(stop, owner, {}, stops=True)
    tree.write(dst, xml_declaration=True, encoding="utf-8")


def build_kvconfig(src, dst, settings):
    # Kvantum reads these with QSettings; keep keys' case and allow Arc's duplicates.
    ini = configparser.ConfigParser(interpolation=None, strict=False)
    ini.optionxform = str
    ini.read(src)
    for section, values in settings.items():
        if not ini.has_section(section):
            ini.add_section(section)
        for key, value in values.items():
            ini[section][key] = str(value).lower() if isinstance(value, bool) else str(value)
    with open(dst, "w") as f:
        ini.write(f, space_around_delimiters=False)


def main(src_dir, out_dir, settings):
    src, out = Path(src_dir), Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    build_svg(src / "KvArcDark.svg", out / "Glass.svg")
    build_kvconfig(src / "KvArcDark.kvconfig", out / "Glass.kvconfig", json.loads(Path(settings).read_text()))


if __name__ == "__main__":
    main(*sys.argv[1:])
