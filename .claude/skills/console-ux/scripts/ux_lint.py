#!/usr/bin/env python3
"""Heuristic checks of HYPERION renderer code against docs/frontend/ux-guidelines.md.

Usage:
    ux_lint.py                 Changed renderer files (working tree vs HEAD, untracked included).
    ux_lint.py PATH...         These files or directories.
    ux_lint.py --all           The whole renderer.

Scans .ts, .tsx, .css and .html under apps/hyperion/src/renderer, skipping tests and test helpers.
Each finding names the guide section it comes from:

    error  breaks a "must" or "never" rule as written (literal colour, glow, spinner, italic, text
           under 0.875rem, emoji, a glyph B612 lacks, horizontal scroll, a modal alert, ...)
    check  needs judgement (a 50% radius may be a contact symbol; a hover handler may have a
           keyboard twin); resolve it or leave a code comment stating the reason

It cannot see rendering, layout at 1280x720, data states or closed-loop commanding: those need
reading and running the code. Exit status is 1 when there is any error.
"""

from __future__ import annotations

import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

sys.dont_write_bytecode = True  # keep the skill directory free of __pycache__
sys.path.insert(0, str(Path(__file__).resolve().parent))
import glyphs  # noqa: E402  (same directory)

RENDERER = "apps/hyperion/src/renderer"
EXTENSIONS = {".ts", ".tsx", ".css", ".html"}
TEXT_MIN_REM = 0.875


@dataclass
class Finding:
    line: int
    severity: str
    rule: str
    message: str
    section: str


# --------------------------------------------------------------------------- source preparation


def blank_comments(src: str, css: bool) -> tuple[str, list[tuple[int, str]]]:
    """Replace comments with spaces (newlines kept, so line numbers hold) and collect string
    literals as (line, text). CSS has no line comments; `//` in a CSS url() is not a comment."""
    out: list[str] = []
    strings: list[tuple[int, str]] = []
    i, n, line = 0, len(src), 1
    while i < n:
        c = src[i]
        nxt = src[i + 1] if i + 1 < n else ""
        if c == "/" and nxt == "*":
            end = src.find("*/", i + 2)
            end = n if end == -1 else end + 2
            chunk = src[i:end]
            out.append("".join("\n" if ch == "\n" else " " for ch in chunk))
            line += chunk.count("\n")
            i = end
        elif not css and c == "/" and nxt == "/":
            end = src.find("\n", i)
            end = n if end == -1 else end
            out.append(" " * (end - i))
            i = end
        elif c in "'\"`":
            start_line = line
            j = i + 1
            while j < n and src[j] != c:
                if src[j] == "\\":
                    j += 1
                elif src[j] == "\n" and c != "`":
                    break
                j += 1
            chunk = src[i : j + 1]
            strings.append((start_line, chunk[1:-1] if chunk.endswith(c) else chunk[1:]))
            out.append(chunk)
            line += chunk.count("\n")
            i = j + 1
        else:
            out.append(c)
            if c == "\n":
                line += 1
            i += 1
    return "".join(out), strings


def blank_strings(code: str) -> str:
    return re.sub(r"(['\"`])(?:\\.|(?!\1).)*\1", lambda m: m.group(1) + " " * (len(m.group(0)) - 2) + m.group(1), code, flags=re.S)


def line_of(text: str, index: int) -> int:
    return text.count("\n", 0, index) + 1


def decode_escapes(text: str) -> str:
    text = re.sub(r"\\u\{([0-9a-fA-F]+)\}", lambda m: chr(int(m.group(1), 16)), text)
    return re.sub(r"\\u([0-9a-fA-F]{4})", lambda m: chr(int(m.group(1), 16)), text)


NOT_UI_LINE = re.compile(
    r"\bthrow\b|Error\(|console\.|^\s*import\b|\bfrom\s+['\"]|addEventListener|removeEventListener|"
    r"querySelector|getElementById|data-testid|\bcase\s|===?\s*['\"]|['\"]\s*===?|\.test\.|expect\(|"
    r"new URL\(|\.startsWith\(|\.endsWith\(|\bkind:\s*['\"]|\btype:\s*['\"]"
)


