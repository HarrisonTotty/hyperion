#!/usr/bin/env python3
"""`just smart-ci`: the part of `just ci` that a change needs, chosen from its diff.

Usage: smart_ci.py [BASE | A...B] [--plan] [--dependents]

The change is every path that differs between the merge-base of HEAD and BASE (by default the
integration branch, `rendering-and-planets`) and the working tree, untracked files included, so it
works before a commit as well as after. `A...B` names the commits of B since its merge-base with A
instead, with no working tree: a plan for another branch, made with `--plan`.

Each changed path maps to the checks that read it (`classify`, and the README's "smart-ci" table),
and the plan runs those checks through the recipes `just ci` runs, scoped to the crates, packages
and files concerned, in `ci`'s order and phases: the checks that need no build directory beside the
cargo steps, then every suite in one hold of the heavy-test lock, then `just test-render` (which
takes the lock itself) when a shader, the engine or the smoke harness changed. A path that no rule
knows, and an input of the whole workspace (the justfile, the root manifest, the lock files, the
toolchain, `.cargo/`), runs the full `just ci` instead.

`--plan` prints the plan, each step with the paths that caused it, and runs nothing; a run prints
the plan first. `--dependents` adds the Clippy and the tests of every crate that depends on a
changed crate (`just smart-ci --help`).

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
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, Dict, Iterable, List, Optional, Sequence, Set, Tuple

sys.dont_write_bytecode = True

# The branch that lanes start from and the orchestrator integrates onto: the default base, unless
# SMART_CI_BASE names another.
INTEGRATION_BRANCH = os.environ.get("SMART_CI_BASE") or "rendering-and-planets"

# The crate whose `cdylib`, which `just gen-surface` builds, is the client's WebAssembly module, and
# where the module's glue lands. The module is this crate and the crates it links (hyperion-base),
# so a change to the sources of either changes it.
SURFACE_CRATE = "hyperion-surface"
SURFACE_GLUE = "apps/hyperion/src/renderer/src/generated/surface/"
# A source that names wasm_bindgen (`src/wasm.rs` today) writes the module's exports, whose `.d.ts`
# the client's type check and oxlint read.
EXPORTS = re.compile(r"\bwasm_bindgen\b")

# The crate whose `export_bindings` tests write the TypeScript bindings (`just gen-protocol`), and
# what it shares with the TypeScript side.
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
# GENERATOR_VERSION heads every golden file, so a change to it runs every crate's tests.
GENERATOR_VERSION_FILE = "crates/hyperion-base/src/version.rs"

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
    "_test-build": ("scoped", [("_test-build-for", ["--workspace"])]),
    "_wasm-fast-build": ("scoped", [("_wasm-preflight", ["wasip1", "browser"]), ("_wasip1-nextest", None),
                                    ("_browser", None)]),
    "_locked": ("as is", []),
    "_test-run": ("scoped", [("_nextest-run", ["--workspace"]), ("_doctest-run", ["--workspace"]),
                             ("_vitest-run", ["", ""])]),
    "_wasm-fast-run": ("scoped", [("_wasip1-nextest", None), ("_browser", None)]),
}
# The recipes smart-ci runs in place of one of `ci`'s for a narrower scope, where the step cannot be
# the narrower one with a wider argument without changing `ci`'s command: Prettier over the changed
# files, `prettier --check` with Prettier's configuration as `format:check` runs it over the tree.
NARROWER = {"_prettier-check-all": "_prettier-check"}


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
    render: Set[str]  # the paths that run `just test-render`
    why: Dict[str, List[str]]  # what each changed path runs
    notes: List[str]

    @property
    def is_full(self) -> bool:
        return bool(self.full)

    def recipes(self) -> List[str]:
        """The recipes the plan runs, `ci` and `test-render` included."""
        names = [step.recipe[0] for step in self.steps]
        return (["ci"] if self.is_full else names) + (["test-render"] if self.render else [])


class Needs:
    """What the changed paths ask for, each demand with the paths that caused it."""

    def __init__(self) -> None:
        self.full: Dict[str, Set[str]] = {}
        self.code: Dict[str, Set[str]] = {}  # crates whose code changed
        self.fmt: Dict[str, Set[str]] = {}
        self.clippy: Dict[str, Set[str]] = {}
        self.tests: Dict[str, Set[str]] = {}  # every suite: native, doctests, wasip1, browser
        self.native: Dict[str, Set[str]] = {}  # the native suite alone
        self.manifests: Dict[str, Set[str]] = {}  # crates whose manifest changed: their dependents run
        self.prettier: Set[str] = set()
        self.prettier_all: Set[str] = set()
        self.typecheck: Set[str] = set()
        self.oxlint: Set[str] = set()
        self.vitest_all: Set[str] = set()
        self.vitest_package: Dict[str, Set[str]] = {}  # package directory -> causes
        self.vitest_files: Dict[str, Dict[str, Set[str]]] = {}  # package directory -> test -> causes
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

    def crate_code(self, crate: str, path: str) -> None:
        for table in (self.code, self.fmt, self.clippy, self.tests):
            self.add(table, crate, path)


def prettier_excluded(path: str, excludes: Sequence[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, p) or path.startswith(p.rstrip("*")) for p in excludes)


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
            needs.crate_code(crate.name, path)
            needs.say(path, f"{crate.name}'s code")
        elif ext == ".md" and "/data/" not in "/" + rel:
            needs.say(path, "documentation: Prettier")
            return
        else:
            # Data: goldens, fixtures, fitted inputs and manifests, a data directory's README.
            needs.add(needs.tests, crate.name, path)
            needs.say(path, f"{crate.name}'s data: its tests")
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
            readers = fixture_readers(path, ws)
            for reader in readers:
                needs.add(needs.tests, reader, path)
            needs.say(path, "a fixture of both sides: the TypeScript side and the tests of "
                      + (", ".join(sorted(readers)) or "no crate"))
        elif ext == ".md":
            needs.say(path, "documentation: Prettier")
        else:
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
        needs.say(path, "nextest's configuration: every crate's tests")
        return

    needs.add(needs.full, "paths no smart-ci rule knows", path)


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
    """The TypeScript packages' import graph, for the test files that read a given file."""

    def __init__(self, ws: Workspace) -> None:
        self.ws = ws
        self.files = sorted(f for f in ws.files if ws.package_of(f) and TS_CODE.search(f))
        self.known = set(ws.files)
        packages = {name: (directory, entry) for directory, (name, entry) in ws.ts_packages.items()}
        self.imports: Dict[str, Set[str]] = {}
        self.importers: Dict[str, Set[str]] = {}
        for file in self.files:
            targets: Set[str] = set()
            for spec in IMPORT_SPEC.findall(ws.read(file)):
                target = self.resolve(file, spec.split("?", 1)[0], packages)
                if target:
                    targets.add(target)
                    self.importers.setdefault(target, set()).add(file)
            self.imports[file] = targets

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

    def tests_importing(self, hits: Iterable[str]) -> Set[str]:
        """The test files among `hits` and among the files that import them, directly or not."""
        closure = set(hits)
        frontier = list(closure)
        while frontier:
            for importer in self.importers.get(frontier.pop(), ()):
                if importer not in closure:
                    closure.add(importer)
                    frontier.append(importer)
        return {f for f in closure if TEST_FILE.search(f)}

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


