#!/usr/bin/env python3
"""The Rust side of `just smart-ci`'s test selection: the tests that can reach a changed item.

A dependency graph of the workspace's Rust code, item by item, built from what each file says:
standard-library Python, no compiler and no rust-analyzer (which the toolchain does not ship, and
which would cost minutes and gigabytes an index). It over-approximates: where it cannot be sure it
adds edges, and where a file defeats the reading it widens to the file's crate. A test function is
selected when it reaches a changed item through the graph, within the crates its binary links.

The nodes are the items of each file's modules (a `fn`, type, trait, `impl` block and each of its
members, `const`, `static`, `macro_rules!`, an item-position macro call), each with a digest of its
tokens, so that a change to one function's body changes that function alone. The items inside a
body (a local `fn`, `impl` or `use`) belong to the item around them. Each `use` import and glob is a
node too, so that code naming an item through a re-export depends on that one import, not on every
other import of the module that re-exports it.

Names are resolved as rustc resolves them, as far as a reading of the source can: through each
compilation unit's module tree (`mod`, `#[path]`, inline modules), each module's items, its imports
(re-exports followed), its globs, the extern prelude and the standard library. The edges, from an
item A:

- to the items its paths resolve to: a path that resolves into a type stops there (`Type::assoc`
  depends on the type), and every import or glob the resolution passes through is a dependency too.
  A path whose first segment is nothing in scope is a local binding. One that resolves part of the
  way, or a name in a module whose globs cannot all be resolved, falls back to every item of that
  name that A could see, by crate and visibility.
- to the traits in scope in its module (the module's own traits, its trait imports and `as _`
  imports, and what its globs bring), for the methods it may call on them.
- Methods: a method call names no item, so `impl` blocks are attached to their self type: the
  type's node depends on every impl of it, inherent or trait. A value of a type comes from code that
  names the type or one of its impls, so a test that holds one reaches the type, and with it every
  impl. An impl whose self type is not its own crate's (`impl Trait for f64`, `impl<T: X> Y for T`,
  a sim trait for a base type) attaches to the items of its own crate that its header names
  instead: by the orphan rule the trait, or a type in its parameters, which code that calls the impl
  has in scope or holds. A type alias attaches what it names as well. An impl's members (its
  methods, constants and types) are nodes of their own, which the impl depends on. In an inherent
  impl, or one of a workspace trait, a member runs only where code names it (`x.m()`, `T::m`,
  `Self::m`, `<T as Tr>::m`, in the code or in a macro it calls), so a change to the member itself
  selects the tests that both reach the impl and reach code naming it. An impl of another crate's
  trait stays whole: that crate's code, operators, `for`, `?` and derives call its members unnamed.
- Macros: A invoking `m!` depends on m's definition, and m's body is A's code: its paths resolve
  from A's module (`$crate` from m's crate), and the names it calls methods by are A's, as are the
  words of A's arguments to it. A call in item position defines what m's body defines (`pub const
  ALL`) and the words of its arguments. A macro is found by its name, a `use` renaming it, or
  failing both, as one that another macro's call defines.
- Paths in other places: a turbofish's types, a format string's inline arguments (`{name}`), a path
  in any attribute's string (`#[serde(serialize_with = "path")]`; `with = "module"` names its
  `serialize` and `deserialize`), a path from the extern prelude (`::a::f`).
- Literals: a string naming a workspace file (`include_str!`, `#[path]`, a read of a path built from
  `CARGO_MANIFEST_DIR`) depends on that file's text, or on that data file; a crate's data, read from
  another crate by a string that names the crate's directory and more, too.
- Code compiled for some configurations only (`cfg`, features, `target_arch = "wasm32"`) counts as
  if every configuration were built, and generic code is its definer's, whatever instantiates it.

A changed file's changed nodes are its items whose digests changed, were added or were removed, and
its imports and globs that changed (their attributes, a `cfg`, included). A removed item, import or
glob also changes what other names in its module may now resolve to, so the module's other items
change with it; a removed `impl` changes the type it was for. A changed `mod` declaration changes
the files it names. A change to a file's module-level attributes changes all of its items. A file
no module tree reaches, a file whose tokens the reader cannot follow, a crate root whose crate
attributes change, a changed global allocator or panic handler, and a module outside its crate's
directory (`#[path = "../../shared/x.rs"]`) widen to the crate.

Doctests are nodes of their own, one per file, resolved as a crate outside it sees the workspace.
Tests that the source does not name, which a derive or a macro generates, are selected with the
item that generates them: a `ts-rs` export test with its type, a macro's tests with the module of
the call. A test selected by module (all of a module's tests) is filtered by the module's path, so
any generated test in it runs too.

`FileFacts` is what one file says; `Facts` keeps them under `target/smart-ci/`, keyed by this
module's own source and each file's content, so that a stale entry is never used. `Graph` assembles
them into nodes and edges; `Graph.select` is the reverse closure from the changed nodes.
"""

from __future__ import annotations

import hashlib
import json
import os
import posixpath
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, Dict, Iterable, List, Optional, Sequence, Set, Tuple

# --------------------------------------------------------------------------------------------------
# Lexing.

_TOKEN = re.compile(
    r"""
    (?P<ws>\s+)
  | (?P<nums>(?:-?\d\w*(?:\.\d\w*)?(?:[eE][-+]?\d\w*)?\s*,\s*)+)
  | (?P<doc>///(?!/)[^\n]*|//![^\n]*)
  | (?P<line>//[^\n]*)
  | (?P<block>/\*)
  | (?P<rstr>[bc]?r(?P<hashes>\#*)")
  | (?P<str>[bc]?"[^"\\]*(?:\\.[^"\\]*)*")
  | (?P<char>b?'(?:[^'\\\n]|\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]{1,6}\}|.))')
  | (?P<life>'[^\W\d]\w*)
  | (?P<ident>r\#[^\W\d]\w*|[^\W\d]\w*)
  | (?P<num>\d\w*(?:\.\d\w*)?)
  | (?P<punct>::|->|=>|\.\.\.|\.\.=|\.\.|.)
    """,
    re.X | re.S,
)
_BLOCK_EDGE = re.compile(r"/\*|\*/")
_ESCAPE = re.compile(r"\\(.)", re.S)

# Rust 2024's strict and reserved keywords, never an identifier; `crate`, `self`, `super` and `Self`
# start paths. The weak keywords (`union`, `default`, `auto`, `safe`, `macro_rules`, `raw`) can name
# items, so they are identifiers here.
KEYWORDS = frozenset("""
    as async await break const continue crate dyn else enum extern false fn for if impl in let loop
    match mod move mut pub ref return self Self static struct super trait true type unsafe use where
    while abstract become box do final macro override priv typeof unsized virtual yield try gen
""".split())
PATH_ROOTS = frozenset(("crate", "self", "super"))
# The words after which an identifier is a new name, not a path.
_DECLARES = frozenset(("fn", "struct", "enum", "union", "trait", "type", "const", "static", "mod", "let", "mut",
                       "ref", "macro_rules"))
# Fields of a doctest's info string that leave it a Rust doctest; `ignore` is compiled by nothing.
_RUST_DOC_ATTRS = frozenset("""
    rust should_panic no_run compile_fail edition2015 edition2018 edition2021 edition2024
    standalone_crate test_harness allow_fail
""".split())
_TEST_ATTRS = frozenset(("test", "wasm_bindgen_test", "rstest", "test_case"))
# Test attributes that make one test per case, named below the function (`t::case_1`), and whose
# arguments are code (`#[test_case(f(3) => 6)]`).
_CASE_ATTRS = frozenset(("rstest", "test_case"))
# Attributes that make an item reach its whole crate, called by nothing that names it.
_GLOBAL_ATTRS = frozenset(("global_allocator", "panic_handler"))
# A string in an attribute that may name an item: `#[serde(default = "crate::x::seed")]`.
_ATTR_PATH = re.compile(r"^(?:[A-Za-z_]\w*::)*[A-Za-z_]\w*$")
# A format string's inline argument, `{name}` or `{name:>8}` (not `{{`): a path of one segment.
_CAPTURE = re.compile(r"\{([A-Za-z_]\w*)(?=[:}])")
# Attributes whose string names a module whose `serialize` and `deserialize` the derive calls.
_WITH_MODULE = frozenset(("with",))
# A one-word string, which a path built a piece at a time (`dir.join("golden")`) may use.
_WORD = re.compile(r"^[A-Za-z0-9_.-]{2,40}$")
# Calls that read a directory's entries: every file under it is read.
_DIRECTORY_READS = frozenset(("read_dir", "WalkDir", "walkdir"))
# No cycle of lookups reached a position on the stack (`Graph.lookup`).
_NO_CYCLE = 1 << 30
# Items of the value namespace alone, which have no associated items: no path goes on past one.
_VALUES = frozenset(("fn", "const", "static"))
# Items with no code to call a method from: a type's definition (its derives call its fields' impls by
# their full paths), an inline module's header, the file's own attributes, a foreign block, and a
# macro's definition (its body is its callers' code, with their traits in scope).
_NO_CODE = frozenset(("struct", "enum", "union", "type", "mod", "file", "extern", "macro"))
# The keywords after which a macro's body defines an item by a name of its own.
_DEFINES = frozenset(("fn", "struct", "enum", "union", "trait", "type", "const", "static", "mod"))
# Item keywords that a statement inside a body can start with.
_ITEMS = frozenset(("fn", "struct", "enum", "union", "trait", "type", "const", "static", "mod", "use", "impl",
                    "macro_rules", "extern"))


class LexError(Exception):
    pass


@dataclass
class Lexed:
    kinds: List[str]  # i ident, p punct, s string, l lifetime or char, n number
    vals: List[str]
    docs: List[Tuple[int, str, str]]  # (line, "outer" | "inner", the comment's text)
    raw: List[bool] = field(default_factory=list)  # an identifier written `r#name`


def lex(text: str) -> Lexed:
    """Rust tokens, less comments and whitespace; doc comments kept apart, with their lines."""
    kinds: List[str] = []
    vals: List[str] = []
    raw: List[bool] = []
    docs: List[Tuple[int, str, str]] = []
    pos, n = 0, len(text)
    match = _TOKEN.match
    counted, lines = 0, 1  # the line at `counted`, for the doc comments' lines

    def line_at(at: int) -> int:
        nonlocal counted, lines
        lines += text.count("\n", counted, at)
        counted = at
        return lines

    def add(kind: str, value: str, is_raw: bool = False) -> None:
        kinds.append(kind)
        vals.append(value)
        raw.append(is_raw)

    while pos < n:
        m = match(text, pos)
        if m is None:  # pragma: no cover - the last alternative matches any character
            raise LexError(f"no token at {pos}")
        kind = m.lastgroup
        end = m.end()
        if kind == "ident":
            value = m.group()
            add("i", value[2:] if value.startswith("r#") else value, value.startswith("r#"))
        elif kind == "punct":
            add("p", m.group())
        elif kind == "str":
            body = m.group()
            body = body[body.index('"') + 1 : -1]
            add("s", _ESCAPE.sub(r"\1", body) if "\\" in body else body)
        elif kind == "rstr":
            closing = '"' + m.group("hashes")
            stop = text.find(closing, end)
            if stop < 0:
                raise LexError("unterminated raw string")
            add("s", text[end:stop])
            end = stop + len(closing)
        elif kind == "doc":
            body = m.group()
            docs.append((line_at(pos), "inner" if body[2] == "!" else "outer", body[3:]))
        elif kind == "block":
            depth, scan = 1, end
            while depth:
                edge = _BLOCK_EDGE.search(text, scan)
                if edge is None:
                    raise LexError("unterminated block comment")
                depth += 1 if edge.group() == "/*" else -1
                scan = edge.end()
            comment = text[pos:scan]
            if (comment.startswith("/**") and not comment.startswith(("/***", "/**/"))) or comment.startswith("/*!"):
                line = line_at(pos)
                for offset, row in enumerate(comment[3:-2].split("\n")):
                    docs.append((line + offset, "inner" if comment[2] == "!" else "outer",
                                 re.sub(r"^\s*\* ?", "", row) if offset else row))
            end = scan
        elif kind in ("char", "life"):
            add("l", m.group())
        elif kind in ("num", "nums"):
            add("n", m.group())  # read by nothing but the digests: a changed number is changed code
        pos = end
    return Lexed(kinds, vals, docs, raw)


# --------------------------------------------------------------------------------------------------
# What one file says.


@dataclass
class FileFacts:
    """The facts of one source file that the graph is built from (JSON-able, for the cache).

    A scope is a module path within the file ("" for the file's own module, "tests" for its inline
    `mod tests`); `test` is 1 for code under `#[cfg(test)]`, which may name dev-dependencies. Item 0
    is the file's own tokens outside any item: its module-level attributes.
    """

    ok: bool = True
    problem: str = ""
    # Items: [id, kind, name, visibility (pub | crate | private), scope, test, digest, test function
    # (0, 1, or 2 for an ignored one), what it generates ("" | "ts" | "tests"), raw name (r#)].
    items: List[list] = field(default_factory=list)
    item_paths: List[List[list]] = field(default_factory=list)  # per item: [test, *segments]
    item_calls: List[List[str]] = field(default_factory=list)  # per item: the macros it calls
    item_literals: List[List[str]] = field(default_factory=list)  # per item: strings naming paths, maybe
    item_methods: List[List[str]] = field(default_factory=list)  # per item: names after `.`, and `Self::`
    # Names the items define: [name, kind, visibility, scope, item]; kind "mod" is an inline module,
    # "maybe" an identifier inside an item-position macro call, "local" an item inside a body.
    defs: List[list] = field(default_factory=list)
    variants: Dict[str, List[str]] = field(default_factory=dict)  # enum -> its variants
    mods: List[list] = field(default_factory=list)  # `mod name;`: [name, #[path], scope, test, attributes]
    uses: List[list] = field(default_factory=list)  # [scope, test, alias, *segments]
    # The attributes of the `use` declarations (their `cfg`s): [attributes, scope, test, alias, *segments]
    use_attrs: List[list] = field(default_factory=list)
    globs: List[list] = field(default_factory=list)  # `use path::*`: [scope, test, *segments]
    # macro_rules!: [name, exported, [body paths], makes tests, item, [macros its body calls],
    # [the names its body calls methods and associated items by], makes impls]
    macros: List[list] = field(default_factory=list)
    impls: List[list] = field(default_factory=list)  # [item, scope, test, [self type's path], [header paths]]
    aliases: Dict[str, List[List[str]]] = field(default_factory=dict)  # `type X = ...`: X -> its paths
    doctests: List[list] = field(default_factory=list)  # [line, imports, paths, macros, methods, globs]
    crate_attrs: str = ""  # inner attributes and global ones, which reach the whole crate

    def to_json(self) -> dict:
        return self.__dict__

    @staticmethod
    def from_json(data: dict) -> "FileFacts":
        facts = FileFacts()
        facts.__dict__.update(data)
        return facts

    @property
    def tests(self) -> int:
        return sum(1 for item in self.items if item[7] == 1)

    @property
    def ignored(self) -> int:
        return sum(1 for item in self.items if item[7] == 2)


