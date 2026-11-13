#!/usr/bin/env python3
"""Heuristic checks of HYPERION renderer code against docs/frontend/ux-guidelines.md.

Usage:
    ux_lint.py                 Changed renderer files (working tree vs HEAD, untracked included).
    ux_lint.py PATH...         These files, and the files under these directories.
    ux_lint.py --all           The whole renderer.

Scans .ts, .tsx, .css and .html under apps/hyperion/src/renderer, skipping tests, test helpers and
declaration files, in explicit paths too. Displayed text is JSX and HTML text, string literals that
read as labels or sentences, and CSS `content:` strings, with escapes and entities decoded. Each
finding names the guide section it comes from:

    error  breaks a "must" or "never" rule as written (literal colour, glow, spinner, italic, text
           under 0.875rem, emoji, a glyph B612 lacks, a space other than U+0020 and U+00A0,
           horizontal scroll, a modal alert, "please", "Loading…", "!", second person, NaN or null
           as a value, a formula written CO(2) or CO₂, an isotope written ³He, a state symbol such
           as CH4(l), a qualified per cent such as wt% or ppmv, a snake_case key, ...)
    check  needs judgement (a 50% radius may be a contact symbol; a hover handler may have a
           keyboard twin; "SUCCESSFUL" or a lone "-" may be fine in context; MG2SIO4 or Co2 may
           be a formula whose case was changed; Fe3+ may be Fe₃⁺; `${layer.material}` may print a
           substance's registry key; decision-ux-numbers-names's sign and temperature rules are
           checks until the guide states them); resolve it or leave a code comment stating the
           reason

The typography and substance rules of signoff-4 item 7, signoff-5 item 8 and
decision-ux-numbers-names (formulas, isotopes, ions, state symbols, fractions, code names, letters'
names, notation, signs, temperatures and substance keys) skip developer text: the smoke pages,
provenance and identifier fields such as `source`, `paper`, `basis` and `key`, and shader sources.
The descent spike is a display and is read. Comments (TSDoc), test files and test names are never
read. The older rules still read the smoke pages.

It cannot see rendering, layout at 1280x720, data states or closed-loop commanding: those need
reading and running the code. Exit status is 1 when there is any error, and 2 on a usage error (an
unknown option, a path that does not exist) or when the glyph check could not run because the
bundled fonts are missing or unreadable (run `pnpm install`); 2 wins over 1.
"""

from __future__ import annotations

import html
import json
import re
import subprocess
import sys
import unicodedata
from dataclasses import dataclass
from functools import cache
from pathlib import Path
from typing import Callable, NoReturn

sys.dont_write_bytecode = True  # keep the skill directory free of __pycache__
sys.path.insert(0, str(Path(__file__).resolve().parent))
import glyphs  # noqa: E402  (same directory)

RENDERER = "apps/hyperion/src/renderer"
SHARED_SHEET = f"{RENDERER}/src/styles.css"
EXTENSIONS = {".ts", ".tsx", ".css", ".html"}
# Typography: "No text smaller than `0.875rem`."
TEXT_MIN_REM = 0.875
# The CSS default root font size, which rem resolves against at 100% interface scale. It only turns
# px, em and % into the guide's rem sizes (Accessibility: "size everything in `rem`").
ROOT_FONT_PX = 16
# Layout: "a `1px` `--line` border"; Controls and commanding: "Focus is always visible as a `2px`
# `--accent` outline". Hairlines this thin may stay in px.
HAIRLINE_MAX_PX = 2
# Layout: "Panels are rectangles with ... square or `2px` corners."
CORNER_MAX_PX = 2
# Motion and sound: "State transitions take 80 to 150 ms with a simple ease-out."
TRANSITION_MIN_MS, TRANSITION_MAX_MS = 80, 150
# font-size keywords in px at a 16px default, as Chromium (so Electron) computes them.
FONT_SIZE_KEYWORDS = {"xx-small": 9, "x-small": 10, "small": 13, "medium": 16, "large": 18, "x-large": 24, "xx-large": 32, "xxx-large": 48}


@dataclass
class Finding:
    line: int
    severity: str
    rule: str
    message: str
    section: str


# --------------------------------------------------------------------------- source preparation


def blank(chars: list[str], start: int, end: int) -> None:
    for k in range(start, end):
        if chars[k] != "\n":
            chars[k] = " "


def scan(src: str, lang: str) -> tuple[str, str, list[tuple[int, int, str]], dict[int, list[tuple[int, int]]]]:
    """Split source into code and string literals, keeping every offset so line numbers hold.

    Returns `code` (comments and regex-literal bodies blanked, strings intact), `bare` (string
    contents blanked too, delimiters kept), the literals as (start, end, text), with each `${…}`
    shown as `{}`, and each template literal's substitutions as (start, end) spans of the
    expressions inside `${` and `}`, by the literal's start. `lang` is "js", "css" or "html": CSS
    has no line comments (`//` in a url() is not one) and HTML has only `<!-- -->`. A quote right
    after a letter or digit is an apostrophe in text (`<p>Don't vent</p>`), not the start of a
    string."""
    code, bare = list(src), list(src)
    strings: list[tuple[int, int, str]] = []
    substitutions: dict[int, list[tuple[int, int]]] = {}
    n = len(src)

    def string_end(i: int) -> int:
        """Index after the literal that opens at i. Records it, and any literal inside `${…}`."""
        quote, j = src[i], i + 1
        parts: list[str] = []
        part = j
        while j < n and src[j] != quote:
            if src[j] == "\\":
                j += 2
                continue
            if src[j] == "\n" and quote != "`":
                break  # unterminated: stop at the end of the line
            if quote == "`" and src.startswith("${", j):
                parts.append(src[part:j])
                k = substitution_end(j + 2)
                substitutions.setdefault(i, []).append((j + 2, k - 1 if src[k - 1 : k] == "}" else k))
                parts.append("{}" + "\n" * src.count("\n", j, k))
                j = part = k
                continue
            j += 1
        j = min(j, n)
        parts.append(src[part:j])
        strings.append((i, j + 1 if j < n and src[j] == quote else j, "".join(parts)))
        blank(bare, i + 1, j)
        return j + 1 if j < n and src[j] == quote else j

    def substitution_end(j: int) -> int:
        depth = 1
        while j < n:
            if src[j] in "'\"`":
                j = string_end(j)
                continue
            depth += {"{": 1, "}": -1}.get(src[j], 0)
            j += 1
            if depth == 0:
                return j
        return n

    def regex_end(i: int) -> int | None:
        """Index after the regex literal that opens at i, if the `/` there can open one."""
        before = "".join(bare[max(0, i - 40) : i]).rstrip()
        if src.startswith("//", i):
            return None  # the `//` of a URL in JSX text: a regex is never empty
        after_value = before[-1:].isalnum() or before[-1:] in ")]}\"'`_$" or before.endswith(("++", "--", "<"))
        after_arrow = before.endswith("=>")
        if before and (after_value or (before.endswith(">") and not after_arrow)):
            if not re.search(r"(?<![\w$.])(?:return|typeof|case|do|else|in|of|void|yield|await|throw|delete|new)$", before):
                return None  # a division, a closing tag, or `/` in JSX text
        j, in_class = i + 1, False
        while j < n and src[j] != "\n":
            if src[j] == "\\":
                j += 2
                continue
            if src[j] == "[":
                in_class = True
            elif src[j] == "]":
                in_class = False
            elif src[j] == "/" and not in_class:
                blank(code, i + 1, j)
                blank(bare, i + 1, j)
                j += 1
                while j < n and src[j].isalpha():
                    j += 1
                return j
            j += 1
        return None

    i = 0
    while i < n:
        c = src[i]
        end: int | None = None
        if lang == "html" and src.startswith("<!--", i):
            end = src.find("-->", i + 4)
            end = n if end == -1 else end + 3
        elif lang != "html" and src.startswith("/*", i):
            end = src.find("*/", i + 2)
            end = n if end == -1 else end + 2
        elif lang == "js" and src.startswith("//", i) and not re.search(r"[A-Za-z]:$", src[max(0, i - 2) : i]):  # not https://
            end = src.find("\n", i)
            end = n if end == -1 else end
        if end is not None:
            blank(code, i, end)
            blank(bare, i, end)
            i = end
        elif lang == "js" and c == "/" and (regex := regex_end(i)) is not None:
            i = regex
        elif (c == "`" and lang == "js") or (c in "'\"" and not (i > 0 and src[i - 1].isalnum())):
            i = string_end(i)
        else:
            i += 1
    return "".join(code), "".join(bare), strings, substitutions


TAG_NAME = re.compile(r"[A-Za-z][\w.:-]*")
# Where a JSX element may start: after an operator or opening bracket, or a keyword such as return.
# After an identifier or a closing bracket a `<` is a generic (`useState<Foo>`) or a comparison.
JSX_MAY_START = re.compile(r"(?:^|[(\[{,;=:?&|!>]|(?<![\w$.])(?:return|yield|case|default|else|do|in|of|await))$")


def jsx_text(bare: str) -> list[list[tuple[int, int]]]:
    """JSX text in `bare` TSX, as groups of runs: each group is the children between two tags,
    and its runs are the stretches between {expressions}. Tags inside expressions are read too."""
    n = len(bare)
    groups: list[list[tuple[int, int]]] = []

    def tag_end(i: int) -> tuple[int, bool] | None:
        """(index after the tag at i, self-closing), or None when bare[i] does not open a tag."""
        j = i + 1
        closing = bare.startswith("/", j)
        j += closing
        if m := TAG_NAME.match(bare, j):
            j = m.end()
        elif not bare.startswith(">", j):
            return None
        while j < n:
            c = bare[j]
            if c == ">":
                return j + 1, False
            if bare.startswith("/>", j) and not closing:
                return j + 2, True
            if c == "{":
                j = expression_end(j)
            elif c in "'\"":
                k = bare.find(c, j + 1)  # contents are blanked, delimiters kept
                if k == -1:
                    return None
                j = k + 1
            elif c.isspace() or c.isalnum() or c in "_$-:.=":
                j += 1
            else:
                return None
        return None

    def opens_element(i: int) -> bool:
        return (
            not bare.startswith("</", i)
            and JSX_MAY_START.search(bare[max(0, i - 40) : i].rstrip()) is not None
            and tag_end(i) is not None
        )

    def expression_end(j: int) -> int:
        depth = 0
        while j < n:
            c = bare[j]
            if c == "<" and opens_element(j):
                j = element_end(j)
                continue
            depth += {"{": 1, "}": -1}.get(c, 0)
            j += 1
            if depth == 0:
                return j
        return n

    def element_end(i: int) -> int:
        parsed = tag_end(i)
        if parsed is None:
            return i + 1
        j, self_closing = parsed
        if self_closing:
            return j
        runs: list[tuple[int, int]] = []
        run = j
        while j < n:
            c = bare[j]
            if c == "{":
                runs.append((run, j))
                j = run = expression_end(j)
            elif c == "<":
                runs.append((run, j))
                groups.append(runs)
                runs = []
                if bare.startswith("</", j):
                    closed = tag_end(j)
                    return closed[0] if closed else j + 1
                j = run = element_end(j)
            else:
                j += 1
        return n

    i = bare.find("<")
    while i != -1:
        i = bare.find("<", element_end(i) if opens_element(i) else i + 1)
    return groups


