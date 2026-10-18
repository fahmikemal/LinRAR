"""
Comprehensive End-to-End Test Suite for LinRAR
Validates all backend operations, formats, edge cases, CLI commands, and GUI dialogs.
"""

import os
import sys
import shutil
import tempfile
import subprocess
import time
import unittest

# Ensure linrar is on sys.path
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if BASE_DIR not in sys.path:
    sys.path.insert(0, BASE_DIR)

from linrar_app.backend import ArchiveBackend, ArchiveEntry


class TestLinrarBackend(unittest.TestCase):
    def setUp(self):
        self.test_dir = tempfile.mkdtemp(prefix="linrar_test_")
        self.src_dir = os.path.join(self.test_dir, "sources")
        os.makedirs(self.src_dir, exist_ok=True)

        # Create sample files
        self.file1 = os.path.join(self.src_dir, "file1.txt")
        with open(self.file1, "w") as f:
            f.write("LinRAR Test File 1 Content - " * 50)

        self.sub_dir = os.path.join(self.src_dir, "nested_folder")
        os.makedirs(self.sub_dir, exist_ok=True)
        self.file2 = os.path.join(self.sub_dir, "subfile.txt")
        with open(self.file2, "w") as f:
            f.write("Nested file content inside subfolder - " * 30)

    def tearDown(self):
        if os.path.isdir(self.test_dir):
            shutil.rmtree(self.test_dir, ignore_errors=True)

    # 1. Format Detection
    def test_archive_detection(self):
        self.assertTrue(ArchiveBackend.is_archive("test.rar"))
        self.assertTrue(ArchiveBackend.is_archive("test.zip"))
        self.assertTrue(ArchiveBackend.is_archive("test.tar.gz"))
        self.assertTrue(ArchiveBackend.is_archive("test.7z"))
        self.assertFalse(ArchiveBackend.is_archive("test.txt"))
        self.assertFalse(ArchiveBackend.is_archive("nonexistent.pdf"))

    # 2. RAR Archive Creation & Listing
    def test_rar_creation_and_listing(self):
        arc_path = os.path.join(self.test_dir, "output.rar")
        ok, msg = ArchiveBackend.create_archive(
            archive_path=arc_path,
            source_paths=[self.file1, self.sub_dir],
            format_type="rar",
            compression_level=3,
            comment="Test RAR Comment for LinRAR"
        )
        self.assertTrue(ok, f"RAR creation failed: {msg}")
        self.assertTrue(os.path.isfile(arc_path))

        # List archive
        entries, comment, info = ArchiveBackend.list_archive(arc_path)
        self.assertGreaterEqual(len(entries), 2)
        self.assertEqual(comment, "Test RAR Comment for LinRAR")
        self.assertEqual(info.get("Type"), "RAR Archive")

        # Check entry contents
        names = [e.name for e in entries]
        self.assertIn("file1.txt", names)
        self.assertIn("subfile.txt", names)

    # 3. RAR Compression Levels & Solid
    def test_rar_compression_levels(self):
        for lvl in [0, 1, 3, 5]: # Store, Fastest, Normal, Best
            arc_p = os.path.join(self.test_dir, f"comp_m{lvl}.rar")
            ok, msg = ArchiveBackend.create_archive(
                archive_path=arc_p,
                source_paths=[self.file1],
                format_type="rar",
                compression_level=lvl,
                solid=True
            )
            self.assertTrue(ok, f"Failed at compression level {lvl}: {msg}")
            self.assertTrue(os.path.isfile(arc_p))

    # 4. RAR Password Encryption & Extraction
    def test_rar_password_protection(self):
        arc_pwd = os.path.join(self.test_dir, "secret.rar")
        password = "LinRAR_Secret_Pass_123!"
        ok, msg = ArchiveBackend.create_archive(
            archive_path=arc_pwd,
            source_paths=[self.file1],
            format_type="rar",
            password=password,
            encrypt_names=True
        )
        self.assertTrue(ok, f"Encrypted archive creation failed: {msg}")

        # Test extraction without password -> should fail
        extr_bad = os.path.join(self.test_dir, "extr_bad")
        ok_bad, _ = ArchiveBackend.extract_archive(arc_pwd, extr_bad, password=None)
        self.assertFalse(ok_bad, "Extraction without password should fail!")

        # Test extraction with correct password -> should succeed
        extr_good = os.path.join(self.test_dir, "extr_good")
        ok_good, msg_good = ArchiveBackend.extract_archive(arc_pwd, extr_good, password=password)
        self.assertTrue(ok_good, f"Extraction with password failed: {msg_good}")
        self.assertTrue(os.path.isfile(os.path.join(extr_good, os.path.basename(self.file1))))

    # 5. RAR Volume Splitting
    def test_rar_volume_splitting(self):
        arc_vol = os.path.join(self.test_dir, "volumes.rar")
        # Split into tiny 20k volumes
        ok, msg = ArchiveBackend.create_archive(
            archive_path=arc_vol,
            source_paths=[self.file1, self.file2],
            format_type="rar",
            volume_size="20k"
        )
        self.assertTrue(ok, f"Volume creation failed: {msg}")
        vols = [f for f in os.listdir(self.test_dir) if "part" in f or "volumes" in f]
        self.assertGreater(len(vols), 0)

    # 6. RAR Testing & Integrity Verification
    def test_rar_integrity_check(self):
        arc_path = os.path.join(self.test_dir, "check.rar")
        ArchiveBackend.create_archive(arc_path, [self.file1], format_type="rar")
        ok, msg = ArchiveBackend.test_archive(arc_path)
        self.assertTrue(ok, f"Valid archive failed test: {msg}")

        # Corrupt the archive bytes
        with open(arc_path, "r+b") as f:
            f.seek(150)
            f.write(b"\x00\x00\x00\x00\x00\x00\x00\x00\xFF\xFF")

        ok_c, _ = ArchiveBackend.test_archive(arc_path)
        self.assertFalse(ok_c, "Corrupted archive must fail test!")

    # 7. RAR Deletion & Locking
    def test_rar_deletion_and_locking(self):
        arc_path = os.path.join(self.test_dir, "modify.rar")
        ArchiveBackend.create_archive(arc_path, [self.file1, self.file2], format_type="rar")

        # Delete file1 from archive
        ok, msg = ArchiveBackend.delete_from_archive(arc_path, [os.path.basename(self.file1)])
        self.assertTrue(ok, f"Deletion failed: {msg}")

        entries, _, _ = ArchiveBackend.list_archive(arc_path)
        names = [e.name for e in entries]
        self.assertNotIn(os.path.basename(self.file1), names)

        # Lock archive
        ok_lock, msg_lock = ArchiveBackend.lock_archive(arc_path)
        self.assertTrue(ok_lock, f"Locking failed: {msg_lock}")

        # Attempt to delete from locked archive -> must fail
        ok_del, _ = ArchiveBackend.delete_from_archive(arc_path, [os.path.basename(self.file2)])
        self.assertFalse(ok_del, "Deleting from locked archive must fail!")

    # 8. ZIP Archive Creation, Listing, Extraction, and Comment
    def test_zip_operations(self):
        zip_p = os.path.join(self.test_dir, "test.zip")
        ok, msg = ArchiveBackend.create_archive(
            archive_path=zip_p,
            source_paths=[self.file1, self.sub_dir],
            format_type="zip",
            compression_level=3,
            comment="ZIP Comment Test"
        )
        self.assertTrue(ok, f"ZIP creation failed: {msg}")

        entries, comment, info = ArchiveBackend.list_archive(zip_p)
        self.assertGreaterEqual(len(entries), 2)
        self.assertEqual(comment, "ZIP Comment Test")
        self.assertEqual(info.get("Type"), "ZIP Archive")

        # Test ZIP integrity
        ok_t, msg_t = ArchiveBackend.test_archive(zip_p)
        self.assertTrue(ok_t, f"ZIP integrity test failed: {msg_t}")

        # Extract ZIP
        extr_zip = os.path.join(self.test_dir, "extr_zip")
        ok_x, msg_x = ArchiveBackend.extract_archive(zip_p, extr_zip)
        self.assertTrue(ok_x, f"ZIP extraction failed: {msg_x}")
        self.assertTrue(os.path.isfile(os.path.join(extr_zip, "file1.txt")))

    # 9. TAR.GZ Archive Operations
    def test_tar_operations(self):
        tar_p = os.path.join(self.test_dir, "test.tar.gz")
        ok, msg = ArchiveBackend.create_archive(
            archive_path=tar_p,
            source_paths=[self.file1, self.sub_dir],
            format_type="tar.gz"
        )
        self.assertTrue(ok, f"TAR.GZ creation failed: {msg}")

        entries, _, info = ArchiveBackend.list_archive(tar_p)
        self.assertGreaterEqual(len(entries), 2)
        self.assertEqual(info.get("Type"), "TAR Archive")

        extr_tar = os.path.join(self.test_dir, "extr_tar")
        ok_x, msg_x = ArchiveBackend.extract_archive(tar_p, extr_tar)
        self.assertTrue(ok_x, f"TAR.GZ extraction failed: {msg_x}")
        self.assertTrue(os.path.isfile(os.path.join(extr_tar, "file1.txt")))


