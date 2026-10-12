#!/usr/bin/env python3
"""`just flowcharts`: the flowcharts of HYPERION's procedural generation, drawn from `data/*.toml`.

Usage: procgen_flowcharts.py [--check] [--only CHARTS] [--format FORMATS] [--detail LEVEL]
                             [--out DIR] [--data DIR] [--keep-dot] [--allow-placeholders]
                             [--dpi DPI] [--timeout SECONDS]

Each `data/*.toml` describes one part of the generation pipeline as a graph (`data/SCHEMA.md`):
its steps, the subsystem clusters they sit in, and the edges between them, some of which name steps
in other files. The files are the single source of truth, and four charts are drawn from them:

  galaxy    Procedural Generation Flowchart — Galactic Properties & Megastructures
  stars     Procedural Generation Flowchart — Star Systems
  planets   Procedural Generation Flowchart — Planets (moons, atmospheres, surfaces and life)
  complete  Procedural Generation Flowchart — Complete Pipeline

A stage chart holds the nodes of the files whose `meta.stage` is its stage; the complete chart
stacks every stage's section in one column. Every step is numbered (G12: the twelfth step of the
galaxy stage), in every chart alike. An edge that would cross the chart is drawn instead from a
reference beside the step it feeds: a stub ("↻ G12  Galaxy seed") for a step of the same stage, an
off-page connector ("from Star Systems: S7 Primary mass") for one in another, and a step's edges to
another stage end at one connector that lists their targets. A hub's edges into other clusters
become stubs, and so does any edge still long once laid out. Stage charts show each node's title,
summary and detail lines; the complete chart adds citations, sources and ids (`--detail`).

Every file is validated before anything is drawn, and each problem printed as `file: id: problem`:
unknown kinds and statuses, ids without the file's prefix, duplicate ids, edges to no node, clusters
that do not exist, text over the schema's limits (the layout's contract), and unresolved
`@part:...` placeholders. `--check` validates and draws nothing, exiting 1 on any problem.
`--allow-placeholders` draws each unresolved placeholder as a marked stub instead, to preview data
that is still being reconciled.

Output is SVG by default (`--format svg,png,pdf` for more) into `out/procgen-flowcharts/`, one file
per chart named for it, plus with `--keep-dot` its positioned DOT, which `neato -n2` draws. The same
data draws byte-identical files: nothing depends on the time, and everything is emitted in the
files' order. Each stage's section is laid out by `dot` on its own, all at once: its clusters apart
to measure them, then packed in bands in the order the data flows through them, then again for any
edge still long. The title, the legend and the sections are stacked on one canvas, and `neato -n2`
draws every format from it, PNGs at a resolution capped so that the complete chart stays viewable.

Standard library only, Python 3.11 or later (tomllib); drawing needs Graphviz (`dot` and `neato`).
`test_procgen_flowcharts.py` beside it holds the validation, the charts and the palette to their
rules, on the fixtures in `testdata/`. The visual language is in the README.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import functools
import json
import math
import os
import re
import shutil
import subprocess
import sys
import textwrap
import time
import tomllib
import unicodedata
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable, Mapping, Sequence

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
DATA_DIR = HERE / "data"
# Gitignored (`out/`): the charts are drawn from the TOML, the one source of truth, so a committed
# copy would only drift from it; and the complete chart's SVG (over a megabyte) and every PNG are
# over the 500 KB that pre-commit's check-added-large-files allows. `--out` names another directory.
OUT_DIR = ROOT / "out" / "procgen-flowcharts"

# ---------------------------------------------------------------------------------------------
# The charts.

STAGES: tuple[tuple[str, str], ...] = (
    ("galaxy", "Galactic Properties & Megastructures"),
    ("stars", "Star Systems"),
    ("planets", "Planets"),
)
STAGE_NAMES: dict[str, str] = dict(STAGES)
COMPLETE = "complete"
CHARTS: tuple[str, ...] = tuple(stage for stage, _ in STAGES) + (COMPLETE,)
CHART_NAMES: dict[str, str] = {**STAGE_NAMES, COMPLETE: "Complete Pipeline"}
TITLE_PREFIX = "Procedural Generation Flowchart — "

DETAILS = ("brief", "standard", "full")
DEFAULT_DETAIL = {**{stage: "standard" for stage in STAGE_NAMES}, COMPLETE: "full"}
FORMATS = ("svg", "png", "pdf")

# The schema's text limits (`data/SCHEMA.md`): the layout is drawn to them.
LABEL_MAX = 32
SUMMARY_MAX = 60
DETAIL_LINES_MAX = 6
DETAIL_MAX = 60
EDGE_LABEL_MAX = 28
# Citations and sources have no limit; longer entries wrap at these widths.
CITE_WRAP = 64
SOURCE_WRAP = 72

STATUSES = ("built", "partial", "planned")
STATUS_MEANING = {
    "built": "in code at the snapshot",
    "partial": "some of it in code",
    "planned": "specified, not yet in code",
}
STATUS_MARK = {"built": "", "partial": "◐ PARTIAL", "planned": "○ PLANNED"}

# ---------------------------------------------------------------------------------------------
# The palette. The chrome and ink are the data-viz reference palette's light steps; the five hues
# are its categorical slots blue, red, green, yellow and aqua: the five-slot subset that best
# separates every pair (flowchart nodes can neighbour any other), checked by the skill's
# validator with `--pairs all` (README, "Colour"). Text is never drawn in a hue: only in ink, on
# light tints of the hue that keep it at 4.5:1 or more (`test_procgen_flowcharts.py`).

SURFACE = "#fcfcfb"
INK = "#0b0b0b"
INK_SECONDARY = "#52514e"
MUTED = "#898781"
HAIRLINE = "#e1e0d9"
BASELINE = "#c3c2b7"
NEUTRAL = "#f0efec"  # the diverging midpoint gray: off-page connectors

BLUE = "#2a78d6"
RED = "#e34948"
GREEN = "#008300"
YELLOW = "#eda100"
AQUA = "#1baf7a"
# Not a node hue: constraint edges only, so that they read apart from every family's colour.
VIOLET = "#6250d6"
# The status palette's "serious": unresolved placeholders only, which are a fault in the data.
SERIOUS = "#ec835a"

TINT_BUILT = 0.18  # the share of the hue in a built node's fill
TINT_PLANNED = 0.06  # and in a planned node's: muted, nearly the surface
CLUSTER_FILLS = (0.40, 0.75, 1.0)  # hairline over the surface, by cluster depth


def mix(colour: str, base: str, amount: float) -> str:
    """`amount` of `colour` over `base`, channel by channel in sRGB (CSS `color-mix(in srgb)`)."""
    a = [int(colour[i : i + 2], 16) for i in (1, 3, 5)]
    b = [int(base[i : i + 2], 16) for i in (1, 3, 5)]
    return "#" + "".join(f"{round(x * amount + y * (1 - amount)):02x}" for x, y in zip(a, b))


@dataclass(frozen=True)
class Family:
    """A colour family: the kinds that share a hue, told apart by their shapes."""

    name: str
    hue: str
    meaning: str

    @property
    def fill(self) -> str:
        return mix(self.hue, SURFACE, TINT_BUILT)

    @property
    def planned_fill(self) -> str:
        return mix(self.hue, SURFACE, TINT_PLANNED)


FAMILIES: dict[str, Family] = {
    family.name: family
    for family in (
        Family("randomness", RED, "seeds, streams and the draws that consume them"),
        Family("inputs", GREEN, "parameters, constants and fitted tables"),
        Family("derivation", BLUE, "deterministic steps and branches"),
        Family("products", YELLOW, "the records generation makes"),
        Family("runtime", AQUA, "access and enrichment at run time"),
    )
}


@dataclass(frozen=True)
class KindStyle:
    """How a node kind is drawn: a family's colour and a shape of its own.

    `fit` names the rule that sizes the shape around its label (`fit_shape`); empty when Graphviz
    sizes it, as it does every box-like shape tightly."""

    family: str
    shape: str
    fit: str
    rounded: bool
    meaning: str


# In the legend's order: by family, then as the schema lists them. ISO 5807's symbols where it has
# one: the terminal oval for a seed, the data parallelogram, the stored-data cylinder, the
# process rectangle, the decision diamond, the document for generated text.
NODE_KINDS: dict[str, KindStyle] = {
    "seed": KindStyle("randomness", "ellipse", "ellipse", False, "root seed, derived key or stream"),
    "draw": KindStyle("randomness", "box", "", True, "samples a distribution"),
    "param": KindStyle("inputs", "polygon", "parallelogram", False, "parameter, setting or constant"),
    "table": KindStyle("inputs", "cylinder", "", False, "fitted or tabulated data"),
    "model": KindStyle("derivation", "box", "", False, "deterministic physics step"),
    "decision": KindStyle("derivation", "diamond", "diamond", False, "branch; edges name the case"),
    "entity": KindStyle("products", "box3d", "", False, "generated record or object"),
    "query": KindStyle("runtime", "cds", "", False, "runtime access, evaluated lazily"),
    "enrich": KindStyle("runtime", "note", "", False, "LLM text enrichment"),
}


@dataclass(frozen=True)
class EdgeStyle:
    """How an edge kind is drawn: each differs from the others in line, colour and arrowhead."""

    colour: str
    style: str
    arrowhead: str
    penwidth: float
    meaning: str


EDGE_KINDS: dict[str, EdgeStyle] = {
    "data": EdgeStyle(INK_SECONDARY, "solid", "normal", 1.2, "a value flows to the next step"),
    "seed": EdgeStyle(RED, "dashed", "empty", 1.4, "seed or stream derivation"),
    "lookup": EdgeStyle(GREEN, "dotted", "obox", 1.8, "a step reads a table"),
    "constraint": EdgeStyle(VIOLET, "bold", "tee", 2.2, "bounds, rejection or feedback"),
}

# DejaVu Sans draws every character the data uses (Greek, ☉ ⊕ ≤ ≥ ≈ × → ± ° ² ³ ◐ ○ ⚠); Noto Sans
# lacks the mathematical and astronomical symbols. Verdana, which shares its metrics' ancestry,
# then the generic family stand in where an SVG is viewed without it.
FONT = "DejaVu Sans,Verdana,sans-serif"
MONO = "DejaVu Sans Mono,Menlo,Consolas,monospace"
GLYPH_FONT = "DejaVu Sans"

# PNGs: this resolution, unless the chart would then be wider or taller than PNG_MAX_SIDE pixels
# or hold more than PNG_MAX_PIXELS, when the resolution drops to fit both.
PNG_DPI = 96.0
PNG_MAX_SIDE = 16000
PNG_MAX_PIXELS = 60_000_000
# PDF viewers refuse pages over 200 inches a side (Acrobat's limit): larger charts are scaled to
# fit, a little inside it for the padding.
PDF_MAX_INCHES = 196

# ---------------------------------------------------------------------------------------------
# The data.

PREFIX = re.compile(r"[a-z][a-z0-9_]*")
ID_TAIL = re.compile(r"[a-z0-9_]+(?:\.[a-z0-9_]+)*")
PLACEHOLDER = re.compile(r"@([a-z0-9][a-z0-9_-]*):(.*\S.*)")


@dataclass(frozen=True)
class Problem:
    """One problem in the data, printed as `file: where: message`. Placeholder problems are only
    warnings under `--allow-placeholders`."""

    file: str
    where: str
    message: str
    placeholder: bool = False

    def __str__(self) -> str:
        return f"{self.file}: {self.where}: {self.message}"


@dataclass(frozen=True)
class Part:
    file: str
    name: str
    stage: str
    prefix: str
    title: str
    snapshot: str


@dataclass(frozen=True)
class Cluster:
    id: str
    label: str
    parent: str
    part: str


@dataclass(frozen=True)
class Node:
    id: str
    label: str
    kind: str
    summary: str
    status: str
    part: str
    cluster: str = ""
    detail: tuple[str, ...] = ()
    cite: tuple[str, ...] = ()
    source: tuple[str, ...] = ()


@dataclass(frozen=True)
class Edge:
    src: str
    dst: str
    kind: str
    label: str
    part: str


@dataclass
class Pipeline:
    """Every file's parts, clusters, nodes and edges, ordered by stage and then by file name, each
    file's in its own order."""

    parts: dict[str, Part] = field(default_factory=dict)
    clusters: dict[str, Cluster] = field(default_factory=dict)
    nodes: dict[str, Node] = field(default_factory=dict)
    edges: list[Edge] = field(default_factory=list)

    def stage_of(self, ref: str) -> str:
        """The stage of a node id or a placeholder's part; empty if it has none."""
        if ref in self.nodes:
            return self.parts[self.nodes[ref].part].stage
        match = PLACEHOLDER.fullmatch(ref)
        if not match:
            return ""
        if match.group(1) in self.parts:
            return self.parts[match.group(1)].stage
        # A part not yet written: parts are named for their stage (`planets-worlds`).
        return next((stage for stage in STAGE_NAMES if match.group(1).split("-")[0] == stage), "")


