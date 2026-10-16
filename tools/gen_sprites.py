#!/usr/bin/env python3
"""仮のドット絵(16x16 PNG)を生成する。本番素材ができたら public/sprites/ を差し替えること。

ファイル名は src/sprite.ts の spritePath と対応する:
  egg.png / {stage}-{branch}-{personality}.png  (stage 2..10)

使い方: python3 tools/gen_sprites.py [出力ディレクトリ] [--sheet コンタクトシートのパス] [--catalog ラベル付き一覧画像のパス]
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

SIZE = 16
BRANCHES = ["poor", "normal", "good"]
PERSONALITIES = {
    "calm": (120, 170, 230),
    "playful": (250, 170, 80),
    "gluttonous": (240, 130, 160),
    "tidy": (120, 210, 180),
}
DARK = (50, 45, 60)
WHITE = (255, 255, 255)
GOLD = (255, 215, 80)
BLUE_DROP = (110, 180, 255)


def mix(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def put(img, x, y, color):
    if 0 <= x < SIZE and 0 <= y < SIZE:
        img.putpixel((x, y), (*color, 255))


def blob(img, cx, cy, rx, ry, fill, outline):
    inside = set()
    for y in range(SIZE):
        for x in range(SIZE):
            if ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1:
                inside.add((x, y))
    for x, y in inside:
        edge = any((x + dx, y + dy) not in inside for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))
        put(img, x, y, outline if edge else fill)
    return inside


def egg():
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    blob(img, 7.5, 9.5, 4.2, 5.6, (250, 245, 230), (150, 140, 130))
    for x, y in ((6, 8), (9, 11), (7, 12), (8, 6)):
        put(img, x, y, (205, 190, 165))
    put(img, 6, 5, WHITE)
    return img


def creature(stage, branch, personality):
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    base = PERSONALITIES[personality]
    fill = mix(base, (130, 130, 130), 0.4) if branch == "poor" else base
    outline = mix(fill, DARK, 0.55)

    rx = min(2.6 + (stage - 2) * 0.5, 6.2)
    ry = rx * 0.92
    if personality == "gluttonous":
        rx, ry = min(rx * 1.1, 6.8), ry * 0.96
    cx = 7.5
    cy = 14.5 - ry

    # 耳 (stage 4~)
    if stage >= 4:
        for sx in (-1, 1):
            ex = round(cx + sx * rx * 0.6)
            ey = round(cy - ry) - 1
            put(img, ex, ey, outline)
            put(img, ex, ey + 1, fill)
    blob(img, cx, cy, rx, ry, fill, outline)
    # 足 (stage 6~)
    if stage >= 6:
        for sx in (-1, 1):
            put(img, round(cx + sx * rx * 0.45), 15, outline)
    # ハイライト
    put(img, round(cx - rx * 0.45), round(cy - ry * 0.45), mix(fill, WHITE, 0.6))

    ex = max(1, round(rx * 0.42))
    ey = round(cy - ry * 0.15)
    for sx in (-1, 1):
        x = round(cx + sx * ex)
        if branch == "good":
            put(img, x, ey, DARK)  # ^ 型
            put(img, x - 1, ey + 1, DARK)
            put(img, x + 1, ey + 1, DARK)
        elif branch == "poor":
            put(img, x, ey, DARK)
            put(img, x + (1 if sx < 0 else -1), ey - 1, DARK)
        else:
            put(img, x, ey, DARK)
            put(img, x, ey + 1, DARK)
    my = round(cy + ry * 0.4)
    mx = round(cx)
    if branch == "good":
        put(img, mx - 1, my, DARK)
        put(img, mx, my + 1, DARK)
        put(img, mx + 1, my, DARK)
        for sx in (-1, 1):  # ほっぺ
            put(img, round(cx + sx * ex * 1.8), my, mix((255, 150, 170), fill, 0.3))
    elif branch == "poor":
        put(img, mx - 1, my + 1, DARK)
        put(img, mx, my, DARK)
        put(img, mx + 1, my + 1, DARK)
    else:
        put(img, mx - 1, my, DARK)
        put(img, mx, my, DARK)

    # 性格ごとの小物
    top = round(cy - ry) - (2 if stage >= 4 else 0)
    if personality == "playful":  # 星
        px, py = min(14, round(cx + rx) + 1), max(1, top)
        for dx, dy in ((0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)):
            put(img, px + dx, py + dy, GOLD)
    elif personality == "tidy":  # リボン
        bx, by = round(cx), max(0, round(cy - ry) - 1)
        for dx in (-1, 1):
            put(img, bx + dx, by, WHITE)
        put(img, bx, by, BLUE_DROP)
    elif personality == "gluttonous":  # おなかの丸
        put(img, round(cx), round(cy + ry * 0.7), mix(fill, WHITE, 0.45))

    # 分岐の演出
    if branch == "good" and stage >= 3:
        for x, y in ((1, 2), (14, 4), (2, 8)):
            put(img, x, y, GOLD)
    if branch == "good" and stage >= 8:
        cxr = round(cx)
        for dx in (-2, 0, 2):
            put(img, cxr + dx, max(0, round(cy - ry) - 3), GOLD)
        for dx in (-2, -1, 0, 1, 2):
            put(img, cxr + dx, max(0, round(cy - ry) - 2), GOLD)
    if branch == "poor":
        put(img, min(14, round(cx + rx * 0.7)), round(cy - ry * 0.4), BLUE_DROP)  # 汗

    return img


def all_sprites():
    yield "egg.png", egg()
    for stage in range(2, 11):
        for branch in BRANCHES:
            for personality in PERSONALITIES:
                yield f"{stage}-{branch}-{personality}.png", creature(stage, branch, personality)


def contact_sheet(sprites, path, scale=4):
    names = dict(sprites)
    cols = 12  # 3 branches x 4 personalities
    rows = 9
    sheet = Image.new("RGBA", (cols * SIZE * scale + 4, (rows + 1) * SIZE * scale + 4), (235, 235, 240, 255))
    sheet.paste(names["egg.png"].resize((SIZE * scale, SIZE * scale), Image.NEAREST), (0, 0))
    for stage in range(2, 11):
        for bi, branch in enumerate(BRANCHES):
            for pi, personality in enumerate(PERSONALITIES):
                im = names[f"{stage}-{branch}-{personality}.png"].resize((SIZE * scale, SIZE * scale), Image.NEAREST)
                sheet.alpha_composite(im, ((bi * 4 + pi) * SIZE * scale, (stage - 1) * SIZE * scale))
    sheet.save(path)


def labeled_catalog(sprites, path, scale=5):
    """ラベル付き一覧画像（docs 用）。行=成長段階、列=分岐×性格。"""
    names = dict(sprites)
    cell = SIZE * scale
    left, top = 64, 56
    cols = len(BRANCHES) * len(PERSONALITIES)
    sheet = Image.new("RGBA", (left + cols * cell, top + 10 * cell), (245, 245, 248, 255))
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.load_default()
    for bi, branch in enumerate(BRANCHES):
        x0 = left + bi * len(PERSONALITIES) * cell
        draw.rectangle([x0, 0, x0 + len(PERSONALITIES) * cell - 2, 18], fill=(225, 228, 238, 255))
        draw.text((x0 + 6, 4), f"branch: {branch}", fill=DARK, font=font)
        for pi, personality in enumerate(PERSONALITIES):
            draw.text((x0 + pi * cell + 4, 30), personality, fill=DARK, font=font)
    for stage in range(1, 11):
        y = top + (stage - 1) * cell
        draw.text((8, y + cell // 2 - 5), f"stage {stage}", fill=DARK, font=font)
        if stage == 1:
            im = names["egg.png"].resize((cell, cell), Image.NEAREST)
            sheet.alpha_composite(im, (left, y))
            draw.text((left + cell + 8, y + cell // 2 - 5), "egg (shared by all branches/personalities)", fill=DARK, font=font)
            continue
        for bi, branch in enumerate(BRANCHES):
            for pi, personality in enumerate(PERSONALITIES):
                im = names[f"{stage}-{branch}-{personality}.png"].resize((cell, cell), Image.NEAREST)
                sheet.alpha_composite(im, (left + (bi * len(PERSONALITIES) + pi) * cell, y))
    sheet.save(path)


def main(argv):
    args = [a for a in argv if not a.startswith("--")]
    out = Path(args[0]) if args else Path(__file__).resolve().parent.parent / "public" / "sprites"
    out.mkdir(parents=True, exist_ok=True)
    sprites = list(all_sprites())
    for name, img in sprites:
        img.save(out / name)
    if "--sheet" in argv:
        contact_sheet(sprites, argv[argv.index("--sheet") + 1])
    if "--catalog" in argv:
        labeled_catalog(sprites, argv[argv.index("--catalog") + 1])
    print(f"wrote {len(sprites)} sprites to {out}")


if __name__ == "__main__":
    main(sys.argv[1:])