def make_plan(changes: Sequence[Tuple[str, bool]], ws: Workspace, dependents: bool) -> Plan:
    needs = Needs()
    for path, deleted in changes:
        classify(path, deleted, ws, needs)
    if needs.full:
        return Plan(needs.full, [], needs.render, needs.why, needs.notes)

    # Reverse dependents: always for a changed manifest; for any code change with --dependents.
    roots = {crate: set(causes) for crate, causes in needs.manifests.items()}
    if dependents:
        for crate, causes in needs.code.items():
            roots.setdefault(crate, set()).update(causes)
    for root in sorted(roots):
        for dependent in sorted(ws.reverse_dependents([root])):
            cause = f"{dependent} depends on {root}"
            needs.add(needs.clippy, dependent, cause)
            needs.add(needs.tests, dependent, cause)
    unchecked = ws.reverse_dependents(needs.code) - set(needs.clippy) - set(needs.tests) - set(needs.native)

    # The TypeScript side's reads of the changed files, and of the surface module.
    outside = [p for p, _ in changes if ws.package_of(p) is None and not p.startswith(".claude/")]
    if not needs.vitest_all and (outside or needs.surface_module):
        graph = TsGraph(ws)
        found = graph.readers(outside)
        for path, readers in found.items():
            needs.say(path, "read by the client's tests: " + ", ".join(short(map(posixpath.basename, readers), 3)))
        if needs.surface_module:
            found["the surface module"] = graph.surface_readers()
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
    tests = sorted(needs.tests)
    native = sorted(set(needs.tests) | set(needs.native))
    wasip1_tests = [c for c in ws.wasip1 if c in needs.tests]
    browser_tests = [c for c in ws.browser if c in needs.tests]
    browser_clippy = [c for c in ws.browser if c in needs.clippy]
    relaxed = sorted(set(clippy) & ws.surface_module_crates())
    fit_check = sorted(set(native) & set(FIT_CHECK_CRATES))
    native_causes = causes_of(needs.tests) | causes_of(needs.native)

    # Setup: `ci`'s own dependencies.
    if needs.surface_module or needs.typecheck or needs.oxlint or vitest_any:
        step("setup", "the surface module", ["gen-surface"],
             needs.surface_module or {"the TypeScript checks below, which read it"})
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
    if clippy or needs.gpu_replay:
        names = clippy + (["tools/gpu-replay"] if needs.gpu_replay else [])
        step("side", "Clippy for the other platforms: " + ", ".join(names),
             ["_cross-clippy-for", ws.cargo_packages(clippy) if clippy else "", "gpu-replay" if needs.gpu_replay else ""],
             causes_of(needs.clippy) | needs.gpu_replay)
    if needs.smart_ci_tests:
        step("side", "smart-ci's own tests", ["_smart-ci-test"], needs.smart_ci_tests)

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
    if native:
        step("main", "build the tests: " + ", ".join(native), ["_test-build-for", ws.cargo_packages(native)],
             native_causes)
    wasip1_args = ws.cargo_packages(wasip1_tests, universe=ws.wasip1)
    if wasip1_tests:
        step("main", "build the wasip1 suites: " + ", ".join(wasip1_tests),
             ["_wasip1-nextest", wasip1_args, "--no-run"], causes_of(needs.tests, wasip1_tests))
    if browser_tests:
        step("main", "build the browser suites: " + ", ".join(browser_tests),
             ["_browser", "prepare", " ".join(browser_tests)], causes_of(needs.tests, browser_tests))

    # Every suite, in one hold of the heavy-test lock.
    if native:
        step("locked", "nextest: " + ", ".join(native), ["_nextest-run", ws.cargo_packages(native)], native_causes)
    if tests:
        step("locked", "doctests: " + ", ".join(tests), ["_doctest-run", ws.cargo_packages(tests)],
             causes_of(needs.tests))
    if needs.vitest_all:
        step("locked", "vitest: every package", ["_vitest-run", "", ""], needs.vitest_all)
    else:
        for directory, (package, _) in sorted(ws.ts_packages.items()):
            tests_here = needs.vitest_files.get(directory, {})
            unsafe = {f for f in tests_here if not SAFE_PATH.match(f)}
            if unsafe:
                needs.vitest_package.setdefault(directory, set()).update(unsafe)
            if directory in needs.vitest_package:
                step("locked", f"vitest: {package}", ["_vitest-run", package, ""], needs.vitest_package[directory])
            elif tests_here:
                rel = sorted(f[len(directory) + 1 :] for f in tests_here)
                step("locked", f"vitest: {package}, {len(rel)} file(s) that read the change",
                     ["_vitest-run", package, " ".join(rel)], causes_of(tests_here))
    if wasip1_tests:
        step("locked", "the wasip1 suites: " + ", ".join(wasip1_tests), ["_wasip1-nextest", wasip1_args, ""],
             causes_of(needs.tests, wasip1_tests))
    if browser_tests:
        step("locked", "the browser suites: " + ", ".join(browser_tests),
             ["_browser", "run", " ".join(browser_tests)], causes_of(needs.tests, browser_tests))

    notes = list(needs.notes)
    if unchecked:
        notes.append(
            "Not checked: " + ", ".join(sorted(unchecked)) + ", which depend on the changed crates. "
            "`--dependents` adds their Clippy and tests; integration's `just ci` runs them anyway."
        )
    return Plan({}, steps, needs.render, needs.why, notes)


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