def display_path(path: Path) -> str:
    """`path` relative to the working directory where it is under it, so that it can be clicked."""
    try:
        return str(path.resolve().relative_to(Path.cwd().resolve()))
    except ValueError:
        return str(path)


class _Reader:
    """Reads one file's tables, recording each problem against the file."""

    def __init__(self, file: str, problems: list[Problem]) -> None:
        self.file = file
        self.problems = problems

    def problem(self, where: str, message: str, placeholder: bool = False) -> None:
        self.problems.append(Problem(self.file, where, message, placeholder))

    def table(self, value: object, where: str, spec: Mapping[str, tuple[type, bool]]) -> dict:
        """The keys of `value` that `spec` names with the type it gives (`str`, or `list` for a
        list of strings), each missing required key and each unknown key a problem."""
        if not isinstance(value, dict):
            self.problem(where, f"must be a table, not {type(value).__name__}")
            return {}
        for key in value:
            if key not in spec:
                self.problem(where, f"unknown key '{key}' (expected {', '.join(spec)})")
        out: dict = {}
        for key, (kind, required) in spec.items():
            if key not in value:
                if required:
                    self.problem(where, f"missing '{key}'")
                continue
            item = value[key]
            if kind is str and not isinstance(item, str):
                self.problem(where, f"'{key}' must be a string")
            elif kind is list and not (isinstance(item, list) and all(isinstance(x, str) for x in item)):
                self.problem(where, f"'{key}' must be a list of strings")
            else:
                out[key] = item
        return out

    def line(self, where: str, key: str, text: str, limit: int = 0, required: bool = True) -> None:
        """`text` is one line, not empty if `required`, and at most `limit` characters."""
        if required and not text.strip():
            self.problem(where, f"'{key}' is empty")
        if any(ch in "\n\r\v\f\x85" or unicodedata.category(ch) in ("Zl", "Zp") for ch in text):
            self.problem(where, f"'{key}' must be one line")
        elif control := next((ch for ch in text if unicodedata.category(ch) == "Cc"), ""):
            self.problem(where, f"'{key}' holds a control character (U+{ord(control):04X})")
        if limit and len(text) > limit:
            self.problem(where, f"'{key}' is {len(text)} characters, over the limit of {limit}: \"{text}\"")

    def ident(self, where: str, ident: str, prefix: str) -> None:
        if not prefix:
            return
        if not ident.startswith(prefix + "."):
            self.problem(where, f"id does not start with the file's prefix '{prefix}.'")
        elif not ID_TAIL.fullmatch(ident[len(prefix) + 1 :]):
            self.problem(where, f"id must be '{prefix}.' and then snake_case (a-z, 0-9, _)")


META_SPEC = {key: (str, True) for key in ("stage", "part", "prefix", "title", "snapshot")}
CLUSTER_SPEC = {"id": (str, True), "label": (str, True), "parent": (str, False)}
NODE_SPEC = {
    "id": (str, True),
    "label": (str, True),
    "kind": (str, True),
    "cluster": (str, False),
    "summary": (str, True),
    "detail": (list, False),
    "cite": (list, False),
    "source": (list, False),
    "status": (str, True),
}
EDGE_SPEC = {"from": (str, True), "to": (str, True), "kind": (str, False), "label": (str, False)}


def _where(table: object, key: str, noun: str, index: int) -> str:
    value = table.get(key) if isinstance(table, dict) else None
    return value if isinstance(value, str) and value else f"{noun} #{index + 1}"


def _list(doc: dict, key: str, reader: _Reader) -> list:
    value = doc.get(key, [])
    if not isinstance(value, list):
        reader.problem(key, f"'{key}' must be an array of tables ([[{key}]])")
        return []
    return value


def _read_file(path: Path, file: str, doc: dict, problems: list[Problem]):
    """One file's part, clusters, nodes and edges, as far as they can be read."""
    reader = _Reader(file, problems)
    for key in doc:
        if key not in ("meta", "cluster", "node", "edge"):
            reader.problem(key, f"unknown table '{key}' (expected meta, cluster, node, edge)")
    meta = reader.table(doc.get("meta"), "meta", META_SPEC) if "meta" in doc else {}
    if "meta" not in doc:
        reader.problem("meta", "missing [meta]")
    stage = meta.get("stage", "")
    if "stage" in meta and stage not in STAGE_NAMES:
        reader.problem("meta", f"unknown stage '{stage}' (expected {', '.join(STAGE_NAMES)})")
    name = meta.get("part", "")
    if "part" in meta and name != path.stem:
        reader.problem("meta", f"part '{name}' is not the file's stem '{path.stem}'")
    prefix = meta.get("prefix", "")
    if "prefix" in meta and not PREFIX.fullmatch(prefix):
        reader.problem("meta", f"prefix '{prefix}' must be snake_case, starting with a letter")
        prefix = ""
    for key in ("title", "snapshot"):
        if key in meta:
            reader.line("meta", key, meta[key])
    part = Part(file, path.stem, stage if stage in STAGE_NAMES else "", prefix, meta.get("title", ""),
                meta.get("snapshot", ""))

    clusters: list[Cluster] = []
    for index, raw in enumerate(_list(doc, "cluster", reader)):
        where = _where(raw, "id", "cluster", index)
        table = reader.table(raw, where, CLUSTER_SPEC)
        if "id" not in table or "label" not in table:
            continue
        reader.ident(where, table["id"], prefix)
        reader.line(where, "label", table["label"])
        clusters.append(Cluster(table["id"], table["label"], table.get("parent", ""), part.name))
    local = {cluster.id: cluster for cluster in clusters}
    for cluster in clusters:
        if cluster.parent and cluster.parent not in local:
            reader.problem(cluster.id, f"unknown parent cluster '{cluster.parent}' (a parent must be in the same file)")
        seen = {cluster.id}
        parent = local.get(cluster.parent)
        while parent is not None:
            if parent.id in seen:
                reader.problem(cluster.id, "clusters' parents form a cycle")
                break
            seen.add(parent.id)
            parent = local.get(parent.parent)

    nodes: list[Node] = []
    for index, raw in enumerate(_list(doc, "node", reader)):
        where = _where(raw, "id", "node", index)
        table = reader.table(raw, where, NODE_SPEC)
        if not all(key in table for key in ("id", "label", "kind", "summary", "status")):
            continue
        reader.ident(where, table["id"], prefix)
        if table["kind"] not in NODE_KINDS:
            reader.problem(where, f"unknown kind '{table['kind']}' (expected {', '.join(NODE_KINDS)})")
        if table["status"] not in STATUSES:
            reader.problem(where, f"unknown status '{table['status']}' (expected {', '.join(STATUSES)})")
        reader.line(where, "label", table["label"], LABEL_MAX)
        reader.line(where, "summary", table["summary"], SUMMARY_MAX)
        detail = table.get("detail", [])
        if len(detail) > DETAIL_LINES_MAX:
            reader.problem(where, f"'detail' has {len(detail)} lines, over the limit of {DETAIL_LINES_MAX}")
        for number, text in enumerate(detail, 1):
            reader.line(where, f"detail line {number}", text, DETAIL_MAX)
        for key in ("cite", "source"):
            for number, text in enumerate(table.get(key, []), 1):
                reader.line(where, f"{key} entry {number}", text)
        cluster = table.get("cluster", "")
        if cluster and cluster not in local:
            reader.problem(where, f"unknown cluster '{cluster}' (a node's cluster must be in its own file)")
            cluster = ""
        nodes.append(Node(table["id"], table["label"], table["kind"], table["summary"], table["status"],
                          part.name, cluster, tuple(detail), tuple(table.get("cite", [])),
                          tuple(table.get("source", []))))

    edges: list[Edge] = []
    for index, raw in enumerate(_list(doc, "edge", reader)):
        ends = raw if isinstance(raw, dict) else {}
        src, dst = ends.get("from"), ends.get("to")
        named = isinstance(src, str) and isinstance(dst, str)
        where = f"edge {src} -> {dst}" if named else f"edge #{index + 1}"
        table = reader.table(raw, where, EDGE_SPEC)
        if "from" not in table or "to" not in table:
            continue
        kind = table.get("kind", "data")
        if kind not in EDGE_KINDS:
            reader.problem(where, f"unknown edge kind '{kind}' (expected {', '.join(EDGE_KINDS)})")
        label = table.get("label", "")
        reader.line(where, "label", label, EDGE_LABEL_MAX, required=False)
        edges.append(Edge(table["from"], table["to"], kind, label, part.name))
    if doc.get("node", []) == []:
        reader.problem("node", "the file has no [[node]]: a part must hold at least one step")
    return part, clusters, nodes, edges