def html_text(bare: str) -> list[list[tuple[int, int]]]:
    """Text between the tags of an HTML file, outside <script> and <style>, one run per group."""
    skip = [m.span() for m in re.finditer(r"<(script|style)\b.*?</\1\s*>", bare, re.S | re.I)]
    return [[m.span(1)] for m in re.finditer(r">([^<>]+)<", bare) if not any(a <= m.start(1) < b for a, b in skip)]


def line_of(text: str, index: int) -> int:
    return text.count("\n", 0, index) + 1


def decode_css_escapes(text: str) -> str:
    """`\\2609` is ☉ in a CSS string, and `\\"` a quote."""

    def char(m: re.Match[str]) -> str:
        if m.group(2) is not None:
            return m.group(2)
        point = int(m.group(1), 16)
        return chr(point) if 0 < point <= 0x10FFFF and not 0xD800 <= point <= 0xDFFF else "\ufffd"

    return re.sub(r"\\(?:([0-9a-fA-F]{1,6})\s?|(.))", char, text, flags=re.S)


# A string is not operator-facing when the code before it makes it a class name, id, test ID,
# module path, test name, event name, selector or developer message. Only that string is skipped.
NOT_UI_BEFORE = re.compile(
    r"(?:\b(?:className|class|id|key|htmlFor|data-[\w-]+)\s*=\s*(?:\{[^{}]*)?"
    r"|\b(?:import|from|require)\s*\(?\s*"
    r"|(?:\bthrow\s+new\s+\w+|\b[A-Z]\w*Error|\bconsole\.\w+|\bnew\s+URL|\b(?:addEventListener|removeEventListener|"
    r"querySelector(?:All)?|getElementById|getByTestId|queryByTestId|matchMedia|startsWith|endsWith|"
    r"NumberFormat|DateTimeFormat|expect|describe|it|test)(?:\.\w+)*)\s*\(\s*"
    r"|\bcase\s+|[!=]==?\s*)$"
)
# In an HTML file only these attributes are read by the operator or a screen reader.
DISPLAYED_ATTRIBUTE = re.compile(r"\b(?:title|alt|placeholder|aria-label|aria-roledescription|aria-valuetext|label|value)\s*=\s*$")
# Developer text that the typography and substance rules skip (decision-substance-names §6: "TSDoc,
# test names, `source` and the other provenance fields, the smoke page" stay as they are): the
# value of a field that says where a figure comes from, as one literal or a `+` chain of them.
# An identifier's field (`key: "H2+e-"`, `id:`) is developer text too.
PROVENANCE_KEY = re.compile(
    r"(?<![\w$])(?:source|sources|paper|licence|license|reduction|citation|reference|references|doi|url|fetched|basis|evidence|provenance|key|id)"
    r"\s*:\s*$"
)
# Source code held in a string, or code that the JSX reader took for text: a shader's WGSL
# (`@group(0) @binding(0)`, `fn main(`), or a `switch` (`; break; case "per_ly3":`).
CODE_LIKE = re.compile(
    r"@(?:group|binding|compute|vertex|fragment|builtin|workgroup_size|location)\b|\bfn\s+\w+\s*\(|\bvar<|\bstruct\s+\w+\s*\{"
    r"|;\s*break\s*;|\bcase\s+[\"'`]"
)
CONCATENATED_LITERAL = re.compile(r"(?:\"[^\"\n]*\"|'[^'\n]*'|`[^`]*`)\s*\+\s*$")
# The smoke pages (`smoke/`, `smoke/spike.ts` among them), which no operator reads. The descent
# spike (`view/spike/`) is a display, `DESCENT SPIKE` in the guide's nomenclature list, and is read.
DEVELOPER_DIRS = {"smoke"}


def chain_head(bare: str, start: int) -> str:
    """The code before the `+` chain of literals that the literal at `start` ends, so that each
    literal of `console.warn("…" + "…")` or `source: "…" + "…"` is judged by what the chain's first
    one follows. `bare` has every string's contents blanked, so a literal is only its quotes."""
    before = bare[max(0, start - 4000) : start]
    while m := CONCATENATED_LITERAL.search(before):
        before = before[: m.start()]
    return before[-200:]


def is_developer_page(path: Path) -> bool:
    """A smoke page, judged below the renderer so that a checkout's own path never counts."""
    posix = path.as_posix()
    below = posix.split(f"{RENDERER}/", 1)[1] if f"{RENDERER}/" in posix else path.name
    return bool(DEVELOPER_DIRS & set(below.split("/")[:-1]))


def ui_strings(lang: str, bare: str, strings: list[tuple[int, int, str]], in_text: bytearray) -> list[tuple[int, int, str, bool]]:
    """String literals an operator may read: labels and sentences rather than keys, paths, class
    names, values compared against or developer messages. Each is (start, line, text, provenance),
    `provenance` when it is the value of a provenance field."""
    texts: list[tuple[int, int, str, bool]] = []
    for start, end, raw in strings:
        if in_text[start]:
            continue  # a quoted word inside JSX or HTML text is read with the text
        before, after = bare[max(0, start - 200) : start], bare[end : end + 8]
        if lang == "html":
            if not DISPLAYED_ATTRIBUTE.search(before):
                continue
        elif NOT_UI_BEFORE.search(head := chain_head(bare, start)) or re.match(r"\s*[!=]==?", after):
            continue
        elif re.search(r"[{,]\s*$", before) and re.match(r"\s*:", after):
            continue  # an object key
        text = glyphs.decode_escapes(raw) if lang == "js" else raw
        if lang == "html" or re.search(r"[\w-]=$", before):
            text = html.unescape(text)  # an attribute value: `title="&rarr;"` shows →
        if not re.search(r"[A-Za-z]", text) and not re.search(r"[^\x00-\x7f]", text):
            continue
        if re.search(r"^[./@#]|://|\.(css|tsx?|js|json|svg|png|woff2?)$", text.strip()):
            continue
        if re.fullmatch(r"[a-z][a-zA-Z0-9_:-]*", text.strip()):  # a key, id or event name
            continue
        texts.append((start, line_of(bare, start), text, lang == "js" and PROVENANCE_KEY.search(head) is not None))
    return texts


def css_content(bare: str, strings: list[tuple[int, int, str]]) -> list[tuple[int, str]]:
    """The strings of CSS `content:` declarations, which the console displays."""
    spans = [m.span(1) for m in re.finditer(r"(?<![\w-])content\s*:([^;{}]*)", bare)]
    return [(line_of(bare, s), decode_css_escapes(text)) for s, _, text in strings if any(a <= s < b for a, b in spans)]


def text_nodes(code: str, groups: list[list[tuple[int, int]]]) -> list[tuple[int, str]]:
    """Each group of text runs joined into one text, an {expression} shown as `{}`, entities decoded."""
    texts: list[tuple[int, str]] = []
    for runs in groups:
        pieces = [code[runs[0][0] : runs[0][1]]]
        for (_, prev_end), (start, end) in zip(runs, runs[1:]):
            pieces.append("{}" + "\n" * code.count("\n", prev_end, start) + code[start:end])
        text = html.unescape("".join(pieces))
        if re.sub(r"\{\}|\s", "", text):
            texts.append((line_of(code, runs[0][0]), text))
    return texts


# --------------------------------------------------------------------------- rules

HEX = re.compile(r"(?<![\w.&$])(?<!url\()#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})\b")
ID_SELECTOR = re.compile(r"[^;{}]*\{")  # after `#fade`, a selector runs on to its block
NAMED_COLOUR = re.compile(
    r"(?:color|background[\w-]*|border[\w-]*|fill|stroke|outline[\w-]*|fillStyle|strokeStyle|"
    r"box-shadow|text-shadow|boxShadow|textShadow|shadowColor)"
    r"\s*[:=]\s*['\"]?[^;'\"{}\n]*\b(white|black|red|green|blue|yellow|orange|purple|gr[ae]y|cyan|magenta|lime|pink)\b",
    re.I,
)
# Voice and nomenclature names these ("No ... "please", no "thinking…"", "Oops, we lost the
# connection!" as the humour to avoid) and Never names "Loading…": each is an error.
BANNED_PHRASE = re.compile(r"\b(?:please|oops)\b|\b(?:loading|thinking)\b[^.…]{0,40}?(?:…|\.\.\.)", re.I)
# Web and game phrasing that the guide's voice implies but does not name: a check.
WEAK_PHRASE = re.compile(r"\b(sorry|success(?:ful(?:ly)?)?|welcome|click here|tap here|awesome|hooray|congratulations)\b", re.I)
WEB_IDIOM = re.compile(r"\b(spinner|skeleton|toast|hamburger|snackbar|confetti|throbber)\b", re.I)
FICTION_BREAK = re.compile(r"\b(glow\w*|neon|scan-?lines?|hologram\w*|glassmorph\w*|crt|chromatic)\b", re.I)
# Voice: "No exclamation marks, humour or second person" and "No "I"". A bare I can be a numeral
# (`EPS I BUS`), and first person plural is implied rather than named, so those are checks.
PERSON_NAMED = re.compile(r"(?i:\b(?:you|your|yours|yourself|you're|you've|you'll|you'd)\b)|\bI(?:'m|'ve|'ll|'d)\b")
PERSON_OTHER = re.compile(r"(?i:\b(?:me|my|mine|we|we're|we've|we'll|our|ours|us)\b)|\bI\b")
# Data states, Missing: "Never `0`, `NaN`, `null` or an empty field". NaN and null are never a
# reading; `0` and the other stand-ins can be real content (a scale mark, a range), so a check.
MISSING_NAMED = re.compile(r"NaN|null", re.I)
MISSING_OTHER = re.compile(r"N/?A|--|undefined|TBD|\?+|-|0(?:\.0+)?", re.I)
EMOJI = re.compile("[\U0001f000-\U0001faff\ufe0f]")
# Typography: "Neither face has ... a space other than U+0020 and the no-break U+00A0". A space,
# a line or a paragraph separator.
SPACE_CATEGORIES = {"Zs", "Zl", "Zp"}

