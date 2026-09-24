#!/usr/bin/env bash
# Regenerate Android launcher mipmaps + desktop window icons from nemo_logo.png.
#
# Usage (from repo root):
#   ./scripts/generate-app-icons.sh
#   ./scripts/generate-app-icons.sh 0.50    # N tips as fraction of icon radius (default 0.50)
#
# Requires: python3 + Pillow (pip install pillow)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SAFE_R="${1:-0.50}"
SRC="${ROOT}/nemo_logo.png"

if [[ ! -f "${SRC}" ]]; then
  echo "missing ${SRC}" >&2
  exit 1
fi

python3 - "${ROOT}" "${SRC}" "${SAFE_R}" <<'PY'
import math
import sys
from pathlib import Path

from PIL import Image

root = Path(sys.argv[1])
src_path = Path(sys.argv[2])
safe_r = float(sys.argv[3])
if not (0.2 <= safe_r <= 0.95):
    raise SystemExit(f"SAFE_R must be between 0.2 and 0.95, got {safe_r}")

src = Image.open(src_path).convert("RGBA")
bg = src.getpixel((10, 10))
bg_rgb = bg[:3]
tol = 20


def is_bg(c):
    return c[3] < 8 or all(abs(c[i] - bg_rgb[i]) <= tol for i in range(3))


side = max(src.size)
sq = Image.new("RGBA", (side, side), bg)
sq.paste(src, ((side - src.size[0]) // 2, (side - src.size[1]) // 2), src)
px = sq.load()
content = [(x, y) for y in range(side) for x in range(side) if not is_bg(px[x, y])]
if not content:
    raise SystemExit("no logo pixels found in nemo_logo.png")


def make_opaque_icon(canvas: int) -> tuple[Image.Image, int, float]:
    """Full-bleed opaque paper + N; tips stay within safe_r * radius."""
    cx = cy = canvas / 2.0
    radius = canvas / 2.0
    limit = (safe_r * radius) ** 2
    lo, hi = int(canvas * 0.15), int(canvas * 0.95)
    best = lo

    def fits(length: int) -> bool:
        scale = length / side
        ox = (canvas - length) / 2.0
        for x, y in content:
            dx = ox + x * scale - cx
            dy = ox + y * scale - cy
            if dx * dx + dy * dy > limit:
                return False
        return True

    while lo <= hi:
        mid = (lo + hi) // 2
        if fits(mid):
            best = mid
            lo = mid + 1
        else:
            hi = mid - 1

    logo = sq.resize((best, best), Image.Resampling.LANCZOS)
    out = Image.new("RGBA", (canvas, canvas), (*bg_rgb, 255))
    ox = (canvas - best) // 2
    out.paste(logo, (ox, ox), logo)
    r, g, b, _ = out.split()
    out = Image.merge("RGBA", (r, g, b, Image.new("L", (canvas, canvas), 255)))

    mr = 0.0
    p = out.load()
    for y in range(canvas):
        for x in range(canvas):
            rr, gg, bb, aa = p[x, y]
            if aa > 200 and rr + gg + bb < 200:
                mr = max(mr, math.hypot(x + 0.5 - cx, y + 0.5 - cy))
    return out, best, 100.0 * mr / radius


# Android mipmaps (opaque; launcher applies the round mask).
res = root / "apps/compose/src/androidMain/res"
densities = {
    "mdpi": (48, 108),
    "hdpi": (72, 162),
    "xhdpi": (96, 216),
    "xxhdpi": (144, 324),
    "xxxhdpi": (192, 432),
}
for dens, (legacy, fg) in densities.items():
    d = res / f"mipmap-{dens}"
    d.mkdir(parents=True, exist_ok=True)
    icon_legacy, _, _ = make_opaque_icon(legacy)
    icon_fg, length, pct = make_opaque_icon(fg)
    icon_legacy.save(d / "ic_launcher.png", optimize=True)
    icon_legacy.save(d / "ic_launcher_round.png", optimize=True)
    icon_fg.save(d / "ic_launcher_foreground.png", optimize=True)
    print(f"android {dens}: fg={fg}px logo={length} dark={pct:.1f}%R")

# Desktop window / package icons — square original, no circular mask.
desk = root / "apps/compose/src/desktopMain/resources"
desk.mkdir(parents=True, exist_ok=True)
for name, px in [("nemo_logo.png", 256), ("nemo_logo_512.png", 512)]:
    out = sq.resize((px, px), Image.Resampling.LANCZOS)
    r, g, b, _ = out.split()
    out = Image.merge("RGBA", (r, g, b, Image.new("L", (px, px), 255)))
    out.save(desk / name, optimize=True)
    print(f"desktop {name}: {px}px")

# In-app rounded brand mark (onboarding). Near full-size (~90% of radius) with a
# thin paper ring; Compose CircleShape also clips. Not launcher SAFE_R.
BRAND_R = 0.90


def make_circular_brand(canvas: int) -> Image.Image:
    from PIL import ImageDraw

    cx = cy = canvas / 2.0
    limit = (BRAND_R * (canvas / 2.0)) ** 2
    lo, hi = int(canvas * 0.50), int(canvas * 0.99)
    best = lo

    def fits(length: int) -> bool:
        scale = length / side
        ox = (canvas - length) / 2.0
        for x, y in content:
            dx = ox + x * scale - cx
            dy = ox + y * scale - cy
            if dx * dx + dy * dy > limit:
                return False
        return True

    while lo <= hi:
        mid = (lo + hi) // 2
        if fits(mid):
            best = mid
            lo = mid + 1
        else:
            hi = mid - 1

    logo = sq.resize((best, best), Image.Resampling.LANCZOS)
    # Opaque paper disk, then logo centered — transparent outside the circle.
    out = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    disk = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    ImageDraw.Draw(disk).ellipse((0, 0, canvas - 1, canvas - 1), fill=(*bg_rgb, 255))
    out.paste(disk, (0, 0), disk)
    ox = (canvas - best) // 2
    out.paste(logo, (ox, ox), logo)
    mask = Image.new("L", (canvas, canvas), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, canvas - 1, canvas - 1), fill=255)
    out.putalpha(mask)
    return out

brand = make_circular_brand(256)
brand.save(desk / "nemo_brand_round.png", optimize=True)
drawable = res / "drawable"
drawable.mkdir(parents=True, exist_ok=True)
brand.save(drawable / "nemo_brand_round.png", optimize=True)
print(f"brand mark: nemo_brand_round.png (256px, BRAND_R={BRAND_R})")

bg_hex = "#{:02X}{:02X}{:02X}".format(*bg_rgb)
(res / "values").mkdir(parents=True, exist_ok=True)
(res / "values" / "ic_launcher_background.xml").write_text(
    '<?xml version="1.0" encoding="utf-8"?>\n'
    "<resources>\n"
    f'    <color name="ic_launcher_background">{bg_hex}</color>\n'
    "</resources>\n",
    encoding="utf-8",
)
print(f"background color {bg_hex}")
print(f"done (SAFE_R={safe_r})")
PY