def _matching(kinds: Sequence[str], vals: Sequence[str]) -> List[int]:
    """The index of each bracket's partner (-1 elsewhere); LexError if they do not balance."""
    close = [-1] * len(vals)
    stack: List[int] = []
    pairs = {")": "(", "]": "[", "}": "{"}
    for i, (kind, val) in enumerate(zip(kinds, vals)):
        if kind != "p":
            continue
        if val in "([{":
            stack.append(i)
        elif val in ")]}":
            if not stack or vals[stack[-1]] != pairs[val]:
                raise LexError(f"unbalanced {val!r}")
            close[stack.pop()] = i
    if stack:
        raise LexError(f"unclosed {vals[stack[-1]]!r}")
    return close


def _angle_end(vals: Sequence[str], i: int, end: int) -> int:
    """The index after the `>` that closes the `<` at `i` (`->` is one token; `>>` two)."""
    depth = 0
    while i < end:
        val = vals[i]
        if val == "<":
            depth += 1
        elif val == ">":
            depth -= 1
            if depth == 0:
                return i + 1
        elif val in (";", "{", "}"):
            return i
        i += 1
    return end


@dataclass
class _Item:
    start: int
    end: int
    kind: str
    name: str
    vis: str
    scope: str
    test: bool
    testfn: int = 0
    generates: str = ""
    raw: bool = False
    parent: int = -1  # an impl's member: the impl's item


