"""
Icon Generator for LinRAR
Creates modern, clean, custom vector-rendered icons for LinRAR using Cairo.
Replaces all WinRAR-style icons with a fresh, cohesive modern Linux aesthetic.
"""

import cairo
import math
import os

BASE_DIR = os.path.dirname(__file__)
ICONS_DIRS = [
    os.path.join(BASE_DIR, "assets", "icons"),
    os.path.join(BASE_DIR, "linrar_app", "assets", "icons"),
]

for d in ICONS_DIRS:
    os.makedirs(d, exist_ok=True)


def save_surface(surf, filename):
    for d in ICONS_DIRS:
        p = os.path.join(d, filename)
        surf.write_to_png(p)


def round_rect(ctx, x, y, w, h, r):
    ctx.new_sub_path()
    ctx.arc(x + w - r, y + r, r, -math.pi / 2, 0)
    ctx.arc(x + w - r, y + h - r, r, 0, math.pi / 2)
    ctx.arc(x + r, y + h - r, r, math.pi / 2, math.pi)
    ctx.arc(x + r, y + r, r, math.pi, 3 * math.pi / 2)
    ctx.close_path()


def create_app_icon():
    surf = cairo.ImageSurface(cairo.FORMAT_ARGB32, 256, 256)
    ctx = cairo.Context(surf)

    # Background Squircle with subtle gradient
    pat = cairo.LinearGradient(0, 0, 256, 256)
    pat.add_color_stop_rgb(0, 0.10, 0.16, 0.28) # Slate-900 / Deep Navy
    pat.add_color_stop_rgb(1, 0.04, 0.08, 0.16)
    round_rect(ctx, 16, 16, 224, 224, 52)
    ctx.set_source(pat)
    ctx.fill()

    # Border stroke
    round_rect(ctx, 16, 16, 224, 224, 52)
    ctx.set_source_rgba(0.20, 0.60, 0.90, 0.4)
    ctx.set_line_width(4)
    ctx.stroke()

    # Modern isometric-styled archive cube
    # Left face
    ctx.move_to(128, 70)
    ctx.line_to(58, 108)
    ctx.line_to(58, 178)
    ctx.line_to(128, 216)
    ctx.close_path()
    pat_l = cairo.LinearGradient(58, 108, 128, 216)
    pat_l.add_color_stop_rgb(0, 0.08, 0.44, 0.72)
    pat_l.add_color_stop_rgb(1, 0.04, 0.28, 0.52)
    ctx.set_source(pat_l)
    ctx.fill()

    # Right face
    ctx.move_to(128, 70)
    ctx.line_to(198, 108)
    ctx.line_to(198, 178)
    ctx.line_to(128, 216)
    ctx.close_path()
    pat_r = cairo.LinearGradient(128, 70, 198, 178)
    pat_r.add_color_stop_rgb(0, 0.14, 0.60, 0.92)
    pat_r.add_color_stop_rgb(1, 0.08, 0.40, 0.70)
    ctx.set_source(pat_r)
    ctx.fill()

    # Top face
    ctx.move_to(128, 70)
    ctx.line_to(198, 108)
    ctx.line_to(128, 146)
    ctx.line_to(58, 108)
    ctx.close_path()
    pat_t = cairo.LinearGradient(58, 70, 198, 146)
    pat_t.add_color_stop_rgb(0, 0.35, 0.80, 1.00)
    pat_t.add_color_stop_rgb(1, 0.18, 0.65, 0.95)
    ctx.set_source(pat_t)
    ctx.fill()

    # Modern compression zipper / cyber stripe down center
    ctx.set_source_rgb(1, 1, 1)
    ctx.set_line_width(5)
    ctx.move_to(128, 146)
    ctx.line_to(128, 216)
    ctx.stroke()

    # Modern golden lock badge at center
    round_rect(ctx, 112, 134, 32, 38, 8)
    ctx.set_source_rgb(0.96, 0.72, 0.15) # Gold
    ctx.fill()

    # Keyhole
    ctx.set_source_rgb(0.12, 0.15, 0.22)
    ctx.arc(128, 149, 4, 0, 2 * math.pi)
    ctx.fill()
    ctx.rectangle(126, 149, 4, 10)
    ctx.fill()

    save_surface(surf, "winrar.png")
    save_surface(surf, "linrar.png")

    # Generate standard hicolor sizes
    sizes = [16, 24, 32, 48, 64, 128, 256]
    for sz in sizes:
        for d in ICONS_DIRS:
            sub_dir = os.path.join(d, "hicolor", f"{sz}x{sz}", "apps")
            os.makedirs(sub_dir, exist_ok=True)
            s_sz = cairo.ImageSurface(cairo.FORMAT_ARGB32, sz, sz)
            c_sz = cairo.Context(s_sz)
            c_sz.scale(sz / 256.0, sz / 256.0)
            c_sz.set_source_surface(surf, 0, 0)
            c_sz.paint()
            s_sz.write_to_png(os.path.join(sub_dir, "linrar.png"))
            if sz == 64:
                s_sz.write_to_png(os.path.join(d, "winrar_small.png"))


