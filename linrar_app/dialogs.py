"""
LinRAR - Dialog Windows
Faithful reimplementation of WinRAR's signature dialogs:
- GETARC: Add to Archive Dialog
- GETEXTRPATH: Extraction Path and Options Dialog
- INFO: Archive Information Dialog
- REMINDER: 40-Day Evaluation / Trial Reminder Dialog
- ABOUTRARDLG: About WinRAR Dialog
- GETPASSWORD2: Set/Enter Password Dialog
- BENCHMARK: Compression Speed Benchmark Dialog
"""

import os
import sys
import threading
import time
import zlib
import webbrowser
from typing import Optional, List, Tuple, Dict, Any
import gi

gi.require_version("Gtk", "3.0")
from gi.repository import Gtk, Gdk, GdkPixbuf, GLib

APP_DIR = os.path.dirname(os.path.abspath(__file__))
ICONS_DIR = os.path.join(APP_DIR, "assets", "icons")


def get_icon_pixbuf(name: str, size: int = 32) -> Optional[GdkPixbuf.Pixbuf]:
    path = os.path.join(ICONS_DIR, name)
    if os.path.isfile(path):
        try:
            return GdkPixbuf.Pixbuf.new_from_file_at_scale(path, size, size, True)
        except Exception:
            return None
    return None


class TrialReminderDialog(Gtk.Dialog):
    """
    LinRAR Registration and License Notice dialog.
    """
    def __init__(self, parent=None):
        super().__init__(
            title="LinRAR Registration Notice",
            transient_for=parent,
            modal=True,
            destroy_with_parent=True
        )
        self.set_default_size(500, 240)
        self.set_position(Gtk.WindowPosition.CENTER_ON_PARENT)
        self.set_resizable(False)

        box = self.get_content_area()
        box.set_spacing(15)
        box.set_margin_top(20)
        box.set_margin_bottom(20)
        box.set_margin_start(20)
        box.set_margin_end(20)

        hbox = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=20)
        box.pack_start(hbox, True, True, 0)

        # LinRAR Modern Vault Icon
        icon_path = os.path.join(ICONS_DIR, "winrar.png")
        if os.path.isfile(icon_path):
            pixbuf = GdkPixbuf.Pixbuf.new_from_file_at_scale(icon_path, 80, 80, True)
            img = Gtk.Image.new_from_pixbuf(pixbuf)
            img.set_alignment(0.5, 0.1)
            hbox.pack_start(img, False, False, 0)

        vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        hbox.pack_start(vbox, True, True, 0)

        title_lbl = Gtk.Label()
        title_lbl.set_markup("<b><big>LinRAR for Linux (v7.23)</big></b>")
        title_lbl.set_xalign(0)
        vbox.pack_start(title_lbl, False, False, 0)

        msg_lbl = Gtk.Label()
        msg_lbl.set_text(
            "LinRAR is an independent native Linux archive manager powered by the official "
            "RAR 7.23 Linux engine. It provides full support for creating and extracting "
            "RAR, ZIP, TAR, and GZ archives with a modern Linux interface."
        )
        msg_lbl.set_line_wrap(True)
        msg_lbl.set_xalign(0)
        vbox.pack_start(msg_lbl, False, False, 0)

        thank_lbl = Gtk.Label()
        thank_lbl.set_markup("<small><i>Thank you for using LinRAR on Linux!</i></small>")
        thank_lbl.set_xalign(0)
        vbox.pack_start(thank_lbl, False, False, 0)

        # Action Buttons
        btn_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        self.get_action_area().pack_start(btn_box, True, True, 0)

        buy_btn = Gtk.Button(label="RARLAB Official")
        buy_btn.connect("clicked", lambda b: webbrowser.open("https://www.rarlab.com"))
        btn_box.pack_start(buy_btn, True, True, 0)

        how_btn = Gtk.Button(label="Documentation")
        how_btn.connect("clicked", lambda b: webbrowser.open("https://www.rarlab.com/rar/rar.txt"))
        btn_box.pack_start(how_btn, True, True, 0)

        close_btn = Gtk.Button(label="Continue")
        close_btn.connect("clicked", lambda b: self.response(Gtk.ResponseType.CLOSE))
        btn_box.pack_start(close_btn, True, True, 0)

        self.show_all()

        self.show_all()


