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
           under 0.875rem, emoji, a glyph B612 lacks, horizontal scroll, a modal alert, "please",
           "Loading…", "!", second person, NaN or null as a value, ...)
    check  needs judgement (a 50% radius may be a contact symbol; a hover handler may have a
           keyboard twin; "SUCCESSFUL" or a lone "-" may be fine in context); resolve it or leave a
           code comment stating the reason

It cannot see rendering, layout at 1280x720, data states or closed-loop commanding: those need
reading and running the code. Exit status is 1 when there is any error, and 2 on a usage error (an
unknown option, a path that does not exist) or when the glyph check could not run because the
bundled fonts are missing or unreadable (run `pnpm install`); 2 wins over 1.
"""

from __future__ import annotations

import html
import re
import subprocess
import sys
from dataclasses import dataclass
from functools import cache
from pathlib import Path
from typing import NoReturn

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


def scan(src: str, lang: str) -> tuple[str, str, list[tuple[int, int, str]]]:
    """Split source into code and string literals, keeping every offset so line numbers hold.

    Returns `code` (comments and regex-literal bodies blanked, strings intact), `bare` (string
    contents blanked too, delimiters kept) and the literals as (start, end, text), with each
    `${…}` shown as `{}`. `lang` is "js", "css" or "html": CSS has no line comments (`//` in a
    url() is not one) and HTML has only `<!-- -->`. A quote right after a letter or digit is an
    apostrophe in text (`<p>Don't vent</p>`), not the start of a string."""
    code, bare = list(src), list(src)
    strings: list[tuple[int, int, str]] = []
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
    return "".join(code), "".join(bare), strings


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


def ui_strings(lang: str, bare: str, strings: list[tuple[int, int, str]], in_text: bytearray) -> list[tuple[int, str]]:
    """String literals an operator may read: labels and sentences rather than keys, paths, class
    names, values compared against or developer messages."""
    texts: list[tuple[int, str]] = []
    for start, end, raw in strings:
        if in_text[start]:
            continue  # a quoted word inside JSX or HTML text is read with the text
        before, after = bare[max(0, start - 200) : start], bare[end : end + 8]
        if lang == "html":
            if not DISPLAYED_ATTRIBUTE.search(before):
                continue
        elif NOT_UI_BEFORE.search(before) or re.match(r"\s*[!=]==?", after):
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
        texts.append((line_of(bare, start), text))
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


def check_text(texts: list[tuple[int, str]], unchecked: dict[str, str]) -> list[Finding]:
    """Voice, missing-value, emoji and glyph rules over displayed text. A text may span lines, so
    each finding takes the line of its match. Characters the glyph check could not look up (fonts
    missing) are added to `unchecked` with the reason."""
    found: list[Finding] = []
    for line, text in texts:
        stripped = text.strip()
        excerpt = " ".join(text.split())[:50]

        def at(index: int) -> int:
            return line + text.count("\n", 0, index)

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
            found.append(Finding(at(m.start()), "error", "emoji", f"Emoji U+{ord(m.group(0)):04X}: never on a console; draw symbols as SVG.", "Never"))
        for ch in dict.fromkeys(c for c in text if ord(c) > 0x7F and not EMOJI.match(c)):
            try:
                lacking = glyphs.missing(ch)
            except glyphs.FONT_ERRORS as err:
                unchecked.setdefault(ch, str(err))
                continue
            if len(lacking) == 2:
                found.append(Finding(at(text.index(ch)), "error", "glyph", f"'{ch}' (U+{ord(ch):04X}) is not in B612 or B612 Mono: draw it as an inline SVG.", "Typography"))
            elif lacking:
                found.append(Finding(at(text.index(ch)), "check", "glyph", f"'{ch}' (U+{ord(ch):04X}) is missing from {lacking[0]}.", "Typography"))
    return found


def lint(path: Path, root: Path, unchecked: dict[str, str]) -> list[Finding]:
    src = path.read_text(encoding="utf-8")
    lang = {".css": "css", ".html": "html"}.get(path.suffix, "js")
    code, bare, strings = scan(src, lang)
    groups = jsx_text(bare) if path.suffix == ".tsx" else html_text(bare) if lang == "html" else []
    texts = text_nodes(code, groups)
    in_text = bytearray(len(src))
    rule_code, rule_bare = list(code), list(bare)
    for start, end in (run for runs in groups for run in runs):
        in_text[start:end] = b"\x01" * (end - start)
        blank(rule_code, start, end)
        blank(rule_bare, start, end)
    code, bare = "".join(rule_code), "".join(rule_bare)
    root_blocks = [m.span() for m in re.finditer(r":root\s*\{[^}]*\}", code)] if is_shared_sheet(path, root) else []
    findings = check_code(path, code, bare, root_blocks, root) + check_stylesheet_scope(path, code, root)
    texts += css_content(bare, strings) if lang == "css" else ui_strings(lang, bare, strings, in_text)
    findings += check_text(texts, unchecked)
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
