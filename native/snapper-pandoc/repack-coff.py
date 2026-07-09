#!/usr/bin/env python3
"""Flatten a GHC Windows staticlib into a flat MinGW-friendly COFF archive.

Why: ghc -staticlib embeds nested ar members; MinGW ld rejects them.
Also plain `ar x` collapses duplicate basenames (Types.o × N packages).

This reads the ar format once (handles SysV long names + CR in names),
writes uniquely-named members, flattens one level of nested archives, and
repacks with the provided `ar` binary.
"""
from __future__ import annotations

import argparse
import os
import shutil
import struct
import subprocess
import sys
from pathlib import Path


def parse_ar(data: bytes) -> list[tuple[str, bytes]]:
    """Return (member_name, body) for each real object in a GNU/SysV ar."""
    if not data.startswith(b"!<arch>\n"):
        raise ValueError("not a Unix ar archive (missing !<arch> magic)")
    pos = 8
    string_table = b""
    members: list[tuple[str, bytes]] = []
    while pos + 60 <= len(data):
        header = data[pos : pos + 60]
        # End of archive padding / zeros
        if header == b"\x00" * 60:
            break
        name_field = header[0:16]
        size_field = header[48:58]
        fmag = header[58:60]
        if fmag != b"`\n":
            # Corrupt or EOF noise
            break
        try:
            size = int(size_field.decode("ascii", "replace").strip() or "0")
        except ValueError as e:
            raise ValueError(f"bad size field at {pos}: {size_field!r}") from e
        pos += 60
        body = data[pos : pos + size]
        pos += size
        if pos % 2 == 1:
            pos += 1

        raw_name = name_field.decode("ascii", "replace")
        # Windows tools sometimes leave CR in name field
        raw_name = raw_name.replace("\r", "").replace("\n", "")
        name = raw_name.strip()

        # BSD long name: #1/N then name is first N bytes of body
        if name.startswith("#1/"):
            try:
                nlen = int(name[3:])
            except ValueError:
                nlen = 0
            if nlen > 0 and nlen <= len(body):
                real = body[:nlen].decode("ascii", "replace").rstrip("\x00")
                real = real.replace("\r", "").strip()
                body = body[nlen:]
                name = real
            else:
                continue

        # SysV string table
        if name in ("//", "/SYM64/"):
            if name == "//":
                string_table = body
            continue
        # SysV symbol table
        if name == "/":
            continue

        # SysV long name: /offset into string table
        if name.startswith("/") and name[1:].isdigit():
            off = int(name[1:])
            if off < len(string_table):
                end = string_table.find(b"\n", off)
                if end < 0:
                    end = string_table.find(b"/", off)
                if end < 0:
                    end = len(string_table)
                name = string_table[off:end].decode("ascii", "replace")
                name = name.replace("\r", "").rstrip("/").strip()
            else:
                name = f"long_{off}"

        # SysV short names end with /
        if name.endswith("/"):
            name = name[:-1]
        name = name.replace("\r", "").strip()
        if not name:
            name = f"anon_{len(members)}"

        members.append((name, body))
    return members


def is_ar(blob: bytes) -> bool:
    return blob.startswith(b"!<arch>\n")


def safe_name(name: str) -> str:
    base = name.replace("\\", "/").split("/")[-1] or "member"
    return "".join(c if (c.isalnum() or c in "._-") else "_" for c in base) or "member"


def write_flat(members: list[tuple[str, bytes]], out_dir: Path, prefix: str) -> list[Path]:
    out_dir.mkdir(parents=True, exist_ok=True)
    paths: list[Path] = []
    for i, (name, body) in enumerate(members, 1):
        # Nested archive: expand one level with unique names
        if is_ar(body):
            try:
                nested = parse_ar(body)
            except ValueError:
                nested = []
            if nested:
                for j, (nn, nb) in enumerate(nested, 1):
                    # Skip still-nested (rare double nest)
                    if is_ar(nb):
                        continue
                    p = out_dir / f"{prefix}{i:05d}n{j:04d}_{safe_name(nn)}"
                    p.write_bytes(nb)
                    paths.append(p)
                continue
        p = out_dir / f"{prefix}{i:05d}_{safe_name(name)}"
        p.write_bytes(body)
        paths.append(p)
    return paths


def looks_like_coff(path: Path) -> bool:
    """Keep non-empty non-ar blobs; PE/COFF machine is typical but not required."""
    try:
        b = path.read_bytes()[:8]
    except OSError:
        return False
    if len(b) < 2:
        return False
    if b.startswith(b"!<arch>"):
        return False
    # Reject obvious text / empty
    if path.stat().st_size < 4:
        return False
    return True