class TestLinrarCLI(unittest.TestCase):
    def setUp(self):
        self.test_dir = tempfile.mkdtemp(prefix="linrar_cli_test_")
        self.sample_file = os.path.join(self.test_dir, "cli_sample.txt")
        with open(self.sample_file, "w") as f:
            f.write("Sample CLI text for LinRAR testing")

        self.linrar_bin = os.path.join(BASE_DIR, "linrar")

    def tearDown(self):
        if os.path.isdir(self.test_dir):
            shutil.rmtree(self.test_dir, ignore_errors=True)

    def test_cli_help_and_version(self):
        # --help
        res = subprocess.run([self.linrar_bin, "--help"], capture_output=True, text=True)
        self.assertEqual(res.returncode, 0)
        self.assertIn("LinRAR 7.23", res.stdout)
        self.assertIn("Command-line", res.stdout)

        # --version
        res_v = subprocess.run([self.linrar_bin, "--version"], capture_output=True, text=True)
        self.assertEqual(res_v.returncode, 0)
        self.assertIn("LinRAR version 7.23", res_v.stdout)

    def test_cli_add_list_test_extract(self):
        arc = os.path.join(self.test_dir, "cli_arc.rar")

        # linrar a
        res_a = subprocess.run([self.linrar_bin, "a", arc, self.sample_file], capture_output=True, text=True)
        self.assertEqual(res_a.returncode, 0, f"CLI Add failed: {res_a.stderr}")
        self.assertTrue(os.path.isfile(arc))

        # linrar l
        res_l = subprocess.run([self.linrar_bin, "l", arc], capture_output=True, text=True)
        self.assertEqual(res_l.returncode, 0)
        self.assertIn("cli_sample.txt", res_l.stdout)

        # linrar t
        res_t = subprocess.run([self.linrar_bin, "t", arc], capture_output=True, text=True)
        self.assertEqual(res_t.returncode, 0)
        self.assertIn("All files were tested and no errors were found", res_t.stdout)

        # linrar x
        dest = os.path.join(self.test_dir, "cli_extr")
        res_x = subprocess.run([self.linrar_bin, "x", arc, dest], capture_output=True, text=True)
        self.assertEqual(res_x.returncode, 0)
        self.assertTrue(os.path.isfile(os.path.join(dest, "cli_sample.txt")))


