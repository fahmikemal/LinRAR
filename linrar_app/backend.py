"""
LinRAR - Archive Backend Engine
Handles archive listing, creation, extraction, testing, and management.
Integrates with the official RAR 7.23 Linux engine (rar/unrar) and Python standard libraries (zipfile, tarfile).
"""

import os
import sys
import subprocess
import shutil
import zipfile
import tarfile
import tempfile
import time
from dataclasses import dataclass
from typing import List, Optional, Tuple, Dict

import platform

# Resolve paths to bundled binaries
APP_DIR = os.path.dirname(os.path.abspath(__file__))
BIN_DIR = os.path.join(APP_DIR, "bin")
RAR_BIN = os.path.join(BIN_DIR, "rar")
UNRAR_BIN = os.path.join(BIN_DIR, "unrar")

# The bundled precompiled rar/unrar binaries are x86_64 ELF.
# On other architectures (ARM64, etc.), fallback to system binaries if available.
IS_X86_64 = platform.machine().lower() in ("x86_64", "amd64")
if not IS_X86_64 or not os.path.isfile(RAR_BIN):
    RAR_BIN = shutil.which("rar") or (RAR_BIN if os.path.isfile(RAR_BIN) else "rar")
if not IS_X86_64 or not os.path.isfile(UNRAR_BIN):
    UNRAR_BIN = shutil.which("unrar") or (UNRAR_BIN if os.path.isfile(UNRAR_BIN) else "unrar")


@dataclass
class ArchiveEntry:
    name: str                 # Basename or relative segment
    path: str                 # Full path inside archive
    size: int                 # Uncompressed size in bytes
    packed_size: int          # Compressed size in bytes
    ratio: str                # Compression ratio (e.g., '42%')
    is_dir: bool              # Is directory
    mtime: str                # Formatted modification time
    crc32: str                # CRC32 checksum (hex)
    attr: str                 # Attributes (e.g., 'drwxr-xr-x' or '.D.....')
    encrypted: bool = False   # Is password encrypted