def changed_paths(root: Path, base: Optional[str]) -> Tuple[List[Tuple[str, bool]], str, bool]:
    """The changed paths, each with whether it was deleted; what was compared; whether a range."""
    if base and "..." in base:
        start, _, end = base.partition("...")
        start, end = start or "HEAD", end or "HEAD"
        merge = git(root, "merge-base", start, end).strip()
        status = git(root, "diff", "--name-status", "-z", "--no-renames", merge, end)
        what = f"the commits of {end} since {merge[:10]}, its merge-base with {start}"
        return parse_name_status(status), what, True
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
    return sorted(changes), what, False


def parse_name_status(text: str) -> List[Tuple[str, bool]]:
    words = [w for w in text.split("\0") if w]
    return sorted((words[i + 1], words[i].startswith("D")) for i in range(0, len(words) - 1, 2))


def load_workspace(root: Path, just: str) -> Workspace:
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


def describe(plan: Plan, changes: Sequence[Tuple[str, bool]], what: str, dependents: bool) -> str:
    lines = [f"smart-ci: {len(changes)} changed path(s), {what}:"]
    for path, deleted in changes[:40]:
        notes = "; ".join(plan.why.get(path, []))
        lines.append(f"  {path}{' (deleted)' if deleted else ''}" + (f"  [{notes}]" if notes else ""))
    if len(changes) > 40:
        lines.append(f"  and {len(changes) - 40} more")
    lines.append("")
    if plan.is_full:
        lines.append("Plan: the full `just ci`, because of")
        for why, paths in sorted(plan.full.items()):
            lines.append(f"  - {why}: {', '.join(short(paths))}")
    elif not plan.steps:
        lines.append("Plan: no step of `just ci` reads these paths.")
    else:
        lines.append(f"Plan, {'with' if dependents else 'without'} --dependents:")
        for i, step in enumerate(plan.steps, 1):
            lines.append(f"  {i:2}. [{step.phase}] {step.title}")
            lines.append(f"      {quoted(step.recipe)}")
            lines.append(f"      because of {', '.join(step.causes)}")
    if plan.render:
        lines.append(f"Then `just test-render`, under the heavy-test lock, because of {', '.join(short(plan.render))}")
    for note in plan.notes:
        lines.append(f"Note: {note}")
    return "\n".join(lines)


