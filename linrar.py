#!/usr/bin/env python3
"""
LinRAR - Universal Linux Archive Manager (v7.23)
Supports both full GUI (GTK3) and headless CLI operations.
Compatible with all Linux distributions (Ubuntu/Debian, Fedora/RHEL, Arch, openSUSE, Alpine, etc.).
"""

import sys
import os
import shutil

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if SCRIPT_DIR not in sys.path:
    sys.path.insert(0, SCRIPT_DIR)


def print_help():
    print("""LinRAR 7.23 - Universal Linux Archive Manager
Copyright (c) 1993-2026 Alexander Roshal / LinRAR Reimplementation

Usage:
  linrar                          Open LinRAR Graphical File Manager
  linrar <archive_or_folder>      Open specified archive or folder in GUI

Command-line (Headless) Mode:
  linrar a <archive> <files...>   Add/create archive from command line
  linrar x <archive> [dest_dir]   Extract archive with full paths
  linrar l <archive>              List contents of archive
  linrar t <archive>              Test integrity of archive
  linrar -h, --help               Show this help message
  linrar -v, --version            Show version information

Supported Formats:
  RAR (v4 & v5), ZIP, TAR, GZ, BZ2, XZ, 7-Zip
""")


def run_cli_command(cmd: str, args: list):
    from linrar_app.backend import ArchiveBackend

    if cmd == "a":
        if len(args) < 2:
            print("Usage: linrar a <archive> <file1> [file2...]")
            sys.exit(1)
        arc = args[0]
        files = args[1:]
        fmt = "rar"
        if arc.lower().endswith(".zip"):
            fmt = "zip"
        elif arc.lower().endswith((".tar.gz", ".tgz")):
            fmt = "tar.gz"
        print(f"LinRAR: Creating {arc} ({fmt.upper()})...")
        ok, msg = ArchiveBackend.create_archive(arc, files, format_type=fmt)
        print(msg)
        sys.exit(0 if ok else 1)

    elif cmd in ("x", "e"):
        if len(args) < 1:
            print("Usage: linrar x <archive> [dest_dir]")
            sys.exit(1)
        arc = args[0]
        dest = args[1] if len(args) > 1 else "."
        print(f"LinRAR: Extracting {arc} to {dest}...")
        ok, msg = ArchiveBackend.extract_archive(arc, dest)
        print(msg)
        sys.exit(0 if ok else 1)

    elif cmd in ("l", "v"):
        if len(args) < 1:
            print("Usage: linrar l <archive>")
            sys.exit(1)
        arc = args[0]
        try:
            entries, cmt, info = ArchiveBackend.list_archive(arc)
            print(f"Archive: {arc}")
            print(f"Format:  {info.get('Type', 'Unknown')} ({info.get('Details', '')})")
            print(f"{'Attributes':<11} {'Size':>10} {'Packed':>10} {'Ratio':>6}  {'Date/Time':<16}  {'Name'}")
            print("-" * 75)
            for e in entries:
                sz = f"{e.size:,}" if not e.is_dir else ""
                pk = f"{e.packed_size:,}" if not e.is_dir and e.packed_size > 0 else ""
                print(f"{e.attr:<11} {sz:>10} {pk:>10} {e.ratio:>6}  {e.mtime:<16}  {e.path}")
            print("-" * 75)
            print(f"Total: {info.get('Total Files', len(entries))} item(s), {info.get('Uncompressed Size', '0')} -> {info.get('Packed Size', '0')}")
            if cmt:
                print("\n[Archive Comment]:")
                print(cmt[:300] + ("..." if len(cmt) > 300 else ""))
            sys.exit(0)
        except Exception as e:
            print(f"Error listing archive: {e}")
            sys.exit(1)

    elif cmd == "t":
        if len(args) < 1:
            print("Usage: linrar t <archive>")
            sys.exit(1)
        arc = args[0]
        print(f"LinRAR: Testing {arc}...")
        ok, msg = ArchiveBackend.test_archive(arc)
        print(msg)
        sys.exit(0 if ok else 1)


def main():
    if len(sys.argv) > 1:
        first = sys.argv[1]
        if first in ("-h", "--help"):
            print_help()
            sys.exit(0)
        elif first in ("-v", "--version"):
            import platform
            print(f"LinRAR version 7.23 ({platform.machine()}) [RAR 7.23 Linux engine]")
            sys.exit(0)
        elif first in ("a", "x", "e", "l", "v", "t"):
            run_cli_command(first, sys.argv[2:])
            return

    # Check for GUI display environment
    has_display = bool(os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"))
    if not has_display:
        print("LinRAR: No graphical display server (DISPLAY or WAYLAND_DISPLAY) detected.")
        print("Running in headless terminal mode. For CLI usage, run: linrar --help")
        sys.exit(1)

    try:
        import gi
        gi.require_version("Gtk", "3.0")
        from gi.repository import GLib, Gtk

        # Crucial for Wayland and GNOME Shell dock association
        GLib.set_prgname("linrar")
        GLib.set_application_name("LinRAR")
        Gtk.Window.set_default_icon_name("linrar")

        from linrar_app.main_window import MainWindow
    except ImportError as e:
        print("LinRAR Error: PyGObject / GTK 3 is not installed.")
        print("To enable the graphical interface on your distribution:")
        print("  - Debian/Ubuntu:  sudo apt install python3-gi gir1.2-gtk-3.0")
        print("  - Fedora/RHEL:    sudo dnf install python3-gobject gtk3")
        print("  - Arch Linux:     sudo pacman -S python-gobject gtk3")
        print("  - openSUSE:       sudo zypper install python3-gobject gtk3")
        print("  - Alpine Linux:   apk add py3-gobject3 gtk+3.0")
        print("\nNote: Command-line commands (linrar a, linrar x, linrar l, linrar t) work without GTK!")
        sys.exit(1)

    target = sys.argv[1] if len(sys.argv) > 1 else None
    win = MainWindow(target_path=target)
    win.connect("destroy", Gtk.main_quit)
    Gtk.main()


if __name__ == "__main__":
    main()