class _Scanner:
    """A file's items, imports, impls and paths, from its tokens."""

    def __init__(self, lexed: Lexed) -> None:
        self.k = lexed.kinds
        self.v = lexed.vals
        self.raw = lexed.raw or [False] * len(lexed.vals)
        self.n = len(self.v)
        self.close = _matching(self.k, self.v)
        self.facts = FileFacts()
        self.scope = [""] * self.n  # the module scope of each token
        self.test = [False] * self.n
        self.skip = [False] * self.n  # use declarations, attributes, macro_rules bodies: no paths
        self.in_use = [False] * self.n
        self.module_items: Set[int] = set()  # where the module parser read an item
        self.bodies: Dict[int, str] = {}  # `{` of an impl, trait or inline module body -> its kind
        self.items: List[_Item] = [_Item(0, self.n, "file", "", "private", "", False)]
        self.item_of: List[int] = []
        self.unexpected = 0
        self.pending: Dict[str, object] = {}  # what `item` read, for `module` to record
        self._uses: Set[tuple] = set()
        self._use_attrs: Dict[tuple, Set[str]] = {}
        self._attr_text = ""  # the attributes of the `use` declaration being read
        # Items inside bodies, each with the token it starts at, for the item around it.
        self.local_defs: List[list] = []  # [name, "local", "private", scope, token]
        self.local_impls: List[list] = []  # [token, scope, test, head, header]
        self.local_macros: List[list] = []  # [name, exported, body, makes tests, token, calls]

    def val(self, i: int) -> str:
        return self.v[i] if 0 <= i < self.n else ""

    def ident(self, i: int) -> bool:
        return 0 <= i < self.n and self.k[i] == "i" and (self.v[i] not in KEYWORDS or self.raw[i])

    def mark(self, table: list, start: int, stop: int, value: object) -> None:
        for j in range(max(start, 0), min(stop, self.n)):
            table[j] = value

    # -- attributes and visibility -----------------------------------------------------------------

    def attributes(self, i: int) -> Tuple[List[Tuple[bool, int, int]], int]:
        attrs: List[Tuple[bool, int, int]] = []
        while self.val(i) == "#":
            inner = self.val(i + 1) == "!"
            bracket = i + 2 if inner else i + 1
            if self.val(bracket) != "[":
                break
            attr = (inner, bracket + 1, self.close[bracket])
            attrs.append(attr)
            if not set(self.attr_path(attr)[-1:]) & _CASE_ATTRS:  # a case's arguments are code
                self.mark(self.skip, i, self.close[bracket] + 1, True)
            i = self.close[bracket] + 1
        return attrs, i

    def test_attr(self, attr: Tuple[bool, int, int]) -> str:
        """The test attribute an attribute is, also under `cfg_attr(..., test)`: its name, or ""."""
        path = self.attr_path(attr)
        if path[-1:] and path[-1] in _TEST_ATTRS:
            return path[-1]
        _, lo, hi = attr
        if path == ["cfg_attr"] and self.val(lo + 1) == "(":
            stop = self.close[lo + 1]
            j = lo + 2
            while j < stop and self.v[j] != ",":  # past the condition
                j = self.close[j] + 1 if self.v[j] in "([" else j + 1
            for x in range(j, stop):
                if self.k[x] == "i" and self.v[x] in _TEST_ATTRS and self.val(x + 1) != "::":
                    return self.v[x]
        return ""

    def attr_path(self, attr: Tuple[bool, int, int]) -> List[str]:
        _, lo, hi = attr
        path = []
        for j in range(lo, hi):
            if self.k[j] == "i":
                path.append(self.v[j])
            elif self.v[j] != "::":
                break
        return path

    def cfg_test(self, attr: Tuple[bool, int, int]) -> bool:
        _, lo, hi = attr
        return self.val(lo) == "cfg" and "test" in self.v[lo:hi]

    def visibility(self, i: int) -> Tuple[str, int]:
        if self.val(i) != "pub":
            return "private", i
        if self.val(i + 1) == "(":
            return ("private" if self.val(i + 2) == "self" else "crate"), self.close[i + 1] + 1
        return "pub", i + 1

    def qualifiers(self, i: int) -> int:
        while True:
            val, nxt = self.val(i), self.val(i + 1)
            if val in ("unsafe", "async", "safe") and nxt in ("fn", "impl", "trait", "extern", "unsafe", "async", "mod"):
                i += 1
            elif val == "default" and nxt in ("fn", "impl", "type", "const", "unsafe", "async"):
                i += 1
            elif val == "const" and nxt in ("fn", "unsafe", "async", "extern"):
                i += 1
            elif val == "extern":
                # `extern fn`, `extern "C" fn`; not `extern crate` nor a foreign block.
                j = i + 1 + (1 if i + 1 < self.n and self.k[i + 1] == "s" else 0)
                if self.val(j) not in ("fn", "unsafe"):
                    return i
                i = j
            elif val == "auto" and nxt == "trait":
                i += 1
            else:
                return i

    def skip_item(self, i: int, end: int, stop_at_brace: bool = True) -> int:
        """Past the item that starts at `i`: its `;` at depth 0, or its `{...}` body."""
        while i < end:
            val = self.v[i]
            if self.k[i] == "p":
                if val == ";":
                    return i + 1
                if val == "{":
                    if stop_at_brace:
                        return self.close[i] + 1
                    i = self.close[i] + 1
                    continue
                if val in "([":
                    i = self.close[i] + 1
                    continue
            i += 1
        return end

    def body_of(self, i: int, end: int) -> int:
        """The `{` of the item body that the scan from `i` reaches, or -1."""
        while i < end:
            val = self.v[i]
            if val == "{":
                return i
            if val == ";":
                return -1
            i = self.close[i] + 1 if val in ("(", "[") else i + 1
        return -1

    # -- items -------------------------------------------------------------------------------------

    def module(self, i: int, end: int, scope: str, test: bool) -> None:
        """The items of one module scope, the file's own or an inline module's body."""
        while i < end:
            attrs_start = i
            attrs, i = self.attributes(i)
            if not scope:
                self.facts.crate_attrs += "".join(" ".join(self.v[lo:hi]) + ";" for inner, lo, hi in attrs
                                                  if inner or self.attr_path((inner, lo, hi))[-1:]
                                                  in (["global_allocator"], ["panic_handler"]))
            if i >= end:
                break
            outer = [a for a in attrs if not a[0]]
            # Test code: under `cfg(test)`, and a test function itself, which only a test build compiles
            # (so it may name dev-dependencies).
            item_test = test or any(self.cfg_test(a) or self.test_attr(a) for a in outer)
            start = i
            self.module_items.add(i)
            self.pending = {}
            i = self.item(i, end, scope, item_test, outer, module=True, attrs_start=attrs_start)
            if any(set(self.attr_path(a)[-1:]) & _GLOBAL_ATTRS for a in outer):
                self.pending["generates"] = "global"  # a global allocator or a panic handler
            if item_test:
                self.mark(self.test, start, i, True)
            pending = self.pending
            if pending.get("kind"):
                item = _Item(attrs_start, i, str(pending["kind"]), str(pending.get("name", "")),
                             str(pending.get("vis", "private")), scope, item_test,
                             int(pending.get("testfn", 0)), str(pending.get("generates", "")),
                             bool(pending.get("raw", False)))
                self.items.append(item)
                index = len(self.items) - 1
                for name, kind, vis in pending.get("defs", []):  # type: ignore[union-attr]
                    self.facts.defs.append([name, kind, vis, scope, index])
                if "impl" in pending:
                    head, header, trait = pending["impl"]  # type: ignore[misc]
                    self.facts.impls.append([index, scope, 1 if item_test else 0, head, header, trait])
                    for member_start, member_end, member in pending.get("members", []):  # type: ignore[union-attr]
                        self.items.append(_Item(member_start, member_end, "member", f"{item.name}::{member}",
                                                "private", scope, item_test, parent=index))
                if "macro" in pending:
                    name, exported, body, makes, calls, methods, impls, defines = pending["macro"]  # type: ignore[misc]
                    self.facts.macros.append([name, exported, body, makes, index, calls, methods, 1 if impls else 0,
                                              defines])

    def note(self, **info: object) -> None:
        self.pending.update(info)

    def item(self, i: int, end: int, scope: str, test: bool, attrs: List[Tuple[bool, int, int]],
             module: bool, attrs_start: int = 0) -> int:
        """Read the item at `i` (after its attributes); return where it ends. Inside a body
        (`module` false) its items are local: they belong to the item around them."""
        vis, i = self.visibility(i)
        vis = vis if module else "private"
        i = self.qualifiers(i)
        kw = self.val(i)
        local = not module

        at = i

        def define(name: str, kind: str) -> None:
            if local:
                self.local_defs.append([name, "local", "private", scope, at])
            else:
                self.pending.setdefault("defs", []).append((name, kind, vis))  # type: ignore[union-attr]

        if kw == "fn" and self.ident(i + 1):
            name = self.v[i + 1]
            define(name, "fn")
            testfn = 0
            generates = ""
            if module:
                paths = [self.attr_path(a) for a in attrs]
                kinds = {self.test_attr(a) for a in attrs} - {""}
                if kinds:
                    testfn = 2 if any(p == ["ignore"] for p in paths) else 1
                    generates = "cases" if kinds & _CASE_ATTRS else ""
            self.note(kind="fn", name=name, vis=vis, testfn=testfn, raw=self.raw[i + 1], generates=generates)
            return self.skip_item(i + 2, end)
        if kw in ("struct", "enum", "trait") or (kw == "union" and self.ident(i + 1)):
            name = self.v[i + 1] if self.ident(i + 1) else ""
            if name:
                define(name, kw)
                body = self.body_of(i + 2, end)
                if kw == "enum" and body >= 0:
                    self.facts.variants[name] = self.variants(body)
                if kw == "trait" and body >= 0:
                    self.bodies[body] = "trait"
            generates = "ts" if any(self.attr_path(a) == ["ts"] and "export" in self.v[a[1]:a[2]] for a in attrs) else ""
            self.note(kind=kw, name=name, vis=vis, generates=generates, raw=self.raw[i + 1] if name else False)
            return self.skip_item(i + 2, end)
        if kw == "type" and self.ident(i + 1):
            name = self.v[i + 1]
            define(name, "type")
            stop = self.skip_item(i + 2, end, stop_at_brace=False)
            eq = next((j for j in range(i + 2, stop) if self.v[j] == "="), None)
            if eq is not None:
                self.facts.aliases.setdefault(name, []).extend(self.paths_in(eq + 1, stop - 1))
            self.note(kind="type", name=name, vis=vis)
            return stop
        if kw in ("const", "static"):
            j = i + 1
            if self.val(j) == "mut":
                j += 1
            if self.ident(j) or self.val(j) == "_":
                name = self.v[j] if self.val(j) != "_" else "_"
                if name != "_":
                    define(name, kw)
                self.note(kind=kw, name=name, vis=vis)
                return self.skip_item(j + 1, end, stop_at_brace=False)
        if kw == "mod" and self.ident(i + 1):
            name = self.v[i + 1]
            if self.val(i + 2) == ";" and module:
                path_attr = next((self.v[lo + 2] for _, lo, _ in attrs if self.val(lo) == "path"
                                  and self.val(lo + 1) == "=" and self.k[lo + 2] == "s"), "")
                attr_text = " ".join(" ".join(self.v[lo:hi]) for _, lo, hi in attrs)
                self.facts.mods.append([name, path_attr, scope, 1 if test else 0, f"{attr_text} {vis}"])
                # Compared declaration by declaration (`Graph.select`), so no item's digest holds it.
                self.mark(self.in_use, attrs_start, i + 3, True)
                return i + 3
            if self.val(i + 2) == "{":
                body = i + 2
                inner = f"{scope}::{name}" if scope else name
                if module:
                    self.note(kind="mod", name=name, vis=vis, defs=[(name, "mod", vis)])
                    pending = self.pending
                else:
                    self.local_defs.append([name, "local", "private", scope, at])
                    pending = {}
                self.bodies[body] = "mod"
                self.mark(self.scope, body, self.close[body] + 1, inner)  # before its own inline modules
                self.module(body + 1, self.close[body], inner, test)
                self.pending = pending
                return self.close[body] + 1
        if kw == "use":
            stop = self.skip_item(i + 1, end, stop_at_brace=False)
            self.mark(self.skip, i, stop, True)
            self.mark(self.in_use, attrs_start if module else i, stop, True)
            # No digest holds a `use` (`Graph.changed_nodes` compares them one by one), so each import
            # carries its attributes: a changed `cfg` changes the import.
            self._attr_text = " ".join(" ".join(self.v[lo:hi]) for _, lo, hi in attrs)
            self.use_tree(i + 1, stop - 1, [], scope, test)
            self._attr_text = ""
            return stop
        if kw == "impl":
            j = i + 1
            while j < end and self.v[j] not in ("{", ";"):
                j = self.close[j] + 1 if self.v[j] in "([" else j + 1
            head, header, trait = self.impl_header(i + 1, j)
            name = "impl " + " ".join(self.v[i + 1 : j])
            if module:
                self.note(kind="impl", name=name, vis="private", impl=(head, header, trait),
                          members=self.members(j) if self.val(j) == "{" else [])
            else:
                self.local_impls.append([i, scope, 1 if test else 0, head, header, trait])
            if self.val(j) == "{":
                self.bodies[j] = "impl"
                return self.close[j] + 1
            return j + 1
        if kw == "extern":
            if self.val(i + 1) == "crate":
                stop = self.skip_item(i + 2, end, stop_at_brace=False)
                if self.ident(i + 2):
                    alias = self.v[stop - 2] if self.val(stop - 3) == "as" else self.v[i + 2]
                    self._uses.add((scope, 1 if test else 0, alias, self.v[i + 2]))
                return stop
            stop = self.skip_item(i + 1, end)  # a foreign block: no workspace code runs there
            self.note(kind="extern", name="extern", vis=vis)
            return stop
        if kw == "macro_rules" and self.val(i + 1) == "!" and self.ident(i + 2):
            name = self.v[i + 2]
            body = i + 3
            if self.val(body) in ("(", "[", "{"):
                stop = self.close[body]
                self.mark(self.skip, i, stop + 1, True)
                exported = any(self.attr_path(a) == ["macro_export"] for a in attrs)
                # Whether it makes tests: a test attribute in its body (`#[test]`, `#[tokio::test]`).
                makes = any(self.v[x] == "#" and self.val(x + 1) == "[" and self.test_attr((False, x + 2, self.close[x + 1]))
                            for x in range(body, stop))
                calls = sorted({self.v[x] for x in range(body, stop) if self.k[x] == "i" and self.val(x + 1) == "!"
                                and self.val(x - 1) != "$" and self.val(x + 2) != "="})
                # The methods and associated items its body names, which its callers name: `.name`,
                # and any segment after `::` (`Self::name`, `$crate::Type::name`, `<$t as Tr>::name`).
                methods = sorted({self.v[x] for x in range(body, stop) if self.k[x] == "i"
                                  and self.val(x - 1) in (".", "::")})
                impls = any(self.v[x] == "impl" for x in range(body, stop))
                # The items its body defines by a name of its own (`pub const ALL: ... = ...`), which
                # a call in item position defines where it is called.
                defines = sorted({self.v[x + 1] for x in range(body, stop - 1) if self.v[x] in _DEFINES
                                  and self.ident(x + 1) and self.v[x + 1] != "_"})
                record = [name, 1 if exported else 0, self.paths_in(body + 1, stop, macro=True), 1 if makes else 0]
                if module:
                    self.note(kind="macro", name=name, vis="pub" if exported else "crate",
                              macro=(*record, calls, methods, impls, defines))
                else:
                    self.local_macros.append([*record, i, calls, methods, impls, defines])
                return stop + 2 if self.val(stop + 1) == ";" else stop + 1
        if i < self.n and self.k[i] == "i":
            # A macro called in item position, `path::name! { ... }`: it may define items, tests among them.
            j = i
            while self.k[j] == "i" and self.val(j + 1) == "::" and j + 2 < self.n and self.k[j + 2] == "i":
                j += 2
            if self.val(j + 1) == "!" and self.val(j + 2) in ("(", "[", "{"):
                group = j + 2
                stop = self.close[group]
                names = sorted({self.v[x] for x in range(group, stop)
                                if self.k[x] == "i" and self.v[x] not in KEYWORDS and self.val(x - 1) != "$"})
                if module:
                    self.note(kind="call", name=f"{self.v[j]}!", vis="pub",
                              defs=[(name, "maybe", "pub") for name in names], generates="tests")
                return stop + 2 if self.val(stop + 1) == ";" else stop + 1
        if kw == ";":
            return i + 1
        if module:
            self.unexpected += 1
        return i + 1

    def members(self, body: int) -> List[Tuple[int, int, str]]:
        """An impl block's associated items: (start, end, name), attributes included."""
        out: List[Tuple[int, int, str]] = []
        k = body + 1
        stop = self.close[body]
        while k < stop:
            start = k
            _, k = self.attributes(k)
            if k >= stop:
                break
            k = self.qualifiers(self.visibility(k)[1])
            kw = self.val(k)
            if kw == "fn" and self.ident(k + 1):
                name, k = self.v[k + 1], self.skip_item(k + 2, stop)
            elif kw in ("const", "type") and (self.ident(k + 1) or self.val(k + 1) == "_"):
                name, k = self.v[k + 1], self.skip_item(k + 2, stop, stop_at_brace=False)
            elif self.ident(k) and self.val(k + 1) == "!" and self.val(k + 2) in ("(", "[", "{"):
                name = self.v[k] + "!"
                k = self.close[k + 2] + 1
                k += 1 if self.val(k) == ";" else 0
            else:
                k += 1
                continue
            out.append((start, k, name))
        return out

    def variants(self, body: int) -> List[str]:
        out = []
        j = body + 1
        stop = self.close[body]
        expect = True
        while j < stop:
            val = self.v[j]
            if val == "#" and self.val(j + 1) == "[":
                j = self.close[j + 1] + 1
                continue
            if expect and self.k[j] == "i":
                out.append(val)
                expect = False
            elif val == ",":
                expect = True
            if val in "([{":
                j = self.close[j] + 1
                continue
            j += 1
        return out

    def use_tree(self, i: int, end: int, prefix: List[str], scope: str, test: bool) -> int:
        """Record each leaf of a use tree as an import (its alias and path) or a glob."""
        path = list(prefix)
        t = 1 if test else 0
        while i < end:
            val = self.v[i]
            if val == "::":
                if not path and i == end - 1:
                    break
                i += 1
            elif val == "*":
                self.facts.globs.append([scope, t, *path])
                if self._attr_text:
                    self._use_attrs.setdefault((scope, t, "*", *path), set()).add(self._attr_text)
                i += 1
            elif val == "{":
                stop = self.close[i]
                j = i + 1
                while j < stop:
                    j = self.use_tree(j, stop, path, scope, test)
                    if j < stop and self.v[j] == ",":
                        j += 1
                i = stop + 1
            elif val == ",":
                return i
            elif self.k[i] == "i":
                path.append(val)
                i += 1
                if i < end and self.v[i] == "as":
                    alias = self.val(i + 1)
                    i += 2
                elif i >= end or self.v[i] in (",", "}"):
                    alias = path[-2] if path[-1] == "self" and len(path) > 1 else path[-1]
                else:
                    continue
                full = path[:-1] if path[-1] == "self" else path
                if full:
                    record = (scope, t, alias if alias != "_" else "", *full)
                    self._uses.add(record)
                    if self._attr_text:
                        self._use_attrs.setdefault(record, set()).add(self._attr_text)
            else:
                i += 1
        return i

    def impl_header(self, i: int, end: int) -> Tuple[List[str], List[List[str]], List[str]]:
        """An impl's self type's path ([] if generic or not a path), the paths of its header, and its
        trait's path ([] for an inherent impl)."""
        params: Set[str] = set()
        if self.val(i) == "<":
            stop = _angle_end(self.v, i, end)
            depth = 0
            for j in range(i, stop):
                val = self.v[j]
                if val == "<":
                    depth += 1
                elif val == ">":
                    depth -= 1
                elif depth == 1 and self.k[j] == "i" and self.val(j - 1) in ("<", ",", "const"):
                    if val != "const":
                        params.add(val)
            i = stop
        depth = 0
        for_at, where_at = None, end
        for j in range(i, end):
            val = self.v[j]
            if val in ("<", "(", "["):
                depth += 1
            elif val in (">", ")", "]"):
                depth -= 1
            elif depth == 0 and val == "for" and self.val(j + 1) != "<" and for_at is None:
                for_at = j
            elif depth == 0 and val == "where":
                where_at = j
                break
        j = (for_at + 1) if for_at is not None else i
        while j < where_at and (self.v[j] in ("&", "!", "mut") or self.k[j] == "l"):
            j += 1
        head: List[str] = []
        if j < where_at and self.k[j] == "i" and self.v[j] not in ("dyn", "impl", "fn", "unsafe", "extern"):
            head = [self.v[j]]
            while self.val(j + 1) == "::" and j + 2 < self.n and self.k[j + 2] == "i":
                j += 2
                head.append(self.v[j])
            if len(head) == 1 and head[0] in params:
                head = []
        paths = [p for p in self.paths_in(i, end) if not (len(p) == 1 and p[0] in params)]
        trait: List[str] = []
        if for_at is not None:
            t = i
            while t < for_at and self.v[t] in ("!", "?", "const"):
                t += 1
            if t < for_at and (self.k[t] == "i" or self.v[t] == "::"):
                trait, _ = self.path_at(t, for_at) if self.k[t] == "i" else ([""], t)
            trait = trait or ["?"]  # a trait the reading cannot name: not the workspace's, for sure
        return head, paths, trait

    # -- items inside bodies -----------------------------------------------------------------------

    def body_items(self) -> None:
        """Items and imports inside function bodies (and impl blocks' methods): local, but real."""
        stack: List[str] = []
        starters = _ITEMS | {"pub", "unsafe", "async", "default", "safe", "auto"}
        for i in range(self.n):
            kind, val = self.k[i], self.v[i]
            if kind == "p" and val == "{":
                stack.append(self.bodies.get(i, "block"))
                continue
            if kind == "p" and val == "}":
                if stack:
                    stack.pop()
                continue
            if val not in starters or not stack or stack[-1] != "block" or i in self.module_items or self.skip[i]:
                continue
            if i and self.v[i - 1] not in (";", "{", "}", "]"):
                continue
            j = self.qualifiers(self.visibility(i)[1])
            kw = self.val(j)
            if kw not in _ITEMS or (kw == "const" and self.val(j + 1) == "{"):
                continue
            if kw in ("fn", "struct", "enum", "union", "trait", "type", "mod") and not self.ident(j + 1):
                continue
            if kw in ("const", "static") and not (self.ident(j + 1) or self.val(j + 1) in ("mut", "_")):
                continue
            self.item(i, self.n, self.scope[i], self.test[i], [], module=False)

    # -- paths -------------------------------------------------------------------------------------

    def path_at(self, i: int, end: int) -> Tuple[List[str], int]:
        """The path that starts at token `i`, and the index after it (generic arguments skipped)."""
        segments = [self.v[i]]
        j = i + 1
        while j + 1 < end and self.v[j] == "::":
            nxt = j + 1
            if self.v[nxt] == "<":
                j = _angle_end(self.v, nxt, end)
                continue
            if self.k[nxt] == "i" and (self.v[nxt] not in KEYWORDS or self.raw[nxt]) or self.v[nxt] in ("super", "self"):
                segments.append(self.v[nxt])
                j = nxt + 1
                continue
            break
        return segments, j

    def starts_path(self, i: int) -> bool:
        if self.k[i] != "i":
            return False
        val = self.v[i]
        if val in KEYWORDS and val not in PATH_ROOTS and not self.raw[i]:
            return False
        prev = self.v[i - 1] if i else ""
        return prev not in (".", "::", "$", "'") and prev not in _DECLARES

    def paths_in(self, start: int, stop: int, macro: bool = False) -> List[List[str]]:
        """The paths between two tokens, for a macro body, a type alias or an impl's header."""
        found: Set[tuple] = set()
        i = start
        while i < stop:
            # Each path is read from its first token, and the scan goes on from its second: its
            # generic arguments (`$crate::run::<$crate::X>()`) are paths too.
            if macro and self.v[i] == "$" and self.val(i + 1) == "crate" and self.val(i + 2) == "::":
                segments, j = self.path_at(i + 1, stop)
                found.add(("$crate", *segments[1:]))
                i += 2
                continue
            if macro and self.k[i] == "s" and "{" in self.v[i]:
                found.update((name,) for name in _CAPTURE.findall(self.v[i].replace("{{", "")) if name not in KEYWORDS)
            if self.starts_path(i) and not (self.val(i + 1) == "!" and self.val(i + 2) != "="):
                segments, j = self.path_at(i, stop)
                found.add(tuple(segments))
            i += 1
        return [list(p) for p in sorted(found)]

    def finish(self) -> None:
        """Every token's item; then each item's paths, macros, literals and digest."""
        item_of = [0] * self.n
        order = sorted(range(1, len(self.items)), key=lambda x: (self.items[x].start, -self.items[x].end))
        for index in order:
            item = self.items[index]
            for j in range(item.start, min(item.end, self.n)):
                item_of[j] = index
        self.item_of = item_of
        count = len(self.items)
        paths: List[Set[tuple]] = [set() for _ in range(count)]
        calls: List[Set[str]] = [set() for _ in range(count)]
        literals: List[Set[str]] = [set() for _ in range(count)]
        methods: List[Set[str]] = [set() for _ in range(count)]  # `.name` and `Self::name`
        digests = [hashlib.sha1() for _ in range(count)]
        k, v = self.k, self.v
        # Attributes anywhere, a field's and a variant's too (`#[serde(serialize_with = "path")]`).
        in_attr = [False] * self.n
        for j in range(self.n - 1):
            if k[j] == "p" and v[j] == "#":
                bracket = j + 2 if v[j + 1] == "!" else j + 1
                if bracket < self.n and v[bracket] == "[" and self.close[bracket] > bracket:
                    self.mark(in_attr, bracket, self.close[bracket], True)
        for j in range(self.n):
            owner = item_of[j]
            if not self.in_use[j]:
                digests[owner].update(f"{k[j]}{v[j]}\0".encode("utf-8", "surrogatepass"))
            if k[j] == "i" and j and v[j - 1] in (".", "::") and not self.skip[j]:
                methods[owner].add(v[j])  # a method's or an associated item's name: `x.m()`, `<T as Tr>::m`

        def arguments(group: int, owner: int) -> None:
            # A macro call's words may be what its body calls methods by (`$o.$m()`): mentions.
            if self.val(group) in ("(", "[", "{"):
                methods[owner].update(v[x] for x in range(group + 1, self.close[group]) if k[x] == "i")

        i = 0
        while i < self.n:
            kind = k[i]
            owner = item_of[i]
            if kind == "s":
                text = v[i]
                test = 1 if self.test[i] else 0
                if ("/" in text or re.search(r"\.\w{1,8}$", text) or _WORD.match(text)) and len(text) <= 300 \
                        and "\n" not in text:
                    literals[owner].add(text)  # a path, or a word that may be one of a path's directories
                if (self.skip[i] or in_attr[i]) and not self.in_use[i] and _ATTR_PATH.match(text):
                    # A path in an attribute's string, as a derive's `default = "path::to::fn"` names one;
                    # `with = "module"` names the module's `serialize` and `deserialize`.
                    segments = text.split("::")
                    paths[owner].add((test, *segments))
                    if self.val(i - 1) == "=" and self.val(i - 2) in _WITH_MODULE:
                        paths[owner].update({(test, *segments, "serialize"), (test, *segments, "deserialize")})
                elif not self.skip[i] and "{" in text:
                    # A format string's inline arguments, `format!("{ROWS} rows")`: paths of one segment.
                    for name in _CAPTURE.findall(text.replace("{{", "")):
                        if name not in KEYWORDS:
                            paths[owner].add((test, name))
                i += 1
                continue
            if kind == "p" and v[i] == "::" and i + 1 < self.n and k[i + 1] == "i" and not self.skip[i] \
                    and (i == 0 or (k[i - 1] == "i" and v[i - 1] in KEYWORDS and v[i - 1] not in PATH_ROOTS
                                    and v[i - 1] != "Self" and not self.raw[i - 1])
                         or (k[i - 1] not in ("i", "n") and v[i - 1] not in (">", ")", "]"))):
                # A path from the extern prelude, `::a::helper()`.
                segments, j = self.path_at(i + 1, self.n)
                if self.val(j) != "!":
                    paths[owner].add((1 if self.test[i] else 0, "", *segments))
                i += 1
                continue
            if self.skip[i] or not self.starts_path(i):
                i += 1
                continue
            test = 1 if self.test[i] else 0
            if self.val(i + 1) == "!" and self.val(i + 2) != "=":
                calls[owner].add(v[i])
                arguments(i + 2, owner)
                i += 2
                continue
            segments, j = self.path_at(i, self.n)
            if self.val(j) == "!" and self.val(j + 1) != "=":
                calls[owner].add(segments[-1])
                arguments(j + 1, owner)
            elif segments[0] != "Self":
                paths[owner].add((test, *segments))
            else:
                methods[owner].update(segments[1:])  # `Self::name`: an associated item of the impl's type
            # On from the path's second token, not its end: its generic arguments (a turbofish,
            # `parse::<a::X>()`) name paths of their own. Its later segments start no path.
            i += 1
        facts = self.facts
        seen: Dict[str, int] = {}
        for index, item in enumerate(self.items):
            base = f"{item.kind}:{item.scope}:{item.name}"
            seen[base] = seen.get(base, 0) + 1
            ident = base if seen[base] == 1 else f"{base}~{seen[base]}"
            facts.items.append([ident, item.kind, item.name, item.vis, item.scope, 1 if item.test else 0,
                                digests[index].hexdigest(), item.testfn, item.generates, 1 if item.raw else 0,
                                item.parent])
            facts.item_paths.append([list(p) for p in sorted(paths[index])])
            facts.item_calls.append(sorted(calls[index]))
            facts.item_literals.append(sorted(literals[index]))
            facts.item_methods.append(sorted(methods[index]))
        for name, kind, vis, scope, at in self.local_defs:
            facts.defs.append([name, kind, vis, scope, item_of[at]])
        for at, scope, test, head, header, trait in self.local_impls:
            facts.impls.append([item_of[at], scope, test, head, header, trait])
        for name, exported, body, makes, at, calls, macro_methods, impls, defines in self.local_macros:
            facts.macros.append([name, exported, body, makes, item_of[at], calls, macro_methods, 1 if impls else 0,
                                 defines])
        facts.uses = [list(u) for u in sorted(self._uses)]
        facts.use_attrs = [[" | ".join(sorted(texts)), *record] for record, texts in sorted(self._use_attrs.items())]


