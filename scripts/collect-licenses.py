#!/usr/bin/env python3
"""Collect licenses from locked Cargo source packages for binary distributions."""

import json
import subprocess
import sys
from pathlib import Path


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: collect-licenses.py OUTPUT")
    metadata = json.loads(
        subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--locked"])
    )
    sections = [
        "Third-party Rust dependency notices",
        "",
        "Licenses below are declared by the pinned Cargo packages, not by laya-rs.",
        "The corresponding upstream source packages remain under their own terms.",
        "Packages without an included license file list their SPDX expression and URL.",
    ]
    packages = sorted(
        (package for package in metadata["packages"] if package["source"]),
        key=lambda package: (package["name"], package["version"]),
    )
    for package in packages:
        directory = Path(package["manifest_path"]).parent
        license_file = package.get("license_file")
        files = []
        if license_file:
            candidate = Path(license_file)
            files.append(candidate if candidate.is_absolute() else directory / candidate)
        files.extend(
            path
            for path in directory.iterdir()
            if path.is_file()
            and path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE"))
        )
        files = sorted(set(files))
        sections.extend(
            [
                "",
                "=" * 72,
                f"{package['name']} {package['version']}",
                f"Declared license: {package.get('license') or 'not specified'}",
                f"Source: {package.get('repository') or package['source']}",
            ]
        )
        for file in files:
            if file.is_file():
                sections.extend(["", f"--- {file.name} ---", file.read_text(encoding="utf-8", errors="replace")])
    output = Path(sys.argv[1])
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(sections) + "\n", encoding="utf-8")
    print(f"Collected notices from {len(packages)} pinned Cargo packages: {output}")


if __name__ == "__main__":
    main()