# Typography (signoff-4 item 7): "A chemical formula keeps its case and its digits on the line, even
# in an upper-case label: `CO2`, `H2O`, `NH4SH`, as the substance keys are written. Its digits count
# atoms and are not a symbol's subscript, so never `CO(2)`, and `Co` is cobalt where `CO` is carbon
# monoxide. An isotope takes its mass number after a hyphen, `He-3`, `C-14`, even where `¹`, `²`
# and `³` could set it". The natural elements, hydrogen to uranium: no formula on a console names a
# heavier one, and leaving the transuranics out keeps `No2` and `Nh3` (nobelium, nihonium) from
# reading as formulas.
ATOMIC_NUMBER = {
    symbol: z
    for z, symbol in enumerate(
        "H He Li Be B C N O F Ne Na Mg Al Si P S Cl Ar K Ca Sc Ti V Cr Mn Fe Co Ni Cu Zn Ga Ge As Se Br Kr "
        "Rb Sr Y Zr Nb Mo Tc Ru Rh Pd Ag Cd In Sn Sb Te I Xe Cs Ba La Ce Pr Nd Pm Sm Eu Gd Tb Dy Ho Er Tm Yb "
        "Lu Hf Ta W Re Os Ir Pt Au Hg Tl Pb Bi Po At Rn Fr Ra Ac Th Pa U".split(),
        start=1,
    )
}
ELEMENTS = frozenset(ATOMIC_NUMBER)
SYMBOL = re.compile(r"[A-Z][a-z]?")
SUBSCRIPT_DIGIT = "[\u2080-\u2089]"
SUPERSCRIPT_DIGIT = "[\u2070\u00b9\u00b2\u00b3\u2074-\u2079]"
# Sub- and superscript digits and signs as they are written on the line; a minus is the minus sign.
ON_THE_LINE = str.maketrans(
    "\u2080\u2081\u2082\u2083\u2084\u2085\u2086\u2087\u2088\u2089\u2070\u00b9\u00b2\u00b3\u2074\u2075\u2076\u2077\u2078\u2079\u207a\u207b\u208a\u208b",
    "01234567890123456789+\u2212+\u2212",
)
# A formula with a subscript count or a raised sign: `CO₂`, `Fe⁺`, `Fe²⁺`.
SCRIPT_FORMULA = re.compile(
    rf"(?<![\w$.])((?:[A-Z][a-z]?(?:[0-9]+|{SUBSCRIPT_DIGIT}+)?)+)({SUPERSCRIPT_DIGIT}*[\u207a\u207b]|[\u208a\u208b])?(?![\w(\u207a\u207b])"
)
SCRIPT_ELECTRON = re.compile(r"(?<![\w$.])e[\u207b\u208b](?![\w\u207a\u207b])")
# A formula with a count in parentheses: `CO(2)`, `H(2)O`. One symbol with a parenthesised digit,
# `S(1)`, is a symbol's subscript as "Numbers, units and time" writes it, and is not read; nor is a
# group of atoms with its count, `Mg(ClO4)2` (decision-ux-numbers-names, ruling 4).
PAREN_FORMULA = re.compile(r"(?<![\w$.])((?:[A-Z][a-z]?(?:[0-9]+|\([0-9]+\))?)+)(?![\w(])")
# A superscript mass number before an element's symbol: `³He`, `¹⁴C`.
SCRIPT_ISOTOPE = re.compile(rf"(?<![\w$.])({SUPERSCRIPT_DIGIT}+)([A-Z][a-z]?)(?![a-z])")
# The same on the line, `3He`, `40Ar`: read only where the number could be the element's mass number
# (from its atomic number Z to 3Z + 6), so that `4K` and `12V`, a unit with no space, are not.
LINE_ISOTOPE = re.compile(r"(?<![\w$.,+\-\N{MINUS SIGN}])([1-9][0-9]{0,2})([A-Z][a-z]?)(?![\w+\-])")
# The registry's public mirror, whose formula keys are formulas the simulation holds.
SUBSTANCE_FIXTURE = "packages/protocol/fixtures/substances.json"
# A word holding a digit and a capital: a formula whose case was changed (`MG2SIO4`, `H2o`)?
CASE_WORD = re.compile(r"(?<![\w$./'\-\u2212+])[A-Za-z][A-Za-z0-9]*(?![\w$/'\-\u2212+]|\.\w)")

# Voice and nomenclature (signoff-5.md item 8, adopted 2026-10-12, in the guide since c892468e):
# "An ion is its formula with its sign on the line: `Fe+`, and `H−` with the minus sign. Every ion
# the simulation holds is singly charged, since `Fe3+` on the line would read as three atoms."
# One symbol, a count and a sign: `Fe3+`, `Ca2+`. H, C, N and O are left out, since `H3+`, `H2+`,
# `N2+`, `O2+`, `O2−` and `C2−` are singly charged molecular ions.
LINE_CHARGE = re.compile(r"(?<![\w$.])([A-Z][a-z]?)([2-9])([+\-\u2212])(?![\w+\-\u2212])")
DOUBLED_SIGN = re.compile(r"(?<![\w$.])((?:[A-Z][a-z]?[0-9]*)+)(\+\+|--|\u2212\u2212)(?![\w+\-\u2212])")
# signoff-5.md item 8: "A phase is a word beside the name, never part of it ... IUPAC's state
# symbols are not used (`CH4(l)`), since a parenthesis after a symbol holds its subscript".
STATE_SYMBOL = re.compile(r"(?<![\w$.])((?:[A-Z][a-z]?[0-9]*)+)\((s|l|g|aq|cr|am|vit|sln|ads)\)(?![\w(])")
# An ion's minus typed as a hyphen, `H-`, `e-`, where signoff-5 writes it as the minus sign U+2212;
# not a suspended hyphen (`Fe- and Mg-rich`).
HYPHEN_CHARGE = re.compile(r"(?<![\w$.\-+])((?:[A-Z][a-z]?[0-9]*)+|e)-(?=[\s,;:)]|$)(?!\s+(?:and|or|to|nor)\b)")
# signoff-5.md item 8: "A space comes before `%`, and nothing is attached to `%` or to any unit ...
# never `75 wt%`, `75 % w/w`, `75% by mass` or `ppmv` (SI Brochure, 9th edition, §5.4.2 and
# §5.4.7)". §5.4.7 names "percentage by mass", "by volume" and "by amount of substance".
# A ratio of like units is allowed (the same section: "m/m, mol/mol"), so `V/V`, `W/W` and a bare
# `m/m` are not read: `w/w`, `v/v` and `w/v` are read in small letters only, and `m/m` after `%`.
QUALIFIED_FRACTION = re.compile(
    r"(?i:(?<![\w$])(?:wt|vol|mole?)\.?\s?%|(?<![\w$])at\.?%"
    r"|%\s*(?:wt|vol|mol|m/m)\b"
    r"|(?<![\w$])(?:weight|mass|volume|mole|atom(?:ic)?)[ -]?(?:%|per\s?cent(?:age)?)"
    r"|(?<![\w$])pp[mbt]v\b|(?<![\w$])pp[mbt]\s*\((?:v|vol|by volume)\)|(?<![\w$])pp[mbt]\s+by\s+(?:volume|mass|weight)\b"
    r"|(?:[0-9]|\{\})\s*%\s*by\s+(?:mass|weight|volume|amount)\b"
    r"|\bper\s?cent(?:age)?\s+by\s+(?:mass|weight|volume|amount)\b)"
    r"|(?<![\w/])(?:w/w|v/v|w/v)(?![\w/])"
)
UNSPACED_PERCENT = re.compile(r"(?:[0-9]+(?:[.,][0-9]+)*|\{\})%(?![\w%])")
# A string with a CSS function holds lengths, where `50%` takes no space.
CSS_FUNCTION = re.compile(
    r"\b(?:calc|translate(?:[XYZ]|3d)?|scale[XYZ]?|rotate[XYZ]?|rgba?|hsla?|var|clamp|min|max|minmax|repeat|inset|polygon|circle|ellipse|(?:linear|radial|conic)-gradient)\("
)

# Typography: "Where the literature's own name for a thing is a letter's name, the thing keeps it,
# as `DELTA-V`, `H-alpha` and oxygen's `gamma` band do. A symbol that has a name in words is never
# spelled out letter by letter (`delta phi` for the phase shift)." A letter's name beside another
# letter (`delta phi`, `delta T`), or standing for a value (`tau 0.5`, `gamma = 1.3`, `tau(550)`);
# one joined to a word by a hyphen (`H-alpha`) is the literature's name and is not read.
GREEK_NAME = r"(?:alpha|beta|gamma|delta|epsilon|zeta|eta|theta|iota|kappa|lambda|mu|nu|xi|omicron|pi|rho|sigma|tau|upsilon|phi|chi|psi|omega)"
LETTER_PAIR = re.compile(rf"(?i)(?<![\w-])({GREEK_NAME})[ _]+({GREEK_NAME}|[b-z])(?![\w-])")
LETTER_VALUE = re.compile(rf"(?i)(?<![\w-])({GREEK_NAME})(?:\s*=|\(|\s+[\u2212-]?[0-9])")
# Typography: "A power of ten is written in E notation ... A relation is written as `<` or `>`, or
# in words."
POWER = re.compile(r"[0-9A-Za-z)]\s*\^\s*[\u2212+-]?[0-9]")
RELATION = re.compile(r"(?<![<>=!~])(?:<=|>=|!=|~=)(?![<>=])")
# E notation's other spellings: a times sign before 10 and its exponent (`1.2 x 10-6`), a power of
# ten that the faces' raised 1, 2 and 3 can set (`10` raised to 3), and toExponential's small `e`.
TIMES_TEN = re.compile(
    r"(?:[0-9]|\{\})\s*(?:\N{MULTIPLICATION SIGN}|x|\*)\s*10\s*(?:[-+\N{MINUS SIGN}]\s?[0-9]|" + SUPERSCRIPT_DIGIT + r"|\N{SUPERSCRIPT MINUS})"
)
RAISED_TEN = re.compile(r"(?<![\w.])10[\N{SUPERSCRIPT ONE}\N{SUPERSCRIPT TWO}\N{SUPERSCRIPT THREE}]+(?![\w\N{SUPERSCRIPT MINUS}])")
SMALL_E = re.compile(r"(?<![\w.])[0-9]+(?:\.[0-9]+)?e[-+]?[0-9]+(?![\w.])")
# All-capital abbreviations that are also letters' names: estimated time of arrival, pounds per
# square inch, a principal investigator.
LETTER_ABBREVIATIONS = {"ETA", "PSI", "PI"}