class AboutLinrarDialog(Gtk.Dialog):
    """
    Authentic WinRAR About dialog with logo banner and license viewer.
    """
    def __init__(self, parent=None):
        super().__init__(
            title="About LinRAR",
            transient_for=parent,
            modal=True,
            destroy_with_parent=True
        )
        self.set_default_size(520, 360)
        self.set_resizable(False)

        box = self.get_content_area()
        box.set_spacing(15)
        box.set_margin_top(15)
        box.set_margin_bottom(15)
        box.set_margin_start(15)
        box.set_margin_end(15)

        # WinRAR Banner Logo
        logo_path = os.path.join(ICONS_DIR, "winrar_logo.png")
        if not os.path.isfile(logo_path):
            logo_path = os.path.join(ICONS_DIR, "winrar_banner.png")
        if os.path.isfile(logo_path):
            pixbuf = GdkPixbuf.Pixbuf.new_from_file_at_scale(logo_path, 480, 80, True)
            img = Gtk.Image.new_from_pixbuf(pixbuf)
            box.pack_start(img, False, False, 0)

        content_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=15)
        box.pack_start(content_box, True, True, 0)

        # 3-book icon
        icon_path = os.path.join(ICONS_DIR, "winrar.png")
        if os.path.isfile(icon_path):
            pixbuf = GdkPixbuf.Pixbuf.new_from_file_at_scale(icon_path, 64, 64, True)
            img_icon = Gtk.Image.new_from_pixbuf(pixbuf)
            img_icon.set_alignment(0.5, 0.1)
            content_box.pack_start(img_icon, False, False, 0)

        vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=5)
        content_box.pack_start(vbox, True, True, 0)

        app_lbl = Gtk.Label()
        app_lbl.set_markup("<b><big>LinRAR 7.23 (x86_64)</big></b>")
        app_lbl.set_xalign(0)
        vbox.pack_start(app_lbl, False, False, 0)

        sub_lbl = Gtk.Label(label="WinRAR replica for Linux, based on RAR 7.23 engine")
        sub_lbl.set_xalign(0)
        vbox.pack_start(sub_lbl, False, False, 0)

        copy_lbl = Gtk.Label(label="Copyright © 1993-2026 Alexander Roshal")
        copy_lbl.set_xalign(0)
        vbox.pack_start(copy_lbl, False, False, 0)

        pub_lbl = Gtk.Label(label="Published by win.rar GmbH, Berlin/Germany")
        pub_lbl.set_xalign(0)
        vbox.pack_start(pub_lbl, False, False, 0)

        sep = Gtk.Separator(orientation=Gtk.Orientation.HORIZONTAL)
        vbox.pack_start(sep, False, False, 5)

        eval_lbl = Gtk.Label()
        eval_lbl.set_markup("<b>40 days trial copy</b>")
        eval_lbl.set_xalign(0)
        vbox.pack_start(eval_lbl, False, False, 0)

        user_lbl = Gtk.Label(label="Evaluation copy. Please register.")
        user_lbl.set_xalign(0)
        vbox.pack_start(user_lbl, False, False, 0)

        # Buttons
        self.add_button("License", 101)
        self.add_button("Home page", 102)
        self.add_button("OK", Gtk.ResponseType.OK)

        self.connect("response", self._on_response)
        self.show_all()

    def _on_response(self, dialog, response_id):
        if response_id == 101:
            # Show License viewer
            self._show_license()
        elif response_id == 102:
            webbrowser.open("https://www.rarlab.com")
        else:
            self.destroy()

    def _show_license(self):
        lic_win = Gtk.Dialog(
            title="WinRAR End User License Agreement",
            transient_for=self,
            modal=True
        )
        lic_win.set_default_size(600, 400)
        lic_box = lic_win.get_content_area()
        lic_box.set_spacing(10)
        lic_box.set_margin_top(10)
        lic_box.set_margin_bottom(10)
        lic_box.set_margin_start(10)
        lic_box.set_margin_end(10)

        sw = Gtk.ScrolledWindow()
        tv = Gtk.TextView()
        tv.set_editable(False)
        tv.set_wrap_mode(Gtk.WrapMode.WORD)
        
        lic_file = os.path.join(APP_DIR, "..", "extracted", "License.txt")
        lic_text = ""
        if os.path.isfile(lic_file):
            with open(lic_file, "r", errors="replace") as f:
                lic_text = f.read()
        else:
            lic_text = "WinRAR 40-Day Evaluation License.\nAlexander Roshal / win.rar GmbH"

        tv.get_buffer().set_text(lic_text)
        sw.add(tv)
        lic_box.pack_start(sw, True, True, 0)
        lic_win.add_button("Close", Gtk.ResponseType.CLOSE)
        lic_win.connect("response", lambda d, r: d.destroy())
        lic_win.show_all()