def load(data_dir: Path) -> tuple[Pipeline, list[Problem]]:
    """Every `*.toml` in `data_dir`, read and validated: the pipeline, and every problem found."""
    problems: list[Problem] = []
    read = []
    for path in sorted(data_dir.glob("*.toml")):
        file = display_path(path)
        try:
            doc = tomllib.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
            problems.append(Problem(file, "toml", f"cannot be read: {error}"))
            continue
        read.append(_read_file(path, file, doc, problems))
    order = {stage: index for index, (stage, _) in enumerate(STAGES)}
    read.sort(key=lambda r: (order.get(r[0].stage, len(order)), r[0].name))

    pipe = Pipeline()
    # Nodes and clusters are named apart: an edge always joins nodes and a node's `cluster` always
    # names a cluster, so a record and the cluster it heads may share an id.
    owners: dict[str, dict[str, str]] = {"node": {}, "cluster": {}}  # id -> the file declaring it
    prefixes: dict[str, str] = {}
    for part, clusters, nodes, edges in read:
        reader = _Reader(part.file, problems)
        here = Path(part.file).name  # files are named to each other by name: they share a directory
        if part.prefix:
            if part.prefix in prefixes:
                reader.problem("meta", f"prefix '{part.prefix}' is also {prefixes[part.prefix]}'s")
            prefixes.setdefault(part.prefix, here)
        pipe.parts[part.name] = part
        for item in [*clusters, *nodes]:
            noun = "cluster" if isinstance(item, Cluster) else "node"
            owner = owners[noun]
            if item.id in owner:
                elsewhere = "this file" if owner[item.id] == here else owner[item.id]
                reader.problem(item.id, f"duplicate {noun} id: also declared in {elsewhere}")
                continue
            owner[item.id] = here
            if isinstance(item, Cluster):
                pipe.clusters[item.id] = item
            else:
                pipe.nodes[item.id] = item

    seen: dict[tuple[str, str, str], str] = {}
    for part, _, _, edges in read:
        reader = _Reader(part.file, problems)
        for edge in edges:
            where = f"edge {edge.src} -> {edge.dst}"
            ok = True
            for end, ref in (("from", edge.src), ("to", edge.dst)):
                if ref.startswith("@"):
                    match = PLACEHOLDER.fullmatch(ref)
                    if not match:
                        reader.problem(where, f"'{end}' is not a placeholder of the form '@<part>:<description>': '{ref}'")
                        ok = False
                    else:
                        known = "" if match.group(1) in pipe.parts else f" (and no part '{match.group(1)}' is loaded)"
                        reader.problem(where, f"unresolved placeholder in '{end}': '{ref}'{known}", placeholder=True)
                elif ref in pipe.nodes:
                    continue
                elif ref in pipe.clusters:
                    reader.problem(where, f"'{end}' names cluster '{ref}': edges join nodes")
                    ok = False
                else:
                    reader.problem(where, f"dangling '{end}': no node '{ref}' in any file")
                    ok = False
            if edge.src == edge.dst:
                reader.problem(where, "joins a node to itself")
                ok = False
            elif edge.src.startswith("@") and edge.dst.startswith("@"):
                reader.problem(where, "joins two placeholders: one end must be a node of this data")
                ok = False
            key = (edge.src, edge.dst, edge.kind)
            here = Path(part.file).name
            if key in seen:
                elsewhere = "this file" if seen[key] == here else seen[key]
                reader.problem(where, f"duplicate {edge.kind} edge: also declared in {elsewhere}")
                ok = False
            seen.setdefault(key, here)
            if ok:
                pipe.edges.append(edge)
    # File by file, each file's in the order found.
    problems.sort(key=lambda problem: problem.file)
    return pipe, problems


@functools.cache
def font_coverage(family: str = GLYPH_FONT) -> frozenset[int] | None:
    """The code points `family` draws, from fontconfig; None where fontconfig cannot say."""
    try:
        # fc-match answers with a stand-in when the family is missing: only the family itself counts.
        match = subprocess.run(["fc-match", "--format=%{family}\n%{file}", family], capture_output=True, text=True,
                               timeout=30, check=True).stdout
        names, _, found = match.partition("\n")
        if family not in names.split(","):
            return None
        charset = subprocess.run(["fc-query", "--format=%{charset}", found], capture_output=True, text=True,
                                 timeout=30, check=True).stdout
    except (OSError, subprocess.SubprocessError):
        return None
    points: set[int] = set()
    for chunk in charset.split():
        low, _, high = chunk.partition("-")
        try:
            points.update(range(int(low, 16), int(high or low, 16) + 1))
        except ValueError:
            return None
    return frozenset(points) if points else None


def glyph_gaps(pipe: Pipeline, coverage: frozenset[int] | None) -> list[Problem]:
    """A warning for each character of the text that the chart's font does not draw, which would
    fall back to another font or a box. None where fontconfig cannot say what the font covers."""
    if coverage is None:
        return []
    texts: list[tuple[str, str, str]] = []
    for part in pipe.parts.values():
        texts.append((part.file, "meta", part.title))
    for cluster in pipe.clusters.values():
        texts.append((pipe.parts[cluster.part].file, cluster.id, cluster.label))
    for node in pipe.nodes.values():
        for text in (node.label, node.summary, *node.detail, *node.cite, *node.source):
            texts.append((pipe.parts[node.part].file, node.id, text))
    for edge in pipe.edges:
        texts.append((pipe.parts[edge.part].file, f"edge {edge.src} -> {edge.dst}", edge.label))
    warned: set[tuple[str, str, str]] = set()
    out: list[Problem] = []
    for file, where, text in texts:
        for ch in text:
            if ord(ch) not in coverage and not ch.isspace() and (file, where, ch) not in warned:
                warned.add((file, where, ch))
                out.append(Problem(file, where, f"'{ch}' (U+{ord(ch):04X}) is not in {GLYPH_FONT}: it will be drawn in another font, or as a box"))
    return out


# ---------------------------------------------------------------------------------------------
# References. Every step has a number (G12: the twelfth step of the galaxy stage, in the files'
# order), shown on it in every chart. An edge that would cross the chart is drawn instead as a
# reference beside its consumer, naming the number and title of the step it comes from: a stub for
# a step of the same chart, an off-page connector for one in another.

STAGE_LETTERS = {"galaxy": "G", "stars": "S", "planets": "P"}
# A step that feeds this many edges or more is a hub: its edges into other clusters become stubs.
HUB_FANOUT = 5
# A direct edge longer than this (points, centre to centre) once laid out becomes a stub too, and
# the stage is laid out again, up to LONG_EDGE_PASSES times.
LONG_EDGE = 1500.0
LONG_EDGE_PASSES = 2
# A section's subsystems are packed in bands, left to right and top to bottom in the order the data
# flows through them, to this shape (width : height): a stage chart's own, or the complete chart's,
# whose sections share one width.
ASPECT = 1.6
# The pull of a reference's edge to its step, against the gate's pull of 1: it keeps the stub close.
REFERENCE_WEIGHT = 3


def step_numbers(pipe: Pipeline) -> dict[str, str]:
    """Each node's step number: its stage's letter and its place in the stage, in the files' order."""
    counters: dict[str, int] = {}
    numbers: dict[str, str] = {}
    for node in pipe.nodes.values():
        stage = pipe.parts[node.part].stage
        counters[stage] = counters.get(stage, 0) + 1
        numbers[node.id] = f"{STAGE_LETTERS[stage]}{counters[stage]}"
    return numbers


def hub_cuts(pipe: Pipeline, stage: str) -> set[int]:
    """The edges of `stage` (by index in `pipe.edges`) that leave a hub for another cluster."""
    fanout: dict[str, int] = {}
    for edge in pipe.edges:
        fanout[edge.src] = fanout.get(edge.src, 0) + 1
    cuts: set[int] = set()
    for index, edge in enumerate(pipe.edges):
        if edge.src in pipe.nodes and edge.dst in pipe.nodes and fanout[edge.src] >= HUB_FANOUT:
            src, dst = pipe.nodes[edge.src], pipe.nodes[edge.dst]
            if pipe.stage_of(edge.src) == stage == pipe.stage_of(edge.dst) and src.cluster != dst.cluster:
                cuts.add(index)
    return cuts


# ---------------------------------------------------------------------------------------------
# A chart: what is drawn, before it is written as DOT. A chart is laid out in pieces, stacked top to
# bottom on one canvas: its title, its legend, then a section for each of its stages.


@dataclass
class Item:
    """One node of a chart: a pipeline step, a reference (stub, connector or placeholder), or a piece
    of the legend."""

    key: str
    look: str  # a node kind, "stub", "connector", "placeholder", "plain" (legend text) or "plain-box"
    status: str
    label: str  # an HTML-like label's inside
    centred: bool = False
    tooltip: str = ""
    family: str = ""  # a stub's: the family of the step it names


@dataclass
class Box:
    """A cluster of a chart: a stage, a subsystem or the legend."""

    key: str
    label: str
    level: str  # "root", "stage", "cluster" or "legend"
    items: list[str] = field(default_factory=list)
    boxes: list[Box] = field(default_factory=list)
    rows: list[list[str]] = field(default_factory=list)  # the legend's rows, left to right


@dataclass(frozen=True)
class Link:
    src: str
    dst: str
    kind: str
    label: str
    feedback: bool = False  # closes a cycle: written backwards, so that it ranks with the flow
    invisible: bool = False  # orders the layout and is not drawn
    minlen: int = 0
    weight: int = 0  # an invisible edge's pull: 0 holds the ranks apart and pulls nothing
    edge: int = -1  # the pipeline edge it draws directly, by index; -1 for any other


@dataclass
class Section:
    """One stage of a chart, laid out on its own."""

    stage: str
    items: dict[str, Item]
    root: Box
    links: list[Link]
    nodes: int
    edges: int
    stubs: int
    connectors: int
    placeholders: int
    drawn: set[int] = field(default_factory=set)  # the pipeline edges it draws, by index
    units: dict[str, str] = field(default_factory=dict)  # item key -> its unit: a top-level cluster, or a lone step
    bands: list[list[str]] = field(default_factory=list)
    aligned: list[list[str]] = field(default_factory=list)  # items ranked together: each band's top row


@dataclass
class Chart:
    name: str
    title: str
    detail: str
    sections: list[Section]
    legend: Box
    legend_items: dict[str, Item]
    legend_links: list[Link]
    subtitle: str


def esc(text: str) -> str:
    """`text` for an HTML-like label: `&`, `<`, `>` and `"` as entities, and a backslash doubled, since
    Graphviz reads `\\N`, `\\G` and `\\E` in HTML-like labels as the node's, graph's and edge's names."""
    return (text.replace("\\", "\\\\").replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace('"', "&quot;"))


def _font(inner: str, size: float, colour: str = "", face: str = "") -> str:
    attrs = f' POINT-SIZE="{size:g}"'
    if colour:
        attrs += f' COLOR="{colour}"'
    if face:
        attrs += f' FACE="{face}"'
    return f"<FONT{attrs}>{inner}</FONT>"


def _table(rows: Sequence[str], centred: bool) -> str:
    align = "CENTER" if centred else "LEFT"
    cells = "".join(f'<TR><TD ALIGN="{align}" BALIGN="{align}">{row}</TD></TR>' for row in rows)
    return f'<TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="1">{cells}</TABLE>'


def _lines(lines: Iterable[str]) -> str:
    return "<BR/>".join(esc(line) for line in lines)


def wrap_source(text: str, width: int = SOURCE_WRAP) -> list[str]:
    """A source path broken after a `/` where it is longer than `width`."""
    lines: list[str] = []
    while len(text) > width:
        cut = text.rfind("/", 0, width)
        if cut <= 0:
            cut = width - 1
        lines.append(text[: cut + 1])
        text = "  " + text[cut + 1 :]
    return lines + [text]


STUBBED_PER_LINE = 8  # step numbers on each line of "↻ stubs at"
STUBBED_LINES = 3


def stubbed_line(numbers: Sequence[str]) -> str:
    """Where a step's stubs are: "↻ stubs at G12 · G15 · G20", wrapped, the rest summed up."""
    shown = list(numbers[: STUBBED_PER_LINE * STUBBED_LINES])
    lines = [" · ".join(shown[i : i + STUBBED_PER_LINE]) for i in range(0, len(shown), STUBBED_PER_LINE)]
    if len(numbers) > len(shown):
        lines[-1] += f" and {len(numbers) - len(shown)} more"
    lines[0] = "↻ stubs at " + lines[0]
    return _lines(lines)