# decision-ux-numbers-names.md (adopted 2026-10-12; its guide text is queued as sign-off item 9,
# not yet in the guide): "A number's sign in player text is the hyphen-minus, `-` (U+002D),
# everywhere ... the minus sign − (U+2212) stays for that (`H−`, `e−`) and for an operator
# between two terms (`|m − 1|`), and is never a number's sign" (ruling 2); "A temperature in
# player text is in kelvin, `K`, never `°C`" (ruling 1). Checks until the guide's text lands. The
# minus sign before a space (an operator, or the `-` key's sign in `+/− ZOOM`) is not read.
MINUS_SIGN_NUMBER = re.compile(r"\N{MINUS SIGN}(?=[0-9]|\{\})")
CELSIUS = re.compile(r"\N{DEGREE SIGN}\s?[CF]\b|\N{DEGREE CELSIUS}|\N{DEGREE FAHRENHEIT}")
# signoff-5.md item 8: "A key with an underscore ... never reaches the screen": a snake_case word.
SNAKE_KEY = re.compile(r"(?<![\w$.\-/])[a-z][a-z0-9]+(?:_[a-z0-9]+)+(?![\w$\-/.(])")
# One letter, an underscore and a subscript (`c_p`, `r_eff`): a symbol's subscript, which "Numbers,
# units and time" writes in parentheses (`A(V)`).
UNDERSCORE_SUBSCRIPT = re.compile(r"(?<![\w$.\-/])([A-Za-z])_([A-Za-z0-9]+)(?![\w$\-/.(])")

# decision-substance-names §7 (a lean, "for the next console-ux skill edit") and signoff-5.md item
# 8: "A key with an underscore, a variant's identifier or any other code name never reaches the
# screen." A member chain whose last name ends in material, species or substance, in a `${…}` of a
# displayed string or a JSX child `{…}`, that is not passed to substanceName().
SUBSTANCE_MODULE = f"{RENDERER}/src/lib/substances.ts"
MEMBER_CHAIN = re.compile(r"(?<![\w$.])[A-Za-z_$][\w$]*(?:\s*(?:\?\.|!\.|\.)\s*[A-Za-z_$][\w$]*)*")
SUBSTANCE_NAME_END = re.compile(r"(?:[Mm]aterial|[Ss]pecies|[Ss]ubstance)(?:s|Keys?)?$")
# A JSX attribute that the operator or a screen reader reads, given as an expression.
DISPLAYED_JSX_ATTRIBUTE = re.compile(r"(?<![\w-])(?:title|alt|placeholder|label|aria-label|aria-description|aria-roledescription|aria-valuetext)\s*=\s*\{")
# Calls that hand their argument to the screen as it is: `String(layer.material)` prints the key.
PASS_THROUGH_CALLS = {"String", "concat"}
# A formula key, by `SubstanceKey`'s grammar (crates/hyperion-surface/src/substance_key.rs).
FORMULA_KEY = re.compile(r"(?:[A-Z][a-z]?(?:[2-9]|[1-9][0-9]+)?)+[+-]?")
# The chain is compared, called, assigned, tested, or a parameter: it prints nothing.
NOT_PRINTED_AFTER = re.compile(r"\s*(?:[!=]==?|[<>]=?|\(|=(?!=)|\?(?![?.])|\)?\s*=>|instanceof\b|in\b)")
NOT_PRINTED_BEFORE = re.compile(r"(?:[!=]==?|(?<!=)>=?|<=?|\btypeof|\bin|\bcase)\s*$")
# String methods that pass a key through to the screen unchanged in substance.
STRING_METHODS = {"toUpperCase", "toLowerCase", "toLocaleUpperCase", "toLocaleLowerCase", "trim", "toString", "replace", "replaceAll", "padStart", "padEnd", "join"}
# A formula upper-cased in code: a substance's name, or a value named for a substance.
UPPER_SUBSTANCE = re.compile(
    r"(?<![\w$.])substanceName\s*\((?:[^()]|\([^()]*\))*\)\s*\.\s*to(?:Locale)?UpperCase\("
    r"|(?<![\w$])[\w$]*(?:[Mm]aterial|[Ss]pecies|[Ss]ubstance|[Ff]ormula)\s*[?!]?\s*\.\s*to(?:Locale)?UpperCase\("
)
STATE_SELECTOR = re.compile(r":(?:hover|focus(?:-visible|-within)?|active)\b|::selection")
STYLE_LENGTH = re.compile(
    r"\b(?:(?:min|max)(?:Width|Height)|width|height|margin\w*|padding\w*|top|right|bottom|left|inset\w*|"
    r"gap|rowGap|columnGap|letterSpacing|border\w*Width)\s*:\s*(?:(\d+(?:\.\d+)?)\s*(?=[,}\n])|(['\"])(\d+(?:\.\d+)?)px\2)"
)
FONT_SIZE = r"(?:(\d*\.?\d+)(px|rem|em|%)|(xxx-large|xx-large|x-large|large|medium|smaller|xx-small|x-small|small))(?![\w-])"
FONT_SHORTHAND = re.compile(r"^\s*((?:[\w-]+\s+)*?)" + FONT_SIZE + r"(?:\s*/\s*[^\s,]+)?\s+(.+)$", re.S)


def length_px(value: float, unit: str) -> float:
    if unit in ("rem", "em"):
        return value * ROOT_FONT_PX
    return value * ROOT_FONT_PX / 100 if unit == "%" else value


def is_shared_sheet(path: Path, root: Path) -> bool:
    return path.resolve() == (root / SHARED_SHEET).resolve()


@cache
def shared_sheet(root: Path) -> str:
    """styles.css with comments blanked, or "" when there is none."""
    sheet = root / SHARED_SHEET
    return scan(sheet.read_text(encoding="utf-8"), "css")[0] if sheet.exists() else ""


def shared_classes(root: Path) -> set[str]:
    return set(re.findall(r"\.([a-zA-Z][\w-]*)", shared_sheet(root)))


def status_carriers(code: str) -> dict[str, set[str]]:
    """Class name -> the status colours (`caution`, ...) its rules carry outside hover and focus."""
    carriers: dict[str, set[str]] = {}
    for block in re.finditer(r"([^{}]+)\{([^{}]*)\}", code):
        if not STATE_SELECTOR.search(block.group(1)):
            for name in re.findall(r"\.([a-zA-Z][\w-]*)", block.group(1)):
                carriers.setdefault(name, set()).update(re.findall(r"--status-(\w+)", block.group(2)))
    return carriers


@cache
def reduced_motion_guarded(root: Path) -> bool:
    return any("prefers-reduced-motion" in scan(p.read_text(encoding="utf-8"), "css")[0] for p in (root / RENDERER).rglob("*.css"))


def check_stylesheet_scope(path: Path, code: str, root: Path) -> list[Finding]:
    """CSS outside styles.css that restyles a shared class, and motion with no reduced-motion guard."""
    found: list[Finding] = []
    if path.suffix == ".css" and not is_shared_sheet(path, root):
        shared = shared_classes(root)
        for block in re.finditer(r"\s*([^{}]+)\{", code):
            clashes = sorted(set(re.findall(r"\.([a-zA-Z][\w-]*)", block.group(1))) & shared)
            if clashes:
                names = ", ".join("." + c for c in clashes)
                found.append(Finding(line_of(code, block.start(1)), "check", "shared-class",
                    f"Restyles {names} from styles.css for everything that uses it: stations differ in content, never in grammar.",
                    "Principles"))
    motion = re.search(r"@keyframes|\b(?:animation|transition)(?:-name|-duration|Name|Duration)?\s*:(?!\s*['\"]?none\b)", code)
    if motion and not reduced_motion_guarded(root):
        found.append(Finding(line_of(code, motion.start()), "error", "reduced-motion",
            "Motion with no prefers-reduced-motion rule anywhere in the renderer's CSS: drop transitions and replace flashing with steady reverse video under it.",
            "Motion and sound"))
    return found