def ui_texts(path: Path, src: str, code: str, strings: list[tuple[int, str]]) -> list[tuple[int, str]]:
    """Strings an operator may read: JSX text, and string literals that look like labels or
    sentences rather than keys, paths, CSS classes or developer messages."""
    lines = src.splitlines()
    texts: list[tuple[int, str]] = []
    for line, raw in strings:
        text = decode_escapes(re.sub(r"\$\{[^}]*\}", " ", raw))
        context = lines[line - 1] if 0 < line <= len(lines) else ""
        if not re.search(r"[A-Za-z]", text) and not re.search(r"[^\x00-\x7f]", text):
            continue
        if NOT_UI_LINE.search(context) or re.search(r"className\s*=\s*$|className=\{?[`'\"]", context):
            continue
        if re.search(r"^[./@#]|://|\.(css|tsx?|js|json|svg|png|woff2?)$", text.strip()):
            continue
        if re.fullmatch(r"[a-z][a-zA-Z0-9_:-]*", text.strip()):  # a key, id or event name
            continue
        texts.append((line, text))
    if path.suffix == ".tsx":
        bare = blank_strings(code)
        for m in re.finditer(r">([^<>{}]+)<", bare):
            segment = m.group(1)
            if re.search(r"[A-Za-z]", segment) and not re.search(r"=>|&&|\|\||;|==|\breturn\b", segment):
                texts.append((line_of(bare, m.start(1)), segment.strip()))
    return texts


# --------------------------------------------------------------------------- rules

HEX = re.compile(r"(?<![\w.&$])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})\b")
NAMED_COLOUR = re.compile(
    r"(?:color|background(?:-color)?|border(?:-\w+)?|fill|stroke|outline(?:-color)?|fillStyle|strokeStyle)"
    r"\s*[:=]\s*['\"]?[^;'\"]*\b(white|black|red|green|blue|yellow|orange|purple|gr[ae]y|cyan|magenta|lime|pink)\b",
    re.I,
)
UI_BANNED = re.compile(r"\b(loading|please|oops|sorry|success(?:ful(?:ly)?)?|welcome|click here|tap here|awesome|hooray)\b", re.I)
WEB_IDIOM = re.compile(r"\b(spinner|skeleton|toast|hamburger|snackbar|confetti|throbber)\b", re.I)
FICTION_BREAK = re.compile(r"\b(glow\w*|neon|scan-?lines?|hologram\w*|glassmorph\w*|crt|chromatic)\b", re.I)
PERSON = re.compile(r"\b(I|I'm|I've|I'll)\b|\b(me|my|we|our|us|you|your|you're|you've)\b", re.I)
PLACEHOLDER = re.compile(r"^(N/?A|n/a|--|NaN|null|undefined|TBD|\?+|-|0\.0+)$")
EMOJI = re.compile("[\U0001f000-\U0001faff\u2600-\u27bf\ufe0f\u2b50\u2b55\u2934\u2935]")


def length_px(value: float, unit: str) -> float:
    return value * 16 if unit in ("rem", "em") else value