def node_label(node: Node, detail: str, centred: bool, number: str = "", stubbed: Sequence[str] = ()) -> str:
    """A step's label: its number, kind and status above a bold title, the summary, where its stubs are
    (if it has any), then (standard) the detail lines, then (full) its id, citations in italic and
    sources in monospace."""
    eyebrow = [esc(node.kind.upper())]
    if STATUS_MARK[node.status]:
        eyebrow.append(esc(STATUS_MARK[node.status]))
    if detail == "full":
        eyebrow.append(esc(node.id))
    if number:
        eyebrow.insert(0, f"<B>{esc(number)}</B>")
    rows = [
        _font("  ·  ".join(eyebrow), 7, INK_SECONDARY),
        _font(f"<B>{esc(node.label)}</B>", 11),
        _font(esc(node.summary), 9),
    ]
    if stubbed:
        rows.append(_font(stubbed_line(stubbed), 7.5, INK_SECONDARY))
    if detail != "brief" and node.detail:
        rows.append(_font(_lines(node.detail), 8, INK_SECONDARY))
    if detail == "full":
        if node.cite:
            cites = [line for cite in node.cite for line in textwrap.wrap(cite, CITE_WRAP)]
            rows.append(_font(f"<I>{_lines(cites)}</I>", 7.5, INK_SECONDARY))
        if node.source:
            sources = [line for source in node.source for line in wrap_source(source)]
            rows.append(_font(_lines(sources), 7, INK_SECONDARY, MONO))
    return _table(rows, centred)


def stub_label(number: str, node: Node) -> str:
    """A reference to a step elsewhere in the chart: "↻ G12  Galaxy seed"."""
    return f'{_font(f"<B>↻ {esc(number)}</B>", 8)}  {_font(esc(node.label), 9)}'


def connector_label(direction: str, chart: str, number: str, node: Node, detail: str) -> str:
    """The classic off-page reference: "from <chart>: <node>" or "to <chart>: <node>"."""
    lead = f"{'from' if direction == 'in' else 'to'} {CHART_NAMES[chart]}:"
    rows = [_font(esc(lead), 8, INK_SECONDARY), _font(f"<B>{esc(number)}  {esc(node.label)}</B>", 9.5)]
    if detail == "full":
        rows.append(_font(esc(node.id), 7, INK_SECONDARY, MONO))
    return _table(rows, True)


OUTPUT_LINES = 5  # the targets an output connector lists before "and N more"


def outputs_label(chart: str, targets: Sequence[tuple[str, Node]]) -> str:
    """An off-page reference out of a step: "to <chart>:" and the steps there that it feeds."""
    rows = [_font(esc(f"to {CHART_NAMES[chart]}:"), 8, INK_SECONDARY)]
    shown = targets if len(targets) <= OUTPUT_LINES else targets[: OUTPUT_LINES - 1]
    rows += [_font(f"<B>{esc(number)}  {esc(node.label)}</B>", 9.5) for number, node in shown]
    if len(shown) < len(targets):
        rows.append(_font(esc(f"and {len(targets) - len(shown)} more"), 8, INK_SECONDARY))
    return _table(rows, True)


def placeholder_label(ref: str) -> str:
    match = PLACEHOLDER.fullmatch(ref)
    part, description = (match.group(1), match.group(2)) if match else ("?", ref)
    rows = [_font(esc(f"⚠ UNRESOLVED  ·  @{part}"), 7, INK_SECONDARY), _font(f"<B>{esc(description.strip())}</B>", 9)]
    return _table(rows, False)


def node_tooltip(node: Node, number: str) -> str:
    parts = [f"{number} {node.id} ({node.kind}, {node.status})"]
    if node.source:
        parts.append("source: " + "; ".join(node.source))
    return " — ".join(parts)


def plural(count: int, noun: str) -> str:
    return f"{count} {noun}{'' if count == 1 else 's'}"


def _feedback(links: list[Link]) -> list[Link]:
    """`links` with each constraint edge that closes a cycle marked as feedback: written head to
    tail, it ranks with the flow instead of dragging its target down."""
    out: list[Link] = []
    for index, link in enumerate(links):
        if link.kind == "constraint":
            adjacency: dict[str, list[str]] = {}
            for other_index, other in enumerate(links):
                if other_index != index and not other.invisible:
                    adjacency.setdefault(other.src, []).append(other.dst)
            stack, seen = [link.dst], {link.dst}
            while stack:
                current = stack.pop()
                if current == link.src:
                    link = Link(link.src, link.dst, link.kind, link.label, feedback=True, edge=link.edge)
                    break
                for nxt in adjacency.get(current, ()):
                    if nxt not in seen:
                        seen.add(nxt)
                        stack.append(nxt)
        out.append(link)
    return out


def unit_of(pipe: Pipeline, node_id: str) -> str:
    """The unit a step is packed with: its top-level cluster's box, or itself if it has none."""
    cluster = pipe.nodes[node_id].cluster
    if not cluster:
        return f"unit:{node_id}"
    while pipe.clusters[cluster].parent:
        cluster = pipe.clusters[cluster].parent
    return f"cluster:{cluster}"


def build_section(pipe: Pipeline, numbers: Mapping[str, str], stage: str, detail: str,
                  allow_placeholders: bool, cuts: set[int], boxed: bool,
                  bands: Sequence[Sequence[str]] = ()) -> Section:
    """`stage`'s section: its steps in their clusters, inside a stage box if `boxed` (the complete
    chart). An edge with another stage, or a placeholder, ends at a connector beside the step of
    this stage; an edge in `cuts` begins at a stub beside its consumer. One stub or connector for
    each remote step and each cluster that uses it. With `bands`, the units of each band rank below
    every unit of the band before."""
    parts = {part.name for part in pipe.parts.values() if part.stage == stage}
    nodes = [node for node in pipe.nodes.values() if node.part in parts]
    items: dict[str, Item] = {}
    root = Box("root", "", "root")
    home_box = root
    if boxed:
        home_box = Box(f"cluster_stage_{stage}", STAGE_NAMES[stage], "stage")
        root.boxes.append(home_box)
    boxes = {cid: Box(f"cluster:{cid}", c.label, "cluster") for cid, c in pipe.clusters.items() if c.part in parts}
    for cid, box in boxes.items():
        parent = pipe.clusters[cid].parent
        (boxes[parent].boxes if parent in boxes else home_box.boxes).append(box)
    # Where each step's stubs are, so that a reader can follow a hub forward as well as a stub back.
    stubbed: dict[str, list[str]] = {}
    for index in sorted(cuts):
        edge = pipe.edges[index]
        if edge.src in pipe.nodes and edge.dst in pipe.nodes and pipe.stage_of(edge.src) == stage == pipe.stage_of(edge.dst):
            stubbed.setdefault(edge.src, [])
            if numbers[edge.dst] not in stubbed[edge.src]:
                stubbed[edge.src].append(numbers[edge.dst])
    for node in nodes:
        style = NODE_KINDS[node.kind]
        key = f"n:{node.id}"
        where = sorted(stubbed.get(node.id, ()), key=lambda number: int(number[1:]))
        label = node_label(node, detail, bool(style.fit), numbers[node.id], where)
        items[key] = Item(key, node.kind, node.status, label, bool(style.fit), node_tooltip(node, numbers[node.id]))
        (boxes.get(node.cluster) or home_box).items.append(key)

    units = {f"n:{node.id}": unit_of(pipe, node.id) for node in nodes}

    def beside(node_id: str) -> tuple[Box, str]:
        """The box a reference to or from `node_id` goes in, and a token naming it."""
        cluster = pipe.nodes[node_id].cluster
        # Prefixed, since a cluster and a step may share an id (`data/SCHEMA.md`).
        return (boxes[cluster], f"c:{cluster}") if cluster in boxes else (home_box, f"n:{node_id}")

    counts = {"stub": 0, "connector": 0, "placeholder": 0}
    placeholders: dict[str, int] = {}

    def reference(ref: str, near: str, direction: str) -> str:
        box, token = beside(near)
        if ref in pipe.nodes:
            node = pipe.nodes[ref]
            other = pipe.stage_of(ref)
            if other == stage:
                key = f"ref:{ref}@{token}"
                item = Item(key, "stub", "", stub_label(numbers[ref], node), tooltip=f"{numbers[ref]} {ref}",
                            family=NODE_KINDS[node.kind].family)
            else:
                key = f"{direction}:{ref}@{token}"
                item = Item(key, "connector", "", connector_label(direction, other, numbers[ref], node, detail), True,
                            f"{numbers[ref]} {ref}, in {CHART_NAMES[other]}")
        else:
            serial = placeholders.setdefault(ref, len(placeholders) + 1)
            key = f"ph:{serial}@{token}"
            item = Item(key, "placeholder", "", placeholder_label(ref), tooltip=ref)
        if key not in items:
            items[key] = item
            box.items.append(key)
            counts[item.look] += 1
            units[key] = unit_of(pipe, near)
        return key

    # A step's edges to another stage end at one connector for that stage, which lists their targets.
    outputs: dict[tuple[str, str], list[Edge]] = {}
    for edge in pipe.edges:
        if (edge.src in pipe.nodes and pipe.stage_of(edge.src) == stage and edge.dst in pipe.nodes
                and pipe.stage_of(edge.dst) != stage):
            outputs.setdefault((edge.src, pipe.stage_of(edge.dst)), []).append(edge)

    def output(src: str, other: str, kind: str) -> Link:
        """The edge of `kind` from `src` to its connector for `other`: one for each kind, so that a
        constraint stays a constraint; labelled when its edges share one label."""
        key = f"out:{src}@{other}"
        group = outputs[(src, other)]
        if key not in items:
            targets = list(dict.fromkeys(edge.dst for edge in group))
            items[key] = Item(key, "connector", "", outputs_label(other, [(numbers[t], pipe.nodes[t]) for t in targets]),
                              True, "to " + ", ".join(f"{numbers[t]} {t}" for t in targets))
            beside(src)[0].items.append(key)
            counts["connector"] += 1
            units[key] = unit_of(pipe, src)
        labels = {edge.label for edge in group if edge.kind == kind}
        return Link(f"n:{src}", key, kind, labels.pop() if len(labels) == 1 else "", weight=REFERENCE_WEIGHT)

    links: list[Link] = []
    drawn: set[int] = set()
    for index, edge in enumerate(pipe.edges):
        if (edge.src.startswith("@") or edge.dst.startswith("@")) and not allow_placeholders:
            continue
        src_here = edge.src in pipe.nodes and pipe.stage_of(edge.src) == stage
        dst_here = edge.dst in pipe.nodes and pipe.stage_of(edge.dst) == stage
        if not (src_here or dst_here):
            continue
        drawn.add(index)
        if src_here and dst_here and index not in cuts:
            links.append(Link(f"n:{edge.src}", f"n:{edge.dst}", edge.kind, edge.label, edge=index))
        elif dst_here:
            links.append(Link(reference(edge.src, edge.dst, "in"), f"n:{edge.dst}", edge.kind, edge.label,
                              weight=REFERENCE_WEIGHT))
        elif edge.dst in pipe.nodes:
            link = output(edge.src, pipe.stage_of(edge.dst), edge.kind)
            if link not in links:
                links.append(link)
        else:
            links.append(Link(f"n:{edge.src}", reference(edge.dst, edge.src, "out"), edge.kind, edge.label,
                              weight=REFERENCE_WEIGHT))
    links = _feedback(links)
    # Each band below the last: a gate between them, below every step of the band above that feeds no
    # other step of its band, and above every step of the band below that no step of its band feeds.
    band_of = {unit: number for number, band in enumerate(bands) for unit in band}
    within = [link for link in links if band_of.get(units.get(link.src, "")) == band_of.get(units.get(link.dst, ""))]
    # In the direction the DOT is written: feedback runs head to tail (`_emit_links`).
    feeds = {link.dst if link.feedback else link.src for link in within}
    fed = {link.src if link.feedback else link.dst for link in within}
    # Each band hangs its units from one top edge: an invisible anchor at the top of each unit, the
    # band's anchors ranked together, each pulling its unit's first steps up to it. A gate ranks the
    # anchors below every step of the band above. The gate's edges weigh nothing, and an anchor pulls
    # only within its own box: weighted edges also steer dot sideways, and from one gate they pulled
    # the bands into a staircase.
    unit_boxes = {box.key: box for box in boxes.values()}
    aligned: list[list[str]] = []
    previous: list[str] = []
    for number, band in enumerate(bands):
        members = [key for key in items if band_of.get(units.get(key, "")) == number]
        gate = f"gate:{stage}:{number}"
        items[gate] = Item(gate, "gate", "", "")
        home_box.items.append(gate)
        links += [Link(key, gate, "data", "", invisible=True) for key in previous if key not in feeds]
        row: list[str] = []
        for unit in band:
            anchor = f"anchor:{unit}"
            items[anchor] = Item(anchor, "gate", "", "")
            unit_boxes.get(unit, home_box).items.insert(0, anchor)
            row.append(anchor)
            links.append(Link(gate, anchor, "data", "", invisible=True))
            links += [Link(anchor, key, "data", "", invisible=True, weight=1) for key in members
                      if units[key] == unit and key not in fed]
        aligned.append(row)
        # Left to right in flow order, as far as dot's crossing reduction allows.
        links += [Link(a, b, "data", "", invisible=True, weight=1) for a, b in zip(row, row[1:])]
        previous = members
    return Section(stage, items, root, links, len(nodes), len(drawn), counts["stub"], counts["connector"],
                   len(placeholders), drawn, units, [list(band) for band in bands], aligned)


