"""Extract Manrope ExtraBold (wght 800) outlines for the wordmark base -> brand/logo/src/manrope-800.json.
Run: <venv>/python brand/scripts/extract-glyphs.py android/app/src/main/res/font/manrope.ttf
The wordmark is then lettered (modified) in build.mjs; Manrope is OFL 1.1 so modification is allowed.
"""
import json
import sys
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

src = sys.argv[1]
mono = sys.argv[2] if len(sys.argv) > 2 else None
Path("brand/fonts").mkdir(parents=True, exist_ok=True)


def licence(f: TTFont, family: str) -> None:
    names = {n.nameID: n.toUnicode() for n in f["name"].names if n.platformID == 3}
    text = f"{family}\n\n{names.get(0, '')}\n\nLicence: {names.get(13, '')}\n{names.get(14, '')}\n"
    Path(f"brand/fonts/LICENSE-{family}.txt").write_text(text, encoding="utf-8")


licence(TTFont(src), "Manrope")
for path, name in ((src, "Manrope-Variable"), (mono, "GeistMono-Variable")):
    if path:
        w = TTFont(path)
        w.flavor = "woff2"
        w.save(f"brand/fonts/{name}.woff2")
for w, label in ((500, "Medium"), (700, "Bold"), (800, "ExtraBold")):
    inst = instantiateVariableFont(TTFont(src), {"wght": w})
    inst.save(f"brand/fonts/Manrope-{label}.ttf")
if mono:
    licence(TTFont(mono), "GeistMono")
    m = TTFont(mono)
    if "fvar" in m:
        m = instantiateVariableFont(m, {"wght": 500})
    m.save("brand/fonts/GeistMono-Medium.ttf")

font = TTFont(src)
if "fvar" in font:
    font = instantiateVariableFont(font, {"wght": 800})
gs = font.getGlyphSet()
cmap = font.getBestCmap()
upm = font["head"].unitsPerEm
os2 = font["OS/2"]
out = {"upm": upm, "xHeight": os2.sxHeight, "capHeight": os2.sCapHeight, "glyphs": {}}
for ch in "titTI":
    name = cmap[ord(ch)]
    pen = SVGPathPen(gs)
    gs[name].draw(pen)
    out["glyphs"][ch] = {"d": pen.getCommands(), "adv": gs[name].width}
Path("brand/logo/src").mkdir(parents=True, exist_ok=True)
Path("brand/logo/src/manrope-800.json").write_text(json.dumps(out, indent=1), encoding="utf-8")
print("ok", upm, os2.sxHeight, {k: v["adv"] for k, v in out["glyphs"].items()})