def snippet(text: str) -> Tuple[List[List[str]], List[List[str]], List[str], List[str], List[List[str]]]:
    """A doctest's imports ([alias, *segments]), the paths its code names, the macros it calls, the
    methods and its glob imports, read as rustdoc compiles it: the body of a function."""
    facts = parse("fn doctest() {\n" + text + "\n}", doc=False, test=True)
    if not facts.ok:
        raise LexError(facts.problem)
    paths = sorted({tuple(p[1:]) for item in facts.item_paths for p in item})
    calls = sorted({c for item in facts.item_calls for c in item})
    methods = sorted({m for item in facts.item_methods for m in item})
    globs = sorted({tuple(g[2:]) for g in facts.globs if len(g) > 2})
    return [u[2:] for u in facts.uses], [list(p) for p in paths], calls, methods, [list(g) for g in globs]


def doctests(docs: List[Tuple[int, str, str]]) -> List[list]:
    """The code blocks of the doc comments that rustdoc compiles: [line, imports, paths, macros]."""
    out: List[list] = []
    fence: Optional[str] = None
    code: List[str] = []
    start = 0
    rust = False
    last = (-2, "")
    for line, kind, text in docs:
        if (line, kind) != (last[0] + 1, last[1]) and fence is not None:
            fence = None  # a new comment block: an unclosed fence ends with the old one
        last = (line, kind)
        stripped = text[1:] if text.startswith(" ") else text
        opener = re.match(r"^\s*(```+|~~~+)\s*(.*)$", stripped)
        if fence is None:
            if opener:
                fence, start, code = opener.group(1), line, []
                info = [w for w in re.split(r"[\s,]+", opener.group(2).strip()) if w and not w.startswith("{")]
                rust = "ignore" not in info and all(w in _RUST_DOC_ATTRS for w in info)
            continue
        if opener and opener.group(1).startswith(fence[0] * len(fence)) and not opener.group(2).strip():
            if rust:
                body = "\n".join(re.sub(r"^(\s*)# ?", r"\1", row) if re.match(r"^\s*#(\s|$)", row) else row
                                 for row in code)
                try:
                    imports, paths, calls, methods, globs = snippet(body)
                except (LexError, IndexError, KeyError):
                    # Unreadable: every word of it, by name, a method's name too.
                    words = sorted(set(re.findall(r"[^\W\d]\w*", body)))
                    imports, paths, calls, methods, globs = [], [["?"]] + [[w] for w in words], [], words, []
                out.append([start, imports, paths, calls, methods, globs])
            fence = None
            continue
        code.append(stripped)
    return out


def _scanned(lexed: Lexed, test: bool) -> _Scanner:
    """A file's tokens read into its items, paths and facts (`parse`, `item_tokens`)."""
    scanner = _Scanner(lexed)
    scanner.module(0, scanner.n, "", test)
    scanner.body_items()
    scanner.finish()
    return scanner


def parse(text: str, doc: bool = True, test: bool = False) -> FileFacts:
    """What one file says. A file the reader cannot follow is `ok = False`, and widens to its crate."""
    try:
        lexed = lex(text)
        scanner = _scanned(lexed, test)
        facts = scanner.facts
        if doc:
            facts.doctests = doctests(lexed.docs)
    except (LexError, IndexError, KeyError) as err:
        return FileFacts(ok=False, problem=f"{type(err).__name__}: {err}")
    if scanner.unexpected:
        facts.ok = False
        facts.problem = f"{scanner.unexpected} token(s) in a module scope that no item rule reads"
    return facts


def item_tokens(text: str) -> Dict[str, List[Tuple[str, str]]]:
    """Each item's own tokens, `(kind, value)` as `lex` gives them, by the item's id in `parse`'s facts
    (`FileFacts.items`, whose node is `item_node(file, id)`): an item holds its attributes and the
    items inside its bodies, not the members or items it declares (an impl's, an inline module's),
    and not its `use` and `mod` declarations, which name items (the graph's import nodes) and run
    nothing. Empty for a file the reader cannot follow."""
    try:
        scanner = _scanned(lex(text), False)
    except (LexError, IndexError, KeyError):
        return {}
    ids = [item[0] for item in scanner.facts.items]
    out: Dict[str, List[Tuple[str, str]]] = {ident: [] for ident in ids}
    for j, owner in enumerate(scanner.item_of):
        if not scanner.in_use[j]:
            out[ids[owner]].append((scanner.k[j], scanner.v[j]))
    return out


# --------------------------------------------------------------------------------------------------
# The cache.

_SELF_HASH = hashlib.sha1(Path(__file__).read_bytes()).hexdigest()


def blob_id(data: bytes) -> str:
    """git's object id of a file's content, so that a file git knows unchanged needs no reading."""
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def index_ids(stage: str, dirty: str) -> Dict[str, str]:
    """The object ids of the files whose working-tree content is the index's, from `git ls-files -z
    --stage` and `git diff-files -z --name-only` (which names every other one: a file it is unsure
    of is read)."""
    changed = set(dirty.split("\0"))
    ids: Dict[str, str] = {}
    for entry in stage.split("\0"):
        meta, _, path = entry.partition("\t")
        fields = meta.split()
        if path and len(fields) == 3 and fields[2] == "0" and path not in changed:
            ids[path] = fields[1]
    return ids


class Facts:
    """Each file's facts, parsed once per content and kept under `target/smart-ci/` between runs.

    An entry is keyed by the git object id of the file's content, and the whole cache by this
    module's own source: a changed reader or a changed file is never served a stale entry. `ids`
    gives the ids of the files the working tree has as git's index has them, so that only the files
    that differ are read.
    """

    def __init__(self, cache: Optional[Path], read: Callable[[str], Optional[bytes]],
                 ids: Optional[Dict[str, str]] = None) -> None:
        self.path = cache
        self.read = read
        self.ids = ids or {}
        self.entries: Dict[str, dict] = {}
        self.parsed = self.cached = 0
        self.dirty = False
        if cache is not None:
            try:
                data = json.loads(cache.read_text(encoding="utf-8"))
                if data.get("parser") == _SELF_HASH:
                    self.entries = data["files"]
            except (OSError, ValueError, KeyError, TypeError):
                self.entries = {}
        self.memo: Dict[str, FileFacts] = {}

    def get(self, path: str, data: Optional[bytes] = None) -> Optional[FileFacts]:
        """The facts of `path` (read from the working tree unless `data` is given); None if absent."""
        digest = self.ids.get(path) if data is None else None
        if digest is None or (digest not in self.memo and digest not in self.entries):
            if data is None:
                data = self.read(path)
                if data is None:
                    return None
            digest = blob_id(data)
        if digest in self.memo:
            return self.memo[digest]
        entry = self.entries.get(digest)
        if entry is not None:
            facts = FileFacts.from_json(entry)
            self.cached += 1
        else:
            if data is None:
                data = self.read(path)
                if data is None:
                    return None
            facts = parse(data.decode("utf-8", errors="replace"))
            self.entries[digest] = facts.to_json()
            self.parsed += 1
            self.dirty = True
        self.memo[digest] = facts
        return facts

    def save(self) -> None:
        """Write the cache, keeping the entries of the contents this run read."""
        if self.path is None or not self.dirty:
            return
        keep = {digest: self.entries[digest] for digest in self.memo if digest in self.entries}
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            temporary = self.path.with_suffix(f".{os.getpid()}.tmp")
            temporary.write_text(json.dumps({"parser": _SELF_HASH, "files": keep}), encoding="utf-8")
            os.replace(temporary, self.path)
        except OSError:
            pass


# --------------------------------------------------------------------------------------------------
# The workspace's units.


@dataclass
class Target:
    """A cargo target of a workspace crate."""

    kind: str  # lib (a proc-macro or cdylib included), bin, test, bench, example
    name: str
    src: str  # repository-relative root file
    runs: bool  # whether `cargo test` (and nextest) runs it: cargo metadata's `test`
    doctest: bool = False


@dataclass
class CrateInfo:
    name: str
    dir: str
    normal: List[str]  # in-workspace normal and build dependencies
    dev: List[str]  # in-workspace dev-dependencies
    targets: List[Target]
    externs: List[str] = field(default_factory=list)  # the names its code gives other dependencies


@dataclass
class Unit:
    """A compilation unit: one target's module tree. `id` is nextest's binary id."""

    id: str
    crate: str
    kind: str
    root: str
    runs: bool
    files: Dict[str, str] = field(default_factory=dict)  # file -> module path ("" for the root)
    children: Dict[str, List[str]] = field(default_factory=dict)
    test_files: Set[str] = field(default_factory=set)  # files under #[cfg(test)] or in a test target

    @property
    def is_lib(self) -> bool:
        return self.kind == "lib"


def crates_from_metadata(meta: dict) -> Dict[str, CrateInfo]:
    """The workspace's crates, their in-workspace dependencies and targets, from `cargo metadata`."""
    root = Path(meta["workspace_root"]).resolve()
    members = set(meta["workspace_members"])
    packages = [p for p in meta["packages"] if p["id"] in members]
    by_dir = {Path(p["manifest_path"]).resolve().parent: p["name"] for p in packages}
    crates: Dict[str, CrateInfo] = {}
    for package in packages:
        directory = Path(package["manifest_path"]).resolve().parent
        normal: List[str] = []
        dev: List[str] = []
        externs: List[str] = []
        for dep in package["dependencies"]:
            member = by_dir.get(Path(dep["path"]).resolve()) if dep.get("path") else None
            if member and member != package["name"]:
                (dev if dep["kind"] == "dev" else normal).append(member)
            else:
                externs.append((dep.get("rename") or dep["name"]).replace("-", "_"))
        targets = []
        for target in package["targets"]:
            kind = target["kind"][0]
            if kind in ("rlib", "dylib", "cdylib", "staticlib", "proc-macro"):
                kind = "lib"
            if kind == "custom-build":
                continue
            try:
                rel = Path(target["src_path"]).resolve().relative_to(root).as_posix()
            except ValueError:
                continue
            targets.append(Target(kind, target["name"], rel, bool(target.get("test", kind != "bench")),
                                  bool(target.get("doctest", False))))
        crates[package["name"]] = CrateInfo(package["name"], directory.relative_to(root).as_posix(),
                                            sorted(set(normal)), sorted(set(dev) - set(normal)), targets,
                                            sorted(set(externs)))
    return crates


def build_scripts(meta: dict) -> Set[str]:
    """The workspace crates with a build script, whose every change reaches the whole crate."""
    members = set(meta["workspace_members"])
    return {p["name"] for p in meta["packages"] if p["id"] in members
            and any(t["kind"][0] == "custom-build" for t in p["targets"])}


def proc_macros(meta: dict) -> Set[str]:
    """The workspace's proc-macro crates: a change to one changes what its users' code expands to,
    which the graph cannot see (a derive's or an attribute's name is no path), so it reaches them all."""
    members = set(meta["workspace_members"])
    return {p["name"] for p in meta["packages"] if p["id"] in members
            and any("proc-macro" in t["kind"] for t in p["targets"])}


def binary_id(crate: str, target: Target) -> str:
    if target.kind == "lib":
        return crate
    if target.kind == "test":
        return f"{crate}::{target.name}"
    return f"{crate}::{target.kind}/{target.name}"


# --------------------------------------------------------------------------------------------------
# Nodes.


def item_node(file: str, ident: str) -> str:
    return f"{file}#{ident}"


def import_node(file: str, modpath: str, alias: str, segments: Sequence[str]) -> str:
    """The node of one `use` import: code that names an item through it depends on it."""
    return f"{file}#use:{modpath}:{alias or '_' + '::'.join(segments)}"


def glob_node(file: str, modpath: str, segments: Sequence[str]) -> str:
    return f"{file}#glob:{modpath}:{'::'.join(segments)}"


def traits_node(file: str, modpath: str) -> str:
    """The traits in scope in a module, for the methods its items call."""
    return f"{file}#traits:{modpath}"


def text_node(file: str) -> str:
    """A source file's text, for what reads it as data (`include_str!`): any change changes it."""
    return f"{file}#text"


def doc_node(file: str) -> str:
    return f"{file}#doctests"


def file_of(node: str) -> str:
    return node.split("#", 1)[0]


_NODE_KINDS = {"text": "its text", "doctests": "its doctests", "traits": "the traits in scope"}


def describe_node(node: str) -> str:
    """A node for people: `path (fn name)`."""
    file, _, rest = node.partition("#")
    if not rest:
        return file
    kind, _, tail = rest.partition(":")
    scope, _, name = tail.partition(":")
    if kind in ("use", "glob"):
        return f"{file} (the {'glob' if kind == 'glob' else 'import'} {name or scope})"
    if kind in _NODE_KINDS:
        return f"{file} ({_NODE_KINDS[kind]})"
    if kind == "file":
        return f"{file} (its module attributes)"
    shown = name.split("~")[0]
    return f"{file} ({kind} {'::'.join(p for p in (scope, shown) if p)})"


@dataclass
class Def:
    node: str  # the item's node
    file: str
    name: str
    kind: str
    vis: str
    modpath: str  # the module the item is in, within its file's units
    crate: str


@dataclass
class Module:
    """One module of a unit: its file, items, imports and globs, each import and glob with its node."""

    unit: str
    path: str
    file: str
    items: Dict[str, List[Def]] = field(default_factory=dict)
    imports: Dict[str, List[Tuple[List[str], str]]] = field(default_factory=dict)
    globs: List[Tuple[List[str], str]] = field(default_factory=list)