def check_code(path: Path, code: str, bare: str, root_blocks: list[tuple[int, int]], root: Path) -> list[Finding]:
    """Rules over comment-free source with strings intact (colours live in strings too) and any
    displayed text blanked. `bare` has the strings blanked as well, for rules about calls."""
    found: list[Finding] = []
    css, tsx = path.suffix == ".css", path.suffix == ".tsx"

    def add(index: int, severity: str, rule: str, message: str, section: str) -> None:
        if any(start <= index < end for start, end in root_blocks):
            return  # token definitions in the shared stylesheet's :root block
        found.append(Finding(line_of(code, index), severity, rule, message, section))

    def font_size(index: int, shown: str, number: str | None, unit: str | None, keyword: str | None) -> None:
        if keyword == "smaller":
            add(index, "check", "font-size", f"{shown}: relative to the parent; make sure the result is at least {TEXT_MIN_REM}rem.", "Typography")
            return
        if unit == "px":
            add(index, "error", "font-size", f"font-size in px ({shown}): size text in rem so interface scale works.", "Accessibility")
        size_px = FONT_SIZE_KEYWORDS[keyword] if keyword else length_px(float(number or 0), unit or "px")
        if size_px < TEXT_MIN_REM * ROOT_FONT_PX:
            add(index, "error", "font-size", f"{shown} is under {TEXT_MIN_REM}rem, the smallest text allowed.", "Typography")

    for m in HEX.finditer(code):
        if not (css and ID_SELECTOR.match(code, m.end())):  # `#fade {` is an id selector
            add(m.start(), "error", "literal-colour", f"Literal colour {m.group(0)}: use a token (var(--…)) from styles.css.", "Colour")
    for m in re.finditer(r"\b(rgba?|hsla?|hwb|lab|lch|oklab|oklch|color-mix)\((?:[^()]|\([^()]*\))*\)?", code):
        if "${" in m.group(0) or "var(" in m.group(0):  # computed, such as a ramp or tint built from tokens
            add(m.start(), "check", "literal-colour", f"Computed colour {m.group(1)}(…): make sure every input comes from a token.", "Colour")
        else:
            add(m.start(), "error", "literal-colour", f"Colour function {m.group(1)}(…): use a token.", "Colour")
    tokens_blanked = re.sub(r"--[\w-]+", lambda m: " " * len(m.group(0)), code)  # `var(--hazard-yellow)` is a token
    for m in NAMED_COLOUR.finditer(tokens_blanked):
        add(m.start(1), "error", "literal-colour", f"Named colour '{m.group(1)}': use a token.", "Colour")
    for m in re.finditer(r"\b(box-shadow|text-shadow|drop-shadow|backdrop-filter|mix-blend-mode|boxShadow|textShadow|backdropFilter|shadowBlur|shadowColor)\b|\bblur\(|\b(?:linear|radial|conic|repeating-linear|repeating-radial)-gradient\(", code):
        add(m.start(), "error", "effect", f"'{m.group(0)}': no gradients, glows, shadows or blurs on console chrome.", "Colour")
    for m in re.finditer(r"\bcreate(?:Linear|Radial|Conic)Gradient\(", code):
        add(m.start(), "check", "effect", f"{m.group(0)}…): a data ramp is allowed, chrome gradients are not.", "Colour")
    for m in re.finditer(r"\bopacity\s*:|\bglobalAlpha\b|\bopacity\s*=", code):
        add(m.start(), "check", "transparency", "Transparency: not allowed as an effect on console chrome.", "Colour")
    for m in FICTION_BREAK.finditer(code):  # class names and strings included
        add(m.start(), "check", "fiction-break", f"'{m.group(0)}' suggests a banned effect (glow, neon, scanlines, hologram, glass).", "Never")

    for m in re.finditer(r"font-size\s*:\s*" + FONT_SIZE, code):
        font_size(m.start(), m.group(0), m.group(1), m.group(2), m.group(3))
    for m in re.finditer(r"fontSize\s*:\s*(?:(['\"])" + FONT_SIZE + r"\1|(\d*\.?\d+)(?![\w.]))", code):
        font_size(m.start(), m.group(0), m.group(2) or m.group(5), m.group(3) or ("px" if m.group(5) else None), m.group(4))
    for m in re.finditer(r"(?<![\w.-])font\s*:\s*(?:(['\"`])(.*?)\1|([^;{}'\"`]+))|\.font\s*=\s*(['\"`])(.*?)\4", code):
        value = m.group(2) if m.group(1) else m.group(3) if m.group(3) is not None else m.group(5)
        shorthand = FONT_SHORTHAND.match(value)
        if shorthand is None:
            continue  # a system font keyword, inherit, or an expression
        styles, family = shorthand.group(1), shorthand.group(5)
        if re.search(r"\b(?:italic|oblique)\b", styles):
            add(m.start(), "error", "italic", "No italics on a console.", "Typography")
        if re.search(r"\b(?:bold|bolder|[6-9]00)\b", styles):
            add(m.start(), "check", "bold", "Bold is for display titles only; emphasis on a console means an alert.", "Typography")
        if m.group(4):  # canvas text
            if shorthand.group(3) == "px":
                add(m.start(), "check", "canvas-font", "Canvas font in px: derive it from the root font size (interface scale 80–150%), at least 0.875rem; readable text belongs in the DOM.", "Typography")
            continue
        font_size(m.start(), m.group(0).strip(), shorthand.group(2), shorthand.group(3), shorthand.group(4))
        if "var(--font-" not in family:
            add(m.start(), "error", "font-family", f"font family '{family.strip()}': use var(--font-sans) or var(--font-mono).", "Typography")
    if css:
        for m in re.finditer(r"^(?!\s*@media).*$", code, re.M):
            for px in re.finditer(r"(?<![\w.-])(\d+(?:\.\d+)?)px\b", m.group(0)):
                if float(px.group(1)) > HAIRLINE_MAX_PX:
                    add(m.start() + px.start(1), "check", "px-length", f"{px.group(1)}px: size in rem so the 80–150% interface scale holds.", "Accessibility")
    if tsx:  # a bare number in a React style object is px
        for m in STYLE_LENGTH.finditer(code):
            number = m.group(1) or m.group(3)
            if float(number) > HAIRLINE_MAX_PX:
                add(m.start(), "check", "px-length", f"{number}px: size in rem so the 80–150% interface scale holds.", "Accessibility")

    for m in re.finditer(r"font-style\s*:\s*(?:italic|oblique)|fontStyle\s*:\s*['\"](?:italic|oblique)|(?<![\w$)\]])<(?:em|i)[\s>]", code):
        add(m.start(), "error", "italic", "No italics on a console.", "Typography")
    for m in re.finditer(r"font-weight\s*:\s*(?:bold|bolder|[6-9]00)|fontWeight\s*:|(?<![\w$)\]])<(?:strong|b)[\s>]", code):
        add(m.start(), "check", "bold", "Bold is for display titles only; emphasis on a console means an alert.", "Typography")
    for m in re.finditer(r"font-family\s*:\s*([^;}]+)|fontFamily\s*:\s*(['\"`])(.*?)\2", code):
        family = m.group(1) if m.group(1) is not None else m.group(3)
        if "var(--font-" not in family:
            add(m.start(), "error", "font-family", f"font-family '{family.strip()}': use var(--font-sans) or var(--font-mono).", "Typography")
    for m in re.finditer(r"fonts\.googleapis|fonts\.gstatic|@import\s+url\(\s*['\"]?https?:|url\(\s*['\"]?https?://[^)]*\.(?:woff2?|ttf|otf)", code):
        add(m.start(), "error", "remote-font", "Fonts are bundled through @fontsource; never fetch one at runtime.", "Typography")
    for m in re.finditer(r"text-transform\s*:\s*(lowercase|capitalize)|textTransform\s*:\s*['\"](lowercase|capitalize)", code):
        add(m.start(), "check", "case", f"text-transform: {m.group(1) or m.group(2)}: labels are upper case, sentences mixed case.", "Typography")

    radii = [(m.start(), m.group(1)) for m in re.finditer(r"border(?:-\w+-\w+)?-radius\s*:\s*([^;}]+)", code)]
    radii += [
        (m.start(), m.group(1) if m.group(1) is not None else m.group(2) + "px")
        for m in re.finditer(r"border(?:[A-Z][a-z]+[A-Z][a-z]+)?Radius\s*:\s*(?:['\"]([^'\"]*)['\"]|(\d+(?:\.\d+)?)(?![\w.]))", code)
    ]
    for index, value in radii:
        for v in re.finditer(r"([\d.]+)(px|rem|em|%)", value):
            if v.group(2) == "%" or length_px(float(v.group(1)), v.group(2)) > CORNER_MAX_PX:
                add(index, "check", "radius", f"border-radius {value.strip()}: panels and controls have square or 2px corners.", "Layout")
                break
    for m in re.finditer(r"overflow-x\s*:\s*(auto|scroll)|overflowX\s*:\s*['\"](auto|scroll)", code):
        add(m.start(), "error", "scroll", "Never scroll horizontally.", "Layout")
    for m in re.finditer(r"overflow(?:-y)?\s*:\s*(auto|scroll)\b|overflowY?\s*:\s*['\"](auto|scroll)", code):
        add(m.start(), "check", "scroll", "Scrolling only inside lists, logs and procedures, showing position and total (12-24 of 87).", "Layout")
    for m in re.finditer(r"writing-mode\s*:|writingMode\s*:|transform\s*:\s*['\"]?rotate\(", code):
        add(m.start(), "check", "vertical-label", "Labels are horizontal.", "Layout")

    for m in re.finditer(r"transition(-duration|Duration)?\s*:\s*(?:(['\"`])(.*?)\2|([^;{}'\"`]+))", code):
        value = m.group(3) if m.group(2) else m.group(4)
        for part in re.split(r",(?![^()]*\))", value):  # one transition per comma, outside cubic-bezier()
            times = re.findall(r"(?<![\w.-])(\d*\.?\d+)(ms|s)\b", part)
            for number, unit in times if m.group(1) else times[:1]:  # a second time in the shorthand is the delay
                ms = float(number) * (1000 if unit == "s" else 1)
                if ms > TRANSITION_MAX_MS:
                    add(m.start(), "error", "motion", f"Transition of {number}{unit}: state transitions take 80–150 ms.", "Motion and sound")
                elif 1 <= ms < TRANSITION_MIN_MS:  # under 1 ms is the usual way to switch a transition off
                    add(m.start(), "check", "motion", f"Transition of {number}{unit}: state transitions take 80–150 ms; shorter reads as a flicker.", "Motion and sound")
    for m in re.finditer(r"@keyframes\s+([\w-]+)|\banimation(?:-name|Name)?\s*:|\.animate\(|\binfinite\b", code):
        add(m.start(), "check", "motion", "Animation: only to show a change of state; flashing only for emergency and warning alerts (0.8 Hz text, 3 Hz small indicators, synchronised, 10 s cap).", "Motion and sound")
    for m in re.finditer(r"(?<![.\w])(?:window\.)?(alert|confirm|prompt)\(", bare):  # calls, not text or strings
        add(m.start(), "error", "modal", f"{m.group(1)}(): no modal alert boxes; alerts are server-raised and shown in the alert list.", "Alerts")
    for m in re.finditer(r"<dialog\b|\.showModal\(", code):
        add(m.start(), "check", "modal", "Modal dialogs only for an ARM/EXECUTE decision.", "Never")
    for m in re.finditer(r"\bonContextMenu\b", code):
        add(m.start(), "check", "pointer", "No action may depend on a right click alone.", "Controls and commanding")
    for m in re.finditer(r"\bon(?:MouseEnter|MouseOver|PointerEnter|PointerOver)\b", code):
        add(m.start(), "check", "pointer", "No action may depend on hover; consoles may run on touch screens.", "Controls and commanding")
    for m in re.finditer(r"outline(?:-style|Style)?\s*:\s*['\"]?(?:none|0(?:px)?)(?![\w.])|outline(?:-width|Width)\s*:\s*['\"]?0(?:px)?(?![\w.])", code):
        add(m.start(), "check", "focus", "Focus is always visible as a 2px --accent outline.", "Controls and commanding")
    for m in re.finditer(r"\.toFixed\(|\.toPrecision\(|\.toLocaleString\(|new Intl\.NumberFormat\(", code):
        if tsx:
            add(m.start(), "check", "number-format", "Format values through one shared formatter so a quantity has the same unit and precision everywhere.", "Numbers, units and time")
    for m in WEB_IDIOM.finditer(code):
        add(m.start(), "check", "web-idiom", f"'{m.group(0)}' in a name: no spinners, skeleton loaders, toasts or hamburger menus.", "Never")
    for m in UPPER_SUBSTANCE.finditer(code) if not css and not is_developer_page(path) else ():  # `${…}` too
        add(m.start(), "check", "formula-case", "Upper-cases a substance's name, where a formula keeps its case even in an upper-case label (Co is cobalt, CO carbon monoxide): ask substanceName(key, \"upper\") for the label's form.", "Typography")

    if css:
        # Keeping a state's colour on hover is not a hover effect: flag a status colour only when a
        # hover or focus rule brings it to a class that neither names that state nor carries it.
        carriers = status_carriers(shared_sheet(root))
        for name, statuses in status_carriers(code).items():
            carriers.setdefault(name, set()).update(statuses)
        for block in re.finditer(r"([^{}]+)\{([^{}]*)\}", code):
            selector, body = block.group(1), block.group(2)
            statuses = set(re.findall(r"--status-(\w+)", body))
            for part in selector.split(",") if statuses else []:
                if not STATE_SELECTOR.search(part):
                    continue
                classes = re.findall(r"\.([a-zA-Z][\w-]*)", part)
                introduced = sorted(s for s in statuses if not any(s in c or s in carriers.get(c, set()) for c in classes))
                if introduced:
                    names = ", ".join(f"--status-{s}" for s in introduced)
                    add(block.start(2), "error", "reserved-colour", f"{names} in a hover/focus/active style for a class that does not otherwise carry it: status colours are reserved for the state they name.", "Colour")
                    break
            if "--status-nominal" in body:
                add(block.start(2), "check", "reserved-colour", "Green is only for a state the operator waits to confirm; a normal reading is plain --text.", "Colour")
    return found