def execute(plan: Plan, root: Path, just: str) -> int:
    """Run the plan as `just ci` runs its steps: setup; the side steps, their output held, beside the
    cargo steps; every suite in one hold of the heavy-test lock; then `just test-render`."""

    def run(args: List[str]) -> int:
        print(f"smart-ci: {quoted(args)}", file=sys.stderr, flush=True)
        return subprocess.run([just, *args], cwd=str(root)).returncode

    if plan.is_full:
        status = run(["ci"])
        return run(["test-render"]) if status == 0 and plan.render else status
    phases: Dict[str, List[str]] = {"setup": [], "side": [], "main": [], "locked": []}
    for step in plan.steps:
        phases[step.phase] += step.recipe
    if phases["setup"] and run(phases["setup"]) != 0:
        return 1
    status = side_status = 0
    with tempfile.TemporaryFile(mode="w+") as side_log:
        side = None
        if phases["side"]:
            print(f"smart-ci: {quoted(phases['side'])}, its output held", file=sys.stderr, flush=True)
            side = subprocess.Popen([just, *phases["side"]], cwd=str(root), stdout=side_log, stderr=subprocess.STDOUT)
        if phases["main"]:
            status = run(phases["main"])
        if side is not None:
            side_status = side.wait()
            side_log.seek(0)
            sys.stdout.write(side_log.read())
            sys.stdout.flush()
    if status != 0 or side_status != 0:
        print(f"error: smart-ci failed before the tests (builds and Rust checks: exit {status}; formatting, "
              f"TypeScript, the relaxed-SIMD refusal, gpu-replay, cross-clippy and smart-ci's tests: exit "
              f"{side_status})", file=sys.stderr)
        return 1
    if phases["locked"] and run(["_locked", just, *phases["locked"]]) != 0:
        return 1
    if plan.render and run(["test-render"]) != 0:
        return 1
    return 0