class AddArchiveDialog(Gtk.Dialog):
    """
    WinRAR 'GETARC' Add to Archive dialog.
    """
    def __init__(self, parent=None, default_name="archive.rar", source_files=None):
        super().__init__(
            title="Archive name and parameters",
            transient_for=parent,
            modal=True,
            destroy_with_parent=True
        )
        self.set_default_size(560, 480)
        self.source_files = source_files or []
        self.password = None
        self.encrypt_names = False

        box = self.get_content_area()
        box.set_spacing(10)
        box.set_margin_top(10)
        box.set_margin_bottom(10)
        box.set_margin_start(10)
        box.set_margin_end(10)

        notebook = Gtk.Notebook()
        box.pack_start(notebook, True, True, 0)

        # Tab 1: General
        general_page = self._build_general_tab(default_name)
        notebook.append_page(general_page, Gtk.Label(label="General"))

        # Tab 2: Advanced
        adv_page = self._build_advanced_tab()
        notebook.append_page(adv_page, Gtk.Label(label="Advanced"))

        # Tab 3: Options
        opt_page = self._build_options_tab()
        notebook.append_page(opt_page, Gtk.Label(label="Options"))

        # Tab 4: Comment
        cmt_page = self._build_comment_tab()
        notebook.append_page(cmt_page, Gtk.Label(label="Comment"))

        # Action buttons
        self.add_button("Help", Gtk.ResponseType.HELP)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        ok_btn = self.add_button("OK", Gtk.ResponseType.OK)
        ok_btn.get_style_context().add_class("suggested-action")

        self.show_all()

    def _build_general_tab(self, default_name):
        grid = Gtk.Grid()
        grid.set_column_spacing(12)
        grid.set_row_spacing(12)
        grid.set_margin_top(15)
        grid.set_margin_bottom(15)
        grid.set_margin_start(15)
        grid.set_margin_end(15)

        # Archive name
        lbl = Gtk.Label(label="Archive name:")
        lbl.set_xalign(0)
        grid.attach(lbl, 0, 0, 1, 1)

        name_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self.name_entry = Gtk.Entry()
        self.name_entry.set_text(default_name)
        name_box.pack_start(self.name_entry, True, True, 0)

        browse_btn = Gtk.Button(label="Browse...")
        browse_btn.connect("clicked", self._on_browse_archive_name)
        name_box.pack_start(browse_btn, False, False, 0)
        grid.attach(name_box, 0, 1, 3, 1)

        # Archive format frame
        fmt_frame = Gtk.Frame(label="Archive format")
        fmt_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        fmt_box.set_margin_top(8)
        fmt_box.set_margin_bottom(8)
        fmt_box.set_margin_start(8)
        fmt_box.set_margin_end(8)
        fmt_frame.add(fmt_box)

        self.rad_rar = Gtk.RadioButton.new_with_label(None, "RAR")
        self.rad_rar.connect("toggled", self._on_format_changed)
        fmt_box.pack_start(self.rad_rar, False, False, 0)

        self.rad_zip = Gtk.RadioButton.new_with_label_from_widget(self.rad_rar, "ZIP")
        self.rad_zip.connect("toggled", self._on_format_changed)
        fmt_box.pack_start(self.rad_zip, False, False, 0)

        self.rad_tar = Gtk.RadioButton.new_with_label_from_widget(self.rad_rar, "TAR.GZ")
        self.rad_tar.connect("toggled", self._on_format_changed)
        fmt_box.pack_start(self.rad_tar, False, False, 0)

        grid.attach(fmt_frame, 0, 2, 1, 2)

        # Compression method
        method_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        m_lbl = Gtk.Label(label="Compression method:")
        m_lbl.set_xalign(0)
        method_box.pack_start(m_lbl, False, False, 0)

        self.method_combo = Gtk.ComboBoxText()
        methods = ["Store", "Fastest", "Fast", "Normal", "Good", "Best"]
        for m in methods:
            self.method_combo.append_text(m)
        self.method_combo.set_active(3) # Normal
        method_box.pack_start(self.method_combo, False, False, 0)

        grid.attach(method_box, 1, 2, 2, 1)

        # Dictionary size
        dict_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        d_lbl = Gtk.Label(label="Dictionary size:")
        d_lbl.set_xalign(0)
        dict_box.pack_start(d_lbl, False, False, 0)

        self.dict_combo = Gtk.ComboBoxText()
        dicts = ["32 KB", "1 MB", "4 MB", "16 MB", "32 MB", "64 MB", "128 MB", "1024 MB"]
        for d in dicts:
            self.dict_combo.append_text(d)
        self.dict_combo.set_active(4) # 32 MB
        dict_box.pack_start(self.dict_combo, False, False, 0)

        grid.attach(dict_box, 1, 3, 2, 1)

        # Archiving options frame
        opt_frame = Gtk.Frame(label="Archiving options")
        opt_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=5)
        opt_box.set_margin_top(8)
        opt_box.set_margin_bottom(8)
        opt_box.set_margin_start(8)
        opt_box.set_margin_end(8)
        opt_frame.add(opt_box)

        self.chk_delete = Gtk.CheckButton(label="Delete files after archiving")
        opt_box.pack_start(self.chk_delete, False, False, 0)

        self.chk_sfx = Gtk.CheckButton(label="Create SFX archive")
        opt_box.pack_start(self.chk_sfx, False, False, 0)

        self.chk_solid = Gtk.CheckButton(label="Create solid archive")
        self.chk_solid.set_active(True)
        opt_box.pack_start(self.chk_solid, False, False, 0)

        self.chk_recovery = Gtk.CheckButton(label="Put recovery record (3%)")
        self.chk_recovery.set_active(True)
        opt_box.pack_start(self.chk_recovery, False, False, 0)

        self.chk_test = Gtk.CheckButton(label="Test archived files")
        opt_box.pack_start(self.chk_test, False, False, 0)

        self.chk_lock = Gtk.CheckButton(label="Lock archive")
        opt_box.pack_start(self.chk_lock, False, False, 0)

        grid.attach(opt_frame, 0, 4, 2, 1)

        # Password button & Volume split
        right_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        
        vol_lbl = Gtk.Label(label="Split to volumes, size:")
        vol_lbl.set_xalign(0)
        right_box.pack_start(vol_lbl, False, False, 0)

        self.vol_combo = Gtk.ComboBoxText.new_with_entry()
        for v in ["", "5M", "100M", "700M - CD", "4480M - DVD", "25000M - BD"]:
            self.vol_combo.append_text(v)
        self.vol_combo.set_active(0)
        right_box.pack_start(self.vol_combo, False, False, 0)

        pwd_btn = Gtk.Button(label="Set password...")
        pwd_btn.connect("clicked", self._on_set_password)
        right_box.pack_start(pwd_btn, False, False, 10)

        self.pwd_status_lbl = Gtk.Label(label="Password: Not set")
        self.pwd_status_lbl.set_xalign(0)
        right_box.pack_start(self.pwd_status_lbl, False, False, 0)

        grid.attach(right_box, 2, 4, 1, 1)

        return grid

    def _build_advanced_tab(self):
        vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        vbox.set_margin_top(15)
        vbox.set_margin_bottom(15)
        vbox.set_margin_start(15)
        vbox.set_margin_end(15)

        lbl = Gtk.Label(label="Advanced compression and volume parameters:")
        lbl.set_xalign(0)
        vbox.pack_start(lbl, False, False, 0)

        frame = Gtk.Frame(label="Recovery volumes")
        f_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        f_box.set_margin_top(8)
        f_box.set_margin_bottom(8)
        f_box.set_margin_start(8)
        f_box.set_margin_end(8)
        frame.add(f_box)

        self.chk_rec_vol = Gtk.CheckButton(label="Create recovery volumes (.rev)")
        f_box.pack_start(self.chk_rec_vol, False, False, 0)
        vbox.pack_start(frame, False, False, 0)

        return vbox

    def _build_options_tab(self):
        vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        vbox.set_margin_top(15)
        vbox.set_margin_bottom(15)
        vbox.set_margin_start(15)
        vbox.set_margin_end(15)

        lbl = Gtk.Label(label="File paths and attributes:")
        lbl.set_xalign(0)
        vbox.pack_start(lbl, False, False, 0)

        self.chk_rel_paths = Gtk.CheckButton(label="Save relative paths (default)")
        self.chk_rel_paths.set_active(True)
        vbox.pack_start(self.chk_rel_paths, False, False, 0)

        self.chk_symlinks = Gtk.CheckButton(label="Save symbolic links as links")
        self.chk_symlinks.set_active(True)
        vbox.pack_start(self.chk_symlinks, False, False, 0)

        return vbox

    def _build_comment_tab(self):
        vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        vbox.set_margin_top(15)
        vbox.set_margin_bottom(15)
        vbox.set_margin_start(15)
        vbox.set_margin_end(15)

        lbl = Gtk.Label(label="Archive comment (optional):")
        lbl.set_xalign(0)
        vbox.pack_start(lbl, False, False, 0)

        sw = Gtk.ScrolledWindow()
        self.cmt_view = Gtk.TextView()
        self.cmt_view.set_wrap_mode(Gtk.WrapMode.WORD)
        sw.add(self.cmt_view)
        vbox.pack_start(sw, True, True, 0)

        return vbox

    def _on_format_changed(self, widget):
        if not widget.get_active():
            return
        cur_name = self.name_entry.get_text()
        base, _ = os.path.splitext(cur_name)
        if self.rad_rar.get_active():
            self.name_entry.set_text(base + ".rar")
            self.chk_solid.set_sensitive(True)
            self.chk_recovery.set_sensitive(True)
            self.chk_lock.set_sensitive(True)
            self.chk_sfx.set_sensitive(True)
            self.dict_combo.set_sensitive(True)
        elif self.rad_zip.get_active():
            self.name_entry.set_text(base + ".zip")
            self.chk_solid.set_sensitive(False)
            self.chk_recovery.set_sensitive(False)
            self.chk_lock.set_sensitive(False)
            self.chk_sfx.set_sensitive(False)
            self.dict_combo.set_sensitive(False)
        elif self.rad_tar.get_active():
            self.name_entry.set_text(base + ".tar.gz")
            self.chk_solid.set_sensitive(False)
            self.chk_recovery.set_sensitive(False)
            self.chk_lock.set_sensitive(False)
            self.chk_sfx.set_sensitive(False)
            self.dict_combo.set_sensitive(False)

    def _on_browse_archive_name(self, btn):
        dialog = Gtk.FileChooserDialog(
            title="Save Archive As",
            parent=self,
            action=Gtk.FileChooserAction.SAVE
        )
        dialog.add_buttons(
            Gtk.STOCK_CANCEL, Gtk.ResponseType.CANCEL,
            Gtk.STOCK_SAVE, Gtk.ResponseType.OK
        )
        dialog.set_do_overwrite_confirmation(True)
        dialog.set_current_name(self.name_entry.get_text())

        if dialog.run() == Gtk.ResponseType.OK:
            self.name_entry.set_text(dialog.get_filename())
        dialog.destroy()

    def _on_set_password(self, btn):
        pwd_dlg = PasswordDialog(parent=self, allow_encrypt_names=self.rad_rar.get_active())
        if pwd_dlg.run() == Gtk.ResponseType.OK:
            pwd, enc_names = pwd_dlg.get_credentials()
            if pwd:
                self.password = pwd
                self.encrypt_names = enc_names
                self.pwd_status_lbl.set_markup("Password: <b><span color='green'>Set</span></b>")
            else:
                self.password = None
                self.pwd_status_lbl.set_text("Password: Not set")
        pwd_dlg.destroy()

    def get_settings(self):
        fmt = "rar"
        if self.rad_zip.get_active():
            fmt = "zip"
        elif self.rad_tar.get_active():
            fmt = "tar.gz"

        dict_map = {
            "32 KB": "32k",
            "1 MB": "1m",
            "4 MB": "4m",
            "16 MB": "16m",
            "32 MB": "32m",
            "64 MB": "64m",
            "128 MB": "128m",
            "1024 MB": "1024m"
        }
        dict_txt = self.dict_combo.get_active_text() or "32 MB"
        dict_val = dict_map.get(dict_txt, "32m")

        buf = self.cmt_view.get_buffer()
        comment_text = buf.get_text(buf.get_start_iter(), buf.get_end_iter(), True).strip()

        vol_val = self.vol_combo.get_child().get_text().strip()
        if " - " in vol_val:
            vol_val = vol_val.split(" - ")[0].strip()

        return {
            "archive_path": os.path.abspath(self.name_entry.get_text()),
            "format": fmt,
            "compression_level": self.method_combo.get_active(), # 0..5
            "dict_size": dict_val,
            "solid": self.chk_solid.get_active(),
            "sfx": self.chk_sfx.get_active(),
            "recovery_record": self.chk_recovery.get_active(),
            "lock": self.chk_lock.get_active(),
            "delete_sources": self.chk_delete.get_active(),
            "volume_size": vol_val if vol_val else None,
            "password": self.password,
            "encrypt_names": self.encrypt_names,
            "comment": comment_text if comment_text else None
        }


