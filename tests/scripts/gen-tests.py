#!/usr/bin/env python3
import argparse

from pathlib import Path


ROOT = Path(__file__).parent.parent
DATA_DIR = ROOT / "data"
OUT_PATH = ROOT / "src" / "ttf.rs"


def main():
    test_string = f"// This file was auto-generated by `{Path(__file__).name}`, do not edit manually.\n\n"
    test_string += "#![allow(non_snake_case)]\n\n"
    # functions = ["cmap", "face_metrics", "glyph_metrics", "glyph_outlines"]
    functions = ["face_metrics", "glyph_metrics", "glyph_outlines_ttf_parser", "glyph_outlines_skrifa", "glyph_outlines_freetype"]
    imports = ", ".join(functions)
    test_string += f"use crate::{{{imports}}};\n\n"

    for p in DATA_DIR.rglob("*.tests"):
        if p.is_file() and p.suffix == ".tests":
            with open(p) as file:
                content = file.read().splitlines()
                for i, line in enumerate(content):
                    if line.startswith("//"):
                        continue

                    for function in functions:
                        function_name = f"{p.stem}_{i}_{function}"
                        parts = line.split(";")
                        print(parts)

                        font_file = parts[0]
                        gids = parts[1]

                        test_string += "#[test] "

                        test_string += f'fn {function_name}() {{{function}("{font_file}", "{gids}")}}\n'

    with open(Path(OUT_PATH), "w+") as file:
        file.write(test_string)


if __name__ == "__main__":
    main()
