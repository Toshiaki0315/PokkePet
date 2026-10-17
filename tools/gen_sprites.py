#!/usr/bin/env python3
"""仮のドット絵(32x32 PNG)を生成する。本番素材ができたら public/sprites/ を同名で差し替えること。

ファイル名は src/sprite.ts の spritePath と対応する:
  egg.png / {stage}-{branch}-{personality}.png  (stage 2..10)

性格ごとに「種族」が違う:
  calm=ふわもち(青) / playful=きつね(橙) / gluttonous=ぽっちゃり(桃) / tidy=うさぎ(緑)

使い方:
  python3 tools/gen_sprites.py [出力ディレクトリ] [--sheet PATH] [--catalog PATH]
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

SIZE = 32
BRANCHES = ["poor", "normal", "good"]
# かくしキャラ（最終段階だけ）。見た目は「good」を豪華にしたもの。
HIDDEN_BRANCH = "legend"
ALL_BRANCHES = BRANCHES + [HIDDEN_BRANCH]
GOOD_LIKE = ("good", HIDDEN_BRANCH)
PERSONALITIES = ["calm", "playful", "gluttonous", "tidy"]

PALETTE = {
    #            body            belly            accent
    "calm": ((110, 165, 235), (228, 241, 255), (255, 255, 255)),
    "playful": ((248, 158, 66), (255, 236, 205), (255, 255, 255)),
    "gluttonous": ((240, 134, 166), (255, 224, 232), (255, 96, 120)),
    "tidy": ((108, 205, 174), (236, 251, 241), (255, 186, 202)),
}
DARK = (46, 38, 56)
WHITE = (255, 255, 255)
GOLD = (255, 214, 72)
TEAR = (120, 185, 255)
PINK = (255, 150, 170)
RED = (230, 70, 70)
LEAF = (90, 190, 90)
GRAY = (130, 130, 138)


def mix(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


class Canvas:
    def __init__(self):
        self.px = {}

    def set(self, x, y, color):
        x, y = int(x), int(y)
        if 0 <= x < SIZE and 0 <= y < SIZE:
            self.px[(x, y)] = color

    def ellipse(self, cx, cy, rx, ry, color, shade=False):
        """楕円を塗る。shade=True で左上ハイライト/右下シェードを付ける。"""
        for y in range(SIZE):
            for x in range(SIZE):
                dx, dy = (x - cx) / rx, (y - cy) / ry
                if dx * dx + dy * dy > 1:
                    continue
                c = color
                if shade:
                    light = -(dx + dy) * 0.5
                    if light > 0.5:
                        c = mix(color, WHITE, 0.28)
                    elif light < -0.45:
                        c = mix(color, DARK, 0.2)
                self.set(x, y, c)

    def polygon(self, points, color):
        xs = [p[0] for p in points]
        ys = [p[1] for p in points]
        for y in range(int(min(ys)), int(max(ys)) + 1):
            for x in range(int(min(xs)), int(max(xs)) + 1):
                if inside(points, x + 0.5, y + 0.5):
                    self.set(x, y, color)

    def outline(self):
        out = {}
        for (x, y), c in self.px.items():
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                n = (x + dx, y + dy)
                if n not in self.px and 0 <= n[0] < SIZE and 0 <= n[1] < SIZE:
                    out[n] = mix(c, DARK, 0.62)
        self.px.update(out)

    def image(self):
        img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
        for (x, y), c in self.px.items():
            img.putpixel((x, y), (*c, 255))
        return img


def inside(points, x, y):
    ok = False
    j = len(points) - 1
    for i in range(len(points)):
        xi, yi = points[i]
        xj, yj = points[j]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi) + xi:
            ok = not ok
        j = i
    return ok


def egg():
    c = Canvas()
    c.ellipse(15.5, 18.5, 8.2, 11, (250, 244, 228), shade=True)
    for x, y, r in ((12, 14, 1.6), (19, 20, 2), (13, 23, 1.4), (17, 11, 1.1), (21, 15, 1.2)):
        c.ellipse(x, y, r, r, (214, 196, 168))
    c.outline()
    for x, y in ((11, 9), (12, 8), (11, 10)):
        c.set(x, y, WHITE)
    return c.image()


def creature(stage, branch, personality, asleep=False):
    body_c, belly_c, accent = PALETTE[personality]
    if branch == "poor":
        body_c = mix(body_c, GRAY, 0.45)
        belly_c = mix(belly_c, GRAY, 0.3)
    elif branch in GOOD_LIKE:
        body_c = mix(body_c, WHITE, 0.1)
    if branch == HIDDEN_BRANCH:
        body_c = mix(body_c, GOLD, 0.22)
        belly_c = mix(belly_c, WHITE, 0.4)

    rx = 4.2 + (stage - 2) * 0.72
    ry = rx * 0.9
    if personality == "gluttonous":
        rx, ry = rx * 1.14, ry * 0.97
    elif personality == "tidy":
        rx, ry = rx * 0.92, ry * 1.02
    cx = 15.5
    cy = 29.2 - ry
    top = cy - ry
    c = Canvas()

    # --- 背面パーツ -------------------------------------------------------
    if branch in GOOD_LIKE and stage >= 10:  # 小さな羽
        for sx in (-1, 1):
            c.polygon([(cx + sx * rx * 0.7, cy - ry * 0.2), (cx + sx * (rx + 5), cy - ry * 0.9),
                       (cx + sx * (rx + 4), cy + ry * 0.2)], (250, 250, 255))
    if stage >= 6:  # しっぽ
        tail(c, personality, cx, cy, rx, ry, body_c, belly_c, accent, stage)

    # --- 耳 ---------------------------------------------------------------
    if stage >= 3:
        ears(c, personality, branch, stage, cx, cy, rx, ry, top, body_c, accent)

    # --- 体 ---------------------------------------------------------------
    c.ellipse(cx, cy, rx, ry, body_c, shade=True)
    c.ellipse(cx, cy + ry * 0.28, rx * 0.62, ry * 0.58, belly_c)  # おなか
    if personality == "playful":  # ほおの白いふさ
        for sx in (-1, 1):
            c.polygon([(cx + sx * rx * 0.95, cy + ry * 0.1), (cx + sx * rx * 0.55, cy + ry * 0.25),
                       (cx + sx * rx * 0.95, cy + ry * 0.45)], (255, 255, 255))
    if personality == "calm" and stage >= 3:  # ぴょんと立つアホ毛
        for i, (dx, dy) in enumerate(((0, 1), (1, 2), (2, 2))):
            c.set(cx - 0.5 + dx, top - dy, mix(body_c, DARK, 0.1))
        c.set(cx + 2, top - 3, mix(body_c, DARK, 0.1))

    # --- 手足 -------------------------------------------------------------
    if stage >= 4:
        for sx in (-1, 1):
            c.ellipse(cx + sx * rx * 0.5, 29.5, max(1.8, rx * 0.26), 1.5, mix(body_c, DARK, 0.1))
    if stage >= 5:
        for sx in (-1, 1):
            c.ellipse(cx + sx * (rx + 0.2), cy + ry * 0.25, max(1.4, rx * 0.2), max(1.8, ry * 0.26),
                      mix(body_c, DARK, 0.05))
    if stage == 2:  # 卵の殻をかぶっている
        shell(c, cx, cy, rx, ry)

    # --- 小物・演出(輪郭を付ける前) ------------------------------------------
    accessory(c, personality, stage, cx, cy, rx, ry, top)
    if branch == HIDDEN_BRANCH:  # 王冠（天使の輪の代わり）
        crown_y = max(2, top - (7 if personality == "tidy" else 3))
        hx = cx - 0.5
        for dx in range(-3, 4):
            c.set(hx + dx, crown_y + 2, GOLD)
        for dx in (-3, 0, 3):
            c.set(hx + dx, crown_y + 1, GOLD)
            c.set(hx + dx, crown_y, GOLD)
        c.set(hx, crown_y + 1, (230, 60, 80))
    elif branch in GOOD_LIKE and stage >= 8:  # 天使の輪
        halo_y = max(1, top - (7 if personality == "tidy" else 3))
        hx = cx - 0.5
        for dx in range(-2, 3):
            c.set(hx + dx, halo_y, GOLD)
            c.set(hx + dx, halo_y + 2, GOLD)
        for dx in (-3, 3):
            c.set(hx + dx, halo_y + 1, GOLD)

    c.outline()

    # --- 表情 -------------------------------------------------------------
    face(c, personality, branch, stage, cx, cy, rx, ry, asleep)

    # --- 分岐の演出 -------------------------------------------------------
    if branch in GOOD_LIKE and stage >= 5:
        for x, y in ((3, 7), (28, 11), (4, 19)):
            sparkle(c, x, y)
        if branch == HIDDEN_BRANCH:  # まわりを星がかこむ
            for x, y in ((8, 3), (24, 4), (29, 22), (2, 26)):
                sparkle(c, x, y)
    if branch == "poor":
        if stage >= 3:  # 汗
            c.set(cx + rx * 0.7, cy - ry * 0.55, TEAR)
            c.set(cx + rx * 0.7, cy - ry * 0.55 + 1, TEAR)
        if stage >= 7:  # どよーん線
            for dx in (-4, -1, 2, 5):
                for dy in range(0, 3 + (dx % 2)):
                    c.set(cx + dx, max(0, top - 2 - dy), mix(GRAY, DARK, 0.2))
    return c.image()


def sparkle(c, x, y):
    c.set(x, y, GOLD)
    for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
        c.set(x + dx, y + dy, mix(GOLD, WHITE, 0.5))


def shell(c, cx, cy, rx, ry):
    """段階2: 卵の殻の下半分をかぶっている。"""
    y0 = cy + ry * 0.1
    for y in range(int(y0), SIZE):
        for x in range(SIZE):
            if (c.px.get((x, y)) is None):
                continue
            zig = 1 if int((x - cx) // 2) % 2 == 0 else 0
            if y >= y0 + zig * 2:
                c.set(x, y, (250, 244, 228) if (x + y) % 7 else (214, 196, 168))


def tail(c, personality, cx, cy, rx, ry, body_c, belly_c, accent, stage):
    s = 1 + (stage - 6) * 0.12
    if personality == "playful":  # ふさふさの大きなしっぽ(先が白)
        c.ellipse(cx + rx + 2.5, cy + ry * 0.35, 3.2 * s, 5.2 * s, body_c, shade=True)
        c.ellipse(cx + rx + 3.4, cy - ry * 0.35, 1.8 * s, 2.2 * s, WHITE)
    elif personality == "tidy":  # まるいぽんぽん
        c.ellipse(cx - rx - 1.2, cy + ry * 0.55, 2.4 * s, 2.4 * s, (255, 255, 255))
    elif personality == "gluttonous":  # くるん
        for x, y in ((cx + rx + 1, cy + ry * 0.6), (cx + rx + 2, cy + ry * 0.5), (cx + rx + 3, cy + ry * 0.3),
                     (cx + rx + 3, cy + ry * 0.1), (cx + rx + 2, cy)):
            c.ellipse(x, y, 0.9, 0.9, mix(body_c, DARK, 0.1))
    else:  # calm: 小さな雫型
        c.ellipse(cx + rx + 1.5, cy + ry * 0.55, 2.3 * s, 1.9 * s, mix(body_c, WHITE, 0.2))


def ears(c, personality, branch, stage, cx, cy, rx, ry, top, body_c, accent):
    g = min(1.0, 0.5 + (stage - 3) * 0.1)
    droop = branch == "poor"
    for sx in (-1, 1):
        if personality == "tidy":  # 長いうさ耳
            h = (2.5 + rx * 0.45) * g * (0.7 if droop else 1)
            ex = cx + sx * rx * (0.62 if droop else 0.5)
            ey = top + 1 - h * (0.3 if droop else 0.62)
            c.ellipse(ex + (sx * 2 if droop else 0), ey, rx * 0.2 + 0.7, h, body_c)
            c.ellipse(ex + (sx * 2 if droop else 0), ey + 0.5, rx * 0.1 + 0.3, h * 0.7, accent)
        elif personality == "playful":  # とがったきつね耳
            h = (3 + rx * 0.7) * g * (0.7 if droop else 1)
            ex = cx + sx * rx * 0.62
            if droop:
                c.polygon([(ex - 2.2, top + 2), (ex + sx * 3.5, top - h * 0.1 + 2), (ex + 2.2, top + 3)], body_c)
            else:
                c.polygon([(ex - 2.6, top + 2.5), (ex + sx * 0.4, top - h), (ex + 2.6, top + 2.5)], body_c)
                c.polygon([(ex - 1, top + 2), (ex + sx * 0.3, top - h * 0.45), (ex + 1, top + 2)], (60, 40, 50))
        elif personality == "gluttonous":  # ちいさな丸耳
            r = 1.8 + rx * 0.14
            c.ellipse(cx + sx * rx * 0.72, top + 1.5, r, r, body_c)
            c.ellipse(cx + sx * rx * 0.72, top + 1.8, r * 0.55, r * 0.55, accent)
        else:  # calm: 大きめのまるい耳(たれ気味)
            r = 2 + rx * 0.2
            c.ellipse(cx + sx * rx * 0.7, top + 2.2 + (1.5 if droop else 0), r, r * 1.1, body_c)


def accessory(c, personality, stage, cx, cy, rx, ry, top):
    if stage < 7:
        return
    if personality == "playful":  # 肩に星
        sx, sy = cx + rx * 0.55, max(2, top - 1)
        for dx, dy in ((0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (2, 0), (-2, 0), (0, 2), (0, -2)):
            c.set(sx + dx, sy + dy, GOLD)
    elif personality == "tidy":  # 耳の根元のリボン
        bx, by = cx - rx * 0.5 + 0.5, top + 2
        for dx, dy in ((-2, -1), (-2, 0), (-2, 1), (-1, 0), (2, -1), (2, 0), (2, 1), (1, 0)):
            c.set(bx + dx, by + dy, (255, 110, 150))
        c.set(bx, by, (255, 190, 210))
    elif personality == "gluttonous":  # りんごを抱えている
        ax, ay = cx + rx * 0.62, cy + ry * 0.55
        c.ellipse(ax, ay, 3, 3, RED, shade=True)
        c.set(ax, ay - 4, LEAF)
        c.set(ax + 1, ay - 4, LEAF)
        c.set(ax, ay - 3, (110, 70, 40))
    else:  # calm: 頭の上のちいさな葉っぱ
        lx, ly = cx + 3, top - 2
        c.polygon([(lx, ly + 1), (lx + 2, ly - 2), (lx + 4, ly + 1)], LEAF)


def face(c, personality, branch, stage, cx, cy, rx, ry, asleep=False):
    ex = max(2.2, rx * 0.42)
    ey = cy - ry * 0.12
    big = stage >= 6
    for sx in (-1, 1):
        x = round(cx - 0.5 + sx * ex)
        y = round(ey)
        if asleep:  # 目を閉じる（下にふくらむ ∪ の線。「にっこり ^ ^」の逆）
            half = 2 if big else 1
            for dx in range(-half, half + 1):
                c.set(x + dx, y + (0 if abs(dx) == half else 1), DARK)
        elif branch in GOOD_LIKE:  # にっこり ^ ^
            for dx, dy in ((-2, 1), (-1, 0), (0, 0), (1, 0), (2, 1)):
                c.set(x + dx, y + dy, DARK)
            if big:
                c.set(x - 2, y + 2, DARK)
                c.set(x + 2, y + 2, DARK)
        else:
            w = 3 if big else 2
            h = 4 if (big or personality == "playful") else 3
            for dx in range(w):
                for dy in range(h):
                    c.set(x - w // 2 + dx, y - h // 2 + dy, DARK)
            c.set(x - w // 2, y - h // 2, WHITE)  # ハイライト
            if personality == "playful":
                c.set(x - w // 2 + 1, y - h // 2 + 2, WHITE)
            if personality == "tidy":  # まつげ
                c.set(x + sx * (w // 2 + 1), y - h // 2, DARK)
            if branch == "poor":
                brow_y = y - h // 2 - 2  # 眉毛(ハの字: 内側が高い)
                c.set(x - sx, brow_y - 1, DARK)
                c.set(x, brow_y, DARK)
                c.set(x + sx, brow_y + 1, DARK)
                c.set(x, y + h // 2 + 1, TEAR)  # なみだ
    mx, my = round(cx - 0.5), round(cy + ry * 0.42)
    if asleep:  # ねむっているので、口はちいさく閉じる
        for dx in range(-1, 2):
            c.set(mx + dx, my, DARK)
    elif branch in GOOD_LIKE:  # 口をあけて笑う
        for dx in range(-2, 3):
            c.set(mx + dx, my, DARK)
        for dx in range(-1, 2):
            c.set(mx + dx, my + 1, (220, 80, 100))
        if stage >= 5:
            c.set(mx, my + 2, DARK)
        for sx in (-1, 1):  # ほっぺ
            c.set(cx - 0.5 + sx * (ex + 3), my - 1, PINK)
            c.set(cx - 0.5 + sx * (ex + 3) + sx, my - 1, PINK)
    elif branch == "poor":
        c.set(mx - 2, my + 1, DARK)
        for dx in range(-1, 2):
            c.set(mx + dx, my, DARK)
        c.set(mx + 2, my + 1, DARK)
    else:
        if personality == "gluttonous":  # もぐもぐ
            for dx in range(-1, 2):
                c.set(mx + dx, my, DARK)
            c.set(mx, my + 1, (220, 80, 100))
        else:
            c.set(mx - 1, my, DARK)
            c.set(mx, my + 1, DARK)
            c.set(mx + 1, my, DARK)
    if personality == "gluttonous":  # ふっくらほお
        for sx in (-1, 1):
            c.ellipse(cx - 0.5 + sx * (ex + 2.8), my - 0.5, 1.9, 1.5, (255, 120, 140) if branch != "poor" else GRAY)


def all_sprites():
    yield "egg.png", egg()
    for stage in range(2, 11):
        for branch in BRANCHES:
            for personality in PERSONALITIES:
                yield f"{stage}-{branch}-{personality}.png", creature(stage, branch, personality)
    for personality in PERSONALITIES:
        yield f"10-{HIDDEN_BRANCH}-{personality}.png", creature(10, HIDDEN_BRANCH, personality)
    # ねむっているときの絵（目を閉じている）。卵は目がないので、同じ絵を使う。
    for stage in range(2, 11):
        for branch in BRANCHES + ([HIDDEN_BRANCH] if stage == 10 else []):
            for personality in PERSONALITIES:
                yield f"{stage}-{branch}-{personality}-sleep.png", creature(stage, branch, personality, asleep=True)


def contact_sheet(sprites, path, scale=3):
    names = dict(sprites)
    cell = SIZE * scale
    cols = len(ALL_BRANCHES) * len(PERSONALITIES)
    sheet = Image.new("RGBA", (cols * cell, 10 * cell), (235, 235, 240, 255))
    sheet.paste(names["egg.png"].resize((cell, cell), Image.NEAREST), (0, 0))
    for stage in range(2, 11):
        for bi, branch in enumerate(ALL_BRANCHES):
            for pi, personality in enumerate(PERSONALITIES):
                key = f"{stage}-{branch}-{personality}.png"
                if key not in names:  # かくしキャラは最終段階だけ
                    continue
                im = names[key].resize((cell, cell), Image.NEAREST)
                sheet.alpha_composite(im, ((bi * 4 + pi) * cell, (stage - 1) * cell))
    sheet.save(path)


def labeled_catalog(sprites, path, scale=3):
    """ラベル付き一覧画像（docs 用）。行=成長段階、列=分岐×性格。"""
    names = dict(sprites)
    cell = SIZE * scale
    left, top = 64, 56
    cols = len(ALL_BRANCHES) * len(PERSONALITIES)
    sheet = Image.new("RGBA", (left + cols * cell, top + 10 * cell), (245, 245, 248, 255))
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.load_default()
    for bi, branch in enumerate(ALL_BRANCHES):
        x0 = left + bi * len(PERSONALITIES) * cell
        draw.rectangle([x0, 0, x0 + len(PERSONALITIES) * cell - 2, 18], fill=(225, 228, 238, 255))
        draw.text((x0 + 6, 4), f"branch: {branch}", fill=DARK, font=font)
        for pi, personality in enumerate(PERSONALITIES):
            draw.text((x0 + pi * cell + 4, 32), personality, fill=DARK, font=font)
    for stage in range(1, 11):
        y = top + (stage - 1) * cell
        draw.text((8, y + cell // 2 - 5), f"stage {stage}", fill=DARK, font=font)
        if stage == 1:
            im = names["egg.png"].resize((cell, cell), Image.NEAREST)
            sheet.alpha_composite(im, (left, y))
            draw.text((left + cell + 8, y + cell // 2 - 5), "egg (shared by all branches/personalities)", fill=DARK, font=font)
            continue
        for bi, branch in enumerate(ALL_BRANCHES):
            for pi, personality in enumerate(PERSONALITIES):
                key = f"{stage}-{branch}-{personality}.png"
                if key not in names:  # かくしキャラは最終段階だけ
                    continue
                im = names[key].resize((cell, cell), Image.NEAREST)
                sheet.alpha_composite(im, (left + (bi * len(PERSONALITIES) + pi) * cell, y))
    sheet.save(path)


def main(argv):
    args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or not argv[i - 1].startswith("--"))]
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