def is_formula(formula: str) -> bool:
    """Whether every symbol in `formula`, as written, is an element's."""
    symbols = SYMBOL.findall(formula)
    return bool(symbols) and all(s in ELEMENTS for s in symbols)


def is_count(digits: str) -> bool:
    """A formula's count of atoms: 2 or more, with no leading zero (`SubstanceKey`'s grammar)."""
    return digits.isdigit() and not digits.startswith("0") and int(digits) >= 2


def formula_symbols(word: str, recased: bool) -> int:
    """The most element symbols `word` parses into as a formula, each with an optional count with
    no leading zero, or 0 when it is no formula. With `recased`, a symbol may be in any case, as in
    a formula that was upper- or lower-cased."""
    n = len(word)

    @cache
    def best(i: int) -> int:  # symbols in the best parse of word[i:]; -1 when there is none
        if i == n:
            return 0
        found = -1
        for size in (1, 2):
            letters = word[i : i + size]
            if len(letters) < size or not letters.isalpha():
                continue
            if recased:
                symbol = letters[0].upper() + letters[1:].lower()
            elif letters[0].isupper() and (size == 1 or letters[1].islower()):
                symbol = letters
            else:
                continue
            j = i + size
            if symbol not in ELEMENTS or (not recased and size == 1 and j < n and word[j].islower()):
                continue  # as written, a small letter after a capital belongs to its symbol
            k = j
            while k < n and word[k].isdigit():
                k += 1
            if k > j and word[j] == "0":
                continue
            if (rest := best(k)) >= 0:
                found = max(found, rest + 1)
        return found

    return max(best(0), 0)


@cache
def registry_formulas(root: Path) -> dict[str, str]:
    """The registry's formula keys (`SUBSTANCE_FIXTURE`), each by its upper-case form; empty when
    the fixture is missing or unreadable, as in a checkout without the protocol package."""
    try:
        rows = json.loads((root / SUBSTANCE_FIXTURE).read_text(encoding="utf-8"))["substances"]
        keys = [row["key"] for row in rows]
    except (OSError, ValueError, KeyError, TypeError):
        return {}
    return {key.upper(): key for key in keys if isinstance(key, str) and FORMULA_KEY.fullmatch(key)}


