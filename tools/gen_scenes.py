#!/usr/bin/env python3
"""画面の背景（src/scenes.css）を作る。

CSS の background-position の % は「画像の p% の点を、枠の p% の点に合わせる」意味なので、
「左上を (x%, y%) に置きたい」を p に直す計算を、ここでまとめて行う。

使い方: python3 tools/gen_scenes.py [出力先 (既定: src/scenes.css)]
"""
import sys
from pathlib import Path


def pos(offset, size):
    """左上を offset% に置くための background-position の %。"""
    if size >= 100:
        return "0"
    return f"{offset / (100 - size) * 100:.2f}%"


def layer(img, x, y, w, h):
    """左上が枠の (x%, y%) で、大きさが w% × h% の 1 枚の層。"""
    return f"{img} {pos(x, w)} {pos(y, h)} / {w}% {h}% no-repeat"


def block(name, layers, after=None):
    out = f".scene-{name} {{\n  background:\n" + ",\n".join("    " + l for l in layers) + ";\n}\n"
    if after:
        out += f".scene-{name}::after {{\n  background:\n" + ",\n".join("    " + l for l in after) + ";\n}\n"
    return out


def build():
    L = layer
    room = block("room", [
        L("linear-gradient(#8a5a2b, #8a5a2b)", 0, 70, 100, 2.5),
        L("repeating-linear-gradient(90deg, rgba(0,0,0,0.08) 0 1px, transparent 1px 22%)", 0, 72.5, 100, 27.5),
        L("linear-gradient(#cf9a5f, #b8803f)", 0, 72.5, 100, 27.5),
        "linear-gradient(#f9ead0, #f1d8ad)",
    ], [
        L("radial-gradient(ellipse at center, #e8745f 0 52%, #f6b3a3 54% 68%, transparent 70%)", 20, 84, 60, 15),
        L("linear-gradient(#8a5a2b, #8a5a2b)", 27, 15, 3, 30),
        L("linear-gradient(#8a5a2b, #8a5a2b)", 12, 28, 34, 3),
        L("linear-gradient(180deg, #bfe6ff, #eaf7ff)", 13.5, 16.5, 31, 27),
        L("linear-gradient(#8a5a2b, #8a5a2b)", 12, 15, 34, 31),
        L("linear-gradient(#ffe9a8, #ffd36b)", 77, 24, 12, 14),
        L("linear-gradient(#8a5a2b, #8a5a2b)", 75.5, 21, 15, 20),
    ])
    tatami = block("tatami", [
        L("repeating-linear-gradient(90deg, #7a5a34 0 1.5%, transparent 1.5% 33.4%)", 0, 0, 100, 62),
        L("repeating-linear-gradient(180deg, #7a5a34 0 2%, transparent 2% 25%)", 0, 0, 100, 62),
        L("linear-gradient(#fbf6e6, #f3ead0)", 0, 0, 100, 62),
        L("linear-gradient(#4f5f27, #4f5f27)", 0, 62, 100, 2.5),
        L("repeating-linear-gradient(90deg, #8d9c52 0 1.2%, transparent 1.2% 50%)", 0, 64.5, 100, 35.5),
        L("repeating-linear-gradient(180deg, #8d9c52 0 1.5%, transparent 1.5% 50%)", 0, 64.5, 100, 35.5),
        "linear-gradient(#c7d48a, #b5c577)",
    ], [
        L("radial-gradient(ellipse at center, #c95d5d 0 58%, #a64343 60% 72%, transparent 74%)", 27, 80, 46, 18),
        L("radial-gradient(circle at center, #fff 0 38%, #f4c2c2 40% 52%, transparent 54%)", 80, 8, 14, 14),
    ])
    meadow = block("meadow", [
        L("linear-gradient(#5fb33a, #4a9a2a)", 0, 90, 100, 10),
        "radial-gradient(ellipse 75% 42% at 18% 100%, #86cf58 98%, transparent 100%)",
        "radial-gradient(ellipse 70% 36% at 88% 100%, #6dbd46 98%, transparent 100%)",
        "linear-gradient(#86cdfa, #e4f6ff 62%)",
    ], [
        L("radial-gradient(ellipse at center, #fff 0 60%, transparent 64%)", 10, 10, 26, 10),
        L("radial-gradient(ellipse at center, #fff 0 60%, transparent 64%)", 28, 6, 20, 9),
        L("radial-gradient(ellipse at center, #fff 0 60%, transparent 64%)", 60, 22, 24, 9),
        L("radial-gradient(circle at center, #fff3a6 0 36%, rgba(255,243,166,0.45) 40% 60%, transparent 64%)", 72, 2, 24, 24),
        L("radial-gradient(circle at center, #ffd1e0 0 30%, transparent 34%)", 8, 88, 6, 5),
        L("radial-gradient(circle at center, #fff 0 30%, transparent 34%)", 38, 92, 6, 5),
        L("radial-gradient(circle at center, #ffe27a 0 30%, transparent 34%)", 84, 90, 6, 5),
    ])
    forest = block("forest", [
        L("linear-gradient(#35452a, #2a3a20)", 0, 88, 100, 12),
        L("linear-gradient(#5b3d24, #4a301b)", 10, 22, 7, 70),
        L("linear-gradient(#5b3d24, #4a301b)", 44, 30, 6, 62),
        L("linear-gradient(#5b3d24, #4a301b)", 80, 18, 8, 74),
        L("radial-gradient(ellipse at center, #3f8f4e 0 70%, transparent 72%)", -4, -6, 34, 40),
        L("radial-gradient(ellipse at center, #46a057 0 70%, transparent 72%)", 30, 4, 30, 34),
        L("radial-gradient(ellipse at center, #3a8446 0 70%, transparent 72%)", 66, -6, 38, 42),
        "linear-gradient(#2f6a3d, #1f4a2c)",
    ], [
        "linear-gradient(105deg, transparent 38%, rgba(255,255,190,0.22) 46%, transparent 56%)",
        "linear-gradient(112deg, transparent 58%, rgba(255,255,190,0.14) 64%, transparent 72%)",
        L("radial-gradient(ellipse at center, #d98b2b 0 55%, transparent 60%)", 20, 92, 8, 5),
        L("radial-gradient(ellipse at center, #c4662a 0 55%, transparent 60%)", 58, 94, 7, 5),
    ])
    beach = block("beach", [
        L("linear-gradient(#f3dfa5, #e6c776)", 0, 68, 100, 32),
        L("repeating-linear-gradient(180deg, rgba(255,255,255,0.55) 0 1.6%, transparent 1.6% 8%)", 0, 46, 100, 22),
        L("linear-gradient(#3aa1d9, #6ec7ee)", 0, 44, 100, 25),
        "linear-gradient(#78cdfb, #d4f0ff 44%)",
    ], [
        L("radial-gradient(circle at center, #fff4a8 0 34%, rgba(255,244,168,0.4) 38% 58%, transparent 62%)", 6, 2, 26, 26),
        L("radial-gradient(ellipse at center, #fff 0 60%, transparent 64%)", 52, 10, 24, 8),
        L("linear-gradient(#fff, #fff)", 0, 68, 100, 1.5),
        L("radial-gradient(circle at center, #f4a6b8 0 40%, transparent 44%)", 78, 88, 8, 6),
        L("radial-gradient(circle at center, #fff 0 40%, transparent 44%)", 22, 91, 7, 5),
    ])
    sakura = block("sakura", [
        L("linear-gradient(#2a2140, #1d1733)", 0, 90, 100, 10),
        L("linear-gradient(#3b2a2a, #2c1f1f)", 4, 28, 7, 66),
        L("radial-gradient(ellipse at center, #ffb7d0 0 62%, transparent 66%)", -6, 2, 36, 28),
        L("radial-gradient(ellipse at center, #ff9ec0 0 62%, transparent 66%)", 10, 6, 34, 26),
        L("radial-gradient(ellipse at center, #ffd0e0 0 62%, transparent 66%)", 22, -4, 30, 22),
        L("radial-gradient(ellipse at center, #ffa9c8 0 62%, transparent 66%)", -4, 20, 24, 20),
        "linear-gradient(#1b1840, #3a2a63 58%, #6a4585)",
    ], [
        L("radial-gradient(circle at center, #fff6d0 0 34%, rgba(255,246,208,0.35) 38% 58%, transparent 62%)", 74, 4, 22, 22),
        L("radial-gradient(circle at center, #ffc7dc 0 42%, transparent 46%)", 40, 30, 3, 3),
        L("radial-gradient(circle at center, #ffc7dc 0 42%, transparent 46%)", 55, 48, 3, 3),
        L("radial-gradient(circle at center, #ffc7dc 0 42%, transparent 46%)", 30, 62, 3, 3),
        L("radial-gradient(circle at center, #ffc7dc 0 42%, transparent 46%)", 82, 58, 3, 3),
        L("radial-gradient(circle at center, #ffc7dc 0 42%, transparent 46%)", 66, 74, 3, 3),
        L("radial-gradient(circle at center, #ffc7dc 0 42%, transparent 46%)", 18, 80, 3, 3),
    ])
    snow = block("snow", [
        L("linear-gradient(#ffffff, #eef4fb)", 0, 80, 100, 20),
        "radial-gradient(ellipse 70% 30% at 20% 100%, #f7fafd 98%, transparent 100%)",
        "radial-gradient(ellipse 60% 26% at 90% 100%, #eef4fb 98%, transparent 100%)",
        "linear-gradient(#9db6d4, #dbe7f3 58%)",
    ], [
        L("radial-gradient(circle at center, #fff 0 36%, transparent 42%)", 12, 10, 4, 4),
        L("radial-gradient(circle at center, #fff 0 36%, transparent 42%)", 30, 24, 3, 3),
        L("radial-gradient(circle at center, #fff 0 36%, transparent 42%)", 48, 8, 4, 4),
        L("radial-gradient(circle at center, #fff 0 36%, transparent 42%)", 64, 30, 3, 3),
        L("radial-gradient(circle at center, #fff 0 36%, transparent 42%)", 80, 14, 4, 4),
        L("radial-gradient(circle at center, #fff 0 36%, transparent 42%)", 90, 40, 3, 3),
        L("radial-gradient(circle at center, #fff 0 36%, transparent 42%)", 20, 46, 3, 3),
        L("radial-gradient(circle at center, #fff 0 36%, transparent 42%)", 56, 52, 3, 3),
        L("radial-gradient(circle at center, #fff 0 46%, #cfe0f2 50% 60%, transparent 64%)", 80, 66, 12, 12),
        L("radial-gradient(circle at center, #fff 0 46%, #cfe0f2 50% 60%, transparent 64%)", 83, 56, 7, 8),
    ])
    autumn = block("autumn", [
        L("linear-gradient(#a8622a, #8a4e1e)", 0, 88, 100, 12),
        L("linear-gradient(#5b3d24, #4a301b)", 14, 30, 6, 62),
        L("linear-gradient(#5b3d24, #4a301b)", 74, 24, 7, 68),
        L("radial-gradient(ellipse at center, #d9531e 0 68%, transparent 72%)", -2, 4, 34, 36),
        L("radial-gradient(ellipse at center, #e8832a 0 68%, transparent 72%)", 8, 12, 24, 24),
        L("radial-gradient(ellipse at center, #c23b22 0 68%, transparent 72%)", 62, 0, 38, 36),
        L("radial-gradient(ellipse at center, #eba033 0 68%, transparent 72%)", 70, 12, 24, 24),
        "linear-gradient(#f4b66a, #fbe3b8 56%)",
    ], [
        L("radial-gradient(ellipse at center, #d9531e 0 55%, transparent 60%)", 30, 92, 7, 4),
        L("radial-gradient(ellipse at center, #eba033 0 55%, transparent 60%)", 48, 94, 7, 4),
        L("radial-gradient(ellipse at center, #c23b22 0 55%, transparent 60%)", 62, 91, 7, 4),
        L("radial-gradient(ellipse at center, #e8832a 0 55%, transparent 60%)", 40, 40, 4, 3),
        L("radial-gradient(ellipse at center, #d9531e 0 55%, transparent 60%)", 54, 62, 4, 3),
    ])
    stars = block("stars", [
        "radial-gradient(ellipse 80% 24% at 22% 100%, #0e1438 98%, transparent 100%)",
        "radial-gradient(ellipse 70% 20% at 86% 100%, #121a44 98%, transparent 100%)",
        "linear-gradient(#0b1030, #1d2b6b 66%, #34459a)",
    ], [
        L("radial-gradient(circle at center, #fff6c8 0 40%, transparent 44%)", 70, 8, 16, 16),
        L("radial-gradient(circle at center, #1b2866 0 40%, transparent 44%)", 74, 6, 14, 14),
        L("radial-gradient(circle at center, #fff 0 30%, transparent 36%)", 10, 8, 3, 3),
        L("radial-gradient(circle at center, #fff 0 30%, transparent 36%)", 26, 20, 2, 2),
        L("radial-gradient(circle at center, #fff 0 30%, transparent 36%)", 42, 6, 3, 3),
        L("radial-gradient(circle at center, #fff 0 30%, transparent 36%)", 56, 28, 2, 2),
        L("radial-gradient(circle at center, #fff 0 30%, transparent 36%)", 34, 38, 2, 2),
        L("radial-gradient(circle at center, #fff 0 30%, transparent 36%)", 90, 32, 3, 3),
        L("radial-gradient(circle at center, #ffe9a8 0 30%, transparent 36%)", 18, 34, 3, 3),
        L("radial-gradient(circle at center, #fff 0 30%, transparent 36%)", 60, 50, 2, 2),
        L("radial-gradient(circle at center, #fff 0 30%, transparent 36%)", 8, 52, 2, 2),
    ])
    season = ".scene-season {\n  background: linear-gradient(135deg, #ffb7d0 0 25%, #86cf58 25% 50%, #e8832a 50% 75%, #cfe0f2 75%);\n}\n"
    header = (
        "/* 画面の背景(景色)。CSS だけで描く。.scene（画面の背景の層）と .scene-thumb（設定の見本）の両方で使う。\n"
        "   このファイルは tools/gen_scenes.py で作る（手で直さず、スクリプトを直して作り直す）。 */\n"
        ".scene-thumb { position: relative; display: inline-block; width: 56px; height: 44px; border-radius: 6px; overflow: hidden; border: 1px solid #0003; flex: none; }\n"
        ".scene-room::after, .scene-tatami::after, .scene-meadow::after, .scene-forest::after, .scene-beach::after, .scene-sakura::after, .scene-snow::after, .scene-autumn::after, .scene-stars::after { content: \"\"; position: absolute; inset: 0; }\n\n"
    )
    return (
        header
        + "/* おへや: 壁・木の床・窓・ラグ */\n" + room
        + "\n/* わしつ: 障子・畳・座ぶとん */\n" + tatami
        + "\n/* そうげん: 空・雲・おか・草 */\n" + meadow
        + "\n/* もり: こもれび・木・おちば */\n" + forest
        + "\n/* うみべ: 空・海・なみ・砂はま */\n" + beach
        + "\n/* よざくら: 夜空・月・さくらの木 */\n" + sakura
        + "\n/* ゆきげしき: ゆき・雪だるま */\n" + snow
        + "\n/* あき: 夕やけ・紅葉・おちば */\n" + autumn
        + "\n/* ほしぞら: 夜空・月・星 */\n" + stars
        + "\n/* 季節にまかせる（設定の見本だけ。画面では、そのときの季節の背景を使う） */\n" + season
    )


if __name__ == "__main__":
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "src" / "scenes.css"
    out.write_text(build())
    print(f"wrote {out}")