HELP = """\
Runs the part of `just ci` that the change needs, chosen from its diff.

The change is every path that differs between the merge-base of HEAD and BASE
(by default {branch}) and the working tree, untracked files included. Each path runs the checks
that read it, through the recipes `just ci` runs, scoped to the crates, packages and files
concerned (`--plan` shows each step and the paths that caused it):

  Markdown                      Prettier on the changed files alone
  TypeScript                    tsc, oxlint and vitest; no TypeScript change, no TypeScript check
  a crate's code                rustfmt, Clippy (native, the other platforms', wasm32 for the
                                browser crates) and the tests (nextest, doctests, wasip1, browser)
                                of that crate, with the features `just ci` gives it
  a crate's Cargo.toml          the same, and its dependents' Clippy and tests
  a crate's data (goldens)      that crate's tests, and the vitest files that read the file
  a configuration file          the checks it configures: nextest.toml every crate's tests,
                                rustfmt.toml and clippy.toml their tool, .oxlintrc.json oxlint,
                                Prettier's files Prettier over every file
  crates/hyperion-protocol      the bindings' check and the TypeScript side
  the surface module's sources  gen-surface and the vitest files that import the module
  shaders, engine, smoke        just test-render
  the whole workspace's inputs  the full `just ci`: justfile, Cargo.toml, Cargo.lock, pnpm-lock.yaml,
                                rust-toolchain.toml, .cargo/, tools/portable/, and any path no rule knows

Dependents: by default only the changed crates are checked (the owner's rule: a task runs the
checks of the crates it touched). Integration's full `just ci` catches what a change breaks in a
crate that depends on them. `--dependents` adds the Clippy and the tests of every crate that
depends on a changed one: use it when the change alters what other crates use (a public item's name,
signature or behaviour, a golden other crates compare with). A changed Cargo.toml always adds them.

Run it as `just ci` is run, capped and through build-slot; its test phase takes the heavy-test lock
as `ci`'s does.
""".format(branch=INTEGRATION_BRANCH)


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(prog="just smart-ci", description=HELP,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("base", nargs="?", metavar="BASE",
                        help=f"the ref whose merge-base with HEAD the change starts from (default {INTEGRATION_BRANCH}), "
                             "or A...B: the commits of B since its merge-base with A, with --plan")
    parser.add_argument("--plan", action="store_true", help="print the plan and run nothing")
    parser.add_argument("--dependents", action="store_true",
                        help="add the Clippy and tests of every crate that depends on a changed one")
    args = parser.parse_args(argv)

    root = Path(__file__).resolve().parents[2]
    just = os.environ.get("SMART_CI_JUST", "just")
    try:
        changes, what, is_range = changed_paths(root, args.base)
        if is_range and not args.plan:
            end = args.base.partition("...")[2] or "HEAD"
            if git(root, "rev-parse", end).strip() != git(root, "rev-parse", "HEAD").strip():
                raise ToolError("A...B plans another branch's commits, and its checks would run on this working "
                                "tree: give --plan")
    except ToolError as err:
        print(f"error: {err}", file=sys.stderr)
        return 2
    if not changes:
        print(f"smart-ci: nothing changed ({what}): nothing to run.")
        return 0
    try:
        plan = make_plan(changes, load_workspace(root, just), args.dependents)
    except (ToolError, KeyError, ValueError) as err:
        plan = Plan({f"smart-ci could not read the workspace ({err})": {"every path"}}, [], set(), {}, [])
    print(describe(plan, changes, what, args.dependents), flush=True)
    if args.plan:
        return 0
    print(flush=True)
    return execute(plan, root, just)


if __name__ == "__main__":
    sys.exit(main())