def typography(text: str, at: Callable[[int], int], excerpt: str, formulas: dict[str, str]) -> tuple[list[Finding], list[tuple[int, int]]]:
    """Formulas, ions, isotopes, state symbols, fractions, letters' names, notation, signs,
    temperatures and code names in one displayed text (signoff-4 item 7's Typography bullets;
    signoff-5 item 8; decision-ux-numbers-names). `formulas` maps each registry formula key,
    upper-cased, to the key. Returns the findings and the spans whose missing glyphs a finding here
    already names, which the glyph check skips."""
    found: list[Finding] = []
    named: list[tuple[int, int]] = []

    def add(index: int, severity: str, rule: str, message: str, section: str) -> None:
        found.append(Finding(at(index), severity, rule, message, section))

    for m in SCRIPT_FORMULA.finditer(text):
        formula, sign = m.group(1), m.group(2) or ""
        counts = re.findall(f"{SUBSCRIPT_DIGIT}+", formula)
        if not is_formula(formula) or not (counts or sign) or not all(is_count(c.translate(ON_THE_LINE)) for c in counts):
            continue  # `N₀` is a symbol's subscript, which the glyph check names
        named.append(m.span())
        written, symbols = m.group(0).translate(ON_THE_LINE), SYMBOL.findall(formula)
        if len(sign) > 1:
            add(m.start(), "error", "ion-charge", f"'{m.group(0)}' in '{excerpt}': every ion the simulation holds is singly charged, its sign on the line (Fe+, H−); a charge number is not written, since Fe3+ on the line would read as three atoms.", "Voice and nomenclature")
        elif len(symbols) == 1 and counts and not sign:  # `V₂` may be a symbol's subscript
            add(m.start(), "error", "formula", f"'{m.group(0)}' in '{excerpt}': B612 has no subscripts. A formula's digits count atoms and stand on the line ({written}); a symbol's subscript is written in parentheses ({symbols[0]}({counts[0].translate(ON_THE_LINE)})).", "Typography")
        else:
            add(m.start(), "error", "formula", f"'{m.group(0)}' in '{excerpt}': a formula's digits count atoms and stand on the line, as an ion's sign does: {written}.", "Typography" if counts else "Voice and nomenclature")
    for m in SCRIPT_ELECTRON.finditer(text):
        named.append(m.span())
        add(m.start(), "error", "formula", f"'{m.group(0)}' in '{excerpt}': the electron is e−, its sign on the line.", "Voice and nomenclature")
    for m in PAREN_FORMULA.finditer(text):
        formula = m.group(1)
        counts = re.findall(r"\(([0-9]+)\)", formula)
        if counts and len(SYMBOL.findall(formula)) >= 2 and is_formula(formula) and all(is_count(c) for c in counts):
            fixed = re.sub(r"\(([0-9]+)\)", r"\1", formula)
            add(m.start(), "error", "formula", f"'{formula}' in '{excerpt}': a formula's digits count atoms and are not a symbol's subscript, so never in parentheses: {fixed}.", "Typography")
    for m in STATE_SYMBOL.finditer(text):
        formula, state = m.group(1), m.group(2)
        if not is_formula(formula):
            continue
        if len(SYMBOL.findall(formula)) == 1 and not re.search(r"[0-9]", formula):  # `P(s)` may be a symbol's subscript
            add(m.start(), "check", "state-symbol", f"'{m.group(0)}' in '{excerpt}': if ({state}) is a phase, write it as a word beside the name (liquid CH4); a parenthesis after a symbol holds its subscript.", "Voice and nomenclature")
        else:
            add(m.start(), "error", "state-symbol", f"'{m.group(0)}' in '{excerpt}': a phase is a word beside the name, never a state symbol (liquid CH4, not CH4(l)), since a parenthesis after a symbol holds its subscript.", "Voice and nomenclature")
    for m in SCRIPT_ISOTOPE.finditer(text):
        mass, symbol = m.group(1).translate(ON_THE_LINE), m.group(2)
        if symbol in ELEMENTS and not mass.startswith("0"):
            named.append(m.span(1))
            add(m.start(), "error", "isotope", f"'{m.group(0)}' in '{excerpt}': an isotope takes its mass number after a hyphen, {symbol}-{mass}, even where ¹, ² and ³ could set it.", "Typography")
    for m in LINE_ISOTOPE.finditer(text):
        mass, symbol = int(m.group(1)), m.group(2)
        z = ATOMIC_NUMBER.get(symbol)
        if z is not None and z <= mass <= 3 * z + 6:
            add(m.start(), "check", "isotope", f"'{m.group(0)}' in '{excerpt}': an isotope takes its mass number after a hyphen ({symbol}-{mass}); a value and its unit take a space ({mass} {symbol}).", "Typography")
    for m in LINE_CHARGE.finditer(text):
        symbol, count = m.group(1), m.group(2)
        if symbol in ELEMENTS and symbol not in {"H", "C", "N", "O"}:
            add(m.start(), "check", "ion-charge", f"'{m.group(0)}' in '{excerpt}': on the line it reads as {count} atoms of {symbol}; every ion the simulation holds is singly charged, its sign on the line ({symbol}+).", "Voice and nomenclature")
    for m in DOUBLED_SIGN.finditer(text):
        if is_formula(m.group(1)):
            add(m.start(), "check", "ion-charge", f"'{m.group(0)}' in '{excerpt}': every ion the simulation holds is singly charged, its sign on the line once ({m.group(1)}+).", "Voice and nomenclature")
    for m in HYPHEN_CHARGE.finditer(text):
        if m.group(1) == "e" or is_formula(m.group(1)):
            add(m.start(), "check", "ion-charge", f"'{m.group(0)}' in '{excerpt}': if it is an ion, its minus is the minus sign U+2212 ({m.group(1)}−), not a hyphen.", "Voice and nomenclature")

    for m in CASE_WORD.finditer(text):
        word = m.group(0)
        if sum(c.isalpha() for c in word) < 2 or not re.search(r"[A-Z]", word):
            continue
        if (known := formulas.get(word.upper())) is not None:
            if word != known:
                add(m.start(), "check", "formula-case", f"'{word}' in '{excerpt}' is the formula {known} with its case changed: a formula keeps its case, even in an upper-case label (Co is cobalt, CO carbon monoxide).", "Typography")
            continue
        if not re.search(r"[0-9]", word):
            continue
        if re.fullmatch(r"Co[0-9]+", word):
            add(m.start(), "check", "formula-case", f"'{word}' in '{excerpt}': Co is cobalt where CO is carbon monoxide; a formula keeps its case (CO2).", "Typography")
        elif (
            formula_symbols(word, recased=False) == 0
            and formula_symbols(word, recased=True) >= 2
            and (not word.isupper() or len(re.findall(r"[0-9]+", word)) >= 2)  # `TANK2` is a designator
        ):
            add(m.start(), "check", "formula-case", f"'{word}' in '{excerpt}' reads as a chemical formula with its case changed: a formula keeps its case, even in an upper-case label (Co is cobalt, CO carbon monoxide).", "Typography")
    if co := re.search(r"(?<![\w$.\-])Co(?![\w\-])", text):
        words = [w for w in re.findall(r"[A-Za-z][A-Za-z0-9]*", text) if w != "Co"]
        if any(w.isupper() and len(w) > 1 for w in words) and all(w.isupper() or formula_symbols(w, recased=False) for w in words):
            add(co.start(), "check", "formula-case", f"'Co' in the upper-case '{excerpt}': Co is cobalt where CO is carbon monoxide, and a formula keeps its case.", "Typography")

    qualified = list(QUALIFIED_FRACTION.finditer(text))
    for m in qualified:
        add(m.start(), "error", "fraction", f"'{' '.join(m.group(0).split())}' in '{excerpt}': a fraction is a quantity named in words, with nothing attached to % or to any unit (mass fraction 75 %; never 75 wt%, 75 % w/w, 75% by mass or ppmv).", "Voice and nomenclature")
    if not CSS_FUNCTION.search(text):
        for m in UNSPACED_PERCENT.finditer(text):
            if not any(q.start() < m.end() and m.start() < q.end() for q in qualified):
                add(m.start(), "check", "fraction", f"'{m.group(0)}' in '{excerpt}': a space comes before % (75 %).", "Voice and nomenclature")
    for m in SNAKE_KEY.finditer(text):
        add(m.start(), "error", "code-name", f"'{m.group(0)}' in '{excerpt}': a key with an underscore, or any other code name, never reaches the screen; show its name in words.", "Voice and nomenclature")
    for m in UNDERSCORE_SUBSCRIPT.finditer(text):
        add(m.start(), "check", "notation", f"'{m.group(0)}' in '{excerpt}': a symbol's subscript is written in parentheses, {m.group(1)}({m.group(2)}), or the quantity is named in words.", "Numbers, units and time")

    pairs = []
    for m in LETTER_PAIR.finditer(text):
        if m.group(1) not in LETTER_ABBREVIATIONS and (m.group(1).lower(), m.group(2).lower()) != ("delta", "v"):  # DELTA-V
            pairs.append(m.span())
            add(m.start(), "check", "letter-name", f"'{m.group(0)}' in '{excerpt}': a symbol that has a name in words is never spelled out letter by letter (phase shift, not delta phi); a letter's name stays only where it is the literature's own (DELTA-V, H-alpha).", "Typography")
    for m in LETTER_VALUE.finditer(text):
        if m.group(1) not in LETTER_ABBREVIATIONS and not any(a <= m.start() < b for a, b in pairs):
            add(m.start(), "check", "letter-name", f"'{m.group(1)}' as a value in '{excerpt}': name the quantity in words, as the literature names it (single-scattering albedo, not omega), never by its letter's name.", "Typography")
    for m in POWER.finditer(text):
        add(m.start(), "check", "notation", f"'{m.group(0)}' in '{excerpt}': a power of ten is written in E notation (1E-4), and a unit's power as the guide composes units (/cm², W/m²).", "Typography")
    for m in TIMES_TEN.finditer(text):
        add(m.start(), "check", "notation", f"'{m.group(0)}' in '{excerpt}': a power of ten is written in E notation (1.20E-6).", "Typography")
    for m in RAISED_TEN.finditer(text):
        add(m.start(), "check", "notation", f"'{m.group(0)}' in '{excerpt}': a power of ten is written in E notation (1E3), even where ¹, ² and ³ could set it.", "Typography")
    for m in SMALL_E.finditer(text):
        add(m.start(), "check", "notation", f"'{m.group(0)}' in '{excerpt}': E notation takes a capital E, as the guide writes it (5.20E10, 1E-4).", "Numbers, units and time")
    for m in RELATION.finditer(text):
        add(m.start(), "check", "notation", f"'{m.group(0)}' in '{excerpt}': a relation is written as < or >, or in words.", "Typography")

    for m in MINUS_SIGN_NUMBER.finditer(text):
        add(m.start(), "check", "sign", f"The minus sign U+2212 before a number in '{excerpt}': a number's sign is the hyphen-minus, - (decision-ux-numbers-names); U+2212 is an ion's charge (H−) or an operator between two terms.", "Numbers, units and time")
    for m in CELSIUS.finditer(text):
        add(m.start(), "check", "temperature", f"'{m.group(0)}' in '{excerpt}': a temperature is in kelvin, K (310 K), never °C (decision-ux-numbers-names).", "Numbers, units and time")
    return found, named


def call_of(expression: str, index: int) -> str | None:
    """The name of the call whose argument list holds `index` directly (`phaseWord` for
    `phaseWord(layer.material)`), or None when `index` is in no call's arguments or deeper, inside
    an arrow function's body or a nested bracket."""
    depth, j = 0, index - 1
    while j >= 0:
        c = expression[j]
        if c in ")]}":
            depth += 1
        elif c in "([{":
            if depth == 0:
                if c != "(":
                    return None
                head = expression[:j]
                between = expression[j + 1 : index]
                if "=>" in between:
                    return None  # an arrow function's body: it returns the key
                callee = re.search(r"([A-Za-z_$][\w$]*)\s*$", head)
                return callee.group(1) if callee and not re.search(r"(?<![\w$.])(?:if|while|for|switch|return|typeof|await)\s*$", head) else None
            depth -= 1
        j -= 1
    return None


def substance_keys(expression: str) -> list[tuple[int, str]]:
    """(offset, chain) of each member chain in a `${…}` or JSX `{…}` expression that prints a
    substance's registry key: its last name ends in material, species or substance (or their
    plurals and keys), and it is neither passed to substanceName() or any other call that names it,
    nor an index, a comparison, a call or an attribute that is never shown. `expression` has its
    string literals blanked."""
    keys: list[tuple[int, str]] = []
    for m in MEMBER_CHAIN.finditer(expression):
        chain = re.sub(r"\s+", "", m.group(0))
        before, after = expression[: m.start()], expression[m.end() :]
        names = re.split(r"\?\.|!\.|\.", chain)
        # `layer.material.toUpperCase()` and `absorber.species.join(", ")` still print the key.
        transformed = len(names) > 1 and names[-1] in STRING_METHODS and after.lstrip().startswith("(")
        if not SUBSTANCE_NAME_END.search(names[-2 if transformed else -1]):
            continue
        if (not transformed and NOT_PRINTED_AFTER.match(after)) or NOT_PRINTED_BEFORE.search(before):
            continue
        if before.rstrip().endswith("[") and after.lstrip().startswith("]"):
            continue  # an index into a table of words
        if re.search(r"\b(?:key|className|class|id|htmlFor|data-[\w-]+)\s*=\s*\{\s*$", before):
            continue  # an attribute that is never shown
        if "." not in chain and re.search(r"[{,]\s*$", before) and re.match(r"\s*[:,}]", after):
            continue  # an object's key, or its shorthand
        if (callee := call_of(expression, m.start())) is not None and callee not in PASS_THROUGH_CALLS:
            continue  # substanceName(), or another call that turns the key into words
        keys.append((m.start(), chain))
    return keys