# What a name resolves to: an item, a module, a crate, or something outside the workspace.
Entry = Tuple[str, object]
EXTERNAL: Entry = ("ext", None)
# The standard library's crates and prelude, and the primitive types: names that resolve outside
# the workspace when nothing in scope shadows them.
STD = frozenset(("std", "core", "alloc", "proc_macro", "test"))
# The standard library's macros: a call of one names no workspace item.
STD_MACROS = frozenset("""
    assert assert_eq assert_ne debug_assert debug_assert_eq debug_assert_ne panic unreachable todo
    unimplemented print println eprint eprintln format format_args write writeln vec matches dbg
    concat stringify env option_env include include_str include_bytes file line column module_path
    cfg compile_error thread_local macro_rules
""".split())
PRELUDE = frozenset("""
    Option Some None Result Ok Err Vec String Box ToString ToOwned Clone Copy Send Sync Sized Unpin
    Drop Fn FnMut FnOnce Iterator IntoIterator DoubleEndedIterator ExactSizeIterator Extend Default
    Eq PartialEq Ord PartialOrd AsRef AsMut Into From TryFrom TryInto FromIterator Debug Hash
    bool char str u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize f32 f64 drop
""".split())


@dataclass
class TestFn:
    node: str
    name: str  # nextest's name of it within its binary
    module: str
    ignored: bool


@dataclass
class Selection:
    """What the change reaches: per unit its selected tests, and per crate its doctests."""

    tests: Dict[str, Dict[str, Set[str]]] = field(default_factory=dict)  # unit -> {test name: causes}
    # unit -> {a filter term for tests the source does not name: causes}
    generated: Dict[str, Dict[str, Set[str]]] = field(default_factory=dict)
    doctests: Dict[str, Dict[str, Set[str]]] = field(default_factory=dict)  # crate -> {file: causes}
    whole: Dict[str, Set[str]] = field(default_factory=dict)  # crates every test of which runs: why
    notes: List[str] = field(default_factory=list)
    via: Dict[str, List[str]] = field(default_factory=dict)  # "unit test" -> a chain to its first cause


# --------------------------------------------------------------------------------------------------
# The graph.


