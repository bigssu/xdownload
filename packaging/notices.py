"""Regenerates THIRD_PARTY_NOTICES.md from the crates linked into the Windows build.

Run from the repository root after changing dependencies:  python packaging/notices.py
"""
import subprocess

HEADER = """# Third-party notices

Xdownload2 is built from the open-source Rust crates listed below. All of them are under
permissive licenses (MIT, Apache-2.0, BSD, Zlib, ISC, Unicode, BSL, Unlicense and similar);
their full license texts are in each crate's source at https://crates.io.

## Programs downloaded at run time (not part of the exe)

| Program | License | Source |
|---|---|---|
| yt-dlp | Unlicense | https://github.com/yt-dlp/yt-dlp |
| FFmpeg (BtbN win64 GPL build) | GPL-3.0-or-later | https://github.com/BtbN/FFmpeg-Builds, https://ffmpeg.org |

Xdownload2 downloads these from their own release pages on first use, verifies the published
SHA-256 and runs them as separate programs. It does not include or link their code.

## Fonts and icons

- Phosphor Icons (MIT) through the `egui-phosphor` crate.
- egui's bundled default fonts (OFL-1.1, Ubuntu Font Licence 1.0). Text is drawn with the
  Segoe UI and Malgun Gothic fonts already installed with Windows; no font file is shipped.

## Rust crates

| Crate | Version | License |
|---|---|---|
"""


def main():
    lines = subprocess.run(
        ["cargo", "tree", "--prefix", "none", "--target", "x86_64-pc-windows-msvc", "-e", "normal", "--format", "{p}|{l}"],
        capture_output=True, text=True, check=True,
    ).stdout.splitlines()
    rows = set()
    for line in lines:
        package, _, license_name = line.partition("|")
        parts = package.split()
        if len(parts) < 2 or parts[0] == "xdownload2":
            continue
        rows.add((parts[0], parts[1].lstrip("v"), license_name.strip() or "see crate"))
    with open("THIRD_PARTY_NOTICES.md", "w", encoding="utf-8", newline="\n") as out:
        out.write(HEADER)
        for name, version, license_name in sorted(rows):
            out.write(f"| {name} | {version} | {license_name} |\n")
    print(f"{len(rows)} crates")


if __name__ == "__main__":
    main()