def check_text(texts: list[tuple[int, str, bool]], unchecked: dict[str, str], formulas: dict[str, str]) -> list[Finding]:
    """Voice, missing-value, emoji, glyph and typography rules over displayed text, each a (line,
    text, developer) triple; developer text skips the typography rules, and `formulas` is
    registry_formulas()'s. A text may span lines, so
    each finding takes the line of its match. Characters the glyph check could not look up (fonts
    missing) are added to `unchecked` with the reason."""
    found: list[Finding] = []
    for line, text, developer in texts:
        stripped = text.strip()
        excerpt = " ".join(text.split())[:50]

        def at(index: int) -> int:
            return line + text.count("\n", 0, index)

        typed, named = ([], []) if developer or CODE_LIKE.search(text) else typography(text, at, excerpt, formulas)
        found += typed

        banned = list(BANNED_PHRASE.finditer(text))
        for m in banned:
            found.append(Finding(at(m.start()), "error", "voice", f"'{' '.join(m.group(0).split())}' in '{excerpt}': terse, literal, impersonal (NO CARRIER, not 'Loading…').", "Voice and nomenclature"))
        for m in WEAK_PHRASE.finditer(text):
            found.append(Finding(at(m.start()), "check", "voice", f"'{m.group(0)}' in '{excerpt}': web or game phrasing; state the fact, as in DOCKED or NO CARRIER.", "Voice and nomenclature"))
        if m := re.search(r"[A-Za-z0-9)}]!(?:\s|$)", text):
            found.append(Finding(at(m.start()), "error", "voice", f"Exclamation mark in '{excerpt}'.", "Voice and nomenclature"))
        if len(text.split()) > 1:  # a single word such as US is more likely an abbreviation
            if m := PERSON_NAMED.search(text):
                found.append(Finding(at(m.start()), "error", "voice", f"Second or first person '{m.group(0)}' in '{excerpt}': the ship never speaks as a person.", "Voice and nomenclature"))
            elif (m := PERSON_OTHER.search(text)) and (m.group(0) != "I" or re.search(r"[a-z]", text)):
                found.append(Finding(at(m.start()), "check", "voice", f"First or second person in '{excerpt}': the ship never speaks as a person.", "Voice and nomenclature"))
        if (m := re.search(r"…|\.\.\.", text)) and not any(re.search(r"…|\.\.\.", b.group(0)) for b in banned):
            found.append(Finding(at(m.start()), "check", "voice", f"Ellipsis in '{excerpt}': no 'thinking…' or 'Loading…'.", "Voice and nomenclature"))
        if MISSING_NAMED.fullmatch(stripped):
            found.append(Finding(at(text.index(stripped)), "error", "missing-value", f"'{stripped}' as a value: a missing value is an em dash — in --text-muted.", "Data states"))
        elif MISSING_OTHER.fullmatch(stripped):
            found.append(Finding(at(text.index(stripped)), "check", "missing-value", f"'{stripped}' as a value: if it stands for a missing value, show an em dash — in --text-muted.", "Data states"))
        for m in EMOJI.finditer(text):
            found.append(Finding(at(m.start()), "error", "emoji", f"Emoji U+{ord(m.group(0)):04X}: never on a console; name the thing in words (only a unit symbol or a key's own sign is drawn, as an inline SVG).", "Never"))
        for ch in dict.fromkeys(c for c in text if ord(c) > 0x7F and not EMOJI.match(c)):
            outside = [i for i, c in enumerate(text) if c == ch and not any(a <= i < b for a, b in named)]
            if not outside:
                continue  # every use is in a formula or isotope that a typography finding names
            try:
                lacking = glyphs.missing(ch)
            except glyphs.FONT_ERRORS as err:
                unchecked.setdefault(ch, str(err))
                continue
            space = unicodedata.category(ch) in SPACE_CATEGORIES
            shown = f"U+{ord(ch):04X} {unicodedata.name(ch, '?')}" if space else f"'{ch}' (U+{ord(ch):04X})"
            if len(lacking) == 2 and space:
                found.append(Finding(at(outside[0]), "error", "space", f"{shown} is in neither B612 nor B612 Mono, which hold only the space U+0020 and the no-break U+00A0: write one of those.", "Typography"))
            elif len(lacking) == 2:
                found.append(Finding(at(outside[0]), "error", "glyph", f"{shown} is not in B612 or B612 Mono: name it in words, or draw it as an inline SVG if it is a unit symbol or a key's sign.", "Typography"))
            elif lacking:
                found.append(Finding(at(outside[0]), "check", "space" if space else "glyph", f"{shown} is missing from {lacking[0]}.", "Typography"))
    return found


def brace_end(bare: str, i: int) -> int:
    """Index after the brace that closes the one at `i`; the end of the file when none does."""
    depth = 0
    for j in range(i, len(bare)):
        depth += {"{": 1, "}": -1}.get(bare[j], 0)
        if depth == 0:
            return j + 1
    return len(bare)


def check_substance_keys(code: str, expressions: list[tuple[int, int]], strings: list[tuple[int, int, str]], root: Path) -> list[Finding]:
    """A substance's registry key printed raw from a `${…}` or JSX `{…}` expression, each given as
    its (start, end) in `code`. The check needs no substanceName() to exist: until
    lib/substances.ts lands (P14.T49.f), the message says so."""
    built = (root / SUBSTANCE_MODULE).is_file()
    namer = "substanceName() (lib/substances.ts)" if built else "substanceName() (lib/substances.ts, P14.T49.f, once it lands)"
    found: list[Finding] = []
    for a, b in expressions:
        expression = list(code[a:b])
        for s, e, _ in strings:
            if a <= s and e <= b:
                blank(expression, s - a, e - a)  # a nested literal is read as a string of its own
        for offset, chain in substance_keys("".join(expression)):
            shown = f"${{{chain}}}" if code[a - 2 : a] == "${" else f"{{{chain}}}"
            found.append(Finding(line_of(code, a + offset), "check", "substance-key",
                f"'{shown}' prints a substance's registry key: name the substance through {namer}, never by its key (decision-substance-names).",
                "Voice and nomenclature"))
    return found


def lint(path: Path, root: Path, unchecked: dict[str, str]) -> list[Finding]:
    src = path.read_text(encoding="utf-8")
    lang = {".css": "css", ".html": "html"}.get(path.suffix, "js")
    developer = is_developer_page(path)
    code, bare, strings, substitutions = scan(src, lang)
    groups = jsx_text(bare) if path.suffix == ".tsx" else html_text(bare) if lang == "html" else []
    texts = [(line, text, developer) for line, text in text_nodes(code, groups)]
    # A JSX child expression lies between two runs of its element's text; a displayed attribute's
    # expression runs to its closing brace, found in `bare`, where no string can hold one.
    expressions = [(prev_end + 1, start - 1) for runs in groups if lang == "js" for (_, prev_end), (start, _) in zip(runs, runs[1:])]
    if path.suffix == ".tsx":
        expressions += [(m.end(), brace_end(bare, m.end() - 1) - 1) for m in DISPLAYED_JSX_ATTRIBUTE.finditer(bare)]
    in_text = bytearray(len(src))
    rule_code, rule_bare = list(code), list(bare)
    for start, end in (run for runs in groups for run in runs):
        in_text[start:end] = b"\x01" * (end - start)
        blank(rule_code, start, end)
        blank(rule_bare, start, end)
    code, bare = "".join(rule_code), "".join(rule_bare)
    root_blocks = [m.span() for m in re.finditer(r":root\s*\{[^}]*\}", code)] if is_shared_sheet(path, root) else []
    findings = check_code(path, code, bare, root_blocks, root) + check_stylesheet_scope(path, code, root)
    if lang == "css":
        texts += [(line, text, developer) for line, text in css_content(bare, strings)]
    else:
        shown = ui_strings(lang, bare, strings, in_text)
        texts += [(line, text, developer or provenance) for _, line, text, provenance in shown]
        # A displayed template literal's `${…}`, when words stand around it: `${a}-${b}` is a key.
        expressions += [
            span
            for start, _, text, provenance in shown
            if not provenance and re.search(r"\s", text.replace("{}", ""))
            for span in substitutions.get(start, [])
        ]
    if lang == "js" and not developer:
        findings += check_substance_keys(code, expressions, strings, root)
    findings += check_text(texts, unchecked, registry_formulas(root))
    return list({(f.line, f.rule, f.message): f for f in findings}.values())


# --------------------------------------------------------------------------- driver


def repo_root() -> Path:
    proc = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    return Path(proc.stdout.strip()) if proc.returncode == 0 else Path(__file__).resolve().parents[4]


def usage_error(message: str) -> NoReturn:
    print(f"ux_lint: {message}\nusage: ux_lint.py [--all | PATH...]", file=sys.stderr)
    sys.exit(2)


def is_source(path: Path, base: Path) -> bool:
    """A file the lint reads: not a test, a test helper, a declaration file or a dependency.
    Directories are judged below `base`, so a checkout under ~/test/ still counts."""
    parts = path.relative_to(base).parts if path.is_relative_to(base) else path.parts
    return (
        path.suffix in EXTENSIONS
        and not re.search(r"\.(?:test|spec)\.tsx?$|\.d\.ts$", path.name)
        and not {"test", "tests", "__tests__", "node_modules"} & set(parts)
    )


def wanted(path: Path, root: Path) -> bool:
    return is_source(path, root) and RENDERER in path.as_posix()


def targets(root: Path, args: list[str]) -> list[Path]:
    if unknown := [a for a in args if a.startswith("-") and a != "--all"]:
        usage_error(f"unknown option {unknown[0]}")
    if "--all" in args:
        if len(args) > 1:
            usage_error("--all takes no paths")
        return sorted(p for p in (root / RENDERER).rglob("*") if p.is_file() and wanted(p, root))
    if args:
        paths: list[Path] = []
        for a in args:
            p = Path(a).resolve()
            if not p.exists():
                p = root / a
            if not p.exists():
                usage_error(f"no such file or directory: {a}")
            base = root if p.is_relative_to(root) else p if p.is_dir() else p.parent
            found = sorted(q for q in p.rglob("*") if q.is_file()) if p.is_dir() else [p]
            paths += [q for q in found if is_source(q, base)]
        return list(dict.fromkeys(paths))
    names = subprocess.run(["git", "-C", str(root), "diff", "--name-only", "HEAD"], capture_output=True, text=True).stdout.split()
    names += subprocess.run(["git", "-C", str(root), "ls-files", "--others", "--exclude-standard"], capture_output=True, text=True).stdout.split()
    return sorted({root / n for n in names if (root / n).exists() and wanted(root / n, root)})


def main() -> None:
    args = sys.argv[1:]
    if args and args[0] in ("-h", "--help"):
        print(__doc__)
        return
    root = repo_root()
    files = targets(root, args)
    if not files:
        print("ux_lint: no renderer files to check (pass paths, or --all).")
        return

    errors = checks = 0
    unchecked: dict[str, str] = {}
    for path in files:
        findings = lint(path, root, unchecked)
        if not findings:
            continue
        print(path.relative_to(root) if path.is_relative_to(root) else path)
        for f in sorted(findings, key=lambda f: (f.line, f.severity != "error")):
            print(f"  {f.line:>4}: {f.severity:<5} [{f.rule}] {f.message} (guide: {f.section})")
            errors += f.severity == "error"
            checks += f.severity == "check"
    print(f"\n{errors} error(s), {checks} check(s) in {len(files)} file(s) scanned. Heuristic: confirm each line against the guide.")
    if unchecked:
        sys.stdout.flush()  # the warning goes after the findings, not before them
        reason = next(iter(unchecked.values()))
        print(f"ux_lint: glyph check did not run for {len(unchecked)} character(s) ({' '.join(unchecked)}): {reason}", file=sys.stderr)
        sys.exit(2)
    sys.exit(1 if errors else 0)


if __name__ == "__main__":
    main()