def chart_stages(pipe: Pipeline, chart: str) -> list[str]:
    """The stages `chart` draws: those with a step."""
    stages = list(STAGE_NAMES) if chart == COMPLETE else [chart]
    return [stage for stage in stages if any(pipe.parts[node.part].stage == stage for node in pipe.nodes.values())]


def subtitle(pipe: Pipeline, chart: str, sections: Sequence[Section], detail: str) -> str:
    stages = {section.stage for section in sections}
    parts = [part for part in pipe.parts.values() if part.stage in stages]
    snapshots: dict[str, list[str]] = {}
    for part in parts:
        snapshots.setdefault(part.snapshot, []).append(part.name)
    if len(snapshots) == 1:
        snap = f"snapshot {next(iter(snapshots))}"
    else:
        snap = "snapshots " + ", ".join(f"{s} ({', '.join(names)})" for s, names in snapshots.items())
    nodes = [node for node in pipe.nodes.values() if pipe.parts[node.part].stage in stages]
    counts = {status: sum(node.status == status for node in nodes) for status in STATUSES}
    tally = ", ".join(f"{counts[status]} {status}" for status in STATUSES)
    pieces = ["HYPERION", snap, f"{len(nodes)} nodes: {tally}"]
    if chart == COMPLETE:
        pieces.append(f"{len(sections)} stages, {len(parts)} parts")
    connectors = sum(section.connectors for section in sections)
    stubs = sum(section.stubs for section in sections)
    pieces.append(f"{plural(connectors, 'connector')} to other stages, {plural(stubs, 'reference stub')}")
    placeholders = sum(section.placeholders for section in sections)
    if placeholders:
        pieces.append(plural(placeholders, "unresolved placeholder"))
    pieces.append(f"detail: {detail}")
    return "  ·  ".join(pieces)


def build_legend(chart: str, detail: str, placeholders: bool) -> tuple[Box, dict[str, Item], list[Link]]:
    """The legend: every node kind with its family, the statuses, the references and the edge kinds,
    each drawn as the chart draws it."""
    legend = Box("cluster_legend", "Legend", "legend")
    items: dict[str, Item] = {}
    links: list[Link] = []

    def add(key: str, look: str, label: str, status: str = "built", centred: bool = False, family: str = "") -> str:
        items[key] = Item(key, look, status, label, centred, family=family)
        legend.items.append(key)
        return key

    def heading(key: str, text: str) -> str:
        return add(key, "plain", _font(f"<B>{esc(text)}</B>", 10, INK_SECONDARY))

    row = [heading("legend:h:kinds", "Node kinds")]
    for kind, style in NODE_KINDS.items():
        label = _table([_font(esc(style.family.upper()), 6.5, INK_SECONDARY), _font(f"<B>{esc(kind)}</B>", 10),
                        _font(esc(style.meaning), 8)], bool(style.fit))
        row.append(add(f"legend:k:{kind}", kind, label, centred=bool(style.fit)))
    legend.rows.append(row)

    row = [heading("legend:h:status", "Status")]
    for status in STATUSES:
        eyebrow = "  ·  ".join(["MODEL", STATUS_MARK[status]]) if STATUS_MARK[status] else "MODEL"
        label = _table([_font(esc(eyebrow), 6.5, INK_SECONDARY), _font(f"<B>{esc(status)}</B>", 10),
                        _font(esc(STATUS_MEANING[status]), 8)], False)
        row.append(add(f"legend:s:{status}", "model", label, status))
    anatomy = "<B>G12</B>  ·  KIND  ·  STATUS" + ("  ·  id" if detail == "full" else "")
    rows = [_font(anatomy, 6.5, INK_SECONDARY), _font("<B>Title</B>", 10), _font("Summary", 8)]
    if detail != "brief":
        rows.append(_font("Detail lines", 7.5, INK_SECONDARY))
    if detail == "full":
        rows.append(_font("<I>Citations</I>", 7.5, INK_SECONDARY))
        rows.append(_font("source/paths.rs, plan tasks", 7, INK_SECONDARY, MONO))
    row.append(add("legend:s:anatomy", "plain-box", _table(rows, False)))
    row.append(add("legend:s:number", "plain", _font("G12: the 12th step of the galaxy stage<BR/>"
                                                     "(S: star systems, P: planets)", 8)))
    legend.rows.append(row)

    row = [heading("legend:h:refs", "References")]
    row.append(add("legend:r:stub", "stub", f'{_font("<B>↻ G12</B>", 8)}  {_font("the step numbered G12", 9)}',
                   family="randomness"))
    row.append(add("legend:r:stub:meaning", "plain", _font("an edge from a hub or from far away, drawn from a stub<BR/>"
                                                           "beside the step it feeds, in the colours of the step it<BR/>"
                                                           "names; that step's line \u201c↻ stubs at G15 · G20\u201d says<BR/>"
                                                           "where its stubs are", 8)))
    connector = _table([_font("from / to another stage:", 8, INK_SECONDARY), _font("<B>S7  the step there</B>", 9.5)],
                       True)
    row.append(add("legend:r:connector", "connector", connector, "", True))
    row.append(add("legend:r:connector:meaning", "plain", _font("an edge to or from another stage: one connector for each<BR/>"
                                                                "step there and cluster here; outputs list what they feed", 8)))
    if placeholders:
        label = _table([_font(esc("⚠ UNRESOLVED  ·  @part"), 6.5, INK_SECONDARY),
                        _font("<B>placeholder awaiting an id</B>", 9)], False)
        row.append(add("legend:r:placeholder", "placeholder", label, ""))
    legend.rows.append(row)

    row = [heading("legend:h:boxes", "Boxes")]
    row.append(add("legend:b:cluster", "box-sample:0", _table([_font("<B>Subsystem</B>", 11, INK_SECONDARY),
                                                                _font("the steps of one subsystem", 8)], False)))
    row.append(add("legend:b:nested", "box-sample:1", _table([_font("<B>Part of it</B>", 11, INK_SECONDARY),
                                                               _font("a box in a box: shaded darker", 8)], False)))
    if chart == COMPLETE:
        row.append(add("legend:b:stage", "box-sample:stage", _table([_font("<B>STAGE</B>", 13, INK),
                                                                      _font("each stage's section", 8)], False)))
    legend.rows.append(row)

    row = [heading("legend:h:edges", "Edges")]
    for kind, style in EDGE_KINDS.items():
        name = add(f"legend:e:{kind}", "plain", _font(f"<B>{esc(kind)}</B>", 9))
        meaning = add(f"legend:e:{kind}:meaning", "plain", _font(esc(style.meaning), 8))
        links.append(Link(name, meaning, kind, "", minlen=3, weight=10))
        row.extend([name, meaning])
    legend.rows.append(row)
    drawn = {(link.src, link.dst) for link in links}
    for row in legend.rows:
        links.extend(Link(a, b, "data", "", invisible=True, weight=10) for a, b in zip(row, row[1:])
                     if (a, b) not in drawn)
    for upper, lower in zip(legend.rows, legend.rows[1:]):
        links.append(Link(upper[0], lower[0], "data", "", invisible=True, weight=10))
    return legend, items, links


# ---------------------------------------------------------------------------------------------
# DOT.


class Html(str):
    """An HTML-like label, written between `<` and `>` rather than quoted."""


def quote(text: str) -> str:
    """A DOT double-quoted string: backslashes and quotes escaped (Graphviz reads `\\N` and its kin
    in some attributes, so a backslash is doubled)."""
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def attrs(values: Mapping[str, str | float | int | bool]) -> str:
    """`[key="value", ...]`, an `Html` value as an HTML-like label."""
    out = []
    for key, value in values.items():
        if isinstance(value, bool):
            text = "true" if value else "false"
            out.append(f"{key}={text}")
        elif isinstance(value, (int, float)):
            out.append(f"{key}={value:g}")
        elif isinstance(value, Html):
            out.append(f"{key}=<{value}>")
        else:
            out.append(f"{key}={quote(value)}")
    return "[" + ", ".join(out) + "]"


def fit_shape(rule: str, width: float, height: float) -> dict[str, str | float | bool]:
    """A fixed size, in inches, for a shape that Graphviz would inflate around a `width` x `height`
    point label (it pads every polygon but the box by √2 and more; a diamond doubles). Each rule
    puts the label's corners just inside the outline, with a little room."""
    if rule == "ellipse":
        w, h = width + 10, height + 6
        outer_h = 1.7 * h
        outer_w = w / math.sqrt(1 - (h / outer_h) ** 2)
        extra: dict[str, str | float | bool] = {}
    elif rule == "diamond":
        # The corners of the label touch the rhombus when w/W + h/H = 1; H is chosen to minimise
        # W + H, between twice and three times the label's height.
        w, h = width + 8, height + 6
        outer_h = min(max(h + math.sqrt(w * h), 2 * h), 3 * h)
        outer_w = w / (1 - h / outer_h)
        extra = {}
    elif rule == "parallelogram":
        # Graphviz shifts the top edge right by d = W·s / (2·√(1 + s²) + s) for skew s, and keeps
        # the width W while s <= 0.5. The label fits when W = w + 2d; d aims at 0.35 H.
        w, h = width + 20, height + 12
        outer_h = h
        d = 0.35 * outer_h
        outer_w = w + 2 * d
        # Inverted: s = 2q / √((1 - q)² - 4q²) for q = d / W. Past s = 0.5 (a narrow label, steeply
        # slanted) Graphviz widens the shape instead, so the skew stops there and W follows from q.
        q = d / outer_w
        q_max = 0.5 / (2 * math.sqrt(1.25) + 0.5)
        if q >= q_max:
            skew, q = 0.5, q_max
            outer_w = w / (1 - 2 * q)
        else:
            skew = 2 * q / math.sqrt((1 - q) ** 2 - 4 * q * q)
        extra = {"sides": 4, "skew": round(skew, 4)}
    elif rule == "pentagon":
        # `invhouse`: its rectangle runs from 0.0955 H to 0.6545 H below the top, its point below.
        # A label centred on the node reaches into the point; it fits when W = w · 0.691 / (1 - h/H).
        w, h = width + 16, height + 6
        outer_h = 2.3 * h
        outer_w = max(w, w * 0.691 / (1 - h / outer_h))
        extra = {}
    else:
        raise ValueError(f"no fitting rule '{rule}'")
    return {"fixedsize": True, "width": round(outer_w / 72, 3), "height": round(outer_h / 72, 3), **extra}