class ExtractArchiveDialog(Gtk.Dialog):
    """
    WinRAR 'GETEXTRPATH' Extraction Path and Options dialog.
    """
    def __init__(self, parent=None, default_dest=""):
        super().__init__(
            title="Extraction path and options",
            transient_for=parent,
            modal=True,
            destroy_with_parent=True
        )
        self.set_default_size(540, 420)

        box = self.get_content_area()
        box.set_spacing(10)
        box.set_margin_top(10)
        box.set_margin_bottom(10)
        box.set_margin_start(10)
        box.set_margin_end(10)

        # Destination path
        lbl = Gtk.Label(label="Destination path (will be created if does not exist):")
        lbl.set_xalign(0)
        box.pack_start(lbl, False, False, 0)

        path_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self.path_entry = Gtk.Entry()
        self.path_entry.set_text(default_dest)
        path_box.pack_start(self.path_entry, True, True, 0)

        browse_btn = Gtk.Button(label="Browse...")
        browse_btn.connect("clicked", self._on_browse_dest)
        path_box.pack_start(browse_btn, False, False, 0)
        box.pack_start(path_box, False, False, 0)

        # Overwrite mode frame
        ow_frame = Gtk.Frame(label="Overwrite mode")
        ow_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=5)
        ow_box.set_margin_top(8)
        ow_box.set_margin_bottom(8)
        ow_box.set_margin_start(8)
        ow_box.set_margin_end(8)
        ow_frame.add(ow_box)

        self.rad_ask = Gtk.RadioButton.new_with_label(None, "Ask before overwrite")
        ow_box.pack_start(self.rad_ask, False, False, 0)

        self.rad_always = Gtk.RadioButton.new_with_label_from_widget(self.rad_ask, "Overwrite without prompt")
        self.rad_always.set_active(True)
        ow_box.pack_start(self.rad_always, False, False, 0)

        self.rad_skip = Gtk.RadioButton.new_with_label_from_widget(self.rad_ask, "Skip existing files")
        ow_box.pack_start(self.rad_skip, False, False, 0)

        box.pack_start(ow_frame, False, False, 0)

        # Misc options
        misc_frame = Gtk.Frame(label="Miscellaneous")
        m_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=5)
        m_box.set_margin_top(8)
        m_box.set_margin_bottom(8)
        m_box.set_margin_start(8)
        m_box.set_margin_end(8)
        misc_frame.add(m_box)

        self.chk_open_folder = Gtk.CheckButton(label="Display files in file manager after extraction")
        self.chk_open_folder.set_active(True)
        m_box.pack_start(self.chk_open_folder, False, False, 0)

        box.pack_start(misc_frame, False, False, 0)

        self.add_button("Help", Gtk.ResponseType.HELP)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        ok_btn = self.add_button("OK", Gtk.ResponseType.OK)
        ok_btn.get_style_context().add_class("suggested-action")

        self.show_all()

    def _on_browse_dest(self, btn):
        dialog = Gtk.FileChooserDialog(
            title="Select Destination Folder",
            parent=self,
            action=Gtk.FileChooserAction.SELECT_FOLDER
        )
        dialog.add_buttons(
            Gtk.STOCK_CANCEL, Gtk.ResponseType.CANCEL,
            Gtk.STOCK_OPEN, Gtk.ResponseType.OK
        )
        if self.path_entry.get_text():
            dialog.set_filename(self.path_entry.get_text())
        if dialog.run() == Gtk.ResponseType.OK:
            self.path_entry.set_text(dialog.get_filename())
        dialog.destroy()

    def get_settings(self):
        return {
            "dest_dir": os.path.abspath(self.path_entry.get_text()),
            "overwrite": self.rad_always.get_active(),
            "open_dest": self.chk_open_folder.get_active()
        }