def check_code(path: Path, code: str, root_block: tuple[int, int] | None) -> list[Finding]:
    """Rules over comment-free source with strings intact (colours live in strings too)."""
    found: list[Finding] = []
    css = path.suffix == ".css"

    def add(index: int, severity: str, rule: str, message: str, section: str) -> None:
        if root_block and root_block[0] <= index < root_block[1]:
            return  # token definitions in the stylesheet's :root block
        found.append(Finding(line_of(code, index), severity, rule, message, section))

    for m in HEX.finditer(code):
        add(m.start(), "error", "literal-colour", f"Literal colour {m.group(0)}: use a token (var(--…)) from styles.css.", "Colour")
    for m in re.finditer(r"\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color-mix)\(", code):
        add(m.start(), "error", "literal-colour", f"Colour function {m.group(0)}…): use a token.", "Colour")
    for m in NAMED_COLOUR.finditer(code):
        add(m.start(1), "error", "literal-colour", f"Named colour '{m.group(1)}': use a token.", "Colour")
    for m in re.finditer(r"\b(box-shadow|text-shadow|drop-shadow|backdrop-filter|mix-blend-mode|boxShadow|textShadow|backdropFilter|shadowBlur|shadowColor)\b|\bblur\(|\b(?:linear|radial|conic|repeating-linear|repeating-radial)-gradient\(", code):
        add(m.start(), "error", "effect", f"'{m.group(0)}': no gradients, glows, shadows or blurs on console chrome.", "Colour")
    for m in re.finditer(r"\bcreate(?:Linear|Radial|Conic)Gradient\(", code):
        add(m.start(), "check", "effect", f"{m.group(0)}…): a data ramp is allowed, chrome gradients are not.", "Colour")
    for m in re.finditer(r"\bopacity\s*:|\bglobalAlpha\b|\bopacity\s*=", code):
        add(m.start(), "check", "transparency", "Transparency: not allowed as an effect on console chrome.", "Colour")
    for m in FICTION_BREAK.finditer(code):  # class names and strings included
        add(m.start(), "check", "fiction-break", f"'{m.group(0)}' suggests a banned effect (glow, neon, scanlines, hologram, glass).", "Never")

    for m in re.finditer(r"font-size\s*:\s*([\d.]+)(px|rem|em|%)", code):
        value, unit = float(m.group(1)), m.group(2)
        if unit == "px":
            add(m.start(), "error", "font-size", f"font-size in px ({m.group(0)}): size text in rem so interface scale works.", "Accessibility")
        size_px = value * 0.16 if unit == "%" else length_px(value, unit)
        if size_px < TEXT_MIN_REM * 16:
            add(m.start(), "error", "font-size", f"{m.group(0)} is under {TEXT_MIN_REM}rem, the smallest text allowed.", "Typography")
    for m in re.finditer(r"fontSize\s*:\s*(['\"]?)([\d.]+)(px|rem|em)?\1", code):
        unit = m.group(3) or "px"
        if unit == "px" or length_px(float(m.group(2)), unit) < TEXT_MIN_REM * 16:
            add(m.start(), "error", "font-size", f"{m.group(0)}: text in rem and at least {TEXT_MIN_REM}rem.", "Typography")
    for m in re.finditer(r"\.font\s*=\s*[`'\"][^`'\"]*?(\d+(?:\.\d+)?)px", code):
        add(m.start(), "check", "canvas-font", "Canvas font in px: derive it from the root font size (interface scale 80–150%), at least 0.875rem; readable text belongs in the DOM.", "Typography")
    if css:
        for m in re.finditer(r"^(?!\s*@media).*?(?<![\w.-])(\d+(?:\.\d+)?)px\b", code, re.M):
            if float(m.group(1)) > 2:
                add(m.start(1), "check", "px-length", f"{m.group(1)}px: size in rem so the 80–150% interface scale holds.", "Accessibility")

    for m in re.finditer(r"font-style\s*:\s*italic|fontStyle\s*:\s*['\"]italic|<(?:em|i)[\s>]", code):
        add(m.start(), "error", "italic", "No italics on a console.", "Typography")
    for m in re.finditer(r"font-weight\s*:\s*(?:bold|bolder|[6-9]00)|fontWeight\s*:|<(?:strong|b)[\s>]", code):
        add(m.start(), "check", "bold", "Bold is for display titles only; emphasis on a console means an alert.", "Typography")
    for m in re.finditer(r"font-family\s*:\s*([^;}]+)", code):
        if "var(--font-" not in m.group(1):
            add(m.start(), "error", "font-family", f"font-family '{m.group(1).strip()}': use var(--font-sans) or var(--font-mono).", "Typography")
    for m in re.finditer(r"fonts\.googleapis|fonts\.gstatic|@import\s+url\(\s*['\"]?https?:|url\(\s*['\"]?https?://[^)]*\.(?:woff2?|ttf|otf)", code):
        add(m.start(), "error", "remote-font", "Fonts are bundled through @fontsource; never fetch one at runtime.", "Typography")
    for m in re.finditer(r"text-transform\s*:\s*(lowercase|capitalize)", code):
        add(m.start(), "check", "case", f"text-transform: {m.group(1)}: labels are upper case, sentences mixed case.", "Typography")

    for m in re.finditer(r"border-radius\s*:\s*([^;}]+)", code):
        for v in re.finditer(r"([\d.]+)(px|rem|em|%)", m.group(1)):
            if v.group(2) == "%" or length_px(float(v.group(1)), v.group(2)) > 2:
                add(m.start(), "check", "radius", f"border-radius {m.group(1).strip()}: panels and controls have square or 2px corners.", "Layout")
                break
    for m in re.finditer(r"overflow-x\s*:\s*(auto|scroll)|overflowX\s*:\s*['\"](auto|scroll)", code):
        add(m.start(), "error", "scroll", "Never scroll horizontally.", "Layout")
    for m in re.finditer(r"overflow(?:-y)?\s*:\s*(auto|scroll)\b|overflowY?\s*:\s*['\"](auto|scroll)", code):
        add(m.start(), "check", "scroll", "Scrolling only inside lists, logs and procedures, showing position and total (12-24 of 87).", "Layout")
    for m in re.finditer(r"writing-mode\s*:|transform\s*:\s*rotate\(", code):
        add(m.start(), "check", "vertical-label", "Labels are horizontal.", "Layout")

    for m in re.finditer(r"transition(?:-duration)?\s*:\s*([^;}]+)", code):
        for d in re.finditer(r"(\d*\.?\d+)(ms|s)\b", m.group(1)):
            ms = float(d.group(1)) * (1000 if d.group(2) == "s" else 1)
            if ms > 150:
                add(m.start(), "error", "motion", f"Transition of {d.group(0)}: state transitions take 80–150 ms.", "Motion and sound")
    for m in re.finditer(r"@keyframes\s+([\w-]+)|\banimation(?:-name)?\s*:|\.animate\(|\binfinite\b", code):
        add(m.start(), "check", "motion", "Animation: only to show a change of state; flashing only for emergency and warning alerts (0.8 Hz text, 3 Hz small indicators, synchronised, 10 s cap).", "Motion and sound")
    for m in re.finditer(r"(?<![.\w])(?:window\.)?(alert|confirm|prompt)\(", code):
        add(m.start(), "error", "modal", f"{m.group(1)}(): no modal alert boxes; alerts are server-raised and shown in the alert list.", "Alerts")
    for m in re.finditer(r"<dialog\b|\.showModal\(", code):
        add(m.start(), "check", "modal", "Modal dialogs only for an ARM/EXECUTE decision.", "Never")
    for m in re.finditer(r"\bonContextMenu\b", code):
        add(m.start(), "check", "pointer", "No action may depend on a right click alone.", "Controls and commanding")
    for m in re.finditer(r"\bon(?:MouseEnter|MouseOver|PointerEnter|PointerOver)\b", code):
        add(m.start(), "check", "pointer", "No action may depend on hover; consoles may run on touch screens.", "Controls and commanding")
    for m in re.finditer(r"outline\s*:\s*(?:none|0)\b|outline-width\s*:\s*0\b", code):
        add(m.start(), "check", "focus", "Focus is always visible as a 2px --accent outline.", "Controls and commanding")
    for m in re.finditer(r"\.toFixed\(|\.toPrecision\(|\.toLocaleString\(|new Intl\.NumberFormat\(", code):
        if path.suffix == ".tsx":
            add(m.start(), "check", "number-format", "Format values through one shared formatter so a quantity has the same unit and precision everywhere.", "Numbers, units and time")
    for m in WEB_IDIOM.finditer(code):
        add(m.start(), "error", "web-idiom", f"'{m.group(0)}': no spinners, skeleton loaders, toasts or hamburger menus.", "Never")

    if css:
        for block in re.finditer(r"([^{}]+)\{([^{}]*)\}", code):
            selector, body = block.group(1), block.group(2)
            if re.search(r":(hover|focus|active)|::selection", selector) and "--status-" in body:
                add(block.start(2), "error", "reserved-colour", "A status colour in a hover/focus/active style: status colours are reserved for the state they name.", "Colour")
            if "--status-nominal" in body:
                add(block.start(2), "check", "reserved-colour", "Green is only for a state the operator waits to confirm; a normal reading is plain --text.", "Colour")
    return found


