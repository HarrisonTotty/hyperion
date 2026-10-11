#!/usr/bin/env python3
"""`just smart-ci`: the part of `just ci` that a change needs, chosen from its diff.

Usage: smart_ci.py [BASE | A...B] [--plan] [--dependents] [--all-tests] [--explain] [--full]

The change is every path that differs between the merge-base of HEAD and BASE (by default the
integration branch, `rendering-and-planets`) and the working tree, untracked files included, so it
works before a commit as well as after. `A...B` names the commits of B since its merge-base with A
instead, with no working tree: a plan for another branch, made with `--plan`.

Each changed path maps to the checks that read it (`classify`, and the README's "smart-ci" table),
and the plan runs those checks through the recipes `just ci` runs, scoped to the crates, packages
and files concerned, in `ci`'s order and phases: the checks that need no build directory, and vitest
(which takes no lock), beside the cargo steps, then every Rust suite in one hold of the heavy-test
lock, with `just test-render`'s run of the smoke harness in the same hold when a shader, the engine
or the harness changed. A path that no rule knows, and an input of the whole workspace (the
justfile, the root manifest, the lock files, the toolchain, `.cargo/`), runs the full `just ci`
instead.

`--plan` prints the plan, each step with the paths that caused it, and runs nothing; a run prints
the plan first. The tests run are those that reach the changed code through the code's graph
(`rust_graph.py`), in the changed crates and in those that depend on them; `--all-tests` runs every
test of the changed crates instead. `--dependents` adds the Clippy of every crate that depends on a
changed crate, and with `--all-tests` their every test (`just smart-ci --help`). `--full` runs the
full `just ci` whatever the diff, its build phases inside the build slot `SMART_CI_SLOT` names and
its suites outside it: integration's run.

Standard library only, Python 3.9 or later (the Command Line Tools' Python on macOS), so that it
runs wherever the justfile does. `test_smart_ci.py` beside it holds the rules to their table and
the steps to `just ci`'s.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import os
import posixpath
import re
import shlex
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import IO, Callable, Dict, Iterable, List, Mapping, Optional, Sequence, Set, Tuple

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import rust_graph  # noqa: E402  (beside this file: the Rust code's graph)

# The branch that lanes start from and the orchestrator integrates onto: the default base, unless
# SMART_CI_BASE names another.
INTEGRATION_BRANCH = os.environ.get("SMART_CI_BASE") or "rendering-and-planets"

# The crate that `just gen-surface` builds as a `cdylib` into the client's WebAssembly module, and
# where the module's glue lands. The module is this crate and the crates it links (hyperion-base),
# so a change to the sources of either changes it.
SURFACE_CRATE = "hyperion-surface"
SURFACE_GLUE = "apps/hyperion/src/renderer/src/generated/surface/"
# A source that names wasm_bindgen (`src/wasm.rs` today) writes the module's exports, whose `.d.ts`
# the client's type check and oxlint read.
EXPORTS = re.compile(r"\bwasm_bindgen\b")

# The crate whose `export_bindings` tests write the TypeScript bindings (`just gen-protocol`), and
# what it shares with the TypeScript side. nextest's default filter (`.config/nextest.toml`) skips
# those tests, so the bindings' check is the only check that runs them.
PROTOCOL_CRATE = "hyperion-protocol"
BINDINGS = "packages/protocol/src/generated/"
FIXTURES = "packages/protocol/fixtures/"

# `just fit-check` checks the fitting crate's lock file and inputs against the sim's tables, so it
# runs whenever either crate's tests do.
FIT_CHECK_CRATES = ("hyperion-fit", "hyperion-sim")

# Tests that read other crates' files as data: a change to a matching path runs the reader's tests
# as well, whatever `--dependents` says. `native` readers read files at run time, from a module
# compiled out on WebAssembly, so only their native suite runs; `all` readers include them at
# compile time, so their WebAssembly suites run too.
RUST_READERS: List[Tuple[str, str, str, str]] = [
    # The fitting crate's tests include_str! the committed tables and compare them with the fits.
    (r"^crates/hyperion-sim/src/tables/", "hyperion-fit", "all", "read by hyperion-fit's tests"),
    # The testkit's `tests/clippy_bans.rs` holds every clippy.toml to one ban list, checks that each
    # crate has one of its own or the root's, and reads the sources of base, the surface crate and
    # the sim for the relaxed-SIMD guards and `target_feature` attributes.
    (r"^(crates/[^/]+/)?clippy\.toml$", "hyperion-testkit", "native", "read by hyperion-testkit's clippy_bans"),
    (r"^crates/[^/]+/Cargo\.toml$", "hyperion-testkit", "native", "read by hyperion-testkit's clippy_bans"),
    (r"^crates/hyperion-(base|surface|sim)/.*\.rs$", "hyperion-testkit", "native",
     "read by hyperion-testkit's clippy_bans"),
]
# The sources of the tests that the justfile's `wasm_sky` leaves out of the wasip1 runs, of the test
# code they reach (the sky's modules and the sim's shared test fixtures) and of the testkit's golden
# checks: smart-ci's tests hold each such test to pinning no value (`test_smart_ci.py`'s guard, which
# checks that it reads no other file), so a change here runs them.
WASM_SKY_SOURCES = re.compile(r"^crates/hyperion-(sim/src/.*\.rs|testkit/src/golden\.rs)$")
# GENERATOR_VERSION heads every golden file, so a change to it runs every crate's tests.
GENERATOR_VERSION_FILE = "crates/hyperion-base/src/version.rs"
# A crate's goldens, under its `tests/golden/`, which its tests read by name through the testkit's
# `golden!` and `golden::check`: a changed golden runs the tests of its crate that reach that reader,
# and those that name the file's path (`rust_graph`).
GOLDEN_DIR = "tests/golden/"
GOLDEN_READERS = [(GOLDEN_DIR, "crates/hyperion-testkit/src/golden.rs")]
# The longest filter expression a step passes: the recipe's command line holds it, and Windows limits
# a command line to 32,767 characters. A longer one is made coarser (`Chosen.nextest_filter`).
MAX_FILTER = 20000

# The engine, the smoke harness and every shader run `just test-render` (R01.T9.e), as the validate
# skill routed them; so do the client's build configuration and pnpm's lock file, since the harness
# runs the client as built, on the Electron that the lock file names.
RENDER_PATHS = re.compile(
    r"^apps/hyperion/(src/renderer/src/view/(engine|shaders)/|src/renderer/src/smoke/|src/smoke/|"
    r"src/renderer/smoke\.html$|scripts/testRender\.sh$|electron\.vite\.config\.mts$)|\.wgsl$|"
    r"^pnpm-lock\.yaml$"
)
# The shader catalogue: the files it imports hold the materials' specs that the harness renders.
CATALOGUE = "apps/hyperion/src/renderer/src/view/engine/catalogue.ts"

# A path passed to a recipe as one word of a list: the shell splits the list, and Prettier and vitest
# read their arguments as patterns, so any other character sends the step back to its whole scope.
SAFE_PATH = re.compile(r"^[A-Za-z0-9._/@+-]+$")
TS_CODE = re.compile(r"\.(ts|tsx|mts|cts|js|mjs|cjs|jsx)$")
# The files the packages' vitest configurations include: `src/**/*.test.ts` and `.test.tsx`.
TEST_FILE = re.compile(r"/src/.*\.test\.tsx?$")
# What Prettier formats with this repository's configuration (no plugins): a changed file with one
# of these extensions is checked (and Prettier skips one its ignore file names).
PRETTIER_EXTENSIONS = {
    ".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".mts", ".cts", ".json", ".json5", ".jsonc",
    ".css", ".scss", ".less", ".html", ".htm", ".vue", ".md", ".markdown", ".mdx", ".yaml", ".yml",
    ".graphql", ".gql", ".hbs", ".handlebars",
}

# The inputs of the whole workspace, which run the full `just ci`. The root manifest, Cargo's lock
# file, `.cargo/` and the toolchain reach every crate and, through the surface module (built by that
# toolchain, lock file and profiles, with wasm-bindgen pinned in them) and the protocol bindings
# (ts-rs, and `.cargo/config.toml`'s TS_RS_EXPORT_DIR), the TypeScript side too, so "the Rust side
# in full" would be all of `ci` less Prettier. The justfile defines every check, this one's
# included. pnpm's lock file is the TypeScript side's and also the Electron that runs the browser
# target's Rust suites. `tools/portable/` stands in for flock, setsid and timeout in every recipe
# on macOS, where Linux never runs it.
FULL_CI_ROOT = {
    "Cargo.toml": "the workspace manifest: every crate's dependencies, profiles and lints, the surface module's",
    "Cargo.lock": "every crate's dependencies, and the wasm-bindgen that the browser checks pin",
    "rust-toolchain.toml": "the compiler and targets of every Rust check, the surface module's included",
    "justfile": "it defines every check, smart-ci's included",
    "pnpm-lock.yaml": "the TypeScript side's dependencies, and the Electron that runs the browser suites",
}
FULL_CI_PREFIXES = {
    ".cargo/": "cargo's configuration of every build, TS_RS_EXPORT_DIR for the bindings included",
    "tools/portable/": "the stand-ins for flock, setsid and timeout that every recipe runs on macOS",
}


# Every step `just ci` runs, with the recipes each calls in turn (its dependencies, with their
# arguments, and the recipes its body runs through `just`), and how smart-ci runs it. `scoped`: the
# step is `<recipe> "--workspace"` (or the like) and smart-ci calls that recipe with a narrower scope;
# `as is`: smart-ci calls the step itself, unchanged, when the change needs it. `test_smart_ci.py`
# holds this table to the justfile both ways, so that a step added to `ci`, or a command added to one
# of its steps, fails `just ci` until smart-ci knows it.
CI_STEPS: Dict[str, Tuple[str, List[Tuple[str, Optional[List[str]]]]]] = {
    "gen-surface": ("as is", [("_wasm-preflight", ["bindgen"])]),
    "_wasm-preflight": ("as is", []),
    "fmt-check": ("scoped", [("_rustfmt-check", ["--all"]), ("_prettier-check-all", [])]),
    "_typecheck-ts": ("as is", []),
    "_oxlint": ("as is", []),
    "_relaxed-simd-refused": ("as is", []),
    "gpu-replay-check": ("as is", [("_gpu-replay-cargo", None)]),
    "cross-clippy": ("scoped", [("_cross-clippy-for", ["--workspace", "gpu-replay"])]),
    "_smart-ci-test": ("as is", []),
    "_clippy": ("scoped", [("_clippy-for", ["--workspace"])]),
    "_browser-clippy": ("scoped", [("_browser-clippy-for", ["browser_crates"])]),
    "gen-protocol-check": ("scoped", [("_gen-protocol-check-for", ["--workspace"])]),
    "fit-check": ("as is", []),
    "_vitest-all": ("scoped", [("_vitest-run", ["", ""])]),
    "_test-build": ("scoped", [("_test-build-for", ["--workspace"])]),
    "_wasm-fast-build": ("scoped", [("_wasm-preflight", ["wasip1", "browser"]), ("_wasip1-build", None),
                                    ("_browser", None)]),
    "_locked": ("as is", []),
    "_test-run": ("scoped", [("_nextest-run", ["--workspace", ""]), ("_doctest-run", ["--workspace", ""])]),
    "_wasm-fast-run": ("scoped", [("_wasm-run", ["wasip1_crates", "wasip1_filter", "browser_crates"])]),
}
# `just ci`'s phases, the `just` arguments of each as its recipe runs them: its dependencies, the side
# steps (backgrounded, their output held), vitest (backgrounded on its own, its output held: it takes
# no lock, decision-test-coverage.md R1), the cargo steps, and the Rust suites under `_locked`. A full
# plan runs these through smart-ci's own phases, so that the build slot is released before the lock
# as in any plan; `test_smart_ci.py` holds them to the recipe.
CI_PHASES: Dict[str, List[List[str]]] = {
    "setup": [["gen-surface"], ["_wasm-preflight", "wasip1", "browser"]],
    "side": [["fmt-check"], ["_typecheck-ts"], ["_oxlint"], ["_relaxed-simd-refused"], ["gpu-replay-check"],
             ["cross-clippy"], ["_smart-ci-test"]],
    "unlocked": [["_vitest-all"]],
    "main": [["_clippy"], ["_browser-clippy"], ["gen-protocol-check"], ["fit-check"], ["_test-build"],
             ["_wasm-fast-build"]],
    "locked": [["_test-run"], ["_wasm-fast-run"]],
}
# `just test-render`'s steps, its dependencies and then its locked run, which a plan runs in its own
# phases when a change needs the smoke harness (R01.T9.e): the client's build beside the other steps,
# and the harness's run last in the same hold of the heavy-test lock as the suites (perf-ts Q1), so
# that the plan queues for the lock once. `test_smart_ci.py` holds them to the recipe.
RENDER_PHASES: Dict[str, List[List[str]]] = {
    "setup": [["gen-surface"]],
    "side": [["_test-render-build"]],
    "locked": [["_test-render-run"]],
}

# The environment variable naming the command that holds a build slot around smart-ci's phases that
# build (`slot_command`); unset, they run directly.
SLOT_VAR = "SMART_CI_SLOT"
# Set to 1 with `--full` (integration's run, and a run that stands in for it), the variable that gives
# its suites priority on the heavy-test lock: `_locked` with `HYPERION_HEAVY_PRIORITY=1` (perf-build
# L5 part a). A lane's own full plan, or `--full` without it, waits its turn as any run does.
PRIORITY_VAR = "SMART_CI_PRIORITY"
# The recipes of `ci`'s setup, side and main phases that run a cargo build, which a group of phases
# holds a build slot for; and the others: tools' checks, formatting, TypeScript, smart-ci's tests, and
# the suites, which run under the heavy-test lock from the builds made before it. `test_smart_ci.py`
# fails if a recipe of `ci` is in neither, or one said not to build runs cargo.
BUILDS = {
    "gen-surface", "_relaxed-simd-refused", "gpu-replay-check", "_gpu-replay-cargo", "cross-clippy",
    "_cross-clippy-for", "_clippy", "_clippy-for", "_browser-clippy", "_browser-clippy-for", "gen-protocol-check",
    "_gen-protocol-check-for", "fit-check", "_test-build", "_test-build-for", "_wasm-fast-build", "_wasip1-build",
    "_wasip1-nextest", "_browser",
}
NO_BUILDS = {
    "_wasm-preflight", "fmt-check", "_rustfmt-check", "_prettier-check-all", "_prettier-check", "_typecheck-ts",
    "_oxlint", "_smart-ci-test", "_locked", "_test-run", "_nextest-run", "_doctest-run", "_vitest-all",
    "_vitest-run", "_wasm-fast-run", "_wasm-run", "_test-render-build", "_test-render-run",
}

# The recipes smart-ci runs in place of one of `ci`'s for a narrower scope, where the step cannot be
# the narrower one with a wider argument without changing `ci`'s command: Prettier over the changed
# files, `prettier --check` with Prettier's configuration as `format:check` runs it over the tree.
NARROWER = {"_prettier-check-all": "_prettier-check"}


# --------------------------------------------------------------------------------------------------
# Clippy for the other platforms, and its guard (decision-test-coverage.md L4 and its §3.4 addendum).

# The workspace crates whose code differs between Linux, macOS and Windows: of the crates a change
# reaches, `_cross-clippy-for` lints these alone for the platforms the host is not (tools/gpu-replay
# has a rule of its own). Every other crate is the same code on all three after cfg expansion, which
# native Clippy lints. A reached crate in which the guard (`os_dependence`) finds such code is linted
# too, and the plan runs smart-ci's own tests, which fail until it is listed here, in the same commit.
# `just ci` keeps `cross-clippy --workspace`, so integration lints every crate on every push.
CROSS_CLIPPY_CRATES: Dict[str, str] = {
    "hyperion-server": "Linux-only and Unix-only code (cfg(unix), cfg(target_os = \"linux\"), std::os::unix)",
}
# The platforms `cross-clippy` checks (the justfile's `cross_targets`), each with the cfg values that
# `rustc --print cfg --target <triple>` gives, less those of no target (`debug_assertions`, `panic`).
# `test_smart_ci.py` holds the table to rustc.
DESKTOP_CFGS: Dict[str, str] = {
    "x86_64-unknown-linux-gnu": (
        'target_abi="" target_arch="x86_64" target_endian="little" target_env="gnu" target_family="unix" '
        'target_feature="fxsr" target_feature="sse" target_feature="sse2" target_has_atomic="16" '
        'target_has_atomic="32" target_has_atomic="64" target_has_atomic="8" target_has_atomic="ptr" '
        'target_has_atomic_primitive_alignment="16" target_has_atomic_primitive_alignment="32" '
        'target_has_atomic_primitive_alignment="64" target_has_atomic_primitive_alignment="8" '
        'target_has_atomic_primitive_alignment="ptr" target_os="linux" target_pointer_width="64" '
        'target_vendor="unknown" unix'),
    "aarch64-apple-darwin": (
        'target_abi="" target_arch="aarch64" target_endian="little" target_env="" target_family="unix" '
        'target_feature="aes" target_feature="crc" target_feature="dit" target_feature="dotprod" '
        'target_feature="dpb" target_feature="dpb2" target_feature="fcma" target_feature="fhm" '
        'target_feature="flagm" target_feature="fp16" target_feature="frintts" target_feature="jsconv" '
        'target_feature="lor" target_feature="lse" target_feature="neon" target_feature="paca" '
        'target_feature="pacg" target_feature="pan" target_feature="pmuv3" target_feature="ras" '
        'target_feature="rcpc" target_feature="rcpc2" target_feature="rdm" target_feature="sb" '
        'target_feature="sha2" target_feature="sha3" target_feature="ssbs" target_feature="vh" '
        'target_has_atomic="128" target_has_atomic="16" target_has_atomic="32" target_has_atomic="64" '
        'target_has_atomic="8" target_has_atomic="ptr" target_has_atomic_primitive_alignment="128" '
        'target_has_atomic_primitive_alignment="16" target_has_atomic_primitive_alignment="32" '
        'target_has_atomic_primitive_alignment="64" target_has_atomic_primitive_alignment="8" '
        'target_has_atomic_primitive_alignment="ptr" target_os="macos" target_pointer_width="64" '
        'target_vendor="apple" unix'),
    "x86_64-pc-windows-msvc": (
        'target_abi="" target_arch="x86_64" target_endian="little" target_env="msvc" target_family="windows" '
        'target_feature="cmpxchg16b" target_feature="fxsr" target_feature="sse" target_feature="sse2" '
        'target_feature="sse3" target_has_atomic="128" target_has_atomic="16" target_has_atomic="32" '
        'target_has_atomic="64" target_has_atomic="8" target_has_atomic="ptr" '
        'target_has_atomic_primitive_alignment="128" target_has_atomic_primitive_alignment="16" '
        'target_has_atomic_primitive_alignment="32" target_has_atomic_primitive_alignment="64" '
        'target_has_atomic_primitive_alignment="8" target_has_atomic_primitive_alignment="ptr" '
        'target_os="windows" target_pointer_width="64" target_vendor="pc" windows'),
}
# The dependencies that are OS interfaces, whichever cfg they sit under: the ruling's four, and the
# other bindings of one platform's system interfaces.
OS_CRATES = ("libc", "nix", "windows-sys", "winapi", "windows", "rustix", "core-foundation", "objc2", "mach2")
# Where Clippy's `--all-targets` finds a crate's code: its library and binaries, its tests, benches
# and examples, and its build script.
CRATE_CODE = ("src/", "tests/", "benches/", "examples/")
def desktop_cfgs() -> Dict[str, Set[Tuple[str, Optional[str]]]]:
    """Each desktop target's cfg options, as (name, value) pairs; a bare option's value is None."""
    out: Dict[str, Set[Tuple[str, Optional[str]]]] = {}
    for triple, text in DESKTOP_CFGS.items():
        options: Set[Tuple[str, Optional[str]]] = set()
        for word in text.split():
            name, eq, value = word.partition("=")
            options.add((name, value.strip('"') if eq else None))
        out[triple] = options
    return out


_DESKTOPS = list(desktop_cfgs().values())
# std's and core's platform modules, each with the submodules that exist on no desktop target, so
# that code using them fails native Clippy as it would the others' (and is gated to WebAssembly).
_PORTABLE_MODULES = {"os": ("wasi",), "arch": ("wasm32", "wasm64")}
# WebAssembly's target features, which `#[target_feature(enable = ...)]` may name without making code
# differ between the desktop targets (none of which has them).
WASM_FEATURES = ("simd128", "relaxed-simd", "atomics", "bulk-memory", "sign-ext", "nontrapping-fptoint",
                 "multivalue", "mutable-globals", "reference-types", "tail-call", "extended-const", "multimemory")
_FEATURE_DETECTED = re.compile(r"^is_\w+_feature_detected$")
# Where a cfg predicate starts, and the rest of what the guard looks for (a use tree's included), in a
# file's text, comments and strings included: what the guard reads a file for.
_CFG_START = re.compile(r"\bcfg(?:_attr)?\s*!?\s*\(")
_CFG_MARKS = re.compile(r'\\.|[()"]', re.S)  # an escape (skipped whole), a parenthesis, a quote
_PLATFORM_WORD = re.compile(r"\b(?:os|arch|c_u?long|is_\w+_feature_detected|target_feature)\b")
_PLATFORM_CODE = re.compile(r"\b(?:std|core)\s*::\s*(?:os|arch)\b|\buse\s+(?:::\s*)?(?:std|core)\b[^;]*\b(?:os|arch)\b"
                            r"|\bffi\s*::\s*c_u?long\b|\bis_\w+_feature_detected\s*!|\btarget_feature\s*\(\s*enable\b")


def _is_target_option(name: str) -> bool:
    """Whether a cfg option depends on the target (`target_*`, `unix`, `windows`); every other option
    (`test`, `feature`, `debug_assertions`, `doc`, a custom one) takes the same value on every target."""
    return name.startswith("target_") or name in ("unix", "windows")


Cfg = Tuple  # ("all" | "any" | "not", [children]) or ("option", name, value or None)


def parse_cfg(vals: Sequence[str], kinds: Sequence[str], i: int) -> Tuple[Optional[Cfg], int]:
    """The cfg predicate at token `i` (`rust_graph.lex`'s tokens) and the index after it; None where
    the tokens are no predicate (a macro's `$meta`, a call of something named `cfg`)."""
    if i >= len(vals) or kinds[i] != "i":
        return None, i
    name = vals[i]
    if i + 1 < len(vals) and vals[i + 1] == "(" and kinds[i + 1] == "p":
        children: List[Cfg] = []
        j = i + 2
        while j < len(vals) and vals[j] != ")":
            child, j = parse_cfg(vals, kinds, j)
            if child is None:
                return None, j
            children.append(child)
            if j < len(vals) and vals[j] == ",":
                j += 1
            elif j >= len(vals) or vals[j] != ")":
                return None, j
        if j >= len(vals) or name not in ("all", "any", "not") or (name == "not" and len(children) != 1):
            return None, j
        return (name, children), j + 1
    if i + 2 < len(vals) and vals[i + 1] == "=" and kinds[i + 2] == "s":
        return ("option", name, vals[i + 2]), i + 3
    return ("option", name, None), i + 1


def _cfg_value(cfg: Cfg, target: Set[Tuple[str, Optional[str]]], free: Mapping[Tuple[str, Optional[str]], bool]) -> bool:
    if cfg[0] == "option":
        option = (cfg[1], cfg[2])
        return option in target if _is_target_option(cfg[1]) else free[option]
    values = [_cfg_value(child, target, free) for child in cfg[1]]
    return all(values) if cfg[0] == "all" else any(values) if cfg[0] == "any" else not values[0]


def _cfg_options(cfg: Cfg) -> Set[Tuple[str, Optional[str]]]:
    if cfg[0] == "option":
        return {(cfg[1], cfg[2])}
    return set().union(*(_cfg_options(child) for child in cfg[1])) if cfg[1] else set()


def differs_between_desktops(cfg: Cfg) -> bool:
    """Whether a cfg predicate can take different values on Linux, macOS and Windows: evaluated on each
    of the three, for every value of its options that no target sets (decision-test-coverage.md
    §3.4 addendum). `all(target_arch = "wasm32", target_os = "unknown")` is false on all three, and
    its `not(...)` true, so neither differs; `any(target_family = "wasm", unix)` does."""
    options = _cfg_options(cfg)
    if not any(_is_target_option(name) for name, _ in options):
        return False
    free = sorted(o for o in options if not _is_target_option(o[0]))
    if len(free) > 10:
        return True  # too many to try: as if it differed
    for bits in range(1 << len(free)):
        assignment = {option: bool(bits >> k & 1) for k, option in enumerate(free)}
        if len({_cfg_value(cfg, target, assignment) for target in _DESKTOPS}) > 1:
            return True
    return False


def cfg_text(cfg: Cfg) -> str:
    if cfg[0] == "option":
        return cfg[1] if cfg[2] is None else f'{cfg[1]} = "{cfg[2]}"'
    return f"{cfg[0]}({', '.join(cfg_text(child) for child in cfg[1])})"


def os_dependent_code(text: str) -> List[str]:
    """What in a Rust file's code differs between Linux, macOS and Windows: each cfg predicate (of
    `#[cfg]`, `#[cfg_attr]`, `cfg!`, `doc(cfg)`) that takes different values on them; each path
    through std's or core's platform modules (`os`, but for WASI's, and `arch`, but for WebAssembly's)
    and the C integers whose width is Windows' own (`ffi::c_long`, `c_ulong`); the CPU-feature
    detection macros; and `#[target_feature(enable = ...)]` of a feature that is not WebAssembly's.
    Comments and strings are no code (`rust_graph.lex`)."""
    if not _maybe_os_dependent(text):
        return []
    try:
        lexed = rust_graph.lex(text)
    except rust_graph.LexError:
        return ["code the guard cannot read"]
    vals, kinds = lexed.vals, lexed.kinds
    found: Set[str] = set()

    def module_path(k: int, end: int) -> Optional[str]:
        """`os::<x>` or `arch::<x>` at token k, if that is a platform module (`os` or `arch` alone is)."""
        after = vals[k + 2] if k + 2 < end and vals[k + 1] == "::" and kinds[k + 2] == "i" else None
        if after in _PORTABLE_MODULES.get(vals[k], ()):
            return None
        return vals[k] + ("::" + after if after else "")

    i = 0
    while i < len(vals):
        word = vals[i] if kinds[i] == "i" else None
        if word in ("cfg", "cfg_attr"):
            j = i + 1
            if word == "cfg" and j < len(vals) and vals[j] == "!":
                j += 1
            if j < len(vals) and vals[j] == "(":
                cfg, _ = parse_cfg(vals, kinds, j + 1)
                if cfg is not None and differs_between_desktops(cfg):
                    found.add(f"cfg({cfg_text(cfg)})")
        elif word in ("std", "core") and vals[i + 1 : i + 2] == ["::"] and vals[i + 2 : i + 3] in (["os"], ["arch"]):
            path = module_path(i + 2, len(vals))
            if path:
                found.add(f"{word}::{path}")
        elif word == "use":
            # `use std::{fs, os::unix::...}`: a use tree of std's or core's that names a platform module.
            end = next((k for k in range(i, len(vals)) if vals[k] == ";"), len(vals))
            root = next((vals[k] for k in range(i + 1, end) if kinds[k] == "i"), "")
            if root in ("std", "core"):
                for k in range(i + 1, end):
                    if kinds[k] == "i" and vals[k] in _PORTABLE_MODULES:
                        path = module_path(k, end)
                        if path:
                            found.add(f"{root}::{path}")
        elif word == "ffi" and vals[i + 1 : i + 2] == ["::"] and vals[i + 2 : i + 3] in (["c_long"], ["c_ulong"]):
            found.add(f"ffi::{vals[i + 2]}")
        elif word and _FEATURE_DETECTED.match(word) and vals[i + 1 : i + 2] == ["!"]:
            found.add(f"{word}!")
        elif word == "target_feature" and vals[i + 1 : i + 3] == ["(", "enable"] and vals[i + 3 : i + 4] == ["="]:
            value = vals[i + 4] if i + 4 < len(vals) and kinds[i + 4] == "s" else "?"
            if any(feature.strip() not in WASM_FEATURES for feature in value.split(",")):
                found.add(f'target_feature(enable = "{value}")')
        i += 1
    return sorted(found)


def _maybe_os_dependent(text: str) -> bool:
    """Whether a file's text, its comments and strings included, has a cfg predicate that differs
    between the desktop targets or a path through `std::os`: a quick test that the code has neither
    (the comments and strings only add to what it finds), so that most files are never lexed whole."""
    if _PLATFORM_WORD.search(text) and _PLATFORM_CODE.search(text):
        return True
    for start in _CFG_START.finditer(text):
        # The predicate's text: to the parenthesis that closes the one opened, outside strings.
        depth, end, quoted = 1, None, False
        for mark in _CFG_MARKS.finditer(text, start.end()):
            c = mark.group()
            if quoted:
                quoted = c != '"'
            elif c == '"':
                quoted = True
            elif c == "(":
                depth += 1
            elif c == ")":
                depth -= 1
                if depth == 0:
                    end = mark.start()
                    break
        if end is None:
            return True
        try:
            lexed = rust_graph.lex(text[start.end():end])
        except rust_graph.LexError:
            return True
        cfg, _ = parse_cfg(lexed.vals, lexed.kinds, 0)
        if cfg is not None and differs_between_desktops(cfg):
            return True
    return False


def manifest_os_dependence(meta: dict) -> Dict[str, List[str]]:
    """Each workspace crate's OS-dependent build, from `cargo metadata`: a dependency on an OS interface
    (`OS_CRATES`), a dependency table of a platform whose cfg differs between Linux, macOS and Windows,
    or a build script."""
    members = set(meta["workspace_members"])
    out: Dict[str, List[str]] = {}
    for package in meta["packages"]:
        if package["id"] not in members:
            continue
        reasons: Set[str] = set()
        for dep in package["dependencies"]:
            if dep["name"] in OS_CRATES:
                reasons.add(f"a dependency on {dep['name']}")
            platform = dep.get("target")
            if platform and platform_differs(platform):
                reasons.add(f"[target.'{platform}'] dependencies")
        if any(t["kind"][0] == "custom-build" for t in package["targets"]):
            reasons.add("a build script")
        if reasons:
            out[package["name"]] = sorted(reasons)
    return out


def platform_differs(platform: str) -> bool:
    """Whether a `[target.<platform>]` table applies on some of the desktop targets and not others: a
    `cfg(...)` by its predicate, a target triple by its name."""
    if not platform.startswith("cfg("):
        return platform in DESKTOP_CFGS
    lexed = rust_graph.lex(platform)
    cfg, end = parse_cfg(lexed.vals, lexed.kinds, 2)
    return cfg is None or end != len(lexed.vals) - 1 or differs_between_desktops(cfg)


def os_dependence(ws: "Workspace", crate: str) -> List[str]:
    """Why a crate's code may differ between Linux, macOS and Windows (none if it cannot): its manifest's
    reasons, and each of its files, over every target Clippy's `--all-targets` lints, with what in it
    differs (decision-test-coverage.md L4)."""
    directory = ws.crates[crate].dir + "/"
    reasons = list(ws.os_manifest.get(crate, []))
    # A range's plan reads the tree at its end, as the code's graph does.
    at_end = ws.rust_files is not None and ws.read_bytes is not None
    for path in ws.rust_files if at_end else ws.files:
        rel = path[len(directory):] if path.startswith(directory) else None
        if rel is None or not path.endswith(".rs") or not (rel.startswith(CRATE_CODE) or rel == "build.rs"):
            continue
        data = ws.read_bytes(path) if at_end else None
        found = os_dependent_code(data.decode("utf-8", "ignore") if data is not None else ws.read(path))
        if found:
            reasons.append(f"{path}: {', '.join(found)}")
    return reasons


# --------------------------------------------------------------------------------------------------
# The workspace: its crates, their graph and features, the TypeScript packages, the files to scan.


@dataclass
class Crate:
    """A member of the Cargo workspace."""

    name: str
    dir: str  # repository-relative, e.g. "crates/hyperion-sim"
    # In-workspace dependencies: (member, kind, features, uses default features, optional, the name
    # this crate's manifest gives it).
    deps: List[Tuple[str, Optional[str], List[str], bool, bool, str]] = field(default_factory=list)
    features: Dict[str, List[str]] = field(default_factory=dict)


@dataclass
class Workspace:
    """What the planner knows of the repository: `load_workspace` reads it, tests build it."""

    crates: Dict[str, Crate]
    # The crates of `just ci`'s WebAssembly suites: the justfile's `wasip1_crates` and
    # `browser_crates`.
    wasip1: List[str]
    browser: List[str]
    # TypeScript packages: directory -> (package name, the entry its bare name imports).
    ts_packages: Dict[str, Tuple[str, Optional[str]]]
    # Every file of the repository (tracked, and untracked but not ignored), for the scans.
    files: List[str]
    read: Callable[[str], str]
    # The negated patterns of package.json's `format:check` (`!.claude/**`): Prettier refuses a file
    # named on its command line that one of them excludes, so they are filtered out beforehand.
    prettier_excludes: List[str]
    catalogue_sources: Set[str] = field(default_factory=set)
    # The justfile's `wasm_sky`: nextest's filter of the tests that `ci`'s wasip1 run leaves to
    # `just test-wasm-sky`, and so the wasip1 run here too; empty, none.
    wasm_sky: str = ""
    # The Rust graph's inputs (`rust_graph`): the crates' targets and dependencies, the crates with a
    # build script, a file's bytes now and at the base, git's ids of the files the index holds as they
    # are, and the cache's file. No crates: the graph is not built, and tests run by crate.
    rust: Dict[str, "rust_graph.CrateInfo"] = field(default_factory=dict)
    build_scripts: Set[str] = field(default_factory=set)
    proc_macros: Set[str] = field(default_factory=set)
    read_bytes: Optional[Callable[[str], Optional[bytes]]] = None
    read_base: Optional[Callable[[str], Optional[bytes]]] = None
    ids: Dict[str, str] = field(default_factory=dict)
    cache: Optional[Path] = None
    rust_files: Optional[List[str]] = None  # the files of the tree the graph reads, if not the working tree's
    # Each crate's OS-dependent build, from its manifest (`manifest_os_dependence`), for the guard.
    os_manifest: Dict[str, List[str]] = field(default_factory=dict)

    def crate_of(self, path: str) -> Optional[Crate]:
        for crate in self.crates.values():
            if path.startswith(crate.dir + "/"):
                return crate
        return None

    def package_of(self, path: str) -> Optional[str]:
        for directory in self.ts_packages:
            if path.startswith(directory + "/"):
                return directory
        return None

    def own_clippy_toml(self, crate: str) -> bool:
        return self.crates[crate].dir + "/clippy.toml" in self.files

    def reverse_dependents(self, names: Iterable[str]) -> Set[str]:
        """Every crate that depends on one of `names`, directly or through others, by any kind."""
        names = set(names)
        found: Set[str] = set()
        frontier = set(names)
        while frontier:
            frontier = {
                crate.name
                for crate in self.crates.values()
                if crate.name not in found | names and any(dep[0] in frontier for dep in crate.deps)
            }
            found |= frontier
        return found

    def linked(self, name: str) -> Set[str]:
        """`name` and the crates it links: its normal dependencies, transitively."""
        found = {name}
        frontier = [name]
        while frontier:
            for dep in self.crates[frontier.pop()].deps:
                if dep[1] is None and dep[0] not in found:
                    found.add(dep[0])
                    frontier.append(dep[0])
        return found

    def surface_module_crates(self) -> Set[str]:
        return self.linked(SURFACE_CRATE) if SURFACE_CRATE in self.crates else set()

    def unified_features(self, selected: Iterable[str]) -> Dict[str, Set[str]]:
        """The features cargo enables on each member when `selected` are built with their tests.

        As cargo's resolver does for `-p` (or `--workspace`): the defaults of the selected packages,
        the features each in-workspace dependency is asked for (dev-dependencies of the selected
        packages only, whose tests are built), and the entries of every enabled feature,
        `crate/feature`, `crate?/feature` and plain names, to a fixed point.
        """
        selected = set(selected)
        enabled: Dict[str, Set[str]] = {name: set() for name in self.crates}
        active: Set[str] = set(selected)
        for name in selected:
            if "default" in self.crates[name].features:
                enabled[name].add("default")
        changed = True

        def enable(crate: str, feature: Optional[str]) -> None:
            nonlocal changed
            if crate not in active:
                active.add(crate)
                changed = True
            if feature is not None and feature not in enabled[crate]:
                enabled[crate].add(feature)
                changed = True

        while changed:
            changed = False
            for name in sorted(active):
                crate = self.crates[name]
                local = {dep[5]: dep[0] for dep in crate.deps}
                entries = [e for f in sorted(enabled[name]) for e in crate.features.get(f, [])]
                for entry in entries:
                    target, slash, sub = entry.partition("/")
                    target = target.rstrip("?")
                    if slash and target in local:
                        enable(local[target], sub)
                    elif not slash and entry in crate.features:
                        enable(name, entry)
                for dep, kind, features, default, optional, as_named in crate.deps:
                    if kind == "dev" and name not in selected:
                        continue
                    if optional and not any(
                        e in (as_named, "dep:" + as_named) or e.startswith((as_named + "/",)) for e in entries
                    ):
                        continue
                    enable(dep, "default" if default and "default" in self.crates[dep].features else None)
                    for feature in features:
                        enable(dep, feature)
        return {name: enabled[name] for name in sorted(active)}

    @staticmethod
    def plain_packages(names: Iterable[str]) -> str:
        """`-p <crate>` for each name, as `cargo fmt` takes them."""
        return " ".join("-p " + name for name in sorted(set(names)))

    def cargo_packages(self, names: Iterable[str], universe: Optional[Iterable[str]] = None) -> str:
        """`-p <crate>` for each name, with `--features` for those `universe` would enable on them.

        `universe` is what `just ci`'s step selects, every member for `--workspace`, so that a
        crate is checked with the features it has there and its build is that build.
        """
        names = sorted(set(names))
        features = self.unified_features(self.crates if universe is None else universe)
        flags = [
            name + "/" + feature
            for name in names
            for feature in sorted(features.get(name, set()) - {"default"})
            if feature in self.crates[name].features
        ]
        return " ".join(["-p " + name for name in names] + (["--features", ",".join(flags)] if flags else []))


# --------------------------------------------------------------------------------------------------
# The plan.


@dataclass
class Step:
    phase: str  # setup, side, main, locked
    title: str
    recipe: List[str]  # `just`'s arguments: a recipe and its arguments
    causes: List[str]


@dataclass
class Plan:
    full: Dict[str, Set[str]]  # why the full `just ci` runs -> the paths
    steps: List[Step]
    render: Set[str]  # the paths that run `just test-render`'s steps
    why: Dict[str, List[str]]  # what each changed path runs
    notes: List[str]
    details: List[str] = field(default_factory=list)  # the tests the code's graph chose, and why

    @property
    def is_full(self) -> bool:
        return bool(self.full)

    def recipes(self) -> List[str]:
        """The recipes the plan runs: its steps', or `ci` and, with the smoke harness, its steps'."""
        if not self.is_full:
            return [step.recipe[0] for step in self.steps]
        return ["ci"] + ([recipe[0] for phase in ("side", "locked") for recipe in RENDER_PHASES[phase]]
                         if self.render else [])


class Needs:
    """What the changed paths ask for, each demand with the paths that caused it."""

    def __init__(self) -> None:
        self.full: Dict[str, Set[str]] = {}
        self.code: Dict[str, Set[str]] = {}  # crates whose code changed
        self.fmt: Dict[str, Set[str]] = {}
        self.clippy: Dict[str, Set[str]] = {}
        self.tests: Dict[str, Set[str]] = {}  # every suite: native, doctests, wasip1, browser
        self.native: Dict[str, Set[str]] = {}  # the native suite alone
        # Changes whose tests the code's graph chooses (Rust code, goldens): every test of the crate
        # with --all-tests, as `tests` holds them.
        self.graph: Dict[str, Set[str]] = {}
        self.manifests: Dict[str, Set[str]] = {}  # crates whose manifest changed: their dependents run
        self.prettier: Set[str] = set()
        self.prettier_all: Set[str] = set()
        self.typecheck: Set[str] = set()
        self.oxlint: Set[str] = set()
        self.vitest_all: Set[str] = set()
        self.vitest_package: Dict[str, Set[str]] = {}  # package directory -> causes
        self.vitest_files: Dict[str, Dict[str, Set[str]]] = {}  # package directory -> test -> causes
        self.vitest_graph: Set[str] = set()  # sources whose vitest files the import graph finds
        self.surface_module: Set[str] = set()
        self.gen_protocol: Set[str] = set()
        self.gpu_replay: Set[str] = set()
        self.smart_ci_tests: Set[str] = set()
        self.render: Set[str] = set()
        self.why: Dict[str, List[str]] = {}
        self.notes: List[str] = []

    @staticmethod
    def add(table: Dict[str, Set[str]], key: str, cause: str) -> None:
        table.setdefault(key, set()).add(cause)

    def say(self, path: str, what: str) -> None:
        self.why.setdefault(path, []).append(what)

    def ts_full(self, path: str) -> None:
        self.typecheck.add(path)
        self.oxlint.add(path)
        self.vitest_all.add(path)

    def crate_code(self, crate: str, path: str, graph: bool = False) -> None:
        for table in (self.code, self.fmt, self.clippy, self.graph if graph else self.tests):
            self.add(table, crate, path)


def prettier_excluded(path: str, excludes: Sequence[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, p) or path.startswith(p.rstrip("*")) for p in excludes)


_DOC_INCLUDE = re.compile(r"\bdoc\s*=\s*include_str!\s*\(\s*\"([^\"]+)\"")


def docs_include(ws: Workspace, crate: str, path: str) -> bool:
    """Whether a crate's library or binary root includes this file as its documentation, so that its
    code blocks are doctests (`#![doc = include_str!("../README.md")]`)."""
    info = ws.rust.get(crate)
    for target in (info.targets if info else ()):
        if target.kind not in ("lib", "bin") or target.src not in ws.files:
            continue
        for included in _DOC_INCLUDE.findall(ws.read(target.src)):
            if posixpath.normpath(posixpath.join(posixpath.dirname(target.src), included)) == path:
                return True
    return False


def classify(path: str, deleted: bool, ws: Workspace, needs: Needs) -> None:
    """Map one changed path to the checks that read it."""
    name = posixpath.basename(path)
    ext = posixpath.splitext(name)[1]
    if not deleted and not prettier_excluded(path, ws.prettier_excludes):
        if ext in PRETTIER_EXTENSIONS:
            needs.prettier.add(path)
    if RENDER_PATHS.search(path) or path in ws.catalogue_sources:
        needs.render.add(path)
        needs.say(path, "just test-render")

    for regex, reader, kinds, why in RUST_READERS:
        if re.search(regex, path) and reader in ws.crates:
            needs.add(needs.tests if kinds == "all" else needs.native, reader, path)
            needs.say(path, why)
    if WASM_SKY_SOURCES.search(path):
        needs.smart_ci_tests.add(path)
        needs.say(path, "smart-ci's tests: the tests `wasm_sky` leaves out of wasip1 pin no value")
    if path == GENERATOR_VERSION_FILE:
        for crate in ws.crates:
            needs.add(needs.tests, crate, path)
        needs.say(path, "GENERATOR_VERSION heads every golden file: every crate's tests")

    # The inputs of the whole workspace.
    if path in FULL_CI_ROOT:
        needs.add(needs.full, f"{path}: {FULL_CI_ROOT[path]}", path)
        return
    for prefix, why in FULL_CI_PREFIXES.items():
        if path.startswith(prefix):
            needs.add(needs.full, f"{prefix}: {why}", path)
            return

    # Agent configuration: nothing in `just ci` reads it (Prettier excludes `.claude/`) but oxlint,
    # which lints any JavaScript or TypeScript there.
    if path.startswith(".claude/"):
        if TS_CODE.search(path):
            needs.oxlint.add(path)
            needs.say(path, "oxlint")
        else:
            needs.say(path, "nothing in `just ci` reads .claude/")
        return

    if path.startswith("docs/"):
        if ext == ".md" or name == ".gitkeep":
            needs.say(path, "documentation: Prettier")
            return
        if path.startswith("docs/measurements/") and "apps/hyperion" in ws.ts_packages:
            # The client's results tests validate every committed results file there.
            needs.add(needs.vitest_package, "apps/hyperion", path)
            needs.say(path, "a measurement the client's vitest suite reads")
            return
        needs.add(needs.full, "paths no smart-ci rule knows", path)
        return

    if path.startswith("crates/"):
        crate = ws.crate_of(path)
        if crate is None:
            needs.add(needs.full, "paths no smart-ci rule knows (in no workspace crate)", path)
            return
        rel = path[len(crate.dir) + 1 :]
        if rel == "Cargo.toml":
            needs.crate_code(crate.name, path)
            # Its features and dependencies are its dependents' build too.
            needs.add(needs.manifests, crate.name, path)
            needs.say(path, f"{crate.name}'s manifest: its checks and its dependents'")
        elif rel == "clippy.toml":
            needs.add(needs.clippy, crate.name, path)
            needs.say(path, f"{crate.name}'s Clippy configuration")
        elif ext == ".rs" or rel in ("rustfmt.toml", ".rustfmt.toml"):
            # Code: the tests that reach it. A crate with a build script, which may read any of its
            # files, runs all of its tests; a proc-macro crate, its own and its dependents' (what
            # their code expands to changes, and the graph sees no derive's name).
            needs.crate_code(crate.name, path, graph=ext == ".rs" and crate.name not in ws.build_scripts
                             and crate.name not in ws.proc_macros)
            if crate.name in ws.proc_macros:
                needs.add(needs.manifests, crate.name, path)
            needs.say(path, f"{crate.name}'s code")
        elif ext == ".md" and "/data/" not in "/" + rel and not docs_include(ws, crate.name, path):
            needs.say(path, "documentation: Prettier")
            return
        elif ext == ".md" and "/data/" not in "/" + rel:
            # `#![doc = include_str!("../README.md")]`: its code blocks are the crate's doctests. As
            # the crate's data, its every test (the graph does not read the file's code blocks).
            needs.add(needs.graph, crate.name, path)
            needs.say(path, f"{crate.name}'s documentation, which its crate root includes: its tests")
        elif rel.startswith(GOLDEN_DIR):
            # A golden: the tests that read it, by name, through the golden reader, and what names it.
            needs.add(needs.graph, crate.name, path)
            needs.say(path, f"{crate.name}'s golden: the tests that read goldens")
        else:
            # Data: fixtures, fitted inputs and manifests, a data directory's README. Its crate's
            # every test, and through the graph the tests of other crates that name the file or its
            # directory (with --all-tests, its crate's alone).
            needs.add(needs.graph, crate.name, path)
            needs.say(path, f"{crate.name}'s data: its tests, and those that name it")
        if crate.name == PROTOCOL_CRATE:
            needs.gen_protocol.add(path)
            needs.ts_full(path)
            needs.say(path, "the protocol: the bindings' check and the TypeScript side that imports them")
        if crate.name in ws.surface_module_crates() and (rel.startswith("src/") or rel == "Cargo.toml"):
            needs.surface_module.add(path)
            needs.say(path, "the surface module: gen-surface and the vitest files that import it")
            if ext == ".rs" and (deleted or EXPORTS.search(ws.read(path))):
                needs.typecheck.add(path)
                needs.oxlint.add(path)
                needs.say(path, "the module's exports (wasm_bindgen), whose .d.ts tsc and oxlint read")
        return

    package = ws.package_of(path)
    if package is not None:
        if path.startswith(BINDINGS):
            needs.gen_protocol.add(path)
            needs.ts_full(path)
            needs.say(path, "the protocol bindings: their check and the TypeScript side")
        elif path.startswith(FIXTURES):
            needs.ts_full(path)
            # The Rust tests that read it: the graph's, or with --all-tests every test of the crates
            # that name it (`fixture_readers`). "" holds data outside every crate.
            needs.add(needs.graph, "", path)
            needs.say(path, "a fixture of both sides: the TypeScript side and the Rust tests that read it")
        elif ext == ".md":
            needs.say(path, "documentation: Prettier")
        elif path[len(package) + 1 :].startswith("src/"):
            # A source of the package: tsc and oxlint, and the vitest files that reach it.
            needs.typecheck.add(path)
            needs.oxlint.add(path)
            needs.vitest_graph.add(path)
            needs.say(path, "TypeScript: tsc, oxlint, and the vitest files that reach it")
        else:
            # The package's configuration (package.json, tsconfig, vitest's and Vite's), its scripts:
            # every check of the TypeScript side.
            needs.ts_full(path)
            needs.say(path, "TypeScript: tsc, oxlint, vitest")
        return
    if path.startswith(("apps/", "packages/")):
        needs.add(needs.full, "paths no smart-ci rule knows (in no pnpm workspace package)", path)
        return

    if path.startswith("tools/gpu-replay/"):
        needs.gpu_replay.add(path)
        needs.say(path, "tools/gpu-replay: its check and its Clippy for the other platforms")
        return
    if path.startswith("tools/smart-ci/"):
        needs.smart_ci_tests.add(path)
        needs.say(path, "smart-ci's own tests")
        return
    if path.startswith("tools/electron-node/"):
        for crate in ws.browser:
            needs.add(needs.tests, crate, path)
        needs.say(path, "the `node` the browser target's suites run on: their crates' tests")
        return

    if "/" not in path:
        if ext == ".md" or name == "NOTICE":
            needs.say(path, "documentation: Prettier")
            return
        if name in ("package.json", "pnpm-workspace.yaml", "tsconfig.base.json"):
            needs.ts_full(path)
            needs.say(path, "the TypeScript side's configuration: the TypeScript side")
            if name == "package.json":
                needs.prettier_all.add(path)
                needs.say(path, "its format:check script: Prettier over every file")
            return
        if name == ".oxlintrc.json":
            needs.oxlint.add(path)
            needs.say(path, "oxlint's configuration")
            return
        if name in (".prettierrc.json", ".prettierignore", ".editorconfig"):
            needs.prettier_all.add(path)
            needs.say(path, "Prettier's configuration: Prettier over every file")
            return
        if name == ".gitignore":
            needs.prettier_all.add(path)
            needs.oxlint.add(path)
            needs.say(path, "Prettier and oxlint skip what git ignores")
            return
        if name == "rustfmt.toml":
            for crate in ws.crates:
                needs.add(needs.fmt, crate, path)
            needs.say(path, "rustfmt's configuration: every crate's formatting")
            return
        if name == "clippy.toml":
            for crate in ws.crates:
                if not ws.own_clippy_toml(crate):
                    needs.add(needs.clippy, crate, path)
            needs.gpu_replay.add(path)
            needs.say(path, "Clippy's configuration of the crates without one of their own, and tools/gpu-replay")
            return
        if name == ".pre-commit-config.yaml":
            needs.say(path, "the git hooks: nothing in `just ci` reads it, so Prettier alone")
            needs.notes.append(
                ".pre-commit-config.yaml changed: nothing in `just ci` reads it, so smart-ci runs Prettier "
                "on it alone; the next `git commit` and `git push` run its hooks."
            )
            return
    if path == ".config/nextest.toml":
        for crate in ws.crates:
            needs.add(needs.tests, crate, path)
        # Its default filter leaves the protocol's export tests to the bindings' check.
        needs.gen_protocol.add(path)
        needs.say(path, "nextest's configuration: every crate's tests, and the bindings' check, which runs the "
                  "export tests nextest's default filter skips")
        return

    needs.add(needs.full, "paths no smart-ci rule knows", path)


def setup_files(ws: Workspace) -> Dict[str, Set[str]]:
    """Each package's vitest setup files (`setupFiles`, `globalSetup` in its vitest configuration),
    which run before its tests: a change they reach reaches all of them. A configuration the reading
    cannot follow names every file of the package, so that any change runs all of its tests."""
    out: Dict[str, Set[str]] = {}
    for directory in ws.ts_packages:
        configs = [f for f in ws.files if posixpath.dirname(f) == directory
                   and re.match(r"^vite(st)?\.config\.[cm]?[jt]s$", posixpath.basename(f))]
        for config in configs:
            text = code_of(ws.read(config))
            for match in re.finditer(r"\b(?:setupFiles|globalSetup)\s*:\s*(\[[^\]]*\]|[\"'`][^\"'`]*[\"'`])", text):
                names = re.findall(r"[\"'`]([^\"'`]+)[\"'`]", match.group(1))
                if not names or re.search(r"[^\s\[\],]", re.sub(r"[\"'`][^\"'`]*[\"'`]", "", match.group(1))):
                    out.setdefault(directory, set()).update(f for f in ws.files if f.startswith(directory + "/"))
                    continue
                for name in names:
                    out.setdefault(directory, set()).add(posixpath.normpath(posixpath.join(directory, name)))
    return out


def fixture_readers(path: str, ws: Workspace) -> Set[str]:
    """The crates whose sources name this shared fixture; every crate that names the directory, if none."""
    base = posixpath.basename(path)
    named: Set[str] = set()
    any_fixture: Set[str] = set()
    for file in ws.files:
        crate = ws.crate_of(file) if file.endswith(".rs") else None
        if crate is None:
            continue
        text = ws.read(file)
        if "protocol/fixtures" in text:
            any_fixture.add(crate.name)
            if "fixtures/" + base in text:
                named.add(crate.name)
    return named or any_fixture


# --------------------------------------------------------------------------------------------------
# The TypeScript side's reads of files outside its packages: imports (goldens and Rust sources as
# `?raw`, the surface module) and quoted paths (data read at run time).

IMPORT_SPEC = re.compile(
    r"""(?:\bfrom\s*|\bimport\s*\(?\s*|\bnew\s+URL\(\s*|\bvi\.(?:mock|importActual)\(\s*|\brequire\(\s*)"""
    r"""["'`]([^"'`\n]+)["'`]"""
)
# A quoted string with a slash in it: a path, perhaps.
QUOTED_PATH = re.compile(r"""["'`]([^"'`\s]*/[^"'`\s]*)["'`]""")
# Comments, which name files without reading them: a line that starts one or continues a JSDoc
# block, and a line comment after code (not `://`).
COMMENT_LINE = re.compile(r"^\s*(\*|/\*|//)")
TRAILING_COMMENT = re.compile(r"(^|\s)//.*$")


def code_of(text: str) -> str:
    """A TypeScript file's text less its comments, as far as a line-by-line look can tell."""
    return "\n".join(TRAILING_COMMENT.sub("", line) for line in text.splitlines() if not COMMENT_LINE.match(line))
RESOLVE_SUFFIXES = ["", ".ts", ".tsx", ".mts", ".cts", ".js", ".mjs", ".d.ts", "/index.ts", "/index.tsx"]


class TsGraph:
    """The TypeScript packages' import graph, for the test files that read a given file.

    An import is any module specifier the code gives: `import`/`export ... from`, a dynamic `import()`,
    a worker's `new URL(...)`, `vi.mock`, `require`, and each file an `import.meta.glob` pattern
    matches (Vite's, as the boundary tests read every renderer source with it). A specifier the graph
    cannot follow (a template with `${...}`, a module of no workspace package) imports nothing it
    knows of; a read at run time is a quoted path (`readers`).
    """

    def __init__(self, ws: Workspace) -> None:
        self.ws = ws
        self.files = sorted(f for f in ws.files if ws.package_of(f) and TS_CODE.search(f))
        self.known = set(ws.files)
        packages = {name: (directory, entry) for directory, (name, entry) in ws.ts_packages.items()}
        self.imports: Dict[str, Set[str]] = {}
        self.importers: Dict[str, Set[str]] = {}
        for file in self.files:
            targets: Set[str] = set()
            text = ws.read(file)
            for spec in IMPORT_SPEC.findall(text):
                target = self.resolve(file, spec.split("?", 1)[0], packages)
                if target:
                    targets.add(target)
            targets |= self.globbed(file, text)
            for target in targets:
                self.importers.setdefault(target, set()).add(file)
                # A file deleted since: its importers name it without its extension.
                stem = re.sub(r"\.(d\.ts|tsx?|mts|cts|jsx?|mjs|cjs)$", "", target)
                if stem != target:
                    self.importers.setdefault(stem, set()).add(file)
            self.imports[file] = targets

    def globbed(self, importer: str, text: str) -> Set[str]:
        """The files `import.meta.glob(...)` patterns in a file match: from the package's root for a
        pattern that starts with `/`, from the file's directory otherwise."""
        package = self.ws.package_of(importer) or ""
        out: Set[str] = set()
        for match in re.finditer(r"import\.meta\.glob(?:<[^>]*>)?\(\s*(\[[^\]]*\]|[\"'`][^\"'`]*[\"'`])", text):
            for pattern in re.findall(r"[\"'`]([^\"'`]+)[\"'`]", match.group(1)):
                negated = pattern.startswith("!")
                pattern = pattern.lstrip("!")
                base = package if pattern.startswith("/") else posixpath.dirname(importer)
                full = posixpath.normpath(posixpath.join(base, pattern.lstrip("/")))
                shapes = {full, full.replace("**/", "")}
                if not negated:
                    out |= {f for f in self.known if any(fnmatch.fnmatchcase(f, s) for s in shapes)}
        return out

    def resolve(self, importer: str, spec: str, packages: Dict[str, Tuple[str, Optional[str]]]) -> Optional[str]:
        if spec.startswith("."):
            target = posixpath.normpath(posixpath.join(posixpath.dirname(importer), spec))
        else:
            name = next((n for n in packages if spec == n or spec.startswith(n + "/")), None)
            if name is None:
                return None
            directory, entry = packages[name]
            rest = spec[len(name) :].lstrip("/")
            target = posixpath.normpath(posixpath.join(directory, rest or entry or "index.ts"))
        candidates = [target + suffix for suffix in RESOLVE_SUFFIXES]
        if target.endswith(".js"):
            candidates += [target[:-3] + ".ts", target[:-3] + ".tsx"]
        return next((c for c in candidates if c in self.known), target)

    def importing(self, hits: Iterable[str]) -> Set[str]:
        """`hits` and the files that import them, directly or not."""
        closure = set(hits)
        frontier = list(closure)
        while frontier:
            for importer in self.importers.get(frontier.pop(), ()):
                if importer not in closure:
                    closure.add(importer)
                    frontier.append(importer)
        return closure

    def tests_importing(self, hits: Iterable[str]) -> Set[str]:
        """The test files among `hits` and among the files that import them, directly or not."""
        return {f for f in self.importing(hits) if TEST_FILE.search(f)}

    def changed(self, paths: Iterable[str]) -> Tuple[Dict[str, Set[str]], Set[str]]:
        """For changed files of the packages: the test files that reach each (itself, if a test, its
        importers, what reads it by a quoted path), and every file that reaches any of them, for the
        setup files that would."""
        paths = sorted(set(paths))
        found = self.readers(paths)
        reaching: Set[str] = set()
        for path in paths:
            mine = self.importing({path} | self.importers.get(path, set()))
            reaching |= mine
            tests = {f for f in mine if TEST_FILE.search(f)}
            if tests:
                found[path] = found.get(path, set()) | tests
        for files in found.values():
            reaching |= files
        return found, reaching

    def readers(self, paths: Iterable[str]) -> Dict[str, Set[str]]:
        """For each path, the test files that read it, directly or through the files they import: an
        import of it (a golden or a Rust source as `?raw`), or a quoted path naming it or its
        directory, as a read at run time does (`"../../crates/hyperion-fit/data/cie_cmf/x.csv"`,
        `join(__dirname, "../../docs/measurements/descent-spike")`). A quoted path counts when, less
        its leading `./` and `../`, it is the path's end in two components or more, or its directory
        in three or more. A path named in a comment does not count: comments cite files they do not
        read, such as the UX guide."""
        paths = sorted(set(paths))
        hits: Dict[str, Set[str]] = {path: set(self.importers.get(path, ())) for path in paths}
        for file in self.files:
            for quoted in set(QUOTED_PATH.findall(code_of(self.ws.read(file)))):
                named = re.sub(r"^(\.{0,2}/)+", "", quoted.split("?", 1)[0]).rstrip("/")
                parts = named.count("/") + 1
                if parts < 2:
                    continue
                for path in paths:
                    if path == named or path.endswith("/" + named) or (parts >= 3 and path.startswith(named + "/")):
                        hits[path].add(file)
        return {path: tests for path, files in hits.items() if (tests := self.tests_importing(files))}

    def surface_readers(self) -> Set[str]:
        """The test files that import the surface module's glue, directly or through others."""
        return self.tests_importing(
            f for f, targets in self.imports.items() if any(t.startswith(SURFACE_GLUE) for t in targets)
        )


# --------------------------------------------------------------------------------------------------
# From the needs to the steps, in `just ci`'s order.


def short(causes: Iterable[str], limit: int = 4) -> List[str]:
    causes = sorted(set(causes))
    if len(causes) > limit:
        return causes[:limit] + [f"and {len(causes) - limit} more"]
    return causes


def causes_of(table: Dict[str, Set[str]], keys: Optional[Iterable[str]] = None) -> Set[str]:
    out: Set[str] = set()
    for key in table if keys is None else keys:
        out |= table.get(key, set())
    return out


@dataclass
class Chosen:
    """The tests the code's graph found reach the changed Rust code and goldens (`rust_graph`)."""

    graph: "rust_graph.Graph"
    selection: "rust_graph.Selection"
    info: str  # what the graph is and what building it took

    def _units(self) -> Dict[str, Set[str]]:
        out: Dict[str, Set[str]] = {}
        for unit_id in set(self.selection.tests) | set(self.selection.generated):
            out.setdefault(self.graph.units[unit_id].crate, set()).add(unit_id)
        return out

    def crates(self, whole: Set[str]) -> Set[str]:
        """The crates some of whose tests the graph chose, less those every test of which runs."""
        return set(self._units()) - whole

    def doc_crates(self, whole: Set[str]) -> Set[str]:
        return set(self.selection.doctests) - whole

    def browser(self, crate: str) -> bool:
        """Whether a crate's browser suite (`--lib --tests`) holds a chosen test: then all of it runs."""
        return any(self.graph.units[u].kind in ("lib", "test") for u in self._units().get(crate, ()))

    def causes(self, crates: Set[str], doctests: bool = False) -> Set[str]:
        out: Set[str] = set()
        for unit_id, tests in list(self.selection.tests.items()) + list(self.selection.generated.items()):
            if self.graph.units[unit_id].crate in crates:
                for causes in tests.values():
                    out |= causes
        if doctests:
            for crate, files in self.selection.doctests.items():
                if crate in crates:
                    for causes in files.values():
                        out |= causes
        return out

    def counts(self, crate: str) -> Tuple[int, int]:
        """How many of the crate's test functions (not ignored) the graph chose, and how many it has."""
        chosen = total = 0
        for unit in self.graph.runnable(crate):
            picked = self.selection.tests.get(unit.id, {})
            for test in self.graph.tests.get(unit.id, ()):
                if not test.ignored:
                    total += 1
                    chosen += test.name in picked
        return chosen, total

    def count(self, crate: str) -> str:
        chosen, total = self.counts(crate)
        return f"{crate} ({chosen} of {total} tests)"

    def nextest_filter(self, crates: Iterable[str], whole: Set[str]) -> str:
        """nextest's `-E` for the chosen tests of `crates`, every test of those in `whole`; empty for
        every test of them all. Coarser where the expression would be too long for a command line:
        modules by their paths, then whole binaries, then whole crates."""
        crates = sorted(set(crates))
        units = self._units()
        if all(c in whole or c not in units for c in crates):
            return ""
        expression = ""
        for level in range(4):
            terms: List[str] = []
            for crate in crates:
                if crate in whole or (level == 3 and crate in units):
                    terms.append(f"package({crate})")
                    continue
                for unit in sorted(self.graph.runnable(crate), key=lambda u: u.id):
                    tests = self.selection.tests.get(unit.id, {})
                    generated = self.selection.generated.get(unit.id, {})
                    if not tests and not generated:
                        continue
                    found = None if level >= 2 else rust_graph.unit_filter(
                        self.graph.tests.get(unit.id, []), tests, generated, coarse=level)
                    terms.append(f"binary_id(={unit.id})" if found is None else f"(binary_id(={unit.id}) & ({found}))")
            expression = " | ".join(terms) or "none()"
            if len(expression) <= MAX_FILTER:
                break
        return expression

    def doctest_filters(self, crates: Sequence[str], whole: Set[str]) -> str:
        """rustdoc's filters for the chosen doctests: a crate's directory for every doctest of it, the
        files' paths for some; empty for all of them."""
        if all(c in whole for c in crates):
            return ""
        words: List[str] = []
        for crate in crates:
            if crate in whole:
                words.append(self.graph.crates[crate].dir + "/")
            else:
                words += sorted(self.selection.doctests.get(crate, {}))
        return "" if any(not SAFE_PATH.match(w) for w in words) else " ".join(words)

    def describe(self, crates: Set[str], whole: Set[str], explain: bool) -> List[str]:
        """The tests chosen, crate by crate and module by module, each with the changes it reaches."""
        lines = [f"The tests that reach the changed Rust code and goldens ({self.info}):"]
        for crate in sorted(crates):
            chosen, total = self.counts(crate)
            lines.append(f"  {crate}: {chosen} of {total} tests")
            for unit in sorted(self.graph.runnable(crate), key=lambda u: u.id):
                tests = self.selection.tests.get(unit.id, {})
                generated = self.selection.generated.get(unit.id, {})
                if not tests and not generated:
                    continue
                modules: Dict[str, List[str]] = {}
                for test in self.graph.tests.get(unit.id, ()):
                    if test.name in tests:
                        modules.setdefault(test.module, []).append(test.name)
                shown = sorted(modules) if explain else sorted(modules, key=lambda m: -len(modules[m]))[:6]
                summary = ", ".join(f"{m or '(root)'} {len(modules[m])}" for m in shown)
                more = len(modules) - len(shown)
                lines.append(f"    {unit.id}: {len(tests)} test(s){': ' + summary if summary else ''}"
                             + (f", and {more} more module(s)" if more > 0 else ""))
                causes = set().union(*tests.values(), *generated.values()) if tests or generated else set()
                lines.append(f"      because of {', '.join(short(causes))}")
                for term in sorted(generated):
                    lines.append(f"      and the tests generated there: {term}")
                if explain:
                    for name in sorted(tests):
                        chain = self.selection.via.get(f"{unit.id} {name}", [])
                        lines.append(f"      {name}: " + " -> ".join(rust_graph.describe_node(n) for n in chain))
        for crate, files in sorted(self.selection.doctests.items()):
            if crate not in whole:
                lines.append(f"  {crate}'s doctests in {len(files)} file(s): {', '.join(short(files, 3))}")
        if not crates and not self.selection.doctests:
            lines.append("  none")
        return lines


def choose_tests(changes: Sequence[Tuple[str, bool]], ws: Workspace) -> Chosen:
    """Build the code's graph (from the cache where the files are unchanged) and choose the tests that
    reach the changed paths."""
    started = time.monotonic()
    facts = rust_graph.Facts(ws.cache, ws.read_bytes or (lambda p: ws.read(p).encode("utf-8")), ws.ids)
    base: Dict[str, Optional[rust_graph.FileFacts]] = {}
    for path, _ in changes:
        if path.endswith(".rs"):
            data = ws.read_base(path) if ws.read_base else None
            base[path] = facts.get(path, data) if data is not None else None
    files = ws.rust_files if ws.rust_files is not None else ws.files
    graph = rust_graph.Graph(ws.rust, files, facts, base, GOLDEN_READERS)
    selection = graph.select(changes)
    facts.save()
    edges = sum(len(targets) for targets in graph.edges.values())
    info = (f"the Rust graph of {len(graph.rs)} files, {len(graph.edges):,} nodes and {edges:,} edges, built in "
            f"{time.monotonic() - started:.1f} s, {facts.parsed} file(s) read and {facts.cached} from its cache")
    return Chosen(graph, selection, info)


def less_wasm_sky(selection: str, wasm_sky: str) -> str:
    """The wasip1 run's `-E`: the chosen tests (every test, for an empty `selection`) less the
    justfile's `wasm_sky`, which `ci`'s wasip1 run leaves to `just test-wasm-sky`. It may pass
    MAX_FILTER by that filter's length, far inside a command line's limit."""
    if not wasm_sky:
        return selection
    rest = f"not ({wasm_sky})"
    return f"({selection}) & {rest}" if selection else rest


def make_plan(changes: Sequence[Tuple[str, bool]], ws: Workspace, dependents: bool,
              all_tests: bool = False, explain: bool = False) -> Plan:
    needs = Needs()
    for path, deleted in changes:
        classify(path, deleted, ws, needs)
    if needs.full:
        return Plan(needs.full, [], needs.render, needs.why, needs.notes)

    # The tests of the changed code: those the code's graph finds reach it, in any crate, or with
    # --all-tests (or no graph) every test of the changed crates, as before the graph.
    chosen: Optional[Chosen] = None
    if needs.graph and (all_tests or not ws.rust):
        for crate, causes in needs.graph.items():
            for cause in causes:
                for reader in ([crate] if crate else sorted(fixture_readers(cause, ws))):
                    needs.add(needs.tests, reader, cause)
    elif needs.graph:
        graph_paths = causes_of(needs.graph)
        chosen = choose_tests([(p, d) for p, d in changes if p in graph_paths], ws)
        for crate, causes in chosen.selection.whole.items():
            for cause in causes:
                needs.add(needs.tests, crate, cause)

    # Reverse dependents: always for a changed manifest, their Clippy and every test; for any code
    # change with --dependents, their Clippy, and every test only with --all-tests (the graph chooses
    # their tests otherwise).
    roots = {crate: set(causes) for crate, causes in needs.manifests.items()}
    if dependents:
        for crate, causes in needs.code.items():
            roots.setdefault(crate, set()).update(causes)
    for root in sorted(roots):
        for dependent in sorted(ws.reverse_dependents([root])):
            cause = f"{dependent} depends on {root}"
            needs.add(needs.clippy, dependent, cause)
            if root in needs.manifests or chosen is None:
                needs.add(needs.tests, dependent, cause)
    graph_crates = chosen.crates(set(needs.tests)) if chosen else set()
    # The dependents whose Clippy does not run (with the graph, their tests are its to choose).
    unchecked = ws.reverse_dependents(needs.code) - set(needs.clippy) - (set() if chosen else set(needs.tests) | set(needs.native))

    # The TypeScript side's reads of the changed files, and of the surface module; with --all-tests,
    # every vitest file for a change to a package's sources, as before the graph.
    if needs.vitest_graph and all_tests:
        needs.vitest_all |= needs.vitest_graph
    outside = [p for p, _ in changes if ws.package_of(p) is None and not p.startswith(".claude/")]
    if not needs.vitest_all and (outside or needs.surface_module or needs.vitest_graph):
        graph = TsGraph(ws)
        found = graph.readers(outside)
        for path, readers in found.items():
            needs.say(path, "read by the client's tests: " + ", ".join(short(map(posixpath.basename, readers), 3)))
        if needs.surface_module:
            found["the surface module"] = graph.surface_readers()
        if needs.vitest_graph:
            sources, reaching = graph.changed(needs.vitest_graph)
            setups = setup_files(ws)
            for directory, files in setups.items():
                if files & reaching:
                    # Setup that every test of the package runs reaches the change: all of them.
                    needs.vitest_package.setdefault(directory, set()).update(
                        p for p in needs.vitest_graph if p in reaching)
            found.update(sources)
        for cause, readers in sorted(found.items()):
            for test in readers:
                package = ws.package_of(test)
                assert package is not None
                needs.vitest_files.setdefault(package, {}).setdefault(test, set()).add(cause)

    steps: List[Step] = []

    def step(phase: str, title: str, recipe: List[str], causes: Iterable[str]) -> None:
        steps.append(Step(phase, title, recipe, short(causes)))

    vitest_any = bool(needs.vitest_all or needs.vitest_package or needs.vitest_files)
    clippy = sorted(needs.clippy)
    whole = set(needs.tests)  # every test of these crates
    native = sorted(whole | set(needs.native) | graph_crates)
    doc_crates = sorted(whole | (chosen.doc_crates(whole) if chosen else set()))
    wasip1_tests = [c for c in ws.wasip1 if c in whole or c in graph_crates]
    browser_tests = [c for c in ws.browser if c in whole or (chosen is not None and chosen.browser(c))]
    browser_clippy = [c for c in ws.browser if c in needs.clippy]
    relaxed = sorted(set(clippy) & ws.surface_module_crates())
    # The fitted tables' check, which is no test suite, runs as before the graph: when the sim's or
    # the fitting crate's code or data changed.
    fit_check = sorted((whole | set(needs.native) | set(needs.graph)) & set(FIT_CHECK_CRATES))
    native_causes = causes_of(needs.tests) | causes_of(needs.native) | causes_of(needs.graph, graph_crates & set(needs.graph))
    if chosen is not None:
        native_causes |= chosen.causes(graph_crates)
    native_filter = chosen.nextest_filter(native, whole | set(needs.native)) if chosen else ""
    wasip1_filter = less_wasm_sky(chosen.nextest_filter(wasip1_tests, whole) if chosen else "", ws.wasm_sky)
    doc_filters = chosen.doctest_filters(doc_crates, whole) if chosen else ""

    # Clippy for the other platforms: of the crates whose Clippy runs, those with code that differs
    # between Linux, macOS and Windows, listed or found by the guard, which runs smart-ci's own tests
    # for a crate it finds that is not listed.
    os_reasons = {crate: os_dependence(ws, crate) for crate in clippy}
    cross = [c for c in clippy if c in CROSS_CLIPPY_CRATES or os_reasons[c]]
    for crate in sorted(c for c in cross if c not in CROSS_CLIPPY_CRATES):
        needs.smart_ci_tests.add(f"{crate}'s OS-dependent code, by the guard")
        needs.notes.append(
            f"{crate} has code that differs between Linux, macOS and Windows ({'; '.join(short(os_reasons[crate], 3))}), "
            "so its Clippy for the other platforms runs, and smart-ci's own tests, which fail until it is in "
            "CROSS_CLIPPY_CRATES (tools/smart-ci/smart_ci.py): add it there in this commit.")
    for crate in sorted(c for c in cross if c in CROSS_CLIPPY_CRATES and not os_reasons[c]):
        needs.smart_ci_tests.add(f"{crate}: no OS-dependent code left, by the guard")
        needs.notes.append(
            f"{crate} is in CROSS_CLIPPY_CRATES but the guard finds no code in it that differs between Linux, macOS "
            "and Windows, so smart-ci's own tests run, which fail until it leaves the list: drop it in this commit.")
    same = [c for c in clippy if c not in cross]
    if same:
        needs.notes.append(
            "Clippy for the other platforms skips " + ", ".join(same) + ": the same code on Linux, macOS and "
            "Windows, by the guard (decision-test-coverage.md L4); integration's `just ci` lints them there.")

    # Setup: `ci`'s own dependencies.
    if needs.surface_module or needs.typecheck or needs.oxlint or vitest_any or needs.render:
        readers = ({"the TypeScript checks below, which read it"} if needs.typecheck or needs.oxlint or vitest_any
                   else set()) | ({"the smoke harness's build below, which bundles it"} if needs.render else set())
        step("setup", "the surface module", ["gen-surface"], needs.surface_module or readers)
    groups = (["wasip1"] if wasip1_tests else []) + (["browser"] if browser_tests or browser_clippy else [])
    if groups:
        step("setup", "the WebAssembly tools", ["_wasm-preflight", *groups], ["the WebAssembly steps below"])

    # Beside the builds: formatting, TypeScript, the relaxed-SIMD refusal, gpu-replay, cross-Clippy.
    if needs.fmt:
        step("side", "rustfmt: " + ", ".join(sorted(needs.fmt)), ["_rustfmt-check", ws.plain_packages(needs.fmt)],
             causes_of(needs.fmt))
    files = sorted(needs.prettier)
    needs.prettier_all |= {f for f in files if not SAFE_PATH.match(f)}
    if needs.prettier_all:
        step("side", "Prettier, every file", ["_prettier-check-all"], needs.prettier_all)
    elif files:
        step("side", "Prettier: the changed files", ["_prettier-check", " ".join(files)], files)
    if needs.typecheck:
        step("side", "tsc", ["_typecheck-ts"], needs.typecheck)
    if needs.oxlint:
        step("side", "oxlint", ["_oxlint"], needs.oxlint)
    if relaxed:
        step("side", "the relaxed-SIMD refusal", ["_relaxed-simd-refused"], causes_of(needs.clippy, relaxed))
    if needs.gpu_replay:
        step("side", "tools/gpu-replay: Clippy and tests", ["gpu-replay-check"], needs.gpu_replay)
    if cross or needs.gpu_replay:
        names = cross + (["tools/gpu-replay"] if needs.gpu_replay else [])
        step("side", "Clippy for the other platforms: " + ", ".join(names),
             ["_cross-clippy-for", ws.cargo_packages(cross) if cross else "", "gpu-replay" if needs.gpu_replay else ""],
             causes_of(needs.clippy, cross) | needs.gpu_replay)
    if needs.smart_ci_tests:
        step("side", "smart-ci's own tests", ["_smart-ci-test"], needs.smart_ci_tests)
    if needs.render:
        for recipe in RENDER_PHASES["side"]:
            step("side", "build the client for the smoke harness (just test-render)", recipe, needs.render)

    # vitest, beside the builds and outside the heavy-test lock (decision-test-coverage.md R1).
    if needs.vitest_all:
        step("unlocked", "vitest: every package", ["_vitest-run", "", ""], needs.vitest_all)
    else:
        for directory, (package, _) in sorted(ws.ts_packages.items()):
            tests_here = needs.vitest_files.get(directory, {})
            unsafe = {f for f in tests_here if not SAFE_PATH.match(f)}
            if unsafe:
                needs.vitest_package.setdefault(directory, set()).update(unsafe)
            if sum(len(f) + 1 for f in tests_here) > MAX_FILTER:
                # Too many files for one command line: the package's whole suite.
                needs.vitest_package.setdefault(directory, set()).update(causes_of(tests_here))
            if directory in needs.vitest_package:
                step("unlocked", f"vitest: {package}", ["_vitest-run", package, ""], needs.vitest_package[directory])
            elif tests_here:
                rel = sorted(f[len(directory) + 1 :] for f in tests_here)
                step("unlocked", f"vitest: {package}, {len(rel)} file(s) that read the change",
                     ["_vitest-run", package, " ".join(rel)], causes_of(tests_here))

    # The cargo steps, one after another.
    if clippy:
        step("main", "Clippy: " + ", ".join(clippy), ["_clippy-for", ws.cargo_packages(clippy)], causes_of(needs.clippy))
    if browser_clippy:
        step("main", "Clippy for wasm32-unknown-unknown: " + ", ".join(browser_clippy),
             ["_browser-clippy-for", " ".join(browser_clippy)], causes_of(needs.clippy, browser_clippy))
    if needs.gen_protocol:
        # Over the test build's selection, as `ci` runs it over the workspace's: the same build.
        step("main", "the protocol bindings are fresh",
             ["_gen-protocol-check-for", ws.cargo_packages(set(native) | {PROTOCOL_CRATE})], needs.gen_protocol)
    if fit_check:
        step("main", "the fitted tables are fresh", ["fit-check"],
             causes_of(needs.tests, fit_check) | causes_of(needs.native, fit_check))
    def scope(crates: Sequence[str]) -> str:
        """The crates of a suite, each with how many of its tests run."""
        return ", ".join(chosen.count(c) if chosen and c in graph_crates and c not in whole else c for c in crates)

    wasm_causes = causes_of(needs.tests, wasip1_tests) | (chosen.causes(set(wasip1_tests)) if chosen else set())
    browser_causes = causes_of(needs.tests, browser_tests) | (chosen.causes(set(browser_tests)) if chosen else set())
    if native:
        step("main", "build the tests: " + ", ".join(native), ["_test-build-for", ws.cargo_packages(native)],
             native_causes)
    wasip1_args = ws.cargo_packages(wasip1_tests, universe=ws.wasip1)
    if wasip1_tests:
        step("main", "build the wasip1 suites and compile their modules: " + scope(wasip1_tests),
             ["_wasip1-build", wasip1_args, wasip1_filter], wasm_causes)
    if browser_tests:
        # Its native lists from the test build's own selection, so that they build nothing again.
        step("main", "build the browser suites: " + ", ".join(browser_tests),
             ["_browser", "prepare", " ".join(browser_tests), ws.cargo_packages(native)], browser_causes)

    # Every suite, in one hold of the heavy-test lock.
    if native:
        step("locked", "nextest: " + scope(native), ["_nextest-run", ws.cargo_packages(native), native_filter],
             native_causes)
    if doc_crates:
        step("locked", "doctests: " + ", ".join(doc_crates), ["_doctest-run", ws.cargo_packages(doc_crates), doc_filters],
             causes_of(needs.tests, doc_crates) | (chosen.causes(set(doc_crates), doctests=True) if chosen else set()))
    if wasip1_tests or browser_tests:
        # Side by side, the browser's one thread beside wasip1's pool (perf-wasm W2).
        # Less `wasm_sky`; the run of a change that reaches only its tests has none, and passes.
        less = ", less `wasm_sky`, which `just test-wasm-sky` runs" if ws.wasm_sky else ""
        suites = (["wasip1 " + scope(wasip1_tests) + less] if wasip1_tests else []) + (
            ["the browser target " + ", ".join(browser_tests)] if browser_tests else [])
        step("locked", "the WebAssembly suites: " + "; ".join(suites),
             ["_wasm-run", wasip1_args if wasip1_tests else "", wasip1_filter if wasip1_tests else "",
              " ".join(browser_tests)], wasm_causes | browser_causes)
    if needs.render:
        for recipe in RENDER_PHASES["locked"]:
            step("locked", "the smoke harness on SwiftShader, both variants (just test-render)", recipe, needs.render)

    notes = list(needs.notes)
    if chosen is not None:
        notes += chosen.selection.notes
        if not graph_crates and not whole and not needs.native:
            notes.append("No test reaches the changed Rust code: no suite runs for it (its Clippy and rustfmt do).")
    if unchecked:
        notes.append(
            "Not checked: " + ", ".join(sorted(unchecked)) + "'s Clippy, which depend on the changed crates. "
            + ("`--dependents` adds it; the graph chooses their tests. " if chosen is not None else
               "`--dependents` adds their Clippy and tests. ")
            + "Integration's `just ci` runs them anyway."
        )
    plan = Plan({}, steps, needs.render, needs.why, notes)
    if chosen is not None:
        plan.details = chosen.describe(graph_crates, whole, explain)
    return plan


# --------------------------------------------------------------------------------------------------
# Reading the repository, printing and running.


class ToolError(Exception):
    pass


def run_out(args: Sequence[str], cwd: Path) -> str:
    try:
        proc = subprocess.run(list(args), cwd=str(cwd), capture_output=True, text=True, check=True)
    except OSError as err:
        raise ToolError(f"{args[0]}: {err}") from err
    except subprocess.CalledProcessError as err:
        raise ToolError(f"`{' '.join(args)}` failed: {err.stderr.strip()}") from err
    return proc.stdout


def git(root: Path, *args: str) -> str:
    return run_out(["git", "-C", str(root), *args], root)


def git_bytes(root: Path, *args: str) -> Optional[bytes]:
    """A git command's output as bytes, or None if it fails (a file absent at a commit)."""
    try:
        return subprocess.run(["git", "-C", str(root), *args], capture_output=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError):
        return None


@dataclass
class Change:
    """What a run compares: the changed paths, each with whether it was deleted, and the commits."""

    paths: List[Tuple[str, bool]]
    what: str
    is_range: bool
    base: str  # the commit the change starts from
    end: Optional[str] = None  # the commit it ends at, for a range; the working tree otherwise


def changed_paths(root: Path, base: Optional[str]) -> Change:
    """The changed paths, each with whether it was deleted; what was compared; the commits."""
    if base and "..." in base:
        start, _, end = base.partition("...")
        start, end = start or "HEAD", end or "HEAD"
        merge = git(root, "merge-base", start, end).strip()
        status = git(root, "diff", "--name-status", "-z", "--no-renames", merge, end)
        what = f"the commits of {end} since {merge[:10]}, its merge-base with {start}"
        return Change(parse_name_status(status), what, True, merge, git(root, "rev-parse", end).strip())
    ref = base or INTEGRATION_BRANCH
    try:
        merge = git(root, "merge-base", "HEAD", ref).strip()
    except ToolError as err:
        raise ToolError(f"no merge-base of HEAD and {ref}: name a BASE, `just smart-ci <ref>` ({err})") from err
    changes = parse_name_status(git(root, "diff", "--name-status", "-z", "--no-renames", merge))
    seen = {path for path, _ in changes}
    for path in git(root, "ls-files", "-z", "--others", "--exclude-standard").split("\0"):
        if path and path not in seen:
            changes.append((path, False))
    what = (f"{merge[:10]}, the merge-base of HEAD and {ref}, against the working tree, uncommitted and "
            "untracked files included")
    return Change(sorted(changes), what, False, merge)


def parse_name_status(text: str) -> List[Tuple[str, bool]]:
    words = [w for w in text.split("\0") if w]
    return sorted((words[i + 1], words[i].startswith("D")) for i in range(0, len(words) - 1, 2))


def just_string(value: object) -> str:
    """A justfile assignment's value as `just --dump` gives it: a string, or strings joined by `+`."""
    if isinstance(value, str):
        return value
    if isinstance(value, list) and value[:1] == ["concatenate"]:
        return "".join(just_string(part) for part in value[1:])
    raise ToolError(f"the justfile assigns no plain string here: {value!r}")


def load_workspace(root: Path, just: str, change: Optional[Change] = None) -> Workspace:
    meta = json.loads(run_out(["cargo", "metadata", "--format-version", "1", "--no-deps"], root))
    workspace_root = Path(meta["workspace_root"]).resolve()
    members = set(meta["workspace_members"])
    packages = [p for p in meta["packages"] if p["id"] in members]
    by_dir: Dict[Path, str] = {}
    crates: Dict[str, Crate] = {}
    for package in packages:
        directory = Path(package["manifest_path"]).resolve().parent
        crates[package["name"]] = Crate(
            package["name"], directory.relative_to(workspace_root).as_posix(), features=package["features"]
        )
        by_dir[directory] = package["name"]
    for package in packages:
        for dep in package["dependencies"]:
            member = by_dir.get(Path(dep["path"]).resolve()) if dep.get("path") else None
            if member:
                crates[package["name"]].deps.append((
                    member, dep["kind"], dep["features"], dep["uses_default_features"], dep["optional"],
                    dep.get("rename") or dep["name"],
                ))
    assignments = json.loads(run_out([just, "--dump", "--dump-format", "json"], root))["assignments"]
    wasip1 = assignments["wasip1_crates"]["value"].split()[1::2]
    browser = assignments["browser_crates"]["value"].split()
    listed = git(root, "ls-files", "-z", "--cached", "--others", "--exclude-standard").split("\0")
    files = sorted({f for f in listed if f and (root / f).is_file()})
    cache: Dict[str, str] = {}

    def read(path: str) -> str:
        if path not in cache:
            try:
                cache[path] = (root / path).read_text(encoding="utf-8", errors="ignore")
            except OSError:
                cache[path] = ""
        return cache[path]

    ts_packages: Dict[str, Tuple[str, Optional[str]]] = {}
    for pattern in pnpm_workspace_patterns(read("pnpm-workspace.yaml")):
        for manifest in (f for f in files if fnmatch.fnmatchcase(f, pattern + "/package.json")):
            data = json.loads(read(manifest))
            exports = data.get("exports")
            entry = exports.get(".") if isinstance(exports, dict) else None
            ts_packages[posixpath.dirname(manifest)] = (data["name"], entry if isinstance(entry, str) else None)
    ws = Workspace(crates, wasip1, browser, ts_packages, files, read, format_check_excludes(read("package.json")))
    ws.catalogue_sources = catalogue_sources(ws)
    ws.wasm_sky = just_string(assignments["wasm_sky"]["value"])
    # The Rust graph's inputs: cargo's view of the crates, the files as git's index has them (read only
    # where the working tree differs), and the cache, in the build directory.
    ws.rust = rust_graph.crates_from_metadata(meta)
    ws.build_scripts = rust_graph.build_scripts(meta)
    ws.proc_macros = rust_graph.proc_macros(meta)
    ws.os_manifest = manifest_os_dependence(meta)
    ws.cache = Path(os.environ.get("CARGO_TARGET_DIR") or root / "target") / "smart-ci" / "rust-facts.json"
    if change is not None and change.end is not None:
        # A range: the graph reads the tree at its end, from git, not the working tree.
        listing = git(root, "ls-tree", "-r", "-z", change.end)
        ws.ids = {}
        for entry in listing.split("\0"):
            meta_part, _, path = entry.partition("\t")
            fields = meta_part.split()
            if path and len(fields) == 3 and fields[1] == "blob":
                ws.ids[path] = fields[2]
        ws.rust_files = sorted(ws.ids)
        ws.read_bytes = lambda path: git_bytes(root, "show", f"{change.end}:{path}")
    else:
        ws.ids = rust_graph.index_ids(git(root, "ls-files", "-z", "--stage"), git(root, "diff-files", "-z", "--name-only"))

        def read_bytes(path: str) -> Optional[bytes]:
            try:
                return (root / path).read_bytes()
            except OSError:
                return None

        ws.read_bytes = read_bytes
    if change is not None:
        ws.read_base = lambda path: git_bytes(root, "show", f"{change.base}:{path}")
    return ws


def pnpm_workspace_patterns(text: str) -> List[str]:
    """The package patterns of pnpm-workspace.yaml's `packages:` list."""
    patterns: List[str] = []
    inside = False
    for line in text.splitlines():
        if re.match(r"^packages:\s*$", line):
            inside = True
        elif inside and re.match(r"^\s+-\s*\S", line):
            patterns.append(line.split("-", 1)[1].strip().strip("'\""))
        elif inside and line.strip() and not line.startswith((" ", "\t")):
            inside = False
    return patterns


def format_check_excludes(package_json: str) -> List[str]:
    """The `!pattern` words of the root package.json's `format:check` script, without the `!`."""
    script = json.loads(package_json).get("scripts", {}).get("format:check", "")
    return [word[1:] for word in shlex.split(script) if word.startswith("!")]


def catalogue_sources(ws: Workspace) -> Set[str]:
    """The files the shader catalogue imports, whose specs the harness renders (R01.T9.e)."""
    out = set()
    for spec in re.findall(r'\bfrom\s+"(\.{1,2}/[^"]+)"', ws.read(CATALOGUE)):
        target = posixpath.normpath(posixpath.join(posixpath.dirname(CATALOGUE), spec))
        found = next((c for c in (target + ".ts", target + ".tsx", target + "/index.ts") if c in ws.files), None)
        if found:
            out.add(found)
    return out


def quoted(recipe: Sequence[str]) -> str:
    return "just " + " ".join(shlex.quote(word) for word in recipe)


def describe(plan: Plan, changes: Sequence[Tuple[str, bool]], what: str, dependents: bool,
             env: Mapping[str, str] = os.environ, explain: bool = False) -> str:
    lines = [f"smart-ci: {len(changes)} changed path(s), {what}:" if changes else f"smart-ci: {what}"]
    for path, deleted in changes[:40]:
        notes = "; ".join(plan.why.get(path, []))
        lines.append(f"  {path}{' (deleted)' if deleted else ''}" + (f"  [{notes}]" if notes else ""))
    if len(changes) > 40:
        lines.append(f"  and {len(changes) - 40} more")
    lines.append("")
    if plan.is_full:
        lines.append("Plan: the full `just ci`, its own phases, because of")
        for why, paths in sorted(plan.full.items()):
            lines.append(f"  - {why}" + (f": {', '.join(short(paths))}" if paths else ""))
        for phase, recipes in phases_of(plan).items():
            lines.append(f"      [{phase}] just {' '.join(quoted(recipe)[len('just '):] for recipe in recipes)}")
    elif not plan.steps:
        lines.append("Plan: no step of `just ci` reads these paths.")
    else:
        lines.append(f"Plan, {'with' if dependents else 'without'} --dependents:")
        for i, step in enumerate(plan.steps, 1):
            lines.append(f"  {i:2}. [{step.phase}] {step.title}")
            shown = [w if explain or len(w) <= 240 else f"{w[:200]}... ({len(w):,} characters; --explain shows it whole)"
                     for w in step.recipe]
            lines.append(f"      {quoted(shown)}")
            lines.append(f"      because of {', '.join(step.causes)}")
    lines += plan.details
    if plan.render and plan.is_full:
        lines.append("With `just test-render`'s steps, its run in the same hold of the heavy-test lock, because of "
                     + ", ".join(short(plan.render)))
    for note in plan.notes:
        lines.append(f"Note: {note}")
    if plan.is_full or plan.steps:
        slot = slot_command(env)
        phases = phases_of(plan)
        groups = [", ".join(p for p in group if phases[p]) for group, recipes in slot_groups(phases)
                  if needs_slot(recipes)]
        if slot is not None and groups:
            lines.append(f"A build slot ({' '.join(slot)}) is held for the {' and then the '.join(groups)} "
                         "phases, which build, and released before the locked phase.")
        if lock_priority(plan, env):
            lines.append(f"The suites take the heavy-test lock with priority ({PRIORITY_VAR}=1): every other run "
                         "waiting for it steps aside.")
        elif env.get(PRIORITY_VAR) == "1":
            lines.append(f"{PRIORITY_VAR}=1 is ignored: it gives priority on the heavy-test lock to `--full`, "
                         "integration's run, alone.")
        if slot is None and env.get("BUILD_SLOT_HELD"):
            lines.append("A build slot is held already (BUILD_SLOT_HELD), and stays held to the end, the locked "
                         f"phase included: run `just smart-ci` with {SLOT_VAR} set instead of inside build-slot.")
    return "\n".join(lines)


PHASES = ("setup", "side", "unlocked", "main", "locked")
# The phases that build, in the groups that hold a build slot: setup on its own, then the side,
# unlocked and main phases together, since they run at once.
SLOT_GROUPS = (("setup",), ("side", "unlocked", "main"))
Phases = Dict[str, List[List[str]]]


FULL_RUN = "--full: integration's full `just ci`, whatever the diff"


def full_plan() -> Plan:
    """`just smart-ci --full`: the full `just ci`, its own phases."""
    return Plan({FULL_RUN: set()}, [], set(), {}, [])


def phases_of(plan: Plan) -> Phases:
    """The recipes of each phase, each with its arguments: the plan's steps, or `ci`'s own phases for a
    full plan, with `just test-render`'s build and run where it needs them (setup has `gen-surface`)."""
    if plan.is_full:
        phases = {phase: [list(recipe) for recipe in CI_PHASES[phase]] for phase in PHASES}
        if plan.render:
            for phase in ("side", "locked"):
                phases[phase] += [list(recipe) for recipe in RENDER_PHASES[phase]]
        return phases
    phases: Phases = {phase: [] for phase in PHASES}
    for step in plan.steps:
        phases[step.phase].append(list(step.recipe))
    return phases


def flat(recipes: Iterable[Sequence[str]]) -> List[str]:
    """One `just` command line of several recipes."""
    return [word for recipe in recipes for word in recipe]


def slot_groups(phases: Phases) -> List[Tuple[Tuple[str, ...], List[List[str]]]]:
    return [(group, [recipe for phase in group for recipe in phases[phase]]) for group in SLOT_GROUPS]


def needs_slot(recipes: Iterable[Sequence[str]]) -> bool:
    """Whether the recipes run a cargo build, which holds a build slot (`BUILDS`)."""
    return any(recipe[0] in BUILDS for recipe in recipes)


def slot_command(env: Mapping[str, str]) -> Optional[List[str]]:
    """The command that holds a build slot around the phases that build, if one is set and none is held.

    `SMART_CI_SLOT` names a command that runs its arguments while it holds one of the machine's
    build slots, such as the orchestration's `build-slot` (a shell script, so on Windows give it with
    its interpreter, `bash <path>`). Unset, the phases run directly, as on any machine without
    one. `BUILD_SLOT_HELD`, which such a command sets, means an outer one holds a slot already.
    """
    value = env.get(SLOT_VAR, "").strip()
    if not value or env.get("BUILD_SLOT_HELD"):
        return None
    return shlex.split(value, posix=os.name != "nt")


class Runner:
    """Runs `just` with the plan's arguments; the tests give a fake one."""

    def __init__(self, root: Path, just: str) -> None:
        self.root, self.just = root, just

    def run(self, args: Sequence[str], env: Optional[Mapping[str, str]] = None) -> int:
        """`just <args>`, with `env` added to the environment."""
        print(f"smart-ci: {quoted(args)}", file=sys.stderr, flush=True)
        return subprocess.run([self.just, *args], cwd=str(self.root),
                              env={**os.environ, **env} if env else None).returncode

    def run_command(self, command: Sequence[str]) -> int:
        return subprocess.run(list(command), cwd=str(self.root)).returncode

    def spawn_held(self, args: Sequence[str], log: IO[str]) -> "subprocess.Popen[str]":
        print(f"smart-ci: {quoted(args)}, its output held", file=sys.stderr, flush=True)
        return subprocess.Popen([self.just, *args], cwd=str(self.root), stdout=log, stderr=subprocess.STDOUT, text=True)


def run_build_phases(phases: Mapping[str, Sequence[Sequence[str]]], runner: Runner) -> int:
    """Setup, then the side steps and vitest, each a job of its own with its output held, beside the
    cargo steps, as `just ci` runs them; whichever of them `phases` holds."""
    setup, side_steps, unlocked, main = (flat(phases.get(phase, [])) for phase in ("setup", "side", "unlocked", "main"))
    if setup and runner.run(setup) != 0:
        return 1
    status = side_status = unlocked_status = 0
    with tempfile.TemporaryFile(mode="w+") as side_log, tempfile.TemporaryFile(mode="w+") as unlocked_log:
        side = runner.spawn_held(side_steps, side_log) if side_steps else None
        tests = runner.spawn_held(unlocked, unlocked_log) if unlocked else None
        if main:
            status = runner.run(main)
        side_status = side.wait() if side is not None else 0
        unlocked_status = tests.wait() if tests is not None else 0
        for job, log in ((side, side_log), (tests, unlocked_log)):
            if job is not None:
                log.seek(0)
                sys.stdout.write(log.read())
        sys.stdout.flush()
    if status != 0 or side_status != 0 or unlocked_status != 0:
        print(f"error: smart-ci failed before the locked suites (builds and Rust checks: exit {status}; formatting, "
              f"TypeScript, the relaxed-SIMD refusal, gpu-replay, cross-clippy and smart-ci's tests: exit "
              f"{side_status}; vitest: exit {unlocked_status})", file=sys.stderr)
        return 1
    return 0


def execute(plan: Plan, root: Path, just: str, env: Mapping[str, str] = os.environ,
            runner: Optional[Runner] = None) -> int:
    """Run the plan as `just ci` runs its steps: setup; the side steps and vitest, their output held,
    beside the cargo steps; every Rust suite, and `just test-render`'s run of the smoke harness, in one
    hold of the heavy-test lock.

    When `SMART_CI_SLOT` names the command that takes a build slot (`slot_command`), each group of
    phases that builds (`SLOT_GROUPS`, `BUILDS`) runs in a child of that command, which releases the
    slot when the group ends: no slot is held while the suites wait for the heavy-test lock or hold
    it, nor for a group that builds nothing. A full plan runs `ci`'s own phases (`CI_PHASES`) the
    same way, rather than `just ci`, whose recipe would keep a slot to its end.
    """
    runner = runner or Runner(root, just)
    phases = phases_of(plan)
    slot = slot_command(env)
    for group, recipes in slot_groups(phases):
        if not recipes:
            continue
        held = {phase: phases[phase] for phase in group}
        if slot is None or not needs_slot(recipes):
            status = run_build_phases(held, runner)
        else:
            directory = root / "target" / "smart-ci"
            directory.mkdir(parents=True, exist_ok=True)
            handoff = directory / f"build-phases-{os.getpid()}.json"
            handoff.write_text(json.dumps(held), encoding="utf-8")
            print(f"smart-ci: the {', '.join(group)} phases, inside a build slot ({' '.join(slot)})",
                  file=sys.stderr, flush=True)
            try:
                status = runner.run_command([*slot, sys.executable, "-B", str(Path(__file__).resolve()),
                                             "--build-phases", str(handoff)])
            finally:
                handoff.unlink(missing_ok=True)
        if status != 0:
            return 1
    locked = flat(phases["locked"])
    if locked and runner.run(["_locked", just, *locked], env=lock_priority(plan, env)) != 0:
        return 1
    return 0


def lock_priority(plan: Plan, env: Mapping[str, str]) -> Optional[Dict[str, str]]:
    """`HYPERION_HEAVY_PRIORITY=1` for `_locked`, for `--full` with `SMART_CI_PRIORITY=1`; else none."""
    return {"HYPERION_HEAVY_PRIORITY": "1"} if FULL_RUN in plan.full and env.get(PRIORITY_VAR) == "1" else None


HELP = """\
Runs the part of `just ci` that the change needs, chosen from its diff.

The change is every path that differs between the merge-base of HEAD and BASE
(by default {branch}) and the working tree, untracked files included. Each path runs the checks
that read it, through the recipes `just ci` runs, scoped to the crates, packages and files
concerned (`--plan` shows each step and the paths that caused it):

  Markdown                      Prettier on the changed files alone
  TypeScript                    tsc, oxlint and vitest (beside the builds, outside the heavy-test
                                lock, at 4 workers); no TypeScript change, no TypeScript check
  a crate's code                rustfmt and Clippy (native, wasm32 for the browser crates, and the
                                other platforms' where its code differs between them: the server,
                                and any crate the guard finds such code in) of that crate, with the
                                features `just ci` gives it, and the tests, in any crate, that reach
                                the changed code through the code's graph (nextest and wasip1 test
                                by test, doctests by file, the browser suites by crate)
  a crate's Cargo.toml          that crate's checks and every test, and its dependents' Clippy and tests
  a crate's goldens             the tests of its crate that read goldens, and those that name the file
  a crate's other data          that crate's tests, those of any crate that name the file, and the
                                vitest files that read it (a Markdown file is data when its crate's
                                root includes it as documentation)
  a configuration file          the checks it configures: nextest.toml every crate's tests and
                                the bindings' check (the export tests nextest skips by default),
                                rustfmt.toml and clippy.toml their tool, .oxlintrc.json oxlint,
                                Prettier's files Prettier over every file
  crates/hyperion-protocol      the bindings' check and the TypeScript side
  the surface module's sources  gen-surface and the vitest files that import the module
  the sim's sources, golden.rs  smart-ci's own tests too, whose guard holds the tests that the
                                wasip1 runs leave to `just test-wasm-sky` (`wasm_sky`) to pinning
                                no value
  shaders, engine, smoke        just test-render's build and run, the run in the suites' hold of
                                the heavy-test lock
  the whole workspace's inputs  the full `just ci`: justfile, Cargo.toml, Cargo.lock, pnpm-lock.yaml,
                                rust-toolchain.toml, .cargo/, tools/portable/, and any path no rule knows

The tests: the code's graph (`tools/smart-ci/rust_graph.py`) links each item of the workspace's Rust
code to the items it names, resolved as rustc resolves them, and each impl to its type; a test runs
when it reaches a changed item, in its own crate or any other. The plan shows how many of each
crate's tests that is, and why; `--explain` lists every test with the chain that reaches the change.
Where the reading cannot be sure it widens: to a crate's every test, as before the graph.
`--all-tests` runs every test of the changed crates instead, as smart-ci did before the graph.

Dependents: by default only the changed crates' Clippy runs (the owner's rule: a task runs the
checks of the crates it touched); their tests are the graph's to choose. Integration's full `just ci`
runs everything. `--dependents` adds the Clippy of every crate that depends on a changed one (with
`--all-tests`, their tests too): use it when the change alters what other crates use (a public item's
name or signature). A changed Cargo.toml always adds their Clippy and tests.

Run it as `just ci` is run, capped; its test phase takes the heavy-test lock as `ci`'s does. With
{slot} naming the command that holds one of the machine's build slots (`build-slot`; on Windows with
its interpreter, `bash <path>`), smart-ci holds a slot only for its setup, side and main phases,
and releases it before the heavy-test lock, so do not wrap it in that command yourself. Unset, as
on any machine without build slots, those phases run directly. A full plan runs `ci`'s own phases
in the same way, and `--full` runs that plan whatever the diff: integration's run, `{slot}=<the
build-slot command> just smart-ci --full`, which holds a slot only while it builds where
`build-slot just ci` holds one through the wait for the lock and the suites. With {priority}=1
as well, `--full`'s suites take the heavy-test lock first: every other run waiting for it steps
aside until they hold it (`_locked`'s `HYPERION_HEAVY_PRIORITY`); a lane's own runs never set it.
""".format(branch=INTEGRATION_BRANCH, slot=SLOT_VAR, priority=PRIORITY_VAR)


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(prog="just smart-ci", description=HELP,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("base", nargs="?", metavar="BASE",
                        help=f"the ref whose merge-base with HEAD the change starts from (default {INTEGRATION_BRANCH}), "
                             "or A...B: the commits of B since its merge-base with A, with --plan")
    parser.add_argument("--plan", action="store_true", help="print the plan and run nothing")
    parser.add_argument("--dependents", action="store_true",
                        help="add the Clippy of every crate that depends on a changed one (with --all-tests, "
                             "their tests too)")
    parser.add_argument("--all-tests", action="store_true",
                        help="run every test of the changed crates, not those the code's graph finds reach the "
                             "change: smart-ci's selection before the graph")
    parser.add_argument("--explain", action="store_true",
                        help="list every test the graph chose, with the chain of items that reaches the change")
    parser.add_argument("--full", action="store_true",
                        help="run the full `just ci`, whatever the diff, as integration does: its own phases, "
                             f"with the build slot that {SLOT_VAR} names held only while they build")
    # Internal: run the build phases that a parent smart-ci wrote to this file, inside the build slot
    # it holds for them (`execute`).
    parser.add_argument("--build-phases", metavar="FILE", help=argparse.SUPPRESS)
    args = parser.parse_args(argv)

    root = Path(__file__).resolve().parents[2]
    just = os.environ.get("SMART_CI_JUST", "just")
    if args.build_phases:
        phases = json.loads(Path(args.build_phases).read_text(encoding="utf-8"))
        return run_build_phases(phases, Runner(root, just))
    if args.full:
        # Integration's run: `ci`'s phases (`CI_PHASES`, which `just ci` defines and the agreement
        # tests hold to it), no test-render, as `ci` has none.
        plan = full_plan()
        print(describe(plan, [], "--full", args.dependents), flush=True)
        if args.plan:
            return 0
        print(flush=True)
        return execute(plan, root, just)
    try:
        change = changed_paths(root, args.base)
        if change.is_range and not args.plan:
            if change.end != git(root, "rev-parse", "HEAD").strip():
                raise ToolError("A...B plans another branch's commits, and its checks would run on this working "
                                "tree: give --plan")
    except ToolError as err:
        print(f"error: {err}", file=sys.stderr)
        return 2
    changes, what = change.paths, change.what
    if not changes:
        print(f"smart-ci: nothing changed ({what}): nothing to run.")
        return 0
    try:
        plan = make_plan(changes, load_workspace(root, just, change), args.dependents, args.all_tests, args.explain)
    except (ToolError, KeyError, ValueError) as err:
        plan = Plan({f"smart-ci could not read the workspace ({err})": {"every path"}}, [], set(), {}, [])
    print(describe(plan, changes, what, args.dependents, explain=args.explain), flush=True)
    if args.plan:
        return 0
    print(flush=True)
    return execute(plan, root, just)


if __name__ == "__main__":
    sys.exit(main())