def look_style(look: str) -> tuple[str, str, Family | None]:
    """The shape, fitting rule and family of a look."""
    if look in NODE_KINDS:
        style = NODE_KINDS[look]
        return style.shape, style.fit, FAMILIES[style.family]
    if look == "connector":
        return "invhouse", "pentagon", None
    if look in ("placeholder", "plain-box"):
        return "box", "", None
    return look, "", None


def tooltip(text: str) -> str:
    """`text` for a tooltip: Graphviz expands `\\N` and its kin there after `quote` has had its say, so
    a backslash is doubled once more (four in the DOT for one drawn)."""
    return text.replace("\\", "\\\\")


def item_attrs(item: Item, size: tuple[float, float] | None) -> dict[str, str | float | bool]:
    shape, fit, family = look_style(item.look)
    out: dict[str, str | float | bool] = {"shape": shape}
    if item.look == "gate":
        return {"shape": "point", "style": "invis", "width": 0.01, "height": 0.01, "ordering": "out"}
    if item.look.startswith("box-sample:"):
        # The legend's picture of a box: drawn as `_emit_box` draws a cluster at that depth.
        depth = item.look.partition(":")[2]
        if depth == "stage":
            look = {"color": MUTED, "fillcolor": SURFACE, "penwidth": 2.4}
        else:
            look = {"color": BASELINE, "penwidth": 1.0,
                    "fillcolor": mix(HAIRLINE, SURFACE, CLUSTER_FILLS[min(int(depth), len(CLUSTER_FILLS) - 1)])}
        return {"shape": "box", "style": "rounded,filled", "margin": "0.12,0.1", **look, "label": Html(item.label)}
    if item.look == "plain":
        # `plaintext`, not `plain`: the margin keeps the legend's words off its sample edges.
        return {"shape": "plaintext", "margin": "0.06,0.02", "label": Html(item.label)}
    if item.look == "stub":
        # A chip: a rounded tag in the muted tint of the family of the step it names.
        named = FAMILIES[item.family]
        chip = (f'<TABLE STYLE="rounded" BORDER="1" COLOR="{named.hue}" BGCOLOR="{named.planned_fill}" '
                f'CELLBORDER="0" CELLSPACING="0" CELLPADDING="3"><TR><TD>{item.label}</TD></TR></TABLE>')
        out = {"shape": "plain", "label": Html(chip)}
        if item.tooltip:
            out["tooltip"] = tooltip(item.tooltip)
        return out
    styles = ["filled"]
    if family is not None:
        style = NODE_KINDS[item.look]
        if style.rounded:
            styles.append("rounded")
        out["color"] = family.hue
        if item.status == "planned":
            styles.append("dashed")
            out["fillcolor"] = family.planned_fill
            out["penwidth"] = 1.4
        elif item.status == "partial":
            # Half built: the built tint on the left, the planned one on the right.
            out["fillcolor"] = f"{family.fill};0.5:{family.planned_fill}"
            out["gradientangle"] = 0
        else:
            out["fillcolor"] = family.fill
    elif item.look == "connector":
        out.update({"color": MUTED, "fillcolor": NEUTRAL, "penwidth": 1.4})
    elif item.look == "placeholder":
        styles.append("dotted")
        out.update({"color": SERIOUS, "fillcolor": mix(SERIOUS, SURFACE, 0.14), "penwidth": 2.2})
    elif item.look == "plain-box":
        out.update({"color": BASELINE, "fillcolor": SURFACE, "penwidth": 1})
    out["style"] = ",".join(styles)
    if shape == "cds":
        out["margin"] = "0.3,0.12"  # room for the point, and for the outline inset at top and bottom
    if fit:
        if size is None:
            raise ValueError(f"{item.key}: no measured size for a fitted shape")
        out.update(fit_shape(fit, *size))
    out["label"] = Html(item.label)
    if item.tooltip:
        out["tooltip"] = tooltip(item.tooltip)
    return out


def link_attrs(link: Link) -> dict[str, str | float | bool]:
    if link.invisible:
        return {"style": "invis", "weight": link.weight}
    style = EDGE_KINDS[link.kind]
    out: dict[str, str | float | bool] = {
        "color": style.colour,
        "style": style.style,
        "arrowhead": style.arrowhead,
        "penwidth": style.penwidth,
    }
    if link.label:
        # On a patch of the surface, so that a line passing behind it does not strike it through.
        out["label"] = Html(f'<TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="1" BGCOLOR="{SURFACE}">'
                            f'<TR><TD>{_font(esc(link.label), 8, INK_SECONDARY)}</TD></TR></TABLE>')
    if link.feedback:
        # Written head to tail and drawn backwards, at weight 0: ranked so that it agrees with the
        # flow and pulls nothing. (`constraint=false`, the usual way, makes dot fail, "trouble in
        # init_rank", on cluster-crossing feedback in charts of this size.)
        out.update({"dir": "back", "arrowtail": style.arrowhead, "arrowhead": "none", "weight": 0})
    if link.minlen:
        out["minlen"] = link.minlen
    if link.weight:
        out["weight"] = link.weight
    if link.edge >= 0:
        out["pf_edge"] = link.edge  # read back from the layout, to find the long edges
    return out


LAYOUT = {
    "newrank": True,
    "splines": "spline",
    "nodesep": 0.28,
    "ranksep": 0.5,
    "outputorder": "edgesfirst",
    # Fewer rounds of crossing reduction than the default, which keeps more of the bands' order, left
    # to right as the data flows (measured on the 2026-10-12 data: every pair of units in order in
    # the stars and planets stages, against 9 of 9 and 70 of 76 at the default, and 16 of 22 in galaxy).
    "mclimit": 0.3,
}


def _graph_head(name: str) -> list[str]:
    return [
        f"digraph {quote(name)} {{",
        f"  graph {attrs({'fontname': FONT, **LAYOUT})};",
        f"  node {attrs({'fontname': FONT, 'fontcolor': INK, 'fontsize': 9, 'margin': '0.14,0.07', 'penwidth': 1.6})};",
        f"  edge {attrs({'fontname': FONT, 'fontcolor': INK_SECONDARY, 'fontsize': 7.5, 'arrowsize': 0.8})};",
    ]


def _emit_box(lines: list[str], box: Box, items: Mapping[str, Item], sizes: Mapping[str, tuple[float, float]],
              depth: int, indent: str) -> None:
    def node_line(key: str, pad: str) -> str:
        return f"{pad}{quote(key)} {attrs(item_attrs(items[key], sizes.get(key)))};"

    if box.level == "stage":
        label = _font(f"<B>{esc(box.label.upper())}</B>", 22, INK)
        look = {"style": "rounded,filled", "color": MUTED, "fillcolor": SURFACE, "penwidth": 2.4, "margin": 24}
    elif box.level == "legend":
        label = _font(f"<B>{esc(box.label)}</B>", 13, INK)
        look = {"style": "rounded,filled", "color": BASELINE, "fillcolor": SURFACE, "penwidth": 1.2, "margin": 14}
    else:
        label = _font(f"<B>{esc(box.label)}</B>", 12, INK_SECONDARY)
        fill = mix(HAIRLINE, SURFACE, CLUSTER_FILLS[min(depth, len(CLUSTER_FILLS) - 1)])
        look = {"style": "rounded,filled", "color": BASELINE, "fillcolor": fill, "penwidth": 1.0, "margin": 12}
    lines.append(f"{indent}subgraph {quote(box.key)} {{")
    lines.append(f"{indent}  graph {attrs({'label': Html(label), 'labeljust': 'l', 'fontname': FONT, **look})};")
    if box.rows:
        for row in box.rows:
            lines.append(f"{indent}  {{")
            lines.append(f"{indent}    rank=same;")
            lines.extend(node_line(key, indent + "    ") for key in row)
            lines.append(f"{indent}  }}")
    else:
        lines.extend(node_line(key, indent + "  ") for key in box.items)
    for child in box.boxes:
        _emit_box(lines, child, items, sizes, depth + (0 if box.level == "stage" else 1), indent + "  ")
    lines.append(f"{indent}}}")


def _emit_links(lines: list[str], links: Iterable[Link]) -> None:
    for link in links:
        tail, head = (link.dst, link.src) if link.feedback else (link.src, link.dst)
        lines.append(f"  {quote(tail)} -> {quote(head)} {attrs(link_attrs(link))};")


def emit_section(section: Section, sizes: Mapping[str, tuple[float, float]]) -> str:
    """A stage's section as DOT, every statement in a fixed order."""
    lines = _graph_head(f"section_{section.stage}")
    order = {unit: index for index, unit in enumerate(u for band in section.bands for u in band)}
    for key in sorted(section.root.items, key=lambda key: order.get(section.units.get(key, ""), 0)):
        lines.append(f"  {quote(key)} {attrs(item_attrs(section.items[key], sizes.get(key)))};")
    # The units in band order: dot starts from the order its input gives, and keeps much of it.
    for box in sorted(section.root.boxes, key=lambda box: order.get(box.key, 0)):
        if box.level == "stage":
            box = Box(box.key, box.label, box.level, box.items, sorted(box.boxes, key=lambda b: order.get(b.key, 0)))
        _emit_box(lines, box, section.items, sizes, 0, "  ")
    for row in section.aligned:
        if len(row) > 1:
            lines.append("  { rank=same; " + " ".join(quote(key) for key in row) + "; }")
    _emit_links(lines, section.links)
    lines.append("}")
    return "\n".join(lines) + "\n"


def emit_legend(chart: Chart, sizes: Mapping[str, tuple[float, float]]) -> str:
    lines = _graph_head("legend")
    _emit_box(lines, chart.legend, chart.legend_items, sizes, 0, "  ")
    _emit_links(lines, chart.legend_links)
    lines.append("}")
    return "\n".join(lines) + "\n"


def emit_title(chart: Chart) -> str:
    size = 40 if chart.name == COMPLETE else 26
    label = Html(f'<TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="3">'
                 f'<TR><TD ALIGN="LEFT">{_font(f"<B>{esc(chart.title)}</B>", size)}</TD></TR>'
                 f'<TR><TD ALIGN="LEFT">{_font(esc(chart.subtitle), size * 0.5, INK_SECONDARY)}</TD></TR></TABLE>')
    lines = _graph_head("title")
    lines.append(f"  \"title\" {attrs({'shape': 'plaintext', 'margin': 0, 'label': label})};")
    lines.append("}")
    return "\n".join(lines) + "\n"


# ---------------------------------------------------------------------------------------------
# Graphviz.


class GraphvizError(Exception):
    pass


def run_graphviz(command: Sequence[str], text: str, timeout: float, env: Mapping[str, str] | None = None) -> bytes:
    try:
        done = subprocess.run(list(command), input=text.encode("utf-8"), capture_output=True, timeout=timeout,
                              env={**os.environ, **env} if env else None)
    except FileNotFoundError as error:
        raise GraphvizError(f"{command[0]} is not installed (Graphviz): {error}") from error
    except subprocess.TimeoutExpired as error:
        raise GraphvizError(f"{' '.join(command)} took longer than {timeout:g} s") from error
    if done.returncode != 0:
        raise GraphvizError(f"{' '.join(command)} failed ({done.returncode}): {done.stderr.decode(errors='replace').strip()}")
    return done.stdout