def check_text(texts: list[tuple[int, str]]) -> list[Finding]:
    found: list[Finding] = []
    for line, text in texts:
        stripped = text.strip()
        for m in UI_BANNED.finditer(stripped):
            found.append(Finding(line, "error", "voice", f"'{m.group(0)}' in '{stripped[:50]}': terse, literal, impersonal (NO CARRIER, not 'Loading…').", "Voice and nomenclature"))
        if re.search(r"[A-Za-z0-9)]!(?:\s|$)", stripped):
            found.append(Finding(line, "error", "voice", f"Exclamation mark in '{stripped[:50]}'.", "Voice and nomenclature"))
        if PERSON.search(stripped) and " " in stripped:
            found.append(Finding(line, "check", "voice", f"First or second person in '{stripped[:50]}': the ship never speaks as a person.", "Voice and nomenclature"))
        if "…" in stripped or "..." in stripped:
            found.append(Finding(line, "check", "voice", f"Ellipsis in '{stripped[:50]}': no 'thinking…' or 'Loading…'.", "Voice and nomenclature"))
        if PLACEHOLDER.fullmatch(stripped):
            found.append(Finding(line, "error", "missing-value", f"'{stripped}' as a value: a missing value is an em dash — in --text-muted.", "Data states"))
        for m in EMOJI.finditer(stripped):
            found.append(Finding(line, "error", "emoji", f"Emoji or pictograph U+{ord(m.group(0)):04X}: never on a console; draw symbols as SVG.", "Never"))
        for ch in dict.fromkeys(c for c in stripped if ord(c) > 0x7F and not EMOJI.match(c)):
            try:
                lacking = glyphs.missing(ch)
            except (FileNotFoundError, ValueError):
                break
            if len(lacking) == 2:
                found.append(Finding(line, "error", "glyph", f"'{ch}' (U+{ord(ch):04X}) is not in B612 or B612 Mono: draw it as an inline SVG.", "Typography"))
            elif lacking:
                found.append(Finding(line, "check", "glyph", f"'{ch}' (U+{ord(ch):04X}) is missing from {lacking[0]}.", "Typography"))
    return found