def write_ar(out_a: Path, objs: list[Path]) -> None:
    """Write a SysV GNU ar of objs. Pure Python — avoids Windows CreateProcess
    argv limits when packing thousands of members via `ar rcs`."""
    out_a.parent.mkdir(parents=True, exist_ok=True)
    if not objs:
        raise SystemExit("no objects to pack")

    # Build string table for names that need more than 15 chars + '/'.
    strtab = bytearray()
    name_refs: list[tuple[str, bytes]] = []  # (header_name_field_content, body)
    for p in objs:
        body = p.read_bytes()
        # ar member name = basename only (ld matches by member name)
        name = p.name
        encoded = name.encode("ascii", "replace")
        # SysV: short name is name + '/' padded to 16; long is /offset into //
        if len(encoded) <= 15:
            field = (encoded + b"/").ljust(16, b" ")
        else:
            off = len(strtab)
            strtab.extend(encoded)
            strtab.append(ord("/"))
            strtab.append(ord("\n"))
            field = f"/{off}".encode("ascii").ljust(16, b" ")
        name_refs.append((field.decode("ascii"), body))

    with out_a.open("wb") as f:
        f.write(b"!<arch>\n")
        # Optional string table member first (after we could put symbol table;
        # GNU ld accepts // early).
        if strtab:
            st = bytes(strtab)
            # even size padding later
            hdr_name = b"//".ljust(16, b" ")
            size_s = f"{len(st)}".encode("ascii").rjust(10, b" ")
            header = (
                hdr_name
                + b"0".rjust(12, b" ")
                + b"0".rjust(6, b" ")
                + b"0".rjust(6, b" ")
                + b"100644".rjust(8, b" ")
                + size_s
                + b"`\n"
            )
            assert len(header) == 60
            f.write(header)
            f.write(st)
            if len(st) % 2 == 1:
                f.write(b"\n")

        for field, body in name_refs:
            name_b = field.encode("ascii")
            if len(name_b) != 16:
                name_b = name_b[:16].ljust(16, b" ")
            size_s = f"{len(body)}".encode("ascii").rjust(10, b" ")
            header = (
                name_b
                + b"0".rjust(12, b" ")
                + b"0".rjust(6, b" ")
                + b"0".rjust(6, b" ")
                + b"100644".rjust(8, b" ")
                + size_s
                + b"`\n"
            )
            assert len(header) == 60, len(header)
            f.write(header)
            f.write(body)
            if len(body) % 2 == 1:
                f.write(b"\n")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("input_a", type=Path, help="ghc -staticlib archive")
    ap.add_argument("output_a", type=Path, help="flattened COFF archive path")
    ap.add_argument("--ar", default="ar", help="unused (kept for CLI compat)")
    ap.add_argument("--work", type=Path, default=None, help="work directory for objects")
    ap.add_argument("--keep-work", action="store_true")
    args = ap.parse_args()

    data = args.input_a.read_bytes()
    members = parse_ar(data)
    print(f"repack-coff: input members={len(members)} size={len(data)}", flush=True)

    work = args.work or (args.output_a.parent / "repack-coff-work")
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)

    written = write_flat(members, work, prefix="m")
    print(f"repack-coff: wrote {len(written)} flat files (after nest flatten)", flush=True)

    coff = [p for p in written if looks_like_coff(p)]
    dropped = len(written) - len(coff)
    print(f"repack-coff: COFF keep={len(coff)} drop={dropped}", flush=True)
    if len(coff) == 0:
        for p in written[:8]:
            head = p.read_bytes()[:16]
            print(f"  sample {p.name} magic={head!r}", flush=True)
        raise SystemExit("zero COFF objects after extract; refusing")

    write_ar(args.output_a, coff)
    print(f"repack-coff: wrote {args.output_a} ({args.output_a.stat().st_size} bytes)", flush=True)

    nm = shutil.which("nm") or shutil.which("nm.exe")
    if nm:
        try:
            out = subprocess.check_output([nm, str(args.output_a)], stderr=subprocess.DEVNULL, text=True, errors="replace")
            hits = [ln for ln in out.splitlines() if "snapper_pandoc_" in ln]
            if hits:
                print("repack-coff: C ABI symbols:", flush=True)
                for ln in hits[:12]:
                    print(f"  {ln}", flush=True)
            else:
                print("repack-coff: WARNING no snapper_pandoc_* via nm", flush=True)
        except subprocess.CalledProcessError:
            print("repack-coff: nm failed (non-fatal)", flush=True)

    if not args.keep_work:
        shutil.rmtree(work, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