def measure(labels: Mapping[str, str], timeout: float) -> dict[str, tuple[float, float]]:
    """The size, in points, of each HTML-like label, as Graphviz lays it out: one `dot` run over
    the labels alone, unconnected, which takes a moment."""
    if not labels:
        return {}
    lines = ["digraph measure {", f"  node {attrs({'shape': 'plain', 'fontname': FONT, 'fontsize': 9})};"]
    for key, label in labels.items():
        lines.append(f"  {quote(key)} [label=<{label}>];")
    lines.append("}")
    out = json.loads(run_graphviz(["dot", "-Tjson"], "\n".join(lines) + "\n", timeout))
    return {obj["name"]: (float(obj["width"]) * 72, float(obj["height"]) * 72) for obj in out.get("objects", [])
            if obj.get("name") in labels}


def fitted_labels(items: Mapping[str, Item]) -> dict[str, str]:
    return {key: item.label for key, item in items.items() if look_style(item.look)[1]}


def positioned_dot(text: str, timeout: float) -> str:
    """`dot`'s layout of a graph as positioned DOT, its long strings joined: Graphviz breaks them with a
    backslash and a newline, even between the two numbers of a point."""
    return run_graphviz(["dot", "-Tdot"], text, timeout).decode("utf-8").replace("\\\n", "")


def lay_out(text: str, timeout: float) -> tuple[str, dict]:
    """`dot`'s layout of a graph: as positioned DOT, for drawing, and as JSON, to measure it."""
    positioned = positioned_dot(text, timeout)
    return positioned, json.loads(run_graphviz(["neato", "-n2", "-Tjson"], positioned, timeout))


def long_edges(layout: dict, threshold: float = LONG_EDGE) -> set[int]:
    """The pipeline edges drawn directly whose ends lie more than `threshold` points apart."""
    objects = layout.get("objects", [])
    out: set[int] = set()
    for edge in layout.get("edges", []):
        index = edge.get("pf_edge")
        tail, head = objects[edge["tail"]], objects[edge["head"]]
        if index is None or "pos" not in tail or "pos" not in head:
            continue
        (x0, y0), (x1, y1) = (map(float, end["pos"].split(",")) for end in (tail, head))
        if math.hypot(x1 - x0, y1 - y0) > threshold:
            out.add(int(index))
    return out


@dataclass
class Piece:
    """A part of a chart laid out on its own: its positioned DOT and where it goes on the canvas."""

    positioned: str
    align: str  # "left" or "centre"


BOUNDS = re.compile(r'\bbb="([-\d.e+]+),([-\d.e+]+),([-\d.e+]+),([-\d.e+]+)"')
POSITIONS = re.compile(r'\b(pos|lp|xlp|head_lp|tail_lp|bb)="([^"]*)"')
POINT = re.compile(r"(-?[\d.]+(?:e[-+]?\d+)?),(-?[\d.]+(?:e[-+]?\d+)?)")


def bounds(positioned: str) -> tuple[float, float, float, float]:
    match = BOUNDS.search(positioned)
    if not match:
        raise GraphvizError("a laid-out graph has no bounding box")
    x0, y0, x1, y1 = (float(v) for v in match.groups())
    return x0, y0, x1, y1


def translate(positioned: str, dx: float, dy: float) -> str:
    """`positioned` moved by (dx, dy) points: every position, label position and bounding box."""

    def shift(match: re.Match[str]) -> str:
        def point(p: re.Match[str]) -> str:
            return f"{float(p.group(1)) + dx:.2f},{float(p.group(2)) + dy:.2f}"

        value = match.group(2)
        if match.group(1) == "bb":
            x0, y0, x1, y1 = (float(v) for v in value.split(","))
            return f'bb="{x0 + dx:.2f},{y0 + dy:.2f},{x1 + dx:.2f},{y1 + dy:.2f}"'
        return f'{match.group(1)}="{POINT.sub(point, value)}"'

    return POSITIONS.sub(shift, positioned)


GAP = 36.0  # points between pieces
# An edge that is not drawn: a gate's, or one of the legend's. Its spline still widens the canvas that
# `neato -n2` draws: in the stars stage, well past the clusters.
INVISIBLE_EDGE = re.compile(r'\n\s*"[^"]*"\s*->\s*"[^"]*"\s*\[[^\]]*\bstyle=invis\b[^\]]*\];')
# An edge's label (`link_attrs`, on its patch of the surface) and its place.
EDGE_LABEL = re.compile(r'("[^"]*")\s*->\s*("[^"]*")(\s*\[[^\]]*?)label=<(<TABLE [^>]*BGCOLOR="'
                        + SURFACE + r'">.*?</TABLE>)>,\s*lp="([^"]*)",?', re.S)


def lift_labels(positioned: str, nodes: list[str]) -> str:
    """`positioned` with each edge's label taken off it and added to `nodes` as a node at its place.
    Graphviz draws a label with its edge, so that every edge drawn after it strikes it through: as a
    node it is drawn after every edge (`outputorder=edgesfirst`), and before every step."""

    def lift(match: re.Match[str]) -> str:
        tail, head, before, label, place = match.groups()
        ends = " -> ".join(end.strip('"') for end in (tail, head))
        key = f"label {len(nodes) + 1}: {ends}"
        nodes.append(f"  {quote(key)} {attrs({'shape': 'plain', 'fontname': FONT, 'pos': place, 'label': Html(label)})};")
        return f"{tail} -> {head}{before}"

    return EDGE_LABEL.sub(lift, positioned)


def compose(name: str, pieces: Sequence[Piece]) -> str:
    """The pieces stacked top to bottom on one canvas, as one positioned graph for `neato -n2`:
    each piece's graph becomes a subgraph, moved into place. The canvas draws edges first, then
    their labels, then the steps (`lift_labels`); `outputorder` is read from the root graph alone."""
    sizes = [bounds(piece.positioned) for piece in pieces]
    width = max(x1 - x0 for x0, _, x1, _ in sizes)
    height = sum(y1 - y0 for _, y0, _, y1 in sizes) + GAP * (len(pieces) - 1)
    body: list[str] = []
    labels: list[str] = []
    top = height
    for index, (piece, (x0, y0, x1, y1)) in enumerate(zip(pieces, sizes)):
        left = (width - (x1 - x0)) / 2 if piece.align == "centre" else 0.0
        moved = translate(piece.positioned, left - x0, top - y1)
        inner = lift_labels(INVISIBLE_EDGE.sub("", moved[moved.index("{") + 1 : moved.rindex("}")]), labels)
        body.append(f"  subgraph {quote(f'piece_{index}')} {{{inner}  }}")
        top -= (y1 - y0) + GAP
    head = {"bb": f"0,0,{width:.2f},{height:.2f}", "bgcolor": SURFACE, "pad": 0.4, "fontname": FONT,
            "outputorder": LAYOUT["outputorder"]}
    return (f"digraph {quote('procgen_' + name)} {{\n  graph {attrs(head)};\n" + "".join(f"{node}\n" for node in labels)
            + "\n".join(body) + "\n}\n")


def png_dpi(positioned: bytes | str, dpi: float) -> float:
    """`dpi`, lowered so that the PNG of a laid-out chart keeps within PNG_MAX_SIDE and
    PNG_MAX_PIXELS."""
    text = positioned.decode("utf-8") if isinstance(positioned, bytes) else positioned
    match = BOUNDS.search(text)
    if not match:
        return dpi
    x0, y0, x1, y1 = (float(v) for v in match.groups())
    width, height = (x1 - x0) / 72, (y1 - y0) / 72  # inches
    if width <= 0 or height <= 0:
        return dpi
    # The image is a little larger than the layout's bounds (padding, rounding): 3% to spare.
    cap = 0.97 * min(PNG_MAX_SIDE / max(width, height), math.sqrt(PNG_MAX_PIXELS / (width * height)))
    return math.floor(min(dpi, cap) * 100) / 100


def flow_order(pipe: Pipeline, stage: str) -> list[str]:
    """The units of `stage` in the order the data flows through them: each after the units that feed
    it, ties (and cycles) broken by the files' order."""
    order: list[str] = []
    for node in pipe.nodes.values():
        if pipe.parts[node.part].stage == stage and unit_of(pipe, node.id) not in order:
            order.append(unit_of(pipe, node.id))
    feeders: dict[str, set[str]] = {unit: set() for unit in order}
    for edge in pipe.edges:
        if edge.src in pipe.nodes and edge.dst in pipe.nodes and pipe.stage_of(edge.src) == stage == pipe.stage_of(edge.dst):
            src, dst = unit_of(pipe, edge.src), unit_of(pipe, edge.dst)
            if src != dst:
                feeders[dst].add(src)
    placed: list[str] = []
    while len(placed) < len(order):
        ready = [unit for unit in order if unit not in placed and feeders[unit] <= set(placed)]
        placed.append(ready[0] if ready else next(unit for unit in order if unit not in placed))
    return placed


def pack(order: Sequence[str], sizes: Mapping[str, tuple[float, float]], width: float) -> list[list[str]]:
    """`order` cut into bands no wider than `width` (each holds one unit at least)."""
    bands: list[list[str]] = [[]]
    used = 0.0
    for unit in order:
        w = sizes[unit][0] + 20
        if bands[-1] and used + w > width:
            bands.append([])
            used = 0.0
        bands[-1].append(unit)
        used += w
    return bands


def packed_height(bands: Sequence[Sequence[str]], sizes: Mapping[str, tuple[float, float]]) -> float:
    return sum(max(sizes[unit][1] for unit in band) + 72 for band in bands)


def choose_width(orders: Sequence[Sequence[str]], sizes: Mapping[str, tuple[float, float]],
                 aspect: float = ASPECT) -> float:
    """The band width that packs every section, stacked, closest to `aspect`."""
    units = [unit for order in orders for unit in order]
    widest = max(sizes[unit][0] + 20 for unit in units)
    candidates = sorted({widest} | {widest * (1.05 ** k) for k in range(1, 200)})
    best, score = widest, math.inf
    for width in candidates:
        height = sum(packed_height(pack(order, sizes, width), sizes) for order in orders)
        actual = max(min(width, max(sum(sizes[u][0] + 20 for u in band) for order in orders
                                    for band in pack(order, sizes, width))), 1.0)
        miss = abs(math.log(actual / height / aspect))
        if miss < score - 1e-9:
            best, score = width, miss
        if sum(sizes[unit][0] + 20 for unit in units) < width:
            break
    return best


def unit_sizes(layout: dict) -> dict[str, tuple[float, float]]:
    """Each unit's size in a layout: a cluster's box, or a lone step's."""
    sizes: dict[str, tuple[float, float]] = {}
    for obj in layout.get("objects", []):
        name = obj.get("name", "")
        if name.startswith("cluster:") and "bb" in obj:
            x0, y0, x1, y1 = (float(v) for v in obj["bb"].split(","))
            sizes[name] = (x1 - x0, y1 - y0)
        elif name.startswith("n:") and "width" in obj:
            sizes["unit:" + name[2:]] = (float(obj["width"]) * 72, float(obj["height"]) * 72)
    return sizes