# --------------------------------------------------------------------------- driver


def repo_root() -> Path:
    proc = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    return Path(proc.stdout.strip()) if proc.returncode == 0 else Path(__file__).resolve().parents[4]


def wanted(path: Path) -> bool:
    name = path.name
    return (
        path.suffix in EXTENSIONS
        and not re.search(r"\.test\.tsx?$|\.d\.ts$", name)
        and "/test/" not in path.as_posix()
        and RENDERER in path.as_posix()
    )


def targets(root: Path, args: list[str]) -> list[Path]:
    if args == ["--all"]:
        return sorted(p for p in (root / RENDERER).rglob("*") if p.is_file() and wanted(p))
    if args:
        paths: list[Path] = []
        for a in args:
            p = Path(a).resolve()
            if not p.exists():
                p = root / a
            paths += sorted(q for q in p.rglob("*") if q.is_file()) if p.is_dir() else [p]
        return [p for p in paths if p.exists() and p.suffix in EXTENSIONS]
    names = subprocess.run(["git", "-C", str(root), "diff", "--name-only", "HEAD"], capture_output=True, text=True).stdout.split()
    names += subprocess.run(["git", "-C", str(root), "ls-files", "--others", "--exclude-standard"], capture_output=True, text=True).stdout.split()
    return sorted({root / n for n in names if (root / n).exists() and wanted(root / n)})


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
    for path in files:
        src = path.read_text(encoding="utf-8")
        css = path.suffix == ".css"
        code, strings = blank_comments(src, css)
        root_block = None
        if css and (m := re.search(r":root\s*\{[^}]*\}", code)):
            root_block = (m.start(), m.end())
        findings = check_code(path, code, root_block)
        if path.suffix in (".tsx", ".ts", ".html"):
            findings += check_text(ui_texts(path, src, code, strings))
        if not findings:
            continue
        print(path.relative_to(root) if path.is_relative_to(root) else path)
        for f in sorted(findings, key=lambda f: (f.line, f.severity != "error")):
            print(f"  {f.line:>4}: {f.severity:<5} [{f.rule}] {f.message} (guide: {f.section})")
            errors += f.severity == "error"
            checks += f.severity == "check"
    print(f"\n{errors} error(s), {checks} check(s) in {len(files)} file(s) scanned. Heuristic: confirm each line against the guide.")
    sys.exit(1 if errors else 0)


if __name__ == "__main__":
    main()
