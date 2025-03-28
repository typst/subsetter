#!/usr/bin/env python3
import re

from pathlib import Path


ROOT = Path(__file__).parent.parent
DATA_DIR = ROOT / "data"
FONT_DIR = ROOT / ".." / "fonts"
SUBSETS_PATH = ROOT / "src" / "subsets.rs"
FONT_TOOLS_PATH = ROOT / "src" / "font_tools.rs"
CFF_PATH = ROOT / "src" / "cff.rs"


def main():
    gen_font_tools_tests()
    gen_cff_tests()
    gen_subset_tests()


def gen_font_tools_tests():
    cff_fonttools_impl("fonttools.tests", Path(FONT_TOOLS_PATH), "test_font_tools")

def gen_cff_tests():
    cff_fonttools_impl("cff.tests", Path(CFF_PATH), "test_cff_dump")

def cff_fonttools_impl(test_src, out_path, fn_name):
    test_string = f"// This file was auto-generated by `{Path(__file__).name}`, do not edit manually.\n\n"
    test_string += "#![allow(non_snake_case)]\n\n"
    test_string += f"use crate::*;\n\n"

    counters = {}
    with open(DATA_DIR / test_src) as file:
        content = file.read().splitlines()
        for line in content:
            if line.startswith("//") or len(line.strip()) == 0:
                continue

            parts = line.split(";")

            font_file = parts[0]
            gids = parts[1]

            if font_file not in counters:
                counters[font_file] = 1

            counter = counters[font_file]
            counters[font_file] += 1

            function_name = f"{font_name_to_function(font_file)}_{counter}"

            test_string += "#[test] "
            test_string += f'fn {function_name}() {{{fn_name}("{font_file}", "{gids}", {counter})}}\n'

    with open(out_path, "w+") as file:
        file.write(test_string)


def gen_subset_tests():
    test_string = f"// This file was auto-generated by `{Path(__file__).name}`, do not edit manually.\n\n"
    test_string += "#![allow(non_snake_case)]\n\n"
    test_string += f"use crate::*;\n\n"

    counters = {}
    with open(DATA_DIR / "subsets.tests") as file:
        content = file.read().splitlines()
        for line in content:
            if line.startswith("//") or len(line.strip()) == 0:
                continue

            parts = line.split(";")

            font_file = parts[0]
            gids = parts[1]

            if font_file not in counters:
                counters[font_file] = 1

            counter = counters[font_file]
            counters[font_file] += 1

            functions = ["glyph_metrics", "glyph_outlines_ttf_parser", "glyph_outlines_skrifa"]

            for function in functions:
                function_name = f"{font_name_to_function(font_file)}_{counter}_{function}"

                test_string += "#[test] "
                test_string += f'fn {function_name}() {{{function}("{font_file}", "{gids}")}}\n'

    with open(Path(SUBSETS_PATH), "w+") as file:
        file.write(test_string)


def font_name_to_function(font_name: str):
    camel_case_pattern = re.compile(r'(?<!^)(?=[A-Z])')
    font_name = camel_case_pattern.sub('_', font_name).lower()
    font_name = (font_name
                 .replace("-", "")
                 .replace(".", "")
                 .replace("[", "")
                 .replace("]", "")
                 .replace(",", "")
                 .replace("ttf", "")
                 .replace("otf", ""))
    return font_name


if __name__ == "__main__":
    main()