class SectionLayout:
    """One stage's section, laid out in passes: its units alone to measure them, then packed in bands,
    then again for each set of edges still too long."""

    def __init__(self, pipe: Pipeline, numbers: Mapping[str, str], stage: str, detail: str,
                 allow_placeholders: bool, boxed: bool, timeout: float) -> None:
        self.pipe, self.numbers, self.stage, self.detail = pipe, numbers, stage, detail
        self.allow, self.boxed, self.timeout = allow_placeholders, boxed, timeout
        self.labels: dict[str, tuple[float, float]] = {}  # each fitted label's size, by its text
        self.seconds = 0.0
        self.passes = 0
        self.order = flow_order(pipe, stage)
        self.hubs = hub_cuts(pipe, stage)
        self.between = {index for index, edge in enumerate(pipe.edges)
                        if edge.src in pipe.nodes and edge.dst in pipe.nodes
                        and pipe.stage_of(edge.src) == stage == pipe.stage_of(edge.dst)
                        and unit_of(pipe, edge.src) != unit_of(pipe, edge.dst)}

    def run(self, cuts: set[int], bands: Sequence[Sequence[str]] = ()) -> tuple[Section, str, dict]:
        started = time.monotonic()
        section = build_section(self.pipe, self.numbers, self.stage, self.detail, self.allow, cuts, self.boxed, bands)
        # By text, not key: a step's label changes between passes as its stubs do.
        fitted = fitted_labels(section.items)
        fresh = [label for label in dict.fromkeys(fitted.values()) if label not in self.labels]
        measured = measure({f"m{n}": label for n, label in enumerate(fresh)}, self.timeout)
        self.labels.update({label: measured[f"m{n}"] for n, label in enumerate(fresh)})
        sizes = {key: self.labels[label] for key, label in fitted.items()}
        positioned, layout = lay_out(emit_section(section, sizes), self.timeout)
        self.seconds += time.monotonic() - started
        self.passes += 1
        return section, positioned, layout

    def measure_units(self) -> dict[str, tuple[float, float]]:
        """Every unit's size, laid out apart: each edge between units cut."""
        return unit_sizes(self.run(self.hubs | self.between)[2])

    def finish(self, bands: Sequence[Sequence[str]]) -> tuple[Section, str]:
        """Laid out in `bands`: an edge between units stays drawn only from one band into the next."""
        band_of = {unit: number for number, band in enumerate(bands) for unit in band}
        cuts = set(self.hubs)
        for index in self.between:
            edge = self.pipe.edges[index]
            src, dst = band_of[unit_of(self.pipe, edge.src)], band_of[unit_of(self.pipe, edge.dst)]
            if dst != src + 1:  # within a band, back up, or past the next: a stub keeps it short
                cuts.add(index)
        for attempt in range(LONG_EDGE_PASSES + 1):
            section, positioned, layout = self.run(cuts, bands)
            far = long_edges(layout) - cuts
            if not far or attempt == LONG_EDGE_PASSES:
                return section, positioned
            cuts |= far
        raise AssertionError("unreachable")


def stacked_aspect(positioned: Sequence[str]) -> float:
    """Width over height of laid-out sections stacked in one column."""
    boxes = [bounds(text) for text in positioned]
    return max(x1 - x0 for x0, _, x1, _ in boxes) / sum(y1 - y0 for _, y0, _, y1 in boxes)


@dataclass
class Layout:
    chart: Chart
    positioned: str
    seconds: float  # the slowest section's layout, every pass of it
    passes: int  # the most passes a section took


def lay_out_chart(pipe: Pipeline, name: str, detail: str, allow_placeholders: bool,
                  timeout: float = 600) -> Layout | None:
    """The chart `name` laid out: its sections at once, each by its own `dot` runs, packed to one band
    width, then its legend and title, stacked on one canvas. None where the chart has no data."""
    stages = chart_stages(pipe, name)
    if not stages:
        return None
    numbers = step_numbers(pipe)
    layouts = [SectionLayout(pipe, numbers, stage, detail, allow_placeholders, name == COMPLETE, timeout)
               for stage in stages]
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(stages)) as pool:
        sizes: dict[str, tuple[float, float]] = {}
        for measured in pool.map(SectionLayout.measure_units, layouts):
            sizes.update(measured)
        width = choose_width([layout.order for layout in layouts], sizes)
        laid = list(pool.map(lambda layout: layout.finish(pack(layout.order, sizes, width)), layouts))
        # Edges between the units of a band stretch it past the estimate: once, widen the bands by
        # the shortfall and lay out again, where that is closer to the shape aimed at.
        shape = stacked_aspect([positioned for _, positioned in laid])
        if abs(math.log(shape / ASPECT)) > math.log(1.25):
            wider = width * math.sqrt(ASPECT / shape)
            again = list(pool.map(lambda layout: layout.finish(pack(layout.order, sizes, wider)), layouts))
            if abs(math.log(stacked_aspect([p for _, p in again]) / ASPECT)) < abs(math.log(shape / ASPECT)):
                laid = again
    sections = [section for section, _ in laid]
    legend, legend_items, legend_links = build_legend(name, detail, any(s.placeholders for s in sections))
    chart = Chart(name, TITLE_PREFIX + CHART_NAMES[name], detail, sections, legend, legend_items, legend_links,
                  subtitle(pipe, name, sections, detail))
    legend_text = emit_legend(chart, measure(fitted_labels(legend_items), timeout))
    pieces = [Piece(positioned_dot(emit_title(chart), timeout), "left"), Piece(positioned_dot(legend_text, timeout), "left")]
    pieces += [Piece(positioned, "centre") for _, positioned in laid]
    return Layout(chart, compose(name, pieces), max(layout.seconds for layout in layouts),
                  max(layout.passes for layout in layouts))


def render(layout: Layout, out_dir: Path, formats: Sequence[str], dpi: float, timeout: float) -> list[Path]:
    """Each format drawn by `neato -n2` from the one layout; the paths written."""
    written: list[Path] = []
    for fmt in formats:
        path = out_dir / f"{layout.chart.name}.{fmt}"
        command = ["neato", "-n2", f"-T{fmt}"]
        env: dict[str, str] = {}
        if fmt == "png":
            command.append(f"-Gdpi={png_dpi(layout.positioned, dpi):g}")
        if fmt == "pdf":
            # Cairo stamps a PDF with its creation time unless SOURCE_DATE_EPOCH fixes it.
            env["SOURCE_DATE_EPOCH"] = "0"
            x0, y0, x1, y1 = bounds(layout.positioned)
            if max(x1 - x0, y1 - y0) / 72 > PDF_MAX_INCHES:
                command.append(f"-Gsize={PDF_MAX_INCHES},{PDF_MAX_INCHES}")
        path.write_bytes(run_graphviz(command, layout.positioned, timeout, env))
        written.append(path)
    return written

# ---------------------------------------------------------------------------------------------
# The command line.


def _choices(value: str, allowed: Sequence[str], what: str) -> list[str]:
    chosen = [v.strip() for v in value.split(",") if v.strip()]
    unknown = [v for v in chosen if v not in allowed]
    if unknown or not chosen:
        raise argparse.ArgumentTypeError(f"unknown {what} {', '.join(unknown) or '(none)'}: expected {','.join(allowed)}")
    return [v for v in allowed if v in chosen]


def _positive(value: str) -> float:
    try:
        number = float(value)
    except ValueError:
        number = math.nan
    if not number > 0 or math.isinf(number):
        raise argparse.ArgumentTypeError(f"must be a positive number, not {value!r}")
    return number


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    # The module docstring's account of the charts, the validation and the output, without its usage.
    parser = argparse.ArgumentParser(prog="just flowcharts", description="\n\n".join((__doc__ or "").split("\n\n")[2:7]),
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="validate the data, draw nothing; exit 1 on a problem")
    parser.add_argument("--only", metavar="CHARTS", default=",".join(CHARTS), type=lambda v: _choices(v, CHARTS, "chart"),
                        help=f"the charts to draw, of {','.join(CHARTS)} (default all)")
    parser.add_argument("--format", metavar="FORMATS", default="svg", type=lambda v: _choices(v, FORMATS, "format"),
                        help=f"the formats to write, of {','.join(FORMATS)} (default svg)")
    parser.add_argument("--detail", choices=DETAILS,
                        help="brief (title, summary), standard (+ detail lines) or full (+ citations, sources, ids); "
                             "default standard for a stage, full for the complete chart")
    parser.add_argument("--out", metavar="DIR", type=Path, default=OUT_DIR, help=f"where to write (default {display_path(OUT_DIR)})")
    parser.add_argument("--data", metavar="DIR", type=Path, default=DATA_DIR, help=f"the TOML files (default {display_path(DATA_DIR)})")
    parser.add_argument("--keep-dot", action="store_true",
                        help="also write each chart's laid-out DOT: its canvas, which `neato -n2` draws as it is")
    parser.add_argument("--allow-placeholders", action="store_true",
                        help="draw unresolved @part:... placeholders as marked stubs instead of refusing (preview)")
    parser.add_argument("--dpi", metavar="DPI", type=_positive, default=PNG_DPI,
                        help=f"PNG resolution (default {PNG_DPI:g}), lowered for charts too large to view")
    parser.add_argument("--timeout", metavar="SECONDS", type=_positive, default=600,
                        help="seconds allowed for each Graphviz run (default 600)")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    if not args.data.is_dir():
        print(f"error: {display_path(args.data)} is not a directory", file=sys.stderr)
        return 2
    if not any(args.data.glob("*.toml")):
        print(f"error: {display_path(args.data)} holds no *.toml files", file=sys.stderr)
        return 2
    pipe, problems = load(args.data)
    errors = [p for p in problems if not (p.placeholder and args.allow_placeholders)]
    warnings = [p for p in problems if p.placeholder and args.allow_placeholders]
    warnings += glyph_gaps(pipe, font_coverage())
    for problem in errors:
        print(problem, file=sys.stderr)
    for problem in warnings:
        suffix = " (drawn as a stub)" if problem.placeholder else ""
        print(f"warning: {problem}{suffix}", file=sys.stderr)
    files = len(pipe.parts)
    if errors:
        print(f"{len(errors)} problem{'s' if len(errors) != 1 else ''} in {display_path(args.data)}"
              + ("" if args.check else ": nothing drawn"), file=sys.stderr)
        return 1
    if args.check:
        print(f"ok: {files} files, {len(pipe.nodes)} nodes, {len(pipe.edges)} edges")
        return 0
    for tool in ("dot", "neato"):
        if shutil.which(tool) is None:
            print(f"error: Graphviz's `{tool}` is not on PATH", file=sys.stderr)
            return 2
    try:
        args.out.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        print(f"error: cannot write to {display_path(args.out)}: {error.strerror or error}", file=sys.stderr)
        return 2
    for name in args.only:
        detail = args.detail or DEFAULT_DETAIL[name]
        try:
            layout = lay_out_chart(pipe, name, detail, args.allow_placeholders, args.timeout)
            if layout is None:
                print(f"{name}: no data files for this chart: skipped", file=sys.stderr)
                continue
            if args.keep_dot:
                (args.out / f"{name}.dot").write_text(layout.positioned, encoding="utf-8")
            written = render(layout, args.out, args.format, args.dpi, args.timeout)
        except GraphvizError as error:
            print(f"error: {name}: {error}", file=sys.stderr)
            return 2
        except OSError as error:
            print(f"error: {name}: cannot write {error.filename or display_path(args.out)}: {error.strerror or error}",
                  file=sys.stderr)
            return 2
        sections = layout.chart.sections
        x0, y0, x1, y1 = bounds(layout.positioned)
        print(f"{name}: {sum(s.nodes for s in sections)} nodes, {len(set().union(*(s.drawn for s in sections)))} edges "
              f"({sum(s.stubs for s in sections)} stubs, {sum(s.connectors for s in sections)} connectors), "
              f"{(x1 - x0) / 72:.0f} x {(y1 - y0) / 72:.0f} in, laid out in {layout.seconds:.1f} s "
              f"({layout.passes} pass{'es' if layout.passes != 1 else ''}): {', '.join(display_path(path) for path in written)}",
              file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