class ArchiveInfoDialog(Gtk.Dialog):
    """
    WinRAR 'INFO' Archive Information dialog.
    """
    def __init__(self, parent=None, archive_path="", info_dict=None, comment=""):
        super().__init__(
            title="Archive Information",
            transient_for=parent,
            modal=True,
            destroy_with_parent=True
        )
        self.set_default_size(500, 380)

        box = self.get_content_area()
        box.set_spacing(10)
        box.set_margin_top(10)
        box.set_margin_bottom(10)
        box.set_margin_start(10)
        box.set_margin_end(10)

        notebook = Gtk.Notebook()
        box.pack_start(notebook, True, True, 0)

        # Tab 1: General Info
        grid = Gtk.Grid()
        grid.set_column_spacing(15)
        grid.set_row_spacing(8)
        grid.set_margin_top(15)
        grid.set_margin_bottom(15)
        grid.set_margin_start(15)
        grid.set_margin_end(15)

        info_dict = info_dict or {}
        rows = [
            ("Archive name:", os.path.basename(archive_path)),
            ("Full path:", archive_path),
            ("Format:", info_dict.get("Type", "Unknown")),
            ("Details:", info_dict.get("Details", "Standard archive")),
            ("Total files:", info_dict.get("Total Files", "0")),
            ("Uncompressed size:", info_dict.get("Uncompressed Size", "0")),
            ("Packed size:", info_dict.get("Packed Size", "0")),
            ("Compression ratio:", info_dict.get("Ratio", "0%")),
            ("Has comment:", "Yes" if comment else "No"),
        ]

        for i, (k, v) in enumerate(rows):
            lbl_k = Gtk.Label(label=k)
            lbl_k.set_xalign(0)
            lbl_k.get_style_context().add_class("dim-label")
            grid.attach(lbl_k, 0, i, 1, 1)

            lbl_v = Gtk.Label(label=str(v))
            lbl_v.set_xalign(0)
            lbl_v.set_selectable(True)
            grid.attach(lbl_v, 1, i, 1, 1)

        notebook.append_page(grid, Gtk.Label(label="General"))

        # Tab 2: Comment
        if comment:
            cmt_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=5)
            cmt_box.set_margin_top(10)
            cmt_box.set_margin_bottom(10)
            cmt_box.set_margin_start(10)
            cmt_box.set_margin_end(10)

            sw = Gtk.ScrolledWindow()
            tv = Gtk.TextView()
            tv.set_editable(False)
            tv.set_wrap_mode(Gtk.WrapMode.WORD)
            tv.get_buffer().set_text(comment)
            sw.add(tv)
            cmt_box.pack_start(sw, True, True, 0)
            notebook.append_page(cmt_box, Gtk.Label(label="Comment"))

        self.add_button("Close", Gtk.ResponseType.CLOSE)
        self.connect("response", lambda d, r: d.destroy())
        self.show_all()