class TestLinrarGUI(unittest.TestCase):
    """
    Validates GUI widgets, dialog lifecycles, and MainWindow initialization.
    """
    @classmethod
    def setUpClass(cls):
        import gi
        gi.require_version("Gtk", "3.0")
        from gi.repository import Gtk, GLib
        cls.Gtk = Gtk
        cls.GLib = GLib

    def test_all_dialogs(self):
        from linrar_app.dialogs import (
            AddArchiveDialog,
            ExtractArchiveDialog,
            ArchiveInfoDialog,
            TrialReminderDialog,
            AboutLinrarDialog,
            BenchmarkDialog,
            PasswordDialog
        )

        # AddArchiveDialog
        d_add = AddArchiveDialog()
        settings = d_add.get_settings()
        self.assertIn("format", settings)
        self.assertIn("compression_level", settings)
        d_add.destroy()

        # ExtractArchiveDialog
        d_ext = ExtractArchiveDialog()
        ext_settings = d_ext.get_settings()
        self.assertIn("dest_dir", ext_settings)
        d_ext.destroy()

        # ArchiveInfoDialog
        d_info = ArchiveInfoDialog(archive_path="test.rar", info_dict={"Type": "RAR", "Details": "RAR 5"}, comment="Test")
        d_info.destroy()

        # TrialReminderDialog
        d_remind = TrialReminderDialog()
        d_remind.destroy()

        # AboutLinrarDialog
        d_about = AboutLinrarDialog()
        d_about.destroy()

        # PasswordDialog
        d_pwd = PasswordDialog()
        pwd, enc = d_pwd.get_credentials()
        d_pwd.destroy()

        # BenchmarkDialog
        d_bench = BenchmarkDialog()
        d_bench.running = False
        d_bench.destroy()

    def test_main_window_modes(self):
        from linrar_app.main_window import MainWindow

        # Test local folder mode
        win_folder = MainWindow(target_path=os.getcwd())
        self.assertFalse(win_folder.is_archive_mode)
        self.assertGreater(len(win_folder.liststore), 0)
        win_folder.destroy()

        # Test archive mode with payload.rar
        payload_rar = os.path.join(BASE_DIR, "payload.rar")
        if os.path.isfile(payload_rar):
            win_arc = MainWindow(target_path=payload_rar)
            self.assertTrue(win_arc.is_archive_mode)
            self.assertEqual(len(win_arc.liststore), 29) # 28 files + '..' entry
            self.assertTrue(win_arc.comment_box.get_visible()) # Comment present in payload.rar
            win_arc.destroy()


if __name__ == "__main__":
    unittest.main(verbosity=2)