class Graph:
    """The workspace's items, imports and files, their units and modules, and the edges between them."""

    def __init__(self, crates: Dict[str, CrateInfo], files: Sequence[str], facts: Facts,
                 base_facts: Optional[Dict[str, Optional[FileFacts]]] = None,
                 golden_readers: Sequence[Tuple[str, str]] = ()) -> None:
        self.crates = crates
        self.base: Dict[str, Optional[FileFacts]] = dict(base_facts or {})
        self.known = set(files)
        self.golden_readers = list(golden_readers)
        self.crate_dirs = sorted(((c.dir, c.name) for c in crates.values()), key=lambda d: -len(d[0]))
        self.rs: Dict[str, FileFacts] = {}
        for path in files:
            if path.endswith(".rs") and self.crate_of(path):
                got = facts.get(path)
                if got is not None:
                    self.rs[path] = got
        self.idents = {name.replace("-", "_"): name for name in crates}
        self.units: Dict[str, Unit] = {}
        self.file_units: Dict[str, Set[str]] = {}
        self.missing_mods: List[str] = []
        self.modules: Dict[str, Dict[str, Module]] = {}  # unit id -> module path -> module
        self._build_units()
        self.orphans = sorted(f for f in self.rs if f not in self.file_units)
        self.allowed_normal = {name: self._closure([name]) for name in crates}
        self.allowed_test = {
            name: self.allowed_normal[name].union(*(self.allowed_normal[d] for d in crates[name].dev))
            for name in crates
        }
        self.lib_unit = {u.crate: u.id for u in self.units.values() if u.is_lib}
        self.edges: Dict[str, Set[str]] = {}
        self.glob_names: Dict[str, Set[str]] = {}  # glob node -> the per-name nodes made through it
        self.glob_targets: Dict[Tuple[str, str, str], Set[str]] = {}  # (unit, module, name) -> per-name nodes
        # (unit, module) -> the (file, scope, glob node) of each glob import of it
        self.glob_into: Dict[Tuple[str, str], Set[Tuple[str, str, str]]] = {}
        # The members of impls whose members code calls by name: member node -> (its impl, its name).
        self.split_members: Dict[str, Tuple[str, str]] = {}
        self.split_impls: Set[str] = set()
        self._lookups: Dict[tuple, List[Entry]] = {}
        self._resolved: Dict[tuple, Tuple[List[Entry], List[str]]] = {}
        self._active: Dict[tuple, int] = {}  # the lookups under way -> their position on the stack
        self._low = _NO_CYCLE  # the lowest position a cycle reached in what is being computed
        self.fallbacks = 0
        self._index()
        self._link()

    # -- structure ---------------------------------------------------------------------------------

    def crate_of(self, path: str) -> Optional[str]:
        """The crate a file (or a node's file) is in."""
        path = file_of(path)
        cache = self.__dict__.setdefault("_crate_cache", {})
        if path not in cache:
            cache[path] = next((name for directory, name in self.crate_dirs if path.startswith(directory + "/")), None)
        return cache[path]

    def _closure(self, roots: Iterable[str]) -> Set[str]:
        found = set(roots)
        frontier = list(found)
        while frontier:
            for dep in self.crates[frontier.pop()].normal:
                if dep not in found:
                    found.add(dep)
                    frontier.append(dep)
        return found

    def _build_units(self) -> None:
        for crate in self.crates.values():
            for target in crate.targets:
                if target.src not in self.rs:
                    continue
                unit = Unit(binary_id(crate.name, target), crate.name, target.kind, target.src, target.runs)
                self.units[unit.id] = unit
                self.modules[unit.id] = {}
                self._walk(unit, target.src, "", test=target.kind in ("test", "bench", "example"))

    def _walk(self, unit: Unit, path: str, modpath: str, test: bool) -> None:
        unit.files[path] = modpath
        self.file_units.setdefault(path, set()).add(unit.id)
        if test:
            unit.test_files.add(path)
        for name, path_attr, scope, cfg_test, *_ in self.rs[path].mods:
            child = next((c for c in self.mod_candidates(path, path == unit.root, name, path_attr, scope)
                          if c in self.rs), None)
            if child is None:
                self.missing_mods.append(f"{path}: mod {name}")
                continue
            if child in unit.files:
                continue
            unit.children.setdefault(path, []).append(child)
            self._walk(unit, child, "::".join(p for p in (modpath, scope, name) if p), test or bool(cfg_test))

    @staticmethod
    def mod_candidates(path: str, is_root: bool, name: str, path_attr: str, scope: str) -> List[str]:
        """Where `mod name;` in `path` (within the inline modules `scope`) finds its file."""
        here = posixpath.dirname(path)
        inline = scope.split("::") if scope else []
        if path_attr:
            return [posixpath.normpath(posixpath.join(here, *inline, path_attr))]
        mod_rs = is_root or posixpath.basename(path) in ("mod.rs", "lib.rs", "main.rs")
        children = here if mod_rs else posixpath.join(here, posixpath.splitext(posixpath.basename(path))[0])
        base = posixpath.join(children, *inline)
        return [posixpath.join(base, name + ".rs"), posixpath.join(base, name, "mod.rs")]

    def modpath(self, path: str) -> str:
        for unit_id in sorted(self.file_units.get(path, ())):
            return self.units[unit_id].files[path]
        return ""

    def is_test_file(self, path: str) -> bool:
        return any(path in self.units[u].test_files for u in self.file_units.get(path, ()))

    @staticmethod
    def join(*parts: str) -> str:
        return "::".join(p for p in parts if p)

    def file_node(self, path: str, facts: Optional[FileFacts] = None) -> str:
        facts = facts or self.rs.get(path)
        return item_node(path, facts.items[0][0] if facts and facts.items else "file::")

    # -- items and modules -------------------------------------------------------------------------

    def _index(self) -> None:
        """Every unit's modules, and the index of items by name for the fallback."""
        self.defs: Dict[str, List[Def]] = {}
        # name -> (file, exported, body paths, makes tests, node, the macros its body calls, the methods)
        # (and whether it makes impls, and the names its body defines items by)
        self.macros: Dict[str, List[Tuple[str, bool, List[List[str]], bool, str, List[str], List[str], bool,
                                          List[str]]]] = {}
        # A macro's other names: `pub use __seven as seven;` -> {"seven": {"__seven"}} (by name alone).
        self.macro_aliases: Dict[str, Set[str]] = {}
        for path, facts in self.rs.items():
            crate = self.crate_of(path) or ""
            units = sorted(self.file_units.get(path, ())) or [""]
            for name, exported, body, makes, index, *more in facts.macros:
                self.macros.setdefault(name, []).append((path, bool(exported), body, bool(makes),
                                                         item_node(path, facts.items[index][0]),
                                                         more[0] if more else [], more[1] if len(more) > 1 else [],
                                                         bool(more[2]) if len(more) > 2 else True,
                                                         more[3] if len(more) > 3 else []))
            for _, _, alias, *segments in facts.uses:
                if alias and segments and alias != segments[-1]:
                    self.macro_aliases.setdefault(alias, set()).add(segments[-1])
            for unit_id in units:
                base = self.units[unit_id].files[path] if unit_id else ""
                for name, kind, vis, scope, index in facts.defs:
                    full = self.join(base, scope)
                    d = Def(item_node(path, facts.items[index][0]), path, name, kind, vis, full, crate)
                    if unit_id == units[0]:
                        self.defs.setdefault(name, []).append(d)
                    if unit_id:
                        self.module(unit_id, full, path).items.setdefault(name, []).append(d)
                        if kind == "mod":
                            self.module(unit_id, self.join(full, name), path)
                if not unit_id:
                    continue
                self.module(unit_id, base, path)
                for scope, _, alias, *segments in facts.uses:
                    full = self.join(base, scope)
                    if alias:
                        self.module(unit_id, full, path).imports.setdefault(alias, []).append(
                            (segments, import_node(path, full, alias, segments)))
                for scope, _, *segments in facts.globs:
                    full = self.join(base, scope)
                    self.module(unit_id, full, path).globs.append((segments, glob_node(path, full, segments)))
        # An item-position macro call defines the items its macro's body names (`domain_tags! { ... }`
        # defines `ALL`), beside the words of its arguments: "maybe" items of its module.
        for path, facts in self.rs.items():
            calls = [item for item in facts.items if item[1] == "call"]
            if not calls:
                continue
            crate = self.crate_of(path) or ""
            units = sorted(self.file_units.get(path, ())) or [""]
            for item in calls:
                names = sorted({n for entry in self._macro_closure(item[2].rstrip("!")) for n in entry[8]})
                for unit_id in units:
                    full = self.join(self.units[unit_id].files[path] if unit_id else "", item[4])
                    for name in names:
                        d = Def(item_node(path, item[0]), path, name, "maybe", "pub", full, crate)
                        if unit_id == units[0]:
                            self.defs.setdefault(name, []).append(d)
                        if unit_id:
                            self.module(unit_id, full, path).items.setdefault(name, []).append(d)
        # A changed file's items at the base, for the fallback: a test that names an item a change
        # removed or renamed reaches the change.
        for path, old in self.base.items():
            if old is None or not old.items:
                continue
            crate = self.crate_of(path) or ""
            base = self.modpath(path)
            for name, kind, vis, scope, index in old.defs:
                node = item_node(path, old.items[index][0])
                if not any(d.node == node for d in self.defs.get(name, ())):
                    self.defs.setdefault(name, []).append(Def(node, path, name, kind, vis, self.join(base, scope), crate))
        # File modules are their parents' children.
        for unit in self.units.values():
            for parent, children in unit.children.items():
                for child in children:
                    childpath = unit.files[child]
                    parentpath, _, name = childpath.rpartition("::")
                    self.module(unit.id, parentpath, parent).items.setdefault(name, []).append(
                        Def(self.file_node(child), child, name, "mod", "pub", parentpath, unit.crate))

    def module(self, unit_id: str, path: str, file: str) -> Module:
        table = self.modules[unit_id]
        if path not in table:
            table[path] = Module(unit_id, path, file)
        return table[path]

    # -- resolution --------------------------------------------------------------------------------

    def lookup(self, unit_id: str, modpath: str, name: str, depth: int = 0) -> List[Entry]:
        """What `name` means in a module: its items and imports, then its globs."""
        key = (unit_id, modpath, name)
        if key in self._lookups:
            return self._lookups[key]
        if key in self._active:
            # A cycle of re-exports and globs: nothing further along it. What the lookups inside the
            # cycle find is incomplete until the cycle's first lookup ends, so only that one's is kept
            # (the lowest position on the stack that a cycle reached, as in Tarjan's algorithm).
            self._low = min(self._low, self._active[key])
            return []
        position = len(self._active)
        self._active[key] = position
        outer_low, self._low = self._low, _NO_CYCLE
        try:
            out = self._lookup(unit_id, modpath, name, depth)
        finally:
            del self._active[key]
        low = self._low
        if low >= position:  # complete: no cycle reached a lookup still under way below it
            self._lookups[key] = out
            low = _NO_CYCLE
        self._low = min(outer_low, low)
        return out

    def _lookup(self, unit_id: str, modpath: str, name: str, depth: int) -> List[Entry]:
        module = self.modules.get(unit_id, {}).get(modpath)
        out: List[Entry] = []
        if module is not None and depth < 16:
            for d in module.items.get(name, ()):
                if d.kind == "mod":
                    out.append(("mod", (unit_id, self.join(modpath, name))))
                else:
                    out.append(("def", d))
            for segments, node in module.imports.get(name, ()):
                # Through an import (a re-export, perhaps): the import is a dependency too, so that a
                # change to what it imports reaches whoever names the name through it. An import's
                # path never starts from the import itself (`pub use bearing::{bearing}` imports the
                # module's function): a first segment of its own name is the module's item or child.
                if segments[0] == name:
                    own = [e for e in out if e[0] in ("def", "mod")]
                    entries, rest = self._descend(own, segments, depth + 1) if own else ([], list(segments))
                else:
                    entries, rest = self.resolve_path(unit_id, modpath, segments, depth + 1)
                out += [("via", node)] + (entries if not rest else [("unresolved", name)])
            if not any(kind in ("def", "mod") for kind, _ in out):
                for segments, node in module.globs:
                    entries, rest = self.resolve_path(unit_id, modpath, segments, depth + 1)
                    if rest:
                        out.append(("unresolved", name))
                        continue
                    found: List[Entry] = []
                    for kind, value in entries:
                        if kind == "mod":
                            other_unit, other_path = value  # type: ignore[misc]
                            got = [e for e in self.lookup(other_unit, other_path, name, depth + 1) if e[0] != "ext"]
                            if any(k in ("def", "mod") for k, _ in got):
                                self.glob_targets.setdefault((other_unit, other_path, name), set()).add(f"{node}:{name}")
                            found += got
                        elif kind == "def":
                            enum = self.rs.get(value.file)  # type: ignore[union-attr]
                            if enum is not None and name in enum.variants.get(value.name, ()):  # type: ignore[union-attr]
                                found.append(("def", value))
                        elif kind == "crate":
                            lib = self.lib_unit.get(value)  # type: ignore[arg-type]
                            if lib:
                                found += self.lookup(lib, "", name, depth + 1)
                        elif kind == "ext":
                            found.append(("maybe-ext", name))
                        elif kind == "via":
                            found.append((kind, value))
                    if any(kind != "via" for kind, _ in found):
                        # Through the glob, for this one name: a change to the glob reaches it.
                        through = f"{node}:{name}"
                        self.glob_names.setdefault(node, set()).add(through)
                        self.edges.setdefault(through, set()).update(self.nodes_of(found))
                        out += [("via", through)] + found
        return out

    def first(self, unit_id: str, modpath: str, name: str, crate: str) -> List[Entry]:
        """The meaning of a path's first segment in a module."""
        if name == "crate":
            return [("mod", (unit_id, ""))]
        if name == "self":
            return [("mod", (unit_id, modpath))]
        if name == "super":
            return [("mod", (unit_id, modpath.rpartition("::")[0]))]
        found = self.lookup(unit_id, modpath, name)
        if any(kind in ("def", "mod") for kind, _ in found):
            return [e for e in found if e[0] != "maybe-ext"]
        if found:
            return found
        if name in self.idents:
            return [("crate", self.idents[name])]
        if name in STD or name in PRELUDE or name in self.crates[crate].externs:
            return [EXTERNAL]
        return []

    def resolve_path(self, unit_id: str, modpath: str, segments: Sequence[str],
                     depth: int = 0) -> Tuple[List[Entry], List[str]]:
        """A path's meaning from a module, and the segments it could not resolve (empty if all)."""
        key = (unit_id, modpath, tuple(segments))
        if key in self._resolved:
            return self._resolved[key]
        outer_low, self._low = self._low, _NO_CYCLE
        result = self._resolve(unit_id, modpath, segments, depth)
        low = self._low
        if low == _NO_CYCLE:  # met no lookup under way (`lookup`): complete
            self._resolved[key] = result
        self._low = min(outer_low, low)
        return result

    def _resolve(self, unit_id: str, modpath: str, segments: Sequence[str],
                 depth: int) -> Tuple[List[Entry], List[str]]:
        crate = self.units[unit_id].crate if unit_id in self.units else ""
        head = segments[0]
        if head == "$crate":
            current: List[Entry] = [("crate", crate)]
        elif head == "":
            current = [("crate", self.idents[segments[1]])] if len(segments) > 1 and segments[1] in self.idents \
                else [EXTERNAL]
            segments = segments[1:]
        else:
            current = self.first(unit_id, modpath, head, crate)
        return self._descend(current, segments, depth)

    def _descend(self, current: List[Entry], segments: Sequence[str], depth: int) -> Tuple[List[Entry], List[str]]:
        """Follow a path's segments from its first one's meaning. An item stops the descent (what
        follows is an associated item, a variant or a field of it); a module or crate in which a
        segment means nothing, or an unresolved import on the way, leaves the rest to the fallback."""

        def meaningful(entries: List[Entry]) -> bool:
            return any(kind not in ("via", "unresolved") for kind, _ in entries)

        if any(kind == "unresolved" for kind, _ in current) or not meaningful(current):
            return [e for e in current if e[0] != "unresolved"], list(segments)
        for index, segment in enumerate(segments[1:], 1):
            following: List[Entry] = []
            lost = False
            for kind, value in current:
                if kind == "mod":
                    other_unit, other_path = value  # type: ignore[misc]
                    if segment == "super":
                        found: List[Entry] = [("mod", (other_unit, other_path.rpartition("::")[0]))]
                    else:
                        found = self.lookup(other_unit, other_path, segment, depth + 1)
                elif kind == "crate":
                    lib = self.lib_unit.get(value)  # type: ignore[arg-type]
                    found = self.lookup(lib, "", segment, depth + 1) if lib else [EXTERNAL]
                elif kind == "def" and value.kind in _VALUES:  # type: ignore[union-attr]
                    continue  # a function's, a constant's or a static's name: no path goes on from it
                else:
                    following.append((kind, value))  # carried: a type's item, the outside, an import passed
                    continue
                lost = lost or not meaningful(found) or any(k == "unresolved" for k, _ in found)
                following += found
            following = [e for e in following if e[0] != "unresolved"]
            if lost or not meaningful(following):
                return following, list(segments[index:])
            current = following
        return current, []

    def visible(self, d: Def, path: str, test: bool) -> bool:
        """Whether code in `path` (test code, or not) could name the item `d`, by crate and visibility."""
        crate = self.crate_of(path) or ""
        if d.crate not in (self.allowed_test if test else self.allowed_normal).get(crate, ()):
            return False
        units = self.file_units.get(path)
        d_units = self.file_units.get(d.file)
        if units is None or d_units is None:
            return d.crate == crate or d.vis == "pub"  # an orphan: no unit to judge by
        shared = units & d_units
        if d.vis == "pub":
            return bool(shared) or any(self.units[u].is_lib for u in d_units)
        if d.vis == "crate":
            return bool(shared)
        for unit_id in shared:
            here = self.units[unit_id].files[path]
            if d.modpath == "" or here == d.modpath or here.startswith(d.modpath + "::") or d.file == path:
                return True
        return False

    def by_name(self, names: Iterable[str], path: str, test: bool) -> Set[str]:
        """The fallback: the nodes of every item of these names that code in `path` could see."""
        self.fallbacks += 1
        return {d.node for name in names for d in self.defs.get(name, ()) if d.kind != "local"
                and self.visible(d, path, test)}

    def nodes_of(self, entries: Iterable[Entry]) -> Set[str]:
        out: Set[str] = set()
        for kind, value in entries:
            if kind == "def":
                out.add(value.node)  # type: ignore[union-attr]
            elif kind == "via":
                out.add(value)  # type: ignore[arg-type]
            elif kind == "mod":
                unit_id, modpath = value  # type: ignore[misc]
                module = self.modules[unit_id].get(modpath)
                if module is not None:
                    out.add(self.file_node(module.file))
        return out

    def path_nodes(self, unit_id: str, modpath: str, segments: Sequence[str], path: str, test: bool) -> Set[str]:
        """The nodes a path names, from a module; the fallback where it resolves only part of the way."""
        entries, rest = self.resolve_path(unit_id, modpath, segments)
        crate = self.crate_of(path) or ""
        allowed = (self.allowed_test if test else self.allowed_normal).get(crate, set())
        out = {n for n in self.nodes_of(entries) if (self.crate_of(file_of(n)) or "") in allowed}
        if rest:
            if len(rest) == len(segments) == 1 and not self._open_globs(unit_id, modpath):
                return out  # a local binding: nothing in scope has that name
            out |= self.by_name([s for s in rest if s not in KEYWORDS], path, test)
        return out

    def _open_globs(self, unit_id: str, modpath: str) -> bool:
        """Whether a module, or one its globs import, has a glob import the graph cannot resolve or an
        item-position `include!` (whose items the graph does not read): then any name may be its."""
        key = ("open", unit_id, modpath)
        if key not in self._lookups:
            open_ = False
            seen = {(unit_id, modpath)}
            frontier = [(unit_id, modpath)]
            while frontier and not open_:
                where, path = frontier.pop()
                module = self.modules.get(where, {}).get(path)
                if module is None:
                    continue
                for segments, _ in module.globs:
                    entries, rest = self.resolve_path(where, path, segments)
                    open_ = open_ or bool(rest)
                    for kind, value in entries:
                        if kind == "mod" and value not in seen:
                            seen.add(value)  # type: ignore[arg-type]
                            frontier.append(value)  # type: ignore[arg-type]
                facts = self.rs.get(module.file)
                base = self.units[where].files.get(module.file, "") if where in self.units else ""
                open_ = open_ or (facts is not None and any(
                    item[1] == "call" and item[2] == "include!" and self.join(base, item[4]) == path
                    for item in facts.items))
            self._lookups[key] = [("open", open_)]
        return bool(self._lookups[key][0][1])

    # -- the edges ---------------------------------------------------------------------------------

    def _link(self) -> None:
        self.literal_index: Dict[str, List[str]] = {}
        for known in self.known:
            self.literal_index.setdefault(posixpath.basename(known), []).append(known)
        self.tests: Dict[str, List[TestFn]] = {}  # unit -> its test functions
        self.generators: Dict[str, Dict[str, str]] = {}  # unit -> {node of a generating item: filter term}
        for path, facts in self.rs.items():
            units = sorted(self.file_units.get(path, ()))
            file_test = self.is_test_file(path)
            for unit_id in units:
                self._link_file(unit_id, path, facts, file_test)
            if not units:
                self._link_orphan(path, facts)
            if facts.doctests:
                self.edges[doc_node(path)] = self._doctest_edges(path, facts)

    def _link_file(self, unit_id: str, path: str, facts: FileFacts, file_test: bool) -> None:
        base = self.units[unit_id].files[path]
        file_item = self.file_node(path, facts)
        parents = self._parents(facts)
        for index, (ident, kind, name, _, scope, test, _, testfn, generates, raw, *more) in enumerate(facts.items):
            node = item_node(path, ident)
            full = self.join(base, scope)
            t = bool(test) or file_test
            out = self.edges.setdefault(node, set())
            for item_test, *segments in facts.item_paths[index]:
                out |= self.path_nodes(unit_id, full, segments, path, bool(item_test) or t)
            for call in facts.item_calls[index]:
                self._macro(unit_id, full, call, path, t, out)
            for literal in facts.item_literals[index]:
                out |= self._literal(path, literal)
            if kind not in _NO_CODE:
                out.add(traits_node(path, full))  # the traits whose methods its code may call
            member_of = more[0] if more else -1
            if member_of >= 0:
                # An impl's member: its impl (its `Self`) and the impl it: whoever reaches the impl
                # may call it (`Graph.select` narrows a change to the member to its callers).
                impl = item_node(path, facts.items[member_of][0])
                out.add(impl)
                self.edges.setdefault(impl, set()).add(node)
            elif index:
                out.add(item_node(path, facts.items[parents[index]][0]) if parents[index] else file_item)
            out.discard(node)
            if testfn:
                test_name = self.join(full, ("r#" + name) if raw else name)
                self.tests.setdefault(unit_id, []).append(TestFn(node, test_name, full, testfn == 2))
                if generates == "cases":  # `#[rstest]`, `#[test_case]`: one test per case, below it
                    self.generators.setdefault(unit_id, {})[node] = f"test(/^{test_name}::/)"
            elif generates == "ts":
                self.generators.setdefault(unit_id, {})[node] = \
                    f"test(/^{self.join(full, 'export_bindings_' + name.lower())}$/)"
            elif generates == "tests" and self._may_make_tests(name.rstrip("!")):
                self.generators.setdefault(unit_id, {})[node] = f"test(/^{full}::/)" if full else "all()"
            makers = [name.rstrip("!")] if kind == "call" else \
                [c for c in facts.item_calls[index] if self._makes_impls(c)]
            if makers:
                # A macro call may expand to impls, in item position or in a body (`const _: () = {
                # imp!(X); };`): attach its item, as an impl is attached, to the types and traits of
                # its own crate that the item or the macro's body name (by the orphan rule an impl it
                # makes is for one of them, or of one of them).
                crate = self.crate_of(path) or ""
                named = [(unit_id, full, segments) for _, *segments in facts.item_paths[index]]
                for macro in makers:
                    for macro_file, _, body, *_ in self.macros.get(macro, ()):
                        macro_unit = self.lib_unit.get(self.crate_of(macro_file) or "", unit_id)
                        named += [(macro_unit, "", ["crate", *s[1:]]) if s[0] == "$crate" else (unit_id, full, s)
                                  for s in body]
                for where, module, segments in named:
                    entries, _ = self.resolve_path(where, module, segments)
                    for entry_kind, value in entries:
                        if entry_kind == "def" and value.crate == crate and value.kind in (  # type: ignore[union-attr]
                                "struct", "enum", "union", "type", "trait", "maybe"):
                            self.edges.setdefault(value.node, set()).add(node)  # type: ignore[union-attr]
        scoped_traits: Dict[str, Set[str]] = {}
        for name, kind, _, scope, index in facts.defs:
            if kind in ("trait", "maybe"):
                scoped_traits.setdefault(scope, set()).add(item_node(path, facts.items[index][0]))
        for scope, test, alias, *segments in facts.uses:
            # An import's node depends on what it imports. The module's items depend on it when they
            # name it (through the lookup), or when it may bring a trait into scope for its methods:
            # a trait, an `as _` import, one the graph cannot resolve.
            full = self.join(base, scope)
            t = bool(test) or file_test
            node = import_node(path, full, alias, segments)
            self.edges.setdefault(node, set()).update(self.path_nodes(unit_id, full, segments, path, t))
            entries, rest = self.resolve_path(unit_id, full, segments)
            if not alias or rest or any((kind == "def" and value.kind in ("trait", "maybe"))  # type: ignore[union-attr]
                                        or kind in ("ext", "maybe-ext") for kind, value in entries):
                scoped_traits.setdefault(scope, set()).add(node)
        for scope, test, *segments in facts.globs:
            full = self.join(base, scope)
            node = glob_node(path, full, segments)
            self.edges.setdefault(node + "#traits", set()).update(
                self._glob_traits(unit_id, full, segments, path, bool(test) or file_test))
            scoped_traits.setdefault(scope, set()).add(node + "#traits")
            for kind, value in self.resolve_path(unit_id, full, segments)[0]:
                if kind == "mod":
                    # Which modules glob-import which: a name removed from one may resolve elsewhere
                    # in the others.
                    self.glob_into.setdefault(value, set()).add((path, scope, node))  # type: ignore[arg-type]
        for scope, nodes in scoped_traits.items():
            self.edges.setdefault(traits_node(path, self.join(base, scope)), set()).update(nodes)
        holds_impls = {index for index, *_ in facts.impls}  # items with an impl inside, or impls
        for index, scope, test, head, header, *more in facts.impls:
            impl = item_node(path, facts.items[index][0])
            full = self.join(base, scope)
            for target in self._attach(unit_id, full, head, header, path, bool(test) or file_test):
                if target != impl:
                    self.edges.setdefault(target, set()).add(impl)
            trait = more[0] if more else None
            if facts.items[index][1] == "impl" and trait is not None and self._split(unit_id, full, trait):
                # An inherent impl, or one of a workspace trait: each member is called only by code
                # that names it. An impl of another crate's trait is called by that crate's code, and
                # by operators, `for`, `?` and derives, which name nothing: it stays whole. So does a
                # member that is a macro call (`setters! { ... }`: its methods have other names) or
                # that holds an impl (which its type's holders call without naming the member).
                for member, item in enumerate(facts.items):
                    if len(item) > 10 and item[10] == index and not item[2].endswith("!") \
                            and member not in holds_impls:
                        self.split_members[item_node(path, item[0])] = (impl, item[2].rsplit("::", 1)[-1])
                self.split_impls.add(impl)

    def _split(self, unit_id: str, modpath: str, trait: List[str]) -> bool:
        """Whether an impl's members are called only by name: an inherent impl ([]), or an impl of a
        trait that resolves to the workspace's own traits alone."""
        if not trait:
            return True
        entries, rest = self.resolve_path(unit_id, modpath, trait)
        defs = [value for kind, value in entries if kind == "def"]
        return (not rest and bool(defs) and all(kind in ("def", "via") for kind, _ in entries)
                and all(d.kind == "trait" for d in defs))  # type: ignore[union-attr]

    @staticmethod
    def _parents(facts: FileFacts) -> List[int]:
        """Each item's inline module item (0 for the file): the item whose attributes reach it."""
        mods = {Graph.join(item[4], item[2]): index for index, item in enumerate(facts.items) if item[1] == "mod"}
        return [mods.get(item[4], 0) for item in facts.items]

    def _may_make_tests(self, macro: str) -> bool:
        """Whether a macro may make tests: one the graph does not know may; a known one if its body,
        or a macro its body calls, has a test attribute."""
        if not self._macros_named(macro):
            return True
        return any(entry[3] for entry in self._macro_closure(macro))

    def _makes_impls(self, macro: str) -> bool:
        """Whether a workspace macro's body, or a macro it calls, writes an `impl`."""
        memo = self.__dict__.setdefault("_impl_makers", {})
        if macro not in memo:
            memo[macro] = any(entry[7] for entry in self._macro_closure(macro))
        return memo[macro]

    def _macro_closure(self, macro: str) -> list:
        """The workspace's macros a call of `macro!` may expand: it, and those their bodies call."""
        seen: Set[str] = set()
        frontier = [macro]
        found: list = []
        while frontier:
            name = frontier.pop()
            if name in seen:
                continue
            seen.add(name)
            known = self._macros_named(name)
            found += known
            for entry in known:
                frontier += entry[5]
        return found

    def _macros_named(self, name: str) -> list:
        """The workspace's macros that a call of `name!` may be: of that name, or that a `use` renames
        to it (`pub use __seven as seven;`)."""
        found = list(self.macros.get(name, ()))
        for other in self.macro_aliases.get(name, ()):
            if other != name:
                found += self.macros.get(other, ())
        return found

    def _link_orphan(self, path: str, facts: FileFacts) -> None:
        """A file no unit reaches: by name, as a fallback."""
        for index, item in enumerate(facts.items):
            node = item_node(path, item[0])
            names = {s for p in facts.item_paths[index] for s in p[1:]}
            self.edges.setdefault(node, set()).update(self.by_name(names, path, True))
        for index, _, _, head, header, *_ in facts.impls:
            named = [head[-1]] if head else [s for p in header for s in p]
            for target in self.by_name(named, path, True):
                self.edges.setdefault(target, set()).add(item_node(path, facts.items[index][0]))

    def _glob_traits(self, unit_id: str, modpath: str, segments: Sequence[str], path: str, test: bool) -> Set[str]:
        """The traits `use <segments>::*` brings into scope: those in scope in the module it names."""
        entries, rest = self.resolve_path(unit_id, modpath, segments)
        out: Set[str] = set()
        for kind, value in entries:
            if kind == "mod":
                other_unit, other_path = value  # type: ignore[misc]
                module = self.modules[other_unit].get(other_path)
                if module is not None:
                    out.add(traits_node(module.file, other_path))
            elif kind == "crate":
                lib = self.lib_unit.get(value)  # type: ignore[arg-type]
                module = self.modules[lib].get("") if lib else None
                if module is not None:
                    out.add(traits_node(module.file, ""))
            elif kind == "via":
                out.add(value)  # type: ignore[arg-type]
        if rest:
            out |= self.by_name(rest, path, test)
        return out

    def _macro(self, unit_id: str, modpath: str, name: str, path: str, test: bool, out: Set[str],
               depth: int = 0) -> None:
        """A call of `name!` from a module: the macro's definition, and its body as the caller's code,
        the macros it calls in turn included."""
        crate = self.crate_of(path) or ""
        allowed = (self.allowed_test if test else self.allowed_normal).get(crate, set())
        known = self._macros_named(name)
        if not known and name not in STD_MACROS:
            # No `macro_rules!` of that name: one that another macro defines (`make_macro!(gen)`
            # makes `gen!`), so the macro calls whose arguments name it.
            out |= {d.node for d in self.defs.get(name, ()) if d.kind == "maybe" and self.visible(d, path, test)}
            return
        for macro_file, exported, body, _, node, calls, *_ in known:
            macro_crate = self.crate_of(macro_file) or ""
            if macro_crate not in allowed or (macro_crate != crate and not exported) or node in out:
                continue
            out.add(node)
            macro_unit = self.lib_unit.get(macro_crate, unit_id)
            for segments in body:
                if segments[0] == "$crate":
                    out |= self.path_nodes(macro_unit, "", ["crate", *segments[1:]], macro_file, True)
                else:
                    out |= self.path_nodes(unit_id, modpath, segments, path, test)
            if depth < 32:
                for inner in calls:
                    self._macro(unit_id, modpath, inner, path, test, out, depth + 1)

    def _attach(self, unit_id: str, modpath: str, head: List[str], header: List[List[str]], path: str,
                test: bool) -> Set[str]:
        """The nodes an impl block in `path` attaches to (each depends on the impl).

        Its self type's, when that type is the impl's own crate's: a value of it reaches its node.
        Otherwise (a foreign or generic self type, or one the graph cannot resolve) the items of its
        own crate that its header names. By the orphan rule one of them is the trait, or a type in
        its parameters, and code that calls the impl has the trait in scope or holds such a value,
        so reaches it; code of the self type's crate cannot call a trait that crate never sees."""
        crate = self.crate_of(path) or ""
        aliased: Set[str] = set()
        if head:
            entries, rest = self.resolve_path(unit_id, modpath, head)
            types: List[Def] = []
            foreign: List[bool] = []  # an alias of a type outside the workspace (`type Meters = f64`)
            for kind, value in entries:
                if kind == "def":
                    types += self._aliased(value, 0, foreign)  # type: ignore[arg-type]
            if types and not rest and all(kind in ("def", "via") for kind, _ in entries) \
                    and all(d.crate == crate for d in types):
                if not foreign:
                    return {d.node for d in types}
                aliased = {d.node for d in types}  # and, as for a foreign type, the header's items
        # The items' nodes, never an import's: code that names an item through a re-export reaches
        # the item through that import.
        named: Set[str] = set()
        for segments in header:
            named |= {t for t in self.path_nodes(unit_id, modpath, segments, path, test)
                      if "#use:" not in t and "#glob:" not in t}
        own = {t for t in named if self.crate_of(file_of(t)) == crate}
        return (own or named) | aliased

    def _aliased(self, d: Def, depth: int, foreign: Optional[List[bool]] = None) -> List[Def]:
        """A type, and for an alias, the types it stands for too; `foreign` gains an entry when an
        alias names something that is no workspace item (a primitive, a std type, an unresolved path)."""
        out = [d]
        if d.kind == "type" and depth < 4:
            facts = self.rs.get(d.file)
            for unit_id in sorted(self.file_units.get(d.file, ())):
                for segments in (facts.aliases.get(d.name, ()) if facts else ()):
                    entries, rest = self.resolve_path(unit_id, d.modpath, segments)
                    if foreign is not None and (rest or not any(kind == "def" for kind, _ in entries)):
                        foreign.append(True)
                    for kind, value in entries:
                        if kind == "def":
                            out += self._aliased(value, depth + 1, foreign)  # type: ignore[arg-type]
        return out

    def _doctest_edges(self, path: str, facts: FileFacts) -> Set[str]:
        """A file's doctests, resolved as a crate that depends on the file's crate sees the workspace:
        through the paths their code names, from the crates it can use, and the macros it calls."""
        crate = self.crate_of(path) or ""
        allowed = self.allowed_test.get(crate, set())
        out: Set[str] = set()

        def public(names: Iterable[str]) -> Set[str]:
            return {d.node for name in names for d in self.defs.get(name, ())
                    if d.vis == "pub" and d.crate in allowed and d.kind != "local"}

        lib = self.lib_unit.get(crate, "")
        for _, imports, paths, *more in facts.doctests:
            calls = more[0] if more else []
            globs = more[2] if len(more) > 2 else []
            if paths[:1] == [["?"]]:
                out |= public(p[0] for p in paths[1:])
                continue
            aliases = {i[0]: i[1:] for i in imports if len(i) > 1}
            named = [(i[1:], False) for i in imports if len(i) > 1] + [(p, False) for p in paths] \
                + [(g, True) for g in globs]
            for segments, is_glob in named:
                segments = list(segments[1:]) if segments[:1] == [""] else list(segments)  # `::a::f`
                if not segments:
                    continue
                for _ in range(4):  # an alias of an alias
                    if segments[0] in aliases and aliases[segments[0]] != segments:
                        segments = aliases[segments[0]] + segments[1:]
                head = segments[0]
                if head in self.idents and self.idents[head] in allowed and self.idents[head] in self.lib_unit:
                    unit = self.lib_unit[self.idents[head]]
                    out |= self.path_nodes(unit, "", ["crate", *segments[1:]], path, True)
                    if is_glob:  # `use a::m::*;`: the traits it brings into scope too
                        out |= self._glob_traits(unit, "", ["crate", *segments[1:]], path, True)
                elif head in STD or head in PRELUDE or head in self.crates[crate].externs:
                    continue
                elif len(segments) > 1 or globs:
                    # Unresolved part of the way, or a name a glob import may bring: by name.
                    out |= public(segments)
            for name in calls:
                # The macro's body, as the doctest's code (a crate outside it calling the macro).
                if lib:
                    self._macro(lib, "", name, path, True, out)
                for macro_file, exported, _, _, node, *_ in self._macros_named(name):
                    if exported and self.crate_of(macro_file) in allowed:
                        out.add(node)
        return out

    def _literal(self, path: str, literal: str) -> Set[str]:
        """The nodes of the workspace files a string literal in `path` names: a data file, or a
        source file's text."""
        text = literal.split("{", 1)[0] if "{" in literal else literal
        found: Set[str] = set()
        crate = self.crate_of(path)
        if text and not text.startswith("/"):
            near = posixpath.normpath(posixpath.join(posixpath.dirname(path), text))
            if near in self.known:
                found.add(near)
        stripped = re.sub(r"^(\.{1,2}/|/)+", "", text)
        if stripped and not stripped.endswith("/"):
            if crate:
                under = posixpath.normpath(posixpath.join(self.crates[crate].dir, stripped))
                if under in self.known:
                    found.add(under)
            if "/" in stripped:
                for candidate in self.literal_index.get(posixpath.basename(stripped), ()):
                    if candidate == stripped or candidate.endswith("/" + stripped):
                        found.add(candidate)
        found.discard(path)
        return {text_node(f) if f.endswith(".rs") else f for f in found}

    # -- the selection -----------------------------------------------------------------------------

    def runnable(self, crate: str) -> List[Unit]:
        return [u for u in self.units.values() if u.crate == crate and u.runs]

    def roots(self, crate: str) -> Set[str]:
        """The root files of the crate's library and binaries, whose crate attributes reach the crate."""
        return {u.root for u in self.units.values() if u.crate == crate and u.kind in ("lib", "bin")}

    def all_nodes(self, path: str) -> Set[str]:
        facts = self.rs.get(path)
        out = {text_node(path), doc_node(path)}
        if facts is not None:
            out |= {item_node(path, item[0]) for item in facts.items}
        return out

    def select(self, changed: Sequence[Tuple[str, bool]]) -> Selection:
        """The tests and doctests that reach the changed paths, unit by unit."""
        sel = Selection()
        starts: Dict[str, Set[str]] = {}  # node -> the changed paths it stands for
        conditional: List[Tuple[str, str, str]] = []  # changed members called by name: (impl, name, path)
        # Goldens: the reader through which a crate's tests read their own goldens, for that crate only.
        own_starts: Dict[str, Dict[str, Set[str]]] = {}
        for path, deleted in changed:
            crate = self.crate_of(path)
            if crate is None:
                # Data outside every crate (a fixture both sides read): what names it, and the items
                # whose strings name a directory it is in. A source file outside them (`#[path =
                # "../../shared/x.rs"] mod x;`) widens to each crate whose module tree names it.
                if path.endswith(".rs"):
                    for owner in sorted(self._mod_declarers(path)):
                        self._widen(owner, path, f"{path} is a module of {owner} outside its directory", sel, starts)
                for node in {path} | self._shared_readers(path):
                    starts.setdefault(node, set()).add(path)
                continue
            if path.endswith(".rs"):
                facts = None if deleted else self.rs.get(path)
                base = self.base.get(path)
                if facts is not None and not facts.ok:
                    self._widen(crate, path, f"{path} could not be read ({facts.problem})", sel, starts)
                elif base is not None and not base.ok:
                    self._widen(crate, path, f"{path} could not be read at the base ({base.problem})", sel, starts)
                elif not deleted and path in self.orphans and not self._read_as_text(path):
                    self._widen(crate, path, f"{path} is in no module tree and no file names it", sel, starts)
                elif (path in self.roots(crate) and base is not None and facts is not None
                      and base.crate_attrs != facts.crate_attrs):
                    self._widen(crate, path, f"{path}'s crate attributes changed", sel, starts)
                elif self._global_changed(base, facts):
                    self._widen(crate, path, f"{path}'s global allocator or panic handler changed", sel, starts)
                else:
                    nodes, members = self.changed_nodes(path, base, facts)
                    for node in nodes | {text_node(path)}:
                        starts.setdefault(node, set()).add(path)
                    conditional += [(impl, name, path) for impl, name in members]
                    # A `mod` declaration added, removed or changed (its `#[path]`, its `cfg`): the
                    # files it named before and names now are changed as well.
                    old = {tuple(m) for m in (base.mods if base else [])}
                    new = {tuple(m) for m in (facts.mods if facts else [])}
                    is_root = any(u.root == path for u in self.units.values())
                    for name, path_attr, scope, *_ in old ^ new:
                        for child in self.mod_candidates(path, is_root, name, path_attr, scope):
                            for node in self.all_nodes(child) if child in self.rs else ():
                                starts.setdefault(node, set()).add(path)
                continue
            # Data: what names it, and its crate's tests: all of them, unless it is a golden, which a
            # crate's tests read by name through the golden reader. What names it: a path's string,
            # a one-word string that is one of its directories (`dir.join("golden")`), or, in its
            # crate, a read of a directory's entries.
            starts.setdefault(path, set()).add(path)
            for node in self._data_readers(path, crate) | self._shared_readers(path, crate):
                starts.setdefault(node, set()).add(path)
            rel = path[len(self.crates[crate].dir) + 1 :]
            golden = next((reader for prefix, reader in self.golden_readers if rel.startswith(prefix)), None)
            if golden is not None and golden in self.rs:
                for node in self.all_nodes(golden):
                    own_starts.setdefault(crate, {}).setdefault(node, set()).add(path)
            else:
                sel.whole.setdefault(crate, set()).add(path)

        reverse: Dict[str, Set[str]] = {}
        for source, targets in self.edges.items():
            for target in targets:
                reverse.setdefault(target, set()).add(source)
        for crate in sorted(self.crates):
            allowed = self.allowed_test[crate] | {""}  # "": data outside any crate, which any may read
            # A crate's code reaches the code of the crates it may use, and any data file, which a
            # crate reads by its path whatever it depends on.
            crate_starts = {node: set(causes) for node, causes in starts.items()
                            if (self.crate_of(file_of(node)) or "") in allowed or "#" not in node}
            for node, causes in own_starts.get(crate, {}).items():
                crate_starts.setdefault(node, set()).update(causes)
            mine = [c for c in conditional if (self.crate_of(file_of(c[0])) or "") in allowed]
            if not crate_starts and not mine:
                continue
            causes, parent = self._reach(crate_starts, reverse, allowed) if crate_starts else ({}, {})
            for impl, name, cause in mine:
                # A changed member of an impl called by name: the code that holds the type (reaches the
                # impl) and names the member, both. A test runs it only when its code does both.
                by_type, _ = self._reach({impl: {cause}}, reverse, allowed)
                callers = {n: {cause} for n in self._mentions().get(name, ())
                           if (self.crate_of(file_of(n)) or "") in allowed}
                by_name, named_parent = self._reach(callers, reverse, allowed) if callers else ({}, {})
                for node in set(by_type) & set(by_name):
                    causes.setdefault(node, set()).add(cause)
                    if node in named_parent:
                        parent.setdefault(node, named_parent[node])
            for unit in self.runnable(crate):
                for test in self.tests.get(unit.id, ()):
                    if test.node in causes:
                        sel.tests.setdefault(unit.id, {})[test.name] = causes[test.node]
                        sel.via.setdefault(f"{unit.id} {test.name}", self._chain(test.node, parent))
                for node, term in self.generators.get(unit.id, {}).items():
                    if node in causes:
                        sel.generated.setdefault(unit.id, {})[term] = causes[node]
            docs = {file_of(node): causes[node] for node in causes
                    if node.endswith("#doctests") and self.crate_of(file_of(node)) == crate}
            if docs:
                sel.doctests[crate] = docs
        return sel

    def changed_nodes(self, path: str, base: Optional[FileFacts],
                      facts: Optional[FileFacts]) -> Tuple[Set[str], List[Tuple[str, str]]]:
        """The nodes of a changed file that changed, and its changed members of impls whose members
        are called by name (`(impl, name)`, for `select` to narrow to their callers): its items added,
        removed or changed, its imports and globs added, removed or changed. A removed item, import or
        glob also changes the rest of its module, whose names may now resolve elsewhere, and the
        per-name nodes of the globs that reached it; a removed impl, the type it was for. A new or
        deleted file is all changed."""
        modpaths = {self.units[u].files[path] for u in self.file_units.get(path, ())} or {self.modpath(path)}
        changed: Set[str] = set()
        members: List[Tuple[str, str]] = []  # changed members of impls called by name: (impl, name)
        old_items = {item[0]: item for item in (base.items if base else [])}
        new_items = {item[0]: item for item in (facts.items if facts else [])}
        for ident in set(old_items) | set(new_items):
            if (old_items.get(ident) or [None] * 7)[6] == (new_items.get(ident) or [None] * 7)[6]:
                continue
            node = item_node(path, ident)
            if node in self.split_members:
                members.append(self.split_members[node])
            elif ident not in new_items and old_items[ident][1] == "member" and base is not None:
                # A member removed: its impl's callers of its name, or the whole impl if it is whole
                # or the member was (a macro call, or a member that held an impl).
                impl = item_node(path, base.items[old_items[ident][10]][0])
                index = next(i for i, item in enumerate(base.items) if item[0] == ident)
                if impl in self.split_impls and not old_items[ident][2].endswith("!") \
                        and all(at != index for at, *_ in base.impls):
                    members.append((impl, old_items[ident][2].rsplit("::", 1)[-1]))
                else:
                    changed.add(impl)
            else:
                changed.add(node)
        rescoped: Set[str] = set()  # scopes whose names may now resolve elsewhere
        removed: Set[Tuple[str, str]] = set()  # (scope, name) of the items and imports a change removed
        for ident in set(old_items) - set(new_items):
            _, kind, name, _, scope, *_ = old_items[ident]
            if kind == "member":
                continue  # no module-level name: nothing else resolves differently
            rescoped.add(scope)
            removed.add((scope, name))
            if kind == "impl" and base is not None:
                for index, _, _, head, header, *_ in base.impls:
                    if base.items[index][0] == ident:
                        named = [head[-1]] if head else [s for p in header for s in p]
                        changed |= self.by_name(named, path, True)

        def keyed(source: Optional[FileFacts]) -> Tuple[Dict[tuple, str], Dict[tuple, str]]:
            # Each import and glob with its attributes, so that a changed `cfg` on one changes it.
            attrs = {tuple(record[1:]): record[0] for record in (getattr(source, "use_attrs", None) or [])}
            uses: Dict[tuple, str] = {}
            globs: Dict[tuple, str] = {}
            for scope, test, alias, *segments in (source.uses if source else []):
                cfg = attrs.get((scope, test, alias, *segments), "")
                for modpath in modpaths:
                    uses[(modpath, scope, alias, cfg, *segments)] = \
                        import_node(path, self.join(modpath, scope), alias, segments)
            for scope, test, *segments in (source.globs if source else []):
                cfg = attrs.get((scope, test, "*", *segments), "")
                for modpath in modpaths:
                    globs[(modpath, scope, cfg, *segments)] = glob_node(path, self.join(modpath, scope), segments)
            return uses, globs

        old_uses, old_globs = keyed(base)
        new_uses, new_globs = keyed(facts)
        for key in set(old_uses) ^ set(new_uses):
            changed.add(old_uses.get(key) or new_uses[key])
            if key in old_uses:
                rescoped.add(key[1])
                removed.add((key[1], key[2]))
        for key in set(old_globs) ^ set(new_globs):
            node = old_globs.get(key) or new_globs[key]
            changed |= {node, node + "#traits"} | self.glob_names.get(node, set())
            rescoped.add(key[1])
        if facts is not None:
            changed |= {item_node(path, item[0]) for item in facts.items if item[4] in rescoped}
        # A name gone from a module may now resolve, in it and in each module that glob-imports it,
        # through a glob to something else: those globs' nodes for the name, and the importers' items.
        for unit_id in self.file_units.get(path, ()):
            for scope, name in removed:
                module_path = self.join(self.units[unit_id].files[path], scope)
                module = self.modules[unit_id].get(module_path)
                for _, node in (module.globs if module else ()):
                    changed.add(f"{node}:{name}")
                changed |= self.glob_targets.get((unit_id, module_path, name), set())
                for importer, importer_scope, node in self.glob_into.get((unit_id, module_path), ()):
                    changed.add(f"{node}:{name}")
                    importer_facts = self.rs.get(importer)
                    if importer_facts is not None:
                        changed |= {item_node(importer, item[0]) for item in importer_facts.items
                                    if item[4] == importer_scope}
        if base is None or facts is None or base.doctests != facts.doctests:
            changed.add(doc_node(path))
        return changed, members

    def _widen(self, crate: str, path: str, why: str, sel: Selection, starts: Dict[str, Set[str]]) -> None:
        """A change the graph cannot place: every test of its crate, and every node of the crate as a
        start, so that the crates depending on it select what reaches any of it."""
        sel.whole.setdefault(crate, set()).add(path)
        sel.notes.append(f"widened to {crate}: {why}")
        for file in self.rs:
            if self.crate_of(file) == crate:
                for node in self.all_nodes(file):
                    starts.setdefault(node, set()).add(path)
        for node in list(self.edges):
            if self.crate_of(file_of(node)) == crate:
                starts.setdefault(node, set()).add(path)

    def _mentions(self) -> Dict[str, Set[str]]:
        """Each name a method or an associated item can have -> the nodes whose code names it: after
        `.`, after `::` (`Self::m`, `<T as Tr>::m`), and in the bodies of the macros they call."""
        if hasattr(self, "_mention_index"):
            return self._mention_index
        index: Dict[str, Set[str]] = {}

        def macro_names(name: str, depth: int, seen: Set[str]) -> Set[str]:
            out: Set[str] = set()
            for _, _, _, _, node, calls, methods, *_ in self._macros_named(name):
                if node in seen:
                    continue
                seen.add(node)
                out |= set(methods)
                if depth < 32:
                    for inner in calls:
                        out |= macro_names(inner, depth + 1, seen)
            return out

        for file, facts in self.rs.items():
            for i, item in enumerate(facts.items):
                node = item_node(file, item[0])
                names = set(facts.item_methods[i]) if i < len(facts.item_methods) else set()
                for entry in facts.item_paths[i]:
                    names.update(entry[2:])
                for call in facts.item_calls[i]:
                    names |= macro_names(call, 0, set())
                for name in names:
                    index.setdefault(name, set()).add(node)
            for _, _, paths, *more in facts.doctests:
                names = set(more[1]) if len(more) > 1 else set()
                for segments in paths:
                    names.update(segments[1:] if segments[:1] != ["?"] else segments)
                for name in names:
                    index.setdefault(name, set()).add(doc_node(file))
        self._mention_index = index
        return index

    def _data_readers(self, path: str, crate: str) -> Set[str]:
        """The items of the data file's crate that may read it without naming its path whole: by one
        of its directories' names (`dir.join("golden")`), or by reading a directory's entries. Such a
        read starts from the crate's own directory (`CARGO_MANIFEST_DIR`); another crate's reader names
        a longer path, which its literal's edge finds."""
        if not hasattr(self, "_words"):
            self._words: Dict[Tuple[str, str], Set[str]] = {}
            self._dir_reads: Dict[str, Set[str]] = {}
            for file, facts in self.rs.items():
                owner = self.crate_of(file) or ""
                for index, item in enumerate(facts.items):
                    node = item_node(file, item[0])
                    for literal in facts.item_literals[index]:
                        if "/" not in literal:
                            self._words.setdefault((owner, literal), set()).add(node)
                    if any(set(p[1:]) & _DIRECTORY_READS for p in facts.item_paths[index]):
                        self._dir_reads.setdefault(owner, set()).add(node)
        rel = path[len(self.crates[crate].dir) + 1 :] if crate in self.crates else path
        directories = [part for part in rel.split("/")[:-1] if part not in ("src", "tests")]
        out = set(self._dir_reads.get(crate, set()))
        for word in directories:
            out |= self._words.get((crate, word), set())
        return out

    def _shared_readers(self, path: str, crate: Optional[str] = None) -> Set[str]:
        """The items whose strings name a directory (two components or more) that a data file is in,
        as `concat!(env!("CARGO_MANIFEST_DIR"), "/../../packages/protocol/fixtures/")` and a name
        joined to it do: for data outside the crates, any such directory; for a crate's data, read
        from another crate (`join("../a/data")`), one that names the crate's directory and more."""
        parts = path.split("/")[:-1]
        if crate is None:
            directories = {"/".join(parts[i:j]) for i in range(len(parts)) for j in range(i + 2, len(parts) + 1)}
        else:
            at = len(self.crates[crate].dir.split("/")) - 1  # the crate directory's own name
            directories = {"/".join(parts[i:j]) for i in range(at + 1) for j in range(at + 2, len(parts) + 1)}
        if not directories:
            return set()
        if not hasattr(self, "_path_literals"):
            self._path_literals: List[Tuple[str, str]] = [
                (literal, item_node(file, facts.items[index][0]))
                for file, facts in self.rs.items() for index, literals in enumerate(facts.item_literals)
                for literal in literals if "/" in literal]
        return {node for literal, node in self._path_literals if any(d in literal for d in directories)}

    def _mod_declarers(self, path: str) -> Set[str]:
        """The crates with a `mod` whose `#[path]` names this file."""
        out: Set[str] = set()
        for file, facts in self.rs.items():
            for name, path_attr, scope, *_ in facts.mods:
                if path_attr and path in self.mod_candidates(file, False, name, path_attr, scope):
                    out.add(self.crate_of(file) or "")
        return out - {""}

    @staticmethod
    def _global_changed(base: Optional[FileFacts], facts: Optional[FileFacts]) -> bool:
        """Whether a global allocator or a panic handler was added, removed or changed: code that
        names nothing calls it, so it reaches its whole crate."""
        def globals_of(source: Optional[FileFacts]) -> Set[Tuple[str, str]]:
            return {(item[0], item[6]) for item in (source.items if source else []) if item[8] == "global"}
        return globals_of(base) != globals_of(facts)

    def _read_as_text(self, path: str) -> bool:
        node = text_node(path)
        return any(node in targets for targets in self.edges.values())

    def _reach(self, starts: Dict[str, Set[str]], reverse: Dict[str, Set[str]],
               allowed: Set[str]) -> Tuple[Dict[str, Set[str]], Dict[str, str]]:
        """Every node within `allowed`'s crates that reaches a start, with the changed paths it reaches."""
        causes: Dict[str, Set[str]] = {node: set(c) for node, c in starts.items()}
        parent: Dict[str, str] = {}
        frontier = list(starts)
        crates: Dict[str, str] = {}
        while frontier:
            node = frontier.pop()
            mine = causes[node]
            for source in reverse.get(node, ()):
                file = file_of(source)
                if file not in crates:
                    crates[file] = self.crate_of(file) or ""
                if crates[file] not in allowed:
                    continue
                known = causes.get(source)
                if known is None:
                    causes[source] = set(mine)
                    parent[source] = node
                    frontier.append(source)
                elif not mine <= known:
                    known |= mine
                    frontier.append(source)
        return causes, parent

    def _chain(self, node: str, parent: Dict[str, str]) -> List[str]:
        chain = [node]
        while chain[-1] in parent and len(chain) < 16:
            chain.append(parent[chain[-1]])
        return chain