class PasswordDialog(Gtk.Dialog):
    """
    WinRAR 'GETPASSWORD2' Password Dialog.
    """
    def __init__(self, parent=None, allow_encrypt_names=True):
        super().__init__(
            title="Enter password",
            transient_for=parent,
            modal=True,
            destroy_with_parent=True
        )
        self.set_default_size(360, 200)
        self.set_resizable(False)

        box = self.get_content_area()
        box.set_spacing(10)
        box.set_margin_top(15)
        box.set_margin_bottom(15)
        box.set_margin_start(15)
        box.set_margin_end(15)

        lbl = Gtk.Label(label="Enter password:")
        lbl.set_xalign(0)
        box.pack_start(lbl, False, False, 0)

        self.pwd_entry = Gtk.Entry()
        self.pwd_entry.set_visibility(False)
        box.pack_start(self.pwd_entry, False, False, 0)

        self.chk_show = Gtk.CheckButton(label="Show password")
        self.chk_show.connect("toggled", lambda b: self.pwd_entry.set_visibility(b.get_active()))
        box.pack_start(self.chk_show, False, False, 0)

        self.chk_names = Gtk.CheckButton(label="Encrypt file names")
        self.chk_names.set_sensitive(allow_encrypt_names)
        box.pack_start(self.chk_names, False, False, 0)

        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        ok_btn = self.add_button("OK", Gtk.ResponseType.OK)
        ok_btn.get_style_context().add_class("suggested-action")

        self.show_all()

    def get_credentials(self):
        return self.pwd_entry.get_text(), self.chk_names.get_active()


