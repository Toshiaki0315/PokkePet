#!/usr/bin/env python3
"""アプリアイコン(1024x1024 PNG)を生成する。ドット絵キャラ(tools/gen_sprites.py)を主役にした独自デザイン。

使い方:
  python3 tools/gen_icon.py [出力PNG] [--candidates PATH]   # --candidates: 候補一覧画像を出力
生成後は `pnpm tauri icon <出力PNG>` で各サイズ(.icns/.ico/PNG)を作る。
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_sprites import creature  # noqa: E402

CANVAS = 1024
TILE = 824  # macOS のアイコンは 1024 の中に 824 の角丸四角を置く
RADIUS = 185

# (キャラの段階, 分岐, 性格, 背景の上の色, 背景の下の色)
CANDIDATES = {
    "calm": (8, "good", "calm", (255, 226, 205), (255, 160, 170)),
    "playful": (8, "good", "playful", (186, 232, 255), (110, 160, 235)),
    "tidy": (8, "good", "tidy", (255, 238, 190), (255, 170, 120)),
    "gluttonous": (8, "good", "gluttonous", (200, 240, 225), (90, 190, 170)),
}


def gradient(size, top, bottom):
    img = Image.new("RGB", (size, size))
    px = img.load()
    for y in range(size):
        t = y / (size - 1)
        row = tuple(round(a + (b - a) * t) for a, b in zip(top, bottom))
        for x in range(size):
            px[x, y] = row
    return img


def trim(img):
    box = img.getbbox()
    return img.crop(box) if box else img


def render(stage, branch, personality, top, bottom):
    canvas = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    tile = gradient(TILE, top, bottom).convert("RGBA")

    # やわらかい光（左上）
    gloss = Image.new("RGBA", (TILE, TILE), (0, 0, 0, 0))
    ImageDraw.Draw(gloss).ellipse([-TILE * 0.2, -TILE * 0.45, TILE * 0.9, TILE * 0.45], fill=(255, 255, 255, 70))
    tile = Image.alpha_composite(tile, gloss.filter(ImageFilter.GaussianBlur(40)))

    # キャラ（ドット絵を整数倍に拡大して、にじませない）
    sprite = trim(creature(stage, branch, personality))
    scale = max(1, int(TILE * 0.74 // max(sprite.size)))
    sprite = sprite.resize((sprite.width * scale, sprite.height * scale), Image.NEAREST)
    sx = (TILE - sprite.width) // 2
    sy = (TILE - sprite.height) // 2 + 28

    # 足もとの影
    shadow = Image.new("RGBA", (TILE, TILE), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).ellipse([sx + sprite.width * 0.1, sy + sprite.height - 26, sx + sprite.width * 0.9, sy + sprite.height + 30], fill=(60, 30, 50, 70))
    tile = Image.alpha_composite(tile, shadow.filter(ImageFilter.GaussianBlur(14)))
    tile.alpha_composite(sprite, (sx, sy))

    # 角丸に切り抜く
    mask = Image.new("L", (TILE, TILE), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, TILE - 1, TILE - 1], radius=RADIUS, fill=255)
    tile.putalpha(mask)

    # 影つきで 1024 の中央に置く
    drop = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    ImageDraw.Draw(drop).rounded_rectangle([100, 112, 100 + TILE, 112 + TILE], radius=RADIUS, fill=(0, 0, 0, 90))
    canvas = Image.alpha_composite(canvas, drop.filter(ImageFilter.GaussianBlur(18)))
    canvas.alpha_composite(tile, (100, 100))
    return canvas


def main(argv):
    args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or not argv[i - 1].startswith("--"))]
    out = Path(args[0]) if args else Path(__file__).resolve().parent.parent / "src-tauri" / "icons" / "source.png"
    if "--candidates" in argv:
        path = argv[argv.index("--candidates") + 1]
        sheet = Image.new("RGBA", (CANVAS * 2 // 2, CANVAS // 2 * 1), (245, 245, 248, 255))
        sheet = Image.new("RGBA", (256 * len(CANDIDATES), 256), (245, 245, 248, 255))
        for i, (name, spec) in enumerate(CANDIDATES.items()):
            sheet.alpha_composite(render(*spec).resize((256, 256), Image.LANCZOS), (i * 256, 0))
        sheet.save(path)
        print("candidates:", ", ".join(CANDIDATES))
        return
    name = next((a for a in argv if a in CANDIDATES), "calm")
    out.parent.mkdir(parents=True, exist_ok=True)
    render(*CANDIDATES[name]).save(out)
    print(f"wrote {out} ({name})")


if __name__ == "__main__":
    main(sys.argv[1:])