# --------------------------------------------------------------------------------------------------
# From a selection to nextest's filter expressions.

# A test or module path that a filter can name as a regular expression without escaping.
_PLAIN_PATH = re.compile(r"^(r#)?[A-Za-z0-9_]+(::(r#)?[A-Za-z0-9_]+)*$")


def unit_filter(tests: Sequence[TestFn], selected: Iterable[str], generated: Iterable[str],
                coarse: int = 0) -> Optional[str]:
    """nextest's filter for the selected tests of one binary, within it; None for all of its tests.

    A module whose own tests are all selected is named by its path, so that tests the source does
    not name (generated ones) run with it, and a subtree all of whose modules are, by its root; the
    rest are named one by one. `coarse` 1 names every module with a selected test by its path, for
    an expression that would be too long otherwise.
    """
    chosen = set(selected)
    extra = sorted(set(generated))
    names = {t.name for t in tests}
    if any(not _PLAIN_PATH.match(n) for n in chosen):
        return None
    by_module: Dict[str, List[str]] = {}
    for t in tests:
        by_module.setdefault(t.module, []).append(t.name)
    touched = {m for m, ns in by_module.items() if any(n in chosen for n in ns)}
    full = {m for m, ns in by_module.items() if all(n in chosen for n in ns)}
    if coarse:
        full |= touched
    if full == set(by_module) and chosen <= names:
        return None

    def below(m: str) -> List[str]:
        return [x for x in by_module if x == m or not m or x.startswith(m + "::")]

    terms: List[str] = []
    done: Set[str] = set()
    for m in sorted(by_module, key=lambda x: (x.count("::") if x else -1, x)):
        if m in done or m not in touched and m not in full:
            continue
        subtree = below(m)
        if m and all(x in full or x not in touched and not by_module[x] for x in subtree) \
                and all(x in full for x in subtree):
            terms.append(f"test(/^{m}::/)")
            done |= set(subtree)
        elif m in full:
            terms.append(f"test(/^{m}::[^:]+$/)" if m else "test(/^[^:]+$/)")
            done.add(m)
        else:
            terms += [f"test(={n})" for n in sorted(by_module[m]) if n in chosen]
            done.add(m)
    terms += extra
    return " | ".join(terms) if terms else "none()"