class BenchmarkDialog(Gtk.Dialog):
    """
    WinRAR Benchmark (Alt+B) CPU test dialog.
    """
    def __init__(self, parent=None):
        super().__init__(
            title="Benchmark",
            transient_for=parent,
            modal=True,
            destroy_with_parent=True
        )
        self.set_default_size(420, 260)
        self.set_resizable(False)
        self.running = True

        box = self.get_content_area()
        box.set_spacing(12)
        box.set_margin_top(15)
        box.set_margin_bottom(15)
        box.set_margin_start(15)
        box.set_margin_end(15)

        title = Gtk.Label()
        title.set_markup("<b><big>Compression Speed Benchmark</big></b>")
        box.pack_start(title, False, False, 0)

        grid = Gtk.Grid()
        grid.set_column_spacing(15)
        grid.set_row_spacing(10)
        box.pack_start(grid, True, True, 0)

        grid.attach(Gtk.Label(label="Current speed:", xalign=0), 0, 0, 1, 1)
        self.speed_lbl = Gtk.Label(label="Calculating...", xalign=0)
        grid.attach(self.speed_lbl, 1, 0, 1, 1)

        grid.attach(Gtk.Label(label="Processed data:", xalign=0), 0, 1, 1, 1)
        self.proc_lbl = Gtk.Label(label="0 MB", xalign=0)
        grid.attach(self.proc_lbl, 1, 1, 1, 1)

        grid.attach(Gtk.Label(label="Elapsed time:", xalign=0), 0, 2, 1, 1)
        self.time_lbl = Gtk.Label(label="00:00:00", xalign=0)
        grid.attach(self.time_lbl, 1, 2, 1, 1)

        self.chk_multi = Gtk.CheckButton(label="Multithreading")
        self.chk_multi.set_active(True)
        box.pack_start(self.chk_multi, False, False, 0)

        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.connect("response", self._on_close)

        self.show_all()

        # Start benchmark worker thread
        self.start_time = time.time()
        self.bytes_processed = 0
        self.thread = threading.Thread(target=self._run_bench, daemon=True)
        self.thread.start()

        GLib.timeout_add(500, self._update_ui)

    def _run_bench(self):
        chunk = os.urandom(1024 * 1024) # 1 MB random payload
        while self.running:
            zlib.compress(chunk, level=6)
            self.bytes_processed += len(chunk)

    def _update_ui(self):
        if not self.running:
            return False
        elapsed = max(0.001, time.time() - self.start_time)
        speed_kb = (self.bytes_processed / 1024) / elapsed
        mb_proc = self.bytes_processed / (1024 * 1024)

        hours = int(elapsed // 3600)
        mins = int((elapsed % 3600) // 60)
        secs = int(elapsed % 60)

        self.speed_lbl.set_markup(f"<b>{speed_kb:,.0f} KB/s</b>")
        self.proc_lbl.set_text(f"{mb_proc:,.1f} MB")
        self.time_lbl.set_text(f"{hours:02d}:{mins:02d}:{secs:02d}")
        return True

    def _on_close(self, dialog, response_id):
        self.running = False
        self.destroy()
