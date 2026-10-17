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
    header = (
        "/* 画面の背景(景色)。CSS だけで描く。.scene（画面の背景の層）と .scene-thumb（設定の見本）の両方で使う。\n"
        "   このファイルは tools/gen_scenes.py で作る（手で直さず、スクリプトを直して作り直す）。 */\n"
        ".scene-thumb { position: relative; display: inline-block; width: 56px; height: 44px; border-radius: 6px; overflow: hidden; border: 1px solid #0003; flex: none; }\n"
        ".scene-room::after, .scene-tatami::after, .scene-meadow::after, .scene-forest::after, .scene-beach::after { content: \"\"; position: absolute; inset: 0; }\n\n"
    )
    return (
        header
        + "/* おへや: 壁・木の床・窓・ラグ */\n" + room
        + "\n/* わしつ: 障子・畳・座ぶとん */\n" + tatami
        + "\n/* そうげん: 空・雲・おか・草 */\n" + meadow
        + "\n/* もり: こもれび・木・おちば */\n" + forest
        + "\n/* うみべ: 空・海・なみ・砂はま */\n" + beach
    )


if __name__ == "__main__":
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "src" / "scenes.css"
    out.write_text(build())
    print(f"wrote {out}")