class ArchiveBackend:
    SUPPORTED_EXTENSIONS = {
        ".rar": "RAR",
        ".rev": "RAR Recovery",
        ".sfx": "RAR SFX",
        ".zip": "ZIP",
        ".zipx": "ZIP",
        ".tar": "TAR",
        ".gz": "GZIP",
        ".tgz": "TAR.GZ",
        ".tar.gz": "TAR.GZ",
        ".bz2": "BZIP2",
        ".tbz2": "TAR.BZ2",
        ".tar.bz2": "TAR.BZ2",
        ".xz": "XZ",
        ".txz": "TAR.XZ",
        ".tar.xz": "TAR.XZ",
        ".7z": "7-Zip",
        ".iso": "ISO",
        ".cab": "CAB",
        ".arj": "ARJ",
        ".lzh": "LZH",
        ".z": "Unix Compress"
    }

    @staticmethod
    def is_archive(filepath: str) -> bool:
        lower = filepath.lower()
        has_ext = any(lower.endswith(ext) for ext in ArchiveBackend.SUPPORTED_EXTENSIONS)
        if not os.path.isfile(filepath):
            return has_ext
        if has_ext:
            return True
        # Check magic numbers
        try:
            with open(filepath, "rb") as f:
                magic = f.read(8)
                if magic.startswith(b"Rar!\x1a\x07"):
                    return True
                if magic.startswith(b"PK\x03\x04"):
                    return True
                if magic.startswith(b"\x1f\x8b"):
                    return True
                if magic.startswith(b"BZh"):
                    return True
                if magic.startswith(b"\xfd7zXZ\x00"):
                    return True
                if magic.startswith(b"7z\xbc\xaf\x27\x1c"):
                    return True
        except Exception:
            pass
        return False

    @staticmethod
    def get_archive_type(filepath: str) -> str:
        lower = filepath.lower()
        if lower.endswith(".tar.gz") or lower.endswith(".tgz"):
            return "TAR.GZ"
        if lower.endswith(".tar.bz2") or lower.endswith(".tbz2"):
            return "TAR.BZ2"
        if lower.endswith(".tar.xz") or lower.endswith(".txz"):
            return "TAR.XZ"
        ext = os.path.splitext(lower)[1]
        return ArchiveBackend.SUPPORTED_EXTENSIONS.get(ext, "Unknown Archive")

    @staticmethod
    def list_archive(filepath: str, password: Optional[str] = None) -> Tuple[List[ArchiveEntry], str, Dict[str, str]]:
        """
        Returns (entries, comment, info_dict)
        """
        lower = filepath.lower()
        if lower.endswith(".rar") or lower.endswith(".rev") or lower.endswith(".sfx") or ArchiveBackend._is_rar_file(filepath):
            return ArchiveBackend._list_rar(filepath, password)
        elif lower.endswith(".zip") or lower.endswith(".zipx") or zipfile.is_zipfile(filepath):
            return ArchiveBackend._list_zip(filepath, password)
        elif tarfile.is_tarfile(filepath):
            return ArchiveBackend._list_tar(filepath)
        else:
            return ArchiveBackend._list_rar(filepath, password)

    @staticmethod
    def _is_rar_file(filepath: str) -> bool:
        try:
            with open(filepath, "rb") as f:
                magic = f.read(7)
                return magic.startswith(b"Rar!\x1a\x07")
        except Exception:
            return False

    @staticmethod
    def _list_rar(filepath: str, password: Optional[str] = None) -> Tuple[List[ArchiveEntry], str, Dict[str, str]]:
        cmd = [UNRAR_BIN, "v", "-c-"]
        if password:
            cmd.append(f"-p{password}")
        else:
            cmd.append("-p-")
        cmd.append(filepath)

        proc = subprocess.run(cmd, capture_output=True, text=True, errors="replace", stdin=subprocess.DEVNULL, timeout=30)
        output = proc.stdout

        # Also get comment with 'unrar vt'
        comment = ""
        info = {
            "Type": "RAR Archive",
            "Details": "",
            "Total Files": "0",
            "Uncompressed Size": "0",
            "Packed Size": "0",
            "Ratio": "0%"
        }

        try:
            cmd_vt = [UNRAR_BIN, "vt"]
            if password:
                cmd_vt.append(f"-p{password}")
            else:
                cmd_vt.append("-p-")
            cmd_vt.append(filepath)
            proc_vt = subprocess.run(cmd_vt, capture_output=True, text=True, errors="replace", stdin=subprocess.DEVNULL, timeout=30)
            vt_out = proc_vt.stdout

            import re
            cmt_m = re.search(r"Archive comment:\n(.*?)\n\nArchive:", vt_out, re.DOTALL)
            if cmt_m:
                comment = cmt_m.group(1).strip()
            
            det_m = re.search(r"Details:\s*([^\n]+)", vt_out)
            if det_m:
                info["Details"] = det_m.group(1).strip()
        except Exception:
            pass

        entries: List[ArchiveEntry] = []
        lines = output.splitlines()
        in_table = False

        for line in lines:
            if line.startswith("-----------"):
                in_table = not in_table
                continue
            if in_table and line.strip():
                parts = line.split()
                if len(parts) >= 8:
                    attr = parts[0]
                    size_val = int(parts[1]) if parts[1].isdigit() else 0
                    packed_val = int(parts[2]) if parts[2].isdigit() else 0
                    ratio_val = parts[3]
                    mtime_val = parts[4] + " " + parts[5]
                    crc_val = parts[6]
                    full_name = " ".join(parts[7:]).strip()
                    
                    is_dir = "D" in attr or full_name.endswith("/") or full_name.endswith("\\")
                    clean_path = full_name.replace("\\", "/").rstrip("/")
                    base_name = os.path.basename(clean_path)
                    
                    encrypted = "*" in attr or "?" in attr

                    entries.append(ArchiveEntry(
                        name=base_name,
                        path=clean_path,
                        size=size_val,
                        packed_size=packed_val,
                        ratio=ratio_val,
                        is_dir=is_dir,
                        mtime=mtime_val,
                        crc32=crc_val,
                        attr=attr,
                        encrypted=encrypted
                    ))
            elif not in_table and line.strip():
                # Totals line at the bottom
                parts = line.split()
                if len(parts) >= 4 and parts[0].isdigit() and parts[1].isdigit():
                    info["Uncompressed Size"] = f"{int(parts[0]):,} bytes"
                    info["Packed Size"] = f"{int(parts[1]):,} bytes"
                    info["Ratio"] = parts[2]
                    info["Total Files"] = parts[3]

        return entries, comment, info

    @staticmethod
    def _list_zip(filepath: str, password: Optional[str] = None) -> Tuple[List[ArchiveEntry], str, Dict[str, str]]:
        entries: List[ArchiveEntry] = []
        comment = ""
        info = {
            "Type": "ZIP Archive",
            "Details": "Deflate/Store",
            "Total Files": "0",
            "Uncompressed Size": "0",
            "Packed Size": "0",
            "Ratio": "0%"
        }

        try:
            with zipfile.ZipFile(filepath, "r") as zf:
                if zf.comment:
                    comment = zf.comment.decode("utf-8", errors="replace")
                
                tot_uncompressed = 0
                tot_compressed = 0

                for zinfo in zf.infolist():
                    clean_path = zinfo.filename.rstrip("/")
                    base_name = os.path.basename(clean_path)
                    is_dir = zinfo.is_dir()
                    
                    tot_uncompressed += zinfo.file_size
                    tot_compressed += zinfo.compress_size
                    
                    ratio = "0%"
                    if zinfo.file_size > 0:
                        r = (zinfo.compress_size / zinfo.file_size) * 100
                        ratio = f"{r:.0f}%"

                    mtime_str = f"{zinfo.date_time[0]:04d}-{zinfo.date_time[1]:02d}-{zinfo.date_time[2]:02d} {zinfo.date_time[3]:02d}:{zinfo.date_time[4]:02d}"
                    crc_str = f"{zinfo.CRC:08X}"
                    attr_str = "drwxr-xr-x" if is_dir else "-rw-r--r--"
                    encrypted = bool(zinfo.flag_bits & 0x1)

                    entries.append(ArchiveEntry(
                        name=base_name,
                        path=clean_path,
                        size=zinfo.file_size,
                        packed_size=zinfo.compress_size,
                        ratio=ratio,
                        is_dir=is_dir,
                        mtime=mtime_str,
                        crc32=crc_str,
                        attr=attr_str,
                        encrypted=encrypted
                    ))

                info["Total Files"] = str(len(entries))
                info["Uncompressed Size"] = f"{tot_uncompressed:,} bytes"
                info["Packed Size"] = f"{tot_compressed:,} bytes"
                if tot_uncompressed > 0:
                    info["Ratio"] = f"{(tot_compressed / tot_uncompressed) * 100:.0f}%"
        except Exception as e:
            info["Details"] = f"Error reading ZIP: {e}"

        return entries, comment, info

    @staticmethod
    def _list_tar(filepath: str) -> Tuple[List[ArchiveEntry], str, Dict[str, str]]:
        entries: List[ArchiveEntry] = []
        comment = ""
        info = {
            "Type": "TAR Archive",
            "Details": "POSIX tar",
            "Total Files": "0",
            "Uncompressed Size": "0",
            "Packed Size": "0",
            "Ratio": "100%"
        }

        try:
            with tarfile.open(filepath, "r:*") as tf:
                tot_uncompressed = 0
                for tinfo in tf.getmembers():
                    clean_path = tinfo.name.rstrip("/")
                    base_name = os.path.basename(clean_path)
                    is_dir = tinfo.isdir()
                    tot_uncompressed += tinfo.size
                    
                    mtime_tuple = time.localtime(tinfo.mtime)
                    mtime_str = time.strftime("%Y-%m-%d %H:%M", mtime_tuple)
                    attr_str = oct(tinfo.mode)[-4:]

                    entries.append(ArchiveEntry(
                        name=base_name,
                        path=clean_path,
                        size=tinfo.size,
                        packed_size=tinfo.size,
                        ratio="100%",
                        is_dir=is_dir,
                        mtime=mtime_str,
                        crc32="",
                        attr=attr_str,
                        encrypted=False
                    ))

                info["Total Files"] = str(len(entries))
                info["Uncompressed Size"] = f"{tot_uncompressed:,} bytes"
                stat = os.stat(filepath)
                info["Packed Size"] = f"{stat.st_size:,} bytes"
                if tot_uncompressed > 0:
                    info["Ratio"] = f"{(stat.st_size / tot_uncompressed) * 100:.0f}%"
        except Exception as e:
            info["Details"] = f"Error reading TAR: {e}"

        return entries, comment, info

    @staticmethod
    def extract_archive(archive_path: str, dest_dir: str, items: Optional[List[str]] = None,
                        password: Optional[str] = None, overwrite: bool = True) -> Tuple[bool, str]:
        """
        Extracts entire archive or specific item paths into dest_dir.
        Returns (success, message)
        """
        os.makedirs(dest_dir, exist_ok=True)
        lower = archive_path.lower()

        if lower.endswith(".rar") or lower.endswith(".rev") or lower.endswith(".sfx") or ArchiveBackend._is_rar_file(archive_path):
            cmd = [UNRAR_BIN, "x"]
            if overwrite:
                cmd.append("-o+")
            else:
                cmd.append("-o-")
            if password:
                cmd.append(f"-p{password}")
            else:
                cmd.append("-p-")
            cmd.append(archive_path)

            if items:
                for item in items:
                    cmd.append(item)
            
            # Destination path must end with slash in unrar
            dest_arg = os.path.abspath(dest_dir)
            if not dest_arg.endswith("/"):
                dest_arg += "/"
            cmd.append(dest_arg)

            proc = subprocess.run(cmd, capture_output=True, text=True, errors="replace", stdin=subprocess.DEVNULL, timeout=60)
            if proc.returncode in (0, 1): # 0 is success, 1 is non-fatal warning
                return True, "Extraction completed successfully."
            else:
                return False, f"UnRAR extraction failed (code {proc.returncode}):\n{proc.stderr or proc.stdout}"

        elif lower.endswith(".zip") or lower.endswith(".zipx") or zipfile.is_zipfile(archive_path):
            try:
                with zipfile.ZipFile(archive_path, "r") as zf:
                    if password:
                        zf.setpassword(password.encode())
                    if items:
                        for item in items:
                            zf.extract(item, path=dest_dir)
                    else:
                        zf.extractall(path=dest_dir)
                return True, "Extraction completed successfully."
            except Exception as e:
                return False, f"ZIP extraction error: {e}"

        elif tarfile.is_tarfile(archive_path):
            try:
                with tarfile.open(archive_path, "r:*") as tf:
                    if items:
                        for member in tf.getmembers():
                            if member.name in items or member.name.rstrip("/") in items:
                                tf.extract(member, path=dest_dir)
                    else:
                        tf.extractall(path=dest_dir)
                return True, "Extraction completed successfully."
            except Exception as e:
                return False, f"TAR extraction error: {e}"

        return False, "Unsupported archive format for extraction."

    @staticmethod
    def create_archive(archive_path: str, source_paths: List[str], format_type: str = "rar",
                       compression_level: int = 3, dict_size: str = "32m", solid: bool = True,
                       sfx: bool = False, password: Optional[str] = None, encrypt_names: bool = False,
                       lock: bool = False, recovery_record: bool = True, delete_sources: bool = False,
                       volume_size: Optional[str] = None, comment: Optional[str] = None) -> Tuple[bool, str]:
        """
        Creates an archive with the given settings.
        format_type: 'rar', 'zip', 'tar.gz'
        compression_level: 0 (Store) .. 5 (Best)
        """
        fmt = format_type.lower()
        if fmt == "rar":
            cmd = [RAR_BIN, "a", "-y", "-ep1"]
            # Compression level -m0..-m5
            cmd.append(f"-m{compression_level}")
            # Dictionary size -md32k..-md1024m
            if dict_size:
                cmd.append(f"-md{dict_size.lower()}")
            # Solid archive
            if solid:
                cmd.append("-s")
            else:
                cmd.append("-s-")
            # SFX
            if sfx:
                cmd.append("-sfx")
            # Recovery record
            if recovery_record:
                cmd.append("-rr3%")
            # Lock archive
            if lock:
                cmd.append("-k")
            # Delete sources
            if delete_sources:
                cmd.append("-df")
            # Volumes
            if volume_size and volume_size.strip():
                cmd.append(f"-v{volume_size.strip()}")
            # Password
            if password:
                if encrypt_names:
                    cmd.append(f"-hp{password}")
                else:
                    cmd.append(f"-p{password}")
            else:
                cmd.append("-p-")
            
            cmd.append(archive_path)
            for src in source_paths:
                cmd.append(src)

            proc = subprocess.run(cmd, capture_output=True, text=True, errors="replace", stdin=subprocess.DEVNULL, timeout=60)
            if proc.returncode == 0:
                # Add comment if provided
                if comment:
                    ArchiveBackend.set_archive_comment(archive_path, comment)
                return True, "Archive created successfully."
            else:
                return False, f"RAR creation failed (code {proc.returncode}):\n{proc.stderr or proc.stdout}"

        elif fmt == "zip":
            try:
                compress_type = zipfile.ZIP_DEFLATED if compression_level > 0 else zipfile.ZIP_STORED
                with zipfile.ZipFile(archive_path, "w", compression=compress_type, compresslevel=compression_level if compress_type == zipfile.ZIP_DEFLATED else None) as zf:
                    if comment:
                        zf.comment = comment.encode("utf-8")
                    for src in source_paths:
                        if os.path.isfile(src):
                            zf.write(src, arcname=os.path.basename(src))
                        elif os.path.isdir(src):
                            for root, _, files in os.walk(src):
                                for f in files:
                                    full_p = os.path.join(root, f)
                                    rel_p = os.path.relpath(full_p, os.path.dirname(src))
                                    zf.write(full_p, arcname=rel_p)
                if delete_sources:
                    for src in source_paths:
                        if os.path.isfile(src):
                            os.remove(src)
                        elif os.path.isdir(src):
                            shutil.rmtree(src)
                return True, "ZIP archive created successfully."
            except Exception as e:
                return False, f"ZIP creation failed: {e}"

        elif fmt in ("tar.gz", "tar"):
            mode = "w:gz" if "gz" in fmt else "w"
            try:
                with tarfile.open(archive_path, mode) as tf:
                    for src in source_paths:
                        tf.add(src, arcname=os.path.basename(src))
                if delete_sources:
                    for src in source_paths:
                        if os.path.isfile(src):
                            os.remove(src)
                        elif os.path.isdir(src):
                            shutil.rmtree(src)
                return True, f"{format_type.upper()} archive created successfully."
            except Exception as e:
                return False, f"TAR creation failed: {e}"

        return False, f"Unsupported format: {format_type}"

    @staticmethod
    def test_archive(archive_path: str, password: Optional[str] = None) -> Tuple[bool, str]:
        lower = archive_path.lower()
        if lower.endswith(".rar") or lower.endswith(".rev") or lower.endswith(".sfx") or ArchiveBackend._is_rar_file(archive_path):
            cmd = [UNRAR_BIN, "t", "-y"]
            if password:
                cmd.append(f"-p{password}")
            else:
                cmd.append("-p-")
            cmd.append(archive_path)
            proc = subprocess.run(cmd, capture_output=True, text=True, errors="replace", stdin=subprocess.DEVNULL, timeout=30)
            if proc.returncode == 0:
                return True, "All files were tested and no errors were found."
            else:
                return False, f"Integrity test failed:\n{proc.stderr or proc.stdout}"
        elif lower.endswith(".zip") or lower.endswith(".zipx") or zipfile.is_zipfile(archive_path):
            try:
                with zipfile.ZipFile(archive_path, "r") as zf:
                    corrupt = zf.testzip()
                    if corrupt is None:
                        return True, "All files in ZIP were tested and no errors were found."
                    else:
                        return False, f"Corrupted file found in ZIP: {corrupt}"
            except Exception as e:
                return False, f"ZIP test error: {e}"
        return False, "Unsupported archive format for testing."

    @staticmethod
    def delete_from_archive(archive_path: str, items: List[str], password: Optional[str] = None) -> Tuple[bool, str]:
        lower = archive_path.lower()
        if lower.endswith(".rar") or ArchiveBackend._is_rar_file(archive_path):
            cmd = [RAR_BIN, "d", "-y"]
            if password:
                cmd.append(f"-p{password}")
            else:
                cmd.append("-p-")
            cmd.append(archive_path)
            cmd.extend(items)
            proc = subprocess.run(cmd, capture_output=True, text=True, errors="replace", stdin=subprocess.DEVNULL, timeout=30)
            if proc.returncode == 0:
                return True, "Items deleted successfully."
            else:
                return False, f"Delete failed:\n{proc.stderr or proc.stdout}"
        return False, "Deleting from this archive format is currently only supported for RAR archives."

    @staticmethod
    def set_archive_comment(archive_path: str, comment: str) -> Tuple[bool, str]:
        lower = archive_path.lower()
        if lower.endswith(".rar") or ArchiveBackend._is_rar_file(archive_path):
            with tempfile.NamedTemporaryFile("w", delete=False, suffix=".txt") as tf:
                tf.write(comment)
                cmt_path = tf.name
            try:
                cmd = [RAR_BIN, "-y", f"-z{cmt_path}", "c", archive_path]
                proc = subprocess.run(cmd, capture_output=True, text=True, errors="replace", stdin=subprocess.DEVNULL, timeout=30)
                if proc.returncode == 0:
                    return True, "Comment updated."
                else:
                    return False, f"Comment update failed:\n{proc.stderr or proc.stdout}"
            finally:
                if os.path.isfile(cmt_path):
                    os.remove(cmt_path)
        elif lower.endswith(".zip") or zipfile.is_zipfile(archive_path):
            try:
                with zipfile.ZipFile(archive_path, "a") as zf:
                    zf.comment = comment.encode("utf-8")
                return True, "Comment updated."
            except Exception as e:
                return False, f"Failed to set ZIP comment: {e}"
        return False, "Unsupported archive format for comments."

    @staticmethod
    def lock_archive(archive_path: str) -> Tuple[bool, str]:
        cmd = [RAR_BIN, "k", "-y", archive_path]
        proc = subprocess.run(cmd, capture_output=True, text=True, errors="replace", stdin=subprocess.DEVNULL, timeout=30)
        if proc.returncode == 0:
            return True, "Archive locked successfully. No further changes allowed."
        return False, f"Failed to lock archive:\n{proc.stderr or proc.stdout}"

    @staticmethod
    def repair_archive(archive_path: str) -> Tuple[bool, str]:
        cmd = [RAR_BIN, "r", "-y", archive_path]
        proc = subprocess.run(cmd, capture_output=True, text=True, errors="replace", stdin=subprocess.DEVNULL, timeout=30)
        if proc.returncode == 0:
            return True, "Archive repair routine completed. Check directory for rebuilt.rar or fixed.rar."
        return False, f"Repair failed:\n{proc.stderr or proc.stdout}"