def base_canvas():
    s = cairo.ImageSurface(cairo.FORMAT_ARGB32, 96, 96)
    c = cairo.Context(s)
    return s, c


def create_toolbar_icons():
    # 1. Add (Modern Archive with Green +)
    s, c = base_canvas()
    round_rect(c, 16, 28, 64, 52, 12)
    pat = cairo.LinearGradient(16, 28, 80, 80)
    pat.add_color_stop_rgb(0, 0.15, 0.55, 0.88)
    pat.add_color_stop_rgb(1, 0.08, 0.35, 0.65)
    c.set_source(pat)
    c.fill()
    round_rect(c, 16, 18, 36, 18, 6)
    c.set_source_rgb(0.20, 0.65, 0.95)
    c.fill()
    c.arc(66, 66, 20, 0, 2 * math.pi)
    c.set_source_rgb(0.12, 0.75, 0.40)
    c.fill()
    c.set_source_rgb(1, 1, 1)
    c.set_line_width(4)
    c.move_to(66, 54); c.line_to(66, 78); c.stroke()
    c.move_to(54, 66); c.line_to(78, 66); c.stroke()
    save_surface(s, "add.png")

    # 2. Extract To (Box with Cyan Arrow Up/Out)
    s, c = base_canvas()
    round_rect(c, 16, 36, 64, 46, 10)
    pat = cairo.LinearGradient(16, 36, 80, 82)
    pat.add_color_stop_rgb(0, 0.20, 0.45, 0.75)
    pat.add_color_stop_rgb(1, 0.10, 0.28, 0.52)
    c.set_source(pat)
    c.fill()
    c.set_source_rgb(0.18, 0.82, 0.98)
    c.move_to(48, 12)
    c.line_to(66, 32)
    c.line_to(54, 32)
    c.line_to(54, 56)
    c.line_to(42, 56)
    c.line_to(42, 32)
    c.line_to(30, 32)
    c.close_path()
    c.fill()
    save_surface(s, "extract.png")

    # 3. Test (Shield with Checkmark)
    s, c = base_canvas()
    c.move_to(48, 14)
    c.line_to(76, 24)
    c.curve_to(76, 56, 64, 76, 48, 84)
    c.curve_to(32, 76, 20, 56, 20, 24)
    c.close_path()
    pat = cairo.LinearGradient(20, 14, 76, 84)
    pat.add_color_stop_rgb(0, 0.15, 0.72, 0.50)
    pat.add_color_stop_rgb(1, 0.08, 0.48, 0.35)
    c.set_source(pat)
    c.fill()
    c.set_source_rgb(1, 1, 1)
    c.set_line_width(6)
    c.set_line_cap(cairo.LINE_CAP_ROUND)
    c.move_to(34, 46); c.line_to(44, 56); c.line_to(62, 34); c.stroke()
    save_surface(s, "test.png")

    # 4. View (Document with Eye / Magnifying Glass)
    s, c = base_canvas()
    round_rect(c, 24, 14, 48, 68, 8)
    c.set_source_rgb(0.92, 0.94, 0.98)
    c.fill()
    c.set_source_rgb(0.70, 0.75, 0.85)
    c.set_line_width(3)
    for y in [30, 42, 54, 66]:
        c.move_to(34, y); c.line_to(62, y); c.stroke()
    c.arc(60, 58, 16, 0, 2 * math.pi)
    c.set_source_rgb(0.15, 0.55, 0.92)
    c.set_line_width(5)
    c.stroke()
    c.set_source_rgba(0.20, 0.70, 1.0, 0.35)
    c.fill()
    c.set_source_rgb(0.15, 0.55, 0.92)
    c.set_line_width(6)
    c.move_to(72, 70); c.line_to(84, 82); c.stroke()
    save_surface(s, "view.png")

    # 5. Delete (Trash Bin / Crimson Badge)
    s, c = base_canvas()
    round_rect(c, 16, 16, 64, 64, 20)
    pat = cairo.LinearGradient(16, 16, 80, 80)
    pat.add_color_stop_rgb(0, 0.95, 0.30, 0.32)
    pat.add_color_stop_rgb(1, 0.75, 0.15, 0.20)
    c.set_source(pat)
    c.fill()
    c.set_source_rgb(1, 1, 1)
    c.set_line_width(4)
    c.move_to(32, 34); c.line_to(64, 34); c.stroke()
    c.move_to(42, 28); c.line_to(54, 28); c.stroke()
    round_rect(c, 36, 38, 24, 32, 4)
    c.stroke()
    c.move_to(44, 44); c.line_to(44, 62); c.stroke()
    c.move_to(52, 44); c.line_to(52, 62); c.stroke()
    save_surface(s, "delete.png")

    # 6. Find (Search glass)
    s, c = base_canvas()
    c.arc(44, 42, 22, 0, 2 * math.pi)
    c.set_source_rgb(0.18, 0.50, 0.90)
    c.set_line_width(6)
    c.stroke()
    c.set_source_rgba(0.30, 0.70, 1.00, 0.25)
    c.fill()
    c.set_source_rgb(0.18, 0.50, 0.90)
    c.set_line_width(8)
    c.set_line_cap(cairo.LINE_CAP_ROUND)
    c.move_to(60, 58); c.line_to(80, 78); c.stroke()
    save_surface(s, "find.png")

    # 7. Info (Blue 'i' badge)
    s, c = base_canvas()
    c.arc(48, 48, 36, 0, 2 * math.pi)
    pat = cairo.LinearGradient(20, 20, 76, 76)
    pat.add_color_stop_rgb(0, 0.15, 0.50, 0.90)
    pat.add_color_stop_rgb(1, 0.08, 0.32, 0.65)
    c.set_source(pat)
    c.fill()
    c.set_source_rgb(1, 1, 1)
    c.arc(48, 32, 5, 0, 2 * math.pi); c.fill()
    round_rect(c, 44, 44, 8, 24, 4); c.fill()
    save_surface(s, "info.png")

    # 8. Repair (Wrench & Gear)
    s, c = base_canvas()
    round_rect(c, 16, 16, 64, 64, 20)
    pat = cairo.LinearGradient(16, 16, 80, 80)
    pat.add_color_stop_rgb(0, 0.95, 0.60, 0.15)
    pat.add_color_stop_rgb(1, 0.80, 0.40, 0.08)
    c.set_source(pat)
    c.fill()
    c.set_source_rgb(1, 1, 1)
    c.set_line_width(6)
    c.set_line_cap(cairo.LINE_CAP_ROUND)
    c.move_to(28, 68); c.line_to(58, 38); c.stroke()
    c.arc(62, 34, 12, 0, 2 * math.pi); c.stroke()
    save_surface(s, "repair.png")

    # 9. VirusScan (Security Shield)
    s, c = base_canvas()
    c.move_to(48, 14)
    c.line_to(76, 24)
    c.curve_to(76, 56, 64, 76, 48, 84)
    c.curve_to(32, 76, 20, 56, 20, 24)
    c.close_path()
    pat = cairo.LinearGradient(20, 14, 76, 84)
    pat.add_color_stop_rgb(0, 0.10, 0.68, 0.82)
    pat.add_color_stop_rgb(1, 0.06, 0.40, 0.60)
    c.set_source(pat)
    c.fill()
    c.set_source_rgb(1, 1, 1)
    c.set_line_width(4)
    c.arc(48, 46, 16, 0, 2 * math.pi); c.stroke()
    c.arc(48, 46, 6, 0, 2 * math.pi); c.fill()
    save_surface(s, "virus.png")

    # 10. Convert (Two circular arrows)
    s, c = base_canvas()
    round_rect(c, 16, 16, 64, 64, 20)
    pat = cairo.LinearGradient(16, 16, 80, 80)
    pat.add_color_stop_rgb(0, 0.20, 0.70, 0.95)
    pat.add_color_stop_rgb(1, 0.10, 0.45, 0.80)
    c.set_source(pat)
    c.fill()
    c.set_source_rgb(1, 1, 1)
    c.set_line_width(5)
    c.set_line_cap(cairo.LINE_CAP_ROUND)
    c.arc(48, 48, 20, -math.pi * 0.7, math.pi * 0.2)
    c.stroke()
    # arrow head top right
    c.move_to(68, 54); c.line_to(68, 42); c.line_to(56, 42); c.stroke()
    c.arc(48, 48, 20, math.pi * 0.3, math.pi * 1.2)
    c.stroke()
    # arrow head bottom left
    c.move_to(28, 42); c.line_to(28, 54); c.line_to(40, 54); c.stroke()
    save_surface(s, "convert.png")

    # 11. Report (Document with Chart)
    s, c = base_canvas()
    round_rect(c, 22, 14, 52, 68, 8)
    c.set_source_rgb(0.95, 0.96, 1.0)
    c.fill()
    c.set_source_rgb(0.20, 0.60, 0.90)
    round_rect(c, 22, 14, 52, 16, 4)
    c.fill()
    # Chart bars
    colors = [(0.2, 0.6, 0.9), (0.15, 0.75, 0.45), (0.95, 0.65, 0.15)]
    heights = [24, 34, 18]
    for i, (col, h) in enumerate(zip(colors, heights)):
        c.set_source_rgb(*col)
        round_rect(c, 32 + i * 14, 72 - h, 10, h, 2)
        c.fill()
    save_surface(s, "report.png")

    # 12. Benchmark (Speedometer / Lightning)
    s, c = base_canvas()
    round_rect(c, 16, 16, 64, 64, 20)
    pat = cairo.LinearGradient(16, 16, 80, 80)
    pat.add_color_stop_rgb(0, 0.98, 0.75, 0.15)
    pat.add_color_stop_rgb(1, 0.88, 0.45, 0.08)
    c.set_source(pat)
    c.fill()
    # Lightning bolt
    c.set_source_rgb(1, 1, 1)
    c.move_to(52, 22)
    c.line_to(34, 50)
    c.line_to(48, 50)
    c.line_to(44, 74)
    c.line_to(64, 44)
    c.line_to(50, 44)
    c.close_path()
    c.fill()
    save_surface(s, "benchmark.png")

    # 13. Folder (Amber directory icon for file list)
    s, c = base_canvas()
    round_rect(c, 12, 30, 72, 52, 8)
    pat = cairo.LinearGradient(12, 30, 84, 82)
    pat.add_color_stop_rgb(0, 0.98, 0.76, 0.22)
    pat.add_color_stop_rgb(1, 0.90, 0.58, 0.10)
    c.set_source(pat)
    c.fill()
    # Tab
    round_rect(c, 12, 18, 32, 18, 5)
    c.set_source_rgb(0.95, 0.68, 0.15)
    c.fill()
    save_surface(s, "folder.png")

    # 14. File (Clean modern document for file list)
    s, c = base_canvas()
    round_rect(c, 22, 14, 52, 68, 6)
    c.set_source_rgb(0.90, 0.93, 0.98)
    c.fill()
    c.set_source_rgb(0.70, 0.76, 0.88)
    c.set_line_width(3)
    c.set_line_cap(cairo.LINE_CAP_ROUND)
    for y in [30, 42, 54, 66]:
        c.move_to(32, y); c.line_to(64, y); c.stroke()
    save_surface(s, "file.png")

    # 15. Archive (Cyan vault cube)
    s, c = base_canvas()
    round_rect(c, 16, 16, 64, 64, 16)
    pat = cairo.LinearGradient(16, 16, 80, 80)
    pat.add_color_stop_rgb(0, 0.12, 0.75, 0.90)
    pat.add_color_stop_rgb(1, 0.06, 0.45, 0.70)
    c.set_source(pat)
    c.fill()
    # Gold lock
    round_rect(c, 36, 38, 24, 26, 6)
    c.set_source_rgb(0.98, 0.80, 0.20)
    c.fill()
    c.set_source_rgb(1, 1, 1)
    c.set_line_width(4)
    c.arc(48, 38, 8, math.pi, 2 * math.pi)
    c.stroke()
    save_surface(s, "archive.png")

    # 16. Up arrow
    s, c = base_canvas()
    round_rect(c, 16, 16, 64, 64, 16)
    c.set_source_rgb(0.20, 0.25, 0.35)
    c.fill()
    c.set_source_rgb(0.30, 0.80, 0.98)
    c.move_to(48, 28)
    c.line_to(64, 48)
    c.line_to(54, 48)
    c.line_to(54, 68)
    c.line_to(42, 68)
    c.line_to(42, 48)
    c.line_to(32, 48)
    c.close_path()
    c.fill()
    save_surface(s, "up.png")

    # 17. Refresh arrow
    s, c = base_canvas()
    round_rect(c, 16, 16, 64, 64, 16)
    c.set_source_rgb(0.20, 0.25, 0.35)
    c.fill()
    c.set_source_rgb(0.30, 0.80, 0.98)
    c.set_line_width(5)
    c.set_line_cap(cairo.LINE_CAP_ROUND)
    c.arc(48, 48, 18, -math.pi * 0.7, math.pi * 0.8)
    c.stroke()
    c.move_to(66, 44); c.line_to(66, 32); c.line_to(54, 34); c.fill()
    save_surface(s, "refresh.png")


if __name__ == "__main__":
    create_app_icon()
    create_toolbar_icons()
    print("All LinRAR visual graphic icons generated successfully!")
