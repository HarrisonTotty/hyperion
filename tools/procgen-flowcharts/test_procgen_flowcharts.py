"""Tests of `just flowcharts` (`procgen_flowcharts.py`).

On the fixtures in `testdata/`, never on `data/`, which research agents edit: `valid/` is a small
pipeline shaped like the real one (three stages, the planets stage in two parts, every node and edge
kind, nested clusters, cross-stage edges and feedback, text at the schema's limits); `invalid/` fails
each check the validator makes at least once (and the tests add a file that is not TOML, which
pre-commit's check-toml would refuse to commit); `placeholders/` has unresolved `@part:...`
references. The tests pin the validator's messages; the step numbers, hubs and band packing; what
each stage's section holds (its steps, a connector for each remote step and cluster, one output
connector for each step and stage, a stub beside each consumer of a cut edge, the gates and top rows
of its bands); the DOT's escaping and stability, the canvas the pieces are stacked on, and the
geometry of the shapes sized around their labels; and the palette's rules: documented hues only,
text at 4.5:1 on every fill, a shape for each kind and a line, colour and arrowhead for each edge
kind. Where Graphviz is installed they also lay the charts out and draw them, twice, and compare the
files byte for byte.

Standard library only, in the Python 3.14 of `pyproject.toml`:
`uv run --project tools/procgen-flowcharts python -B tools/procgen-flowcharts/test_procgen_flowcharts.py`.
"""

from __future__ import annotations

import contextlib
import io
import re
import shutil
import sys
import tempfile
import unittest
from collections.abc import Iterable, Iterator, Sequence
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import procgen_flowcharts as pf

VALID = HERE / "testdata" / "valid"
INVALID = HERE / "testdata" / "invalid"
PLACEHOLDERS = HERE / "testdata" / "placeholders"
DATA = HERE / "data"  # read by one test only, which skips while the data has problems
GRAPHVIZ = shutil.which("dot") is not None and shutil.which("neato") is not None

# The light steps of the data-viz reference palette (`palette.md`): its categorical slots, its status
# colours, and its chrome and ink. Every colour the charts use is one of these or a tint of one.
DOCUMENTED = {
    "#2a78d6",
    "#eb6834",
    "#1baf7a",
    "#eda100",
    "#e87ba4",
    "#008300",
    "#6250d6",
    "#e34948",
    "#0ca30c",
    "#fab219",
    "#ec835a",
    "#d03b3b",
    "#fcfcfb",
    "#f9f9f7",
    "#0b0b0b",
    "#52514e",
    "#898781",
    "#e1e0d9",
    "#c3c2b7",
    "#f0efec",
}


# Every problem `testdata/invalid/` holds, but the one of the file that is not TOML (`invalid_fixtures`).
INVALID_PROBLEMS = [
    (
        "galaxy.toml",
        "g.arms",
        "unknown parent cluster 'g.bulge' (a parent must be in the same file)",
    ),
    ("galaxy.toml", "g.loop_a", "clusters' parents form a cycle"),
    ("galaxy.toml", "g.loop_b", "clusters' parents form a cycle"),
    (
        "galaxy.toml",
        "g.imf",
        "unknown kind 'sample' (expected seed, draw, param, table, model, decision, entity, query, enrich)",
    ),
    ("galaxy.toml", "g.imf", "unknown status 'done' (expected built, partial, planned)"),
    ("galaxy.toml", "x.halo", "id does not start with the file's prefix 'g.'"),
    ("galaxy.toml", "g.Bar-Density", "id must be 'g.' and then snake_case (a-z, 0-9, _)"),
    (
        "galaxy.toml",
        "g.long",
        "unknown key 'notes' (expected id, label, kind, cluster, summary, detail, cite, source, status)",
    ),
    (
        "galaxy.toml",
        "g.long",
        "'label' is 33 characters, over the limit of 32: \"A label of thirty-three chars, ok\"",
    ),
    (
        "galaxy.toml",
        "g.long",
        (
            "'summary' is 61 characters, over the limit of 60: "
            '"A summary of exactly sixty-one characters, one over the limit"'
        ),
    ),
    ("galaxy.toml", "g.long", "'detail' has 7 lines, over the limit of 6"),
    (
        "galaxy.toml",
        "g.long",
        "unknown cluster 'g.nowhere' (a node's cluster must be in its own file)",
    ),
    ("galaxy.toml", "g.wide", "'summary' must be one line"),
    (
        "galaxy.toml",
        "g.wide",
        (
            "'detail line 1' is 61 characters, over the limit of 60: "
            '"a detail line of sixty-one characters, one more than allowed."'
        ),
    ),
    ("galaxy.toml", "g.blank", "'label' is empty"),
    ("galaxy.toml", "g.typed", "'summary' must be a string"),
    ("galaxy.toml", "g.typed", "'detail' must be a list of strings"),
    ("galaxy.toml", "g.tabbed", "'label' holds a control character (U+0009)"),
    ("galaxy.toml", "g.partial_entry", "missing 'summary'"),
    (
        "galaxy.toml",
        "edge g.seed -> g.imf",
        "'label' is 29 characters, over the limit of 28: \"a label of twenty-nine chars.\"",
    ),
    (
        "galaxy.toml",
        "edge g.imf -> s.mass",
        "unknown edge kind 'flows' (expected data, seed, lookup, constraint)",
    ),
    ("galaxy.toml", "g.disc", "duplicate cluster id: also declared in this file"),
    ("galaxy.toml", "g.seed", "duplicate node id: also declared in this file"),
    ("galaxy.toml", "edge g.seed -> g.imf", "duplicate seed edge: also declared in this file"),
    ("galaxy.toml", "edge g.seed -> g.missing", "dangling 'to': no node 'g.missing' in any file"),
    ("galaxy.toml", "edge g.disc -> g.imf", "'from' names cluster 'g.disc': edges join nodes"),
    ("galaxy.toml", "edge g.imf -> g.imf", "joins a node to itself"),
    ("galaxy.toml", "edge g.imf -> s.mass", "dangling 'to': no node 's.mass' in any file"),
    (
        "galaxy.toml",
        "edge @stars:primary mass -> g.imf",
        "unresolved placeholder in 'from': '@stars:primary mass'",
    ),
    (
        "galaxy.toml",
        "edge g.imf -> @no placeholder",
        "'to' is not a placeholder of the form '@<part>:<description>': '@no placeholder'",
    ),
    (
        "galaxy.toml",
        "edge @stars:primary mass -> @planets-systems:disc mass",
        "unresolved placeholder in 'from': '@stars:primary mass'",
    ),
    (
        "galaxy.toml",
        "edge @stars:primary mass -> @planets-systems:disc mass",
        "unresolved placeholder in 'to': '@planets-systems:disc mass' (and no part 'planets-systems' is loaded)",
    ),
    (
        "galaxy.toml",
        "edge @stars:primary mass -> @planets-systems:disc mass",
        "joins two placeholders: one end must be a node of this data",
    ),
    ("nometa.toml", "notes", "unknown table 'notes' (expected meta, cluster, node, edge)"),
    ("nometa.toml", "meta", "missing [meta]"),
    ("nometa.toml", "node", "'node' must be an array of tables ([[node]])"),
    ("planets.toml", "meta", "prefix '9bad' must be snake_case, starting with a letter"),
    ("planets.toml", "edge #1", "must be a table, not int"),
    ("planets.toml", "edge #2", "must be a table, not int"),
    ("planets.toml", "node", "the file has no [[node]]: a part must hold at least one step"),
    ("stars.toml", "meta", "unknown stage 'stellar' (expected galaxy, stars, planets)"),
    ("stars.toml", "meta", "part 'stellar' is not the file's stem 'stars'"),
    ("stars.toml", "meta", "prefix 'g' is also galaxy.toml's"),
    ("stars.toml", "g.imf", "duplicate node id: also declared in galaxy.toml"),
]


def run(argv: Sequence[str]) -> tuple[int, str, str]:
    """`main(argv)`: its exit status, and what it printed to stdout and to stderr."""
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        status = pf.main(list(argv))
    return status, out.getvalue(), err.getvalue()


@contextlib.contextmanager
def invalid_fixtures() -> Iterator[Path]:
    """A copy of `testdata/invalid/` with `broken.toml` added: a file that is not TOML."""
    with tempfile.TemporaryDirectory() as tmp:
        for path in INVALID.glob("*.toml"):
            shutil.copy(path, tmp)
        (Path(tmp) / "broken.toml").write_text(
            '# Test fixture: not TOML.\n[meta\nstage = "planets"\n', encoding="utf-8"
        )
        yield Path(tmp)


def problems(directory: Path) -> list[tuple[str, str, str]]:
    _, found = pf.load(directory)
    return [(Path(p.file).name, p.where, p.message) for p in found]


def write_files(directory: Path, files: dict) -> Path:
    for name, text in files.items():
        (directory / name).write_text(text, encoding="utf-8")
    return directory


META = '[meta]\nstage = "galaxy"\npart = "galaxy"\nprefix = "g"\ntitle = "Galaxy"\nsnapshot = "a6d3f6dd"\n\n'


def node(
    ident: str,
    label: str = "A step",
    summary: str = "What it does",
    kind: str = "model",
    extra: str = "",
) -> str:
    return (
        f'[[node]]\nid = "{ident}"\nlabel = "{label}"\nkind = "{kind}"\nsummary = "{summary}"\n'
        f'status = "built"\n{extra}\n'
    )


def relative_luminance(colour: str) -> float:
    def linear(c: float) -> float:
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b = (linear(int(colour[i : i + 2], 16) / 255) for i in (1, 3, 5))
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a: str, b: str) -> float:
    high, low = sorted((relative_luminance(a), relative_luminance(b)), reverse=True)
    return (high + 0.05) / (low + 0.05)


class Validation(unittest.TestCase):
    """The data's rules, each problem printed as `file: id: problem`."""

    def test_the_fixtures_are_valid_and_ordered_by_stage(self) -> None:
        pipe, found = pf.load(VALID)
        self.assertEqual(found, [])
        self.assertEqual(list(pipe.parts), ["galaxy", "stars", "planets-systems", "planets-worlds"])
        self.assertEqual((len(pipe.nodes), len(pipe.edges)), (49, 76))
        self.assertEqual(next(iter(pipe.nodes)), "g.seed")

    def test_every_problem_is_reported_with_its_file_and_id(self) -> None:
        with invalid_fixtures() as directory:
            found = problems(directory)
        # tomllib's own words vary between Python versions: only the reading is pinned.
        broken = found.pop(0)
        self.assertEqual(broken[:2], ("broken.toml", "toml"))
        self.assertTrue(broken[2].startswith("cannot be read: "), broken)
        self.assertEqual(found, INVALID_PROBLEMS)

    def test_problems_print_as_file_id_problem_and_check_fails(self) -> None:
        with invalid_fixtures() as directory:
            status, out, err = run(["--check", "--data", str(directory)])
            self.assertEqual(status, 1)
            self.assertEqual(out, "")
            self.assertIn(
                "/galaxy.toml: g.imf: unknown status 'done' (expected built, partial, planned)\n",
                err,
            )
            self.assertTrue(
                err.rstrip().endswith(
                    f"{len(INVALID_PROBLEMS) + 1} problems in " + pf.display_path(directory)
                )
            )

    def test_a_valid_directory_passes_check(self) -> None:
        status, out, _ = run(["--check", "--data", str(VALID)])
        self.assertEqual((status, out), (0, "ok: 4 files, 49 nodes, 76 edges\n"))

    def test_text_at_a_limit_passes_and_one_character_over_fails(self) -> None:
        at = "x" * pf.LABEL_MAX
        with tempfile.TemporaryDirectory() as tmp:
            write_files(
                Path(tmp),
                {
                    "galaxy.toml": META
                    + node("g.a", at, "s" * pf.SUMMARY_MAX)
                    + node("g.b", at + "x", extra=detail_lines(pf.DETAIL_LINES_MAX))
                },
            )
            self.assertEqual(
                problems(Path(tmp)),
                [
                    (
                        "galaxy.toml",
                        "g.b",
                        f"'label' is 33 characters, over the limit of 32: \"{at}x\"",
                    ),
                ],
            )

    def test_a_stage_without_steps_is_refused_and_never_drawn(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            write_files(
                Path(tmp),
                {
                    "galaxy.toml": META + node("g.a") + node("g.b"),
                    "stars.toml": META.replace("galaxy", "stars").replace('"g"', '"s"'),
                },
            )
            pipe, found = pf.load(Path(tmp))
            self.assertEqual(
                [(Path(p.file).name, p.where, p.message) for p in found],
                [
                    (
                        "stars.toml",
                        "node",
                        "the file has no [[node]]: a part must hold at least one step",
                    )
                ],
            )
            self.assertEqual(run(["--check", "--data", tmp])[0], 1)
        self.assertEqual(pf.chart_stages(pipe, "stars"), [])
        self.assertEqual(pf.chart_stages(pipe, pf.COMPLETE), ["galaxy"])
        self.assertIsNone(pf.lay_out_chart(pipe, "stars", "standard", False))

    def test_a_missing_font_family_is_not_taken_for_its_stand_in(self) -> None:
        if shutil.which("fc-match") is None:
            self.skipTest("fontconfig is not installed")
        self.assertIsNone(pf.font_coverage("No Such Family Of HYPERION"))

    def test_a_node_and_a_cluster_may_share_an_id(self) -> None:
        pipe, found = pf.load(VALID)
        self.assertEqual(found, [])
        self.assertIn("pw.atmosphere", pipe.nodes)
        self.assertIn("pw.atmosphere", pipe.clusters)

    def test_placeholders_fail_unless_allowed(self) -> None:
        found = pf.load(PLACEHOLDERS)[1]
        self.assertEqual(len(found), 3)
        self.assertTrue(all(p.placeholder for p in found))
        self.assertEqual(run(["--check", "--data", str(PLACEHOLDERS)])[0], 1)
        status, out, err = run(["--check", "--allow-placeholders", "--data", str(PLACEHOLDERS)])
        self.assertEqual((status, out), (0, "ok: 2 files, 4 nodes, 6 edges\n"))
        self.assertEqual(err.count("(drawn as a stub)"), 3)

    def test_a_placeholder_to_a_part_not_yet_written_takes_the_stage_its_name_starts_with(
        self,
    ) -> None:
        pipe = pf.load(PLACEHOLDERS)[0]
        self.assertEqual(pipe.stage_of("@planets-systems:disc mass draw"), "planets")
        self.assertEqual(pipe.stage_of("@galaxy:local [Fe/H] at position"), "galaxy")
        self.assertEqual(pipe.stage_of("@moons:tides"), "")

    def test_characters_the_font_lacks_are_warned_about(self) -> None:
        pipe = pf.load(VALID)[0]
        ascii_only = frozenset(range(0x20, 0x7F))
        gaps = pf.glyph_gaps(pipe, ascii_only)
        self.assertIn(
            (
                "g.imf",
                "'☉' (U+2609) is not in DejaVu Sans: it will be drawn in another font, or as a box",
            ),
            [(p.where, p.message) for p in gaps],
        )
        self.assertEqual(
            len({(p.file, p.where, p.message) for p in gaps}), len(gaps)
        )  # once per node
        coverage = pf.font_coverage()
        if coverage is not None:
            self.assertEqual(
                pf.glyph_gaps(pipe, coverage), [], "the fixtures use DejaVu Sans's glyphs only"
            )


def undrawn(pipe: pf.Pipeline, section: pf.Section) -> list[tuple[str, str]]:
    """The edges of `section`'s stage that it draws nowhere: neither directly, nor from a stub or an
    input connector to the step it feeds, nor from the step to the output connector of its stage."""
    links = [link for link in section.links if not link.invisible]
    out: list[tuple[str, str]] = []
    for index, edge in enumerate(pipe.edges):
        here = [
            ref in pipe.nodes and pipe.stage_of(ref) == section.stage
            for ref in (edge.src, edge.dst)
        ]
        if not any(here):
            continue
        if any(link.edge == index for link in links):
            continue
        if here[1]:
            drawn = any(
                link.dst == f"n:{edge.dst}"
                and not link.src.startswith("n:")
                and link.kind == edge.kind
                and link.label == edge.label
                for link in links
            )
        else:
            target = f"out:{edge.src}@{pipe.stage_of(edge.dst)}"
            drawn = any(
                link.src == f"n:{edge.src}" and link.dst == target and link.kind == edge.kind
                for link in links
            )
        if not drawn:
            out.append((edge.src, edge.dst))
    return out


def detail_lines(count: int) -> str:
    return "detail = [" + ", ".join(f'"line {n}"' for n in range(count)) + "]\n"


class References(unittest.TestCase):
    """Step numbers, hubs, and the order the units are packed in."""

    pipe: pf.Pipeline

    @classmethod
    def setUpClass(cls) -> None:
        cls.pipe = pf.load(VALID)[0]

    def test_every_step_is_numbered_in_its_stage_in_the_files_order(self) -> None:
        numbers = pf.step_numbers(self.pipe)
        self.assertEqual(
            [
                numbers[n]
                for n in (
                    "g.seed",
                    "g.params",
                    "g.imf",
                    "s.stream",
                    "s.star",
                    "ps.stream",
                    "pw.mass",
                )
            ],
            ["G1", "G2", "G9", "S1", "S11", "P1", "P8"],
        )
        self.assertEqual(len(set(numbers.values())), len(numbers))

    def test_a_hubs_edges_into_other_clusters_are_cut(self) -> None:
        cuts = pf.hub_cuts(self.pipe, "galaxy")
        self.assertEqual(
            sorted((self.pipe.edges[i].src, self.pipe.edges[i].dst) for i in cuts),
            [
                ("g.params", "g.arms"),
                ("g.params", "g.density"),
                ("g.params", "g.metallicity"),
                ("g.params", "g.potential"),
                ("g.params", "g.sfh"),
            ],
        )
        self.assertEqual(
            pf.hub_cuts(self.pipe, "stars"),
            set(),
            "s.star's other edges stay in its cluster or leave the stage",
        )

    def test_units_flow_after_the_units_that_feed_them(self) -> None:
        self.assertEqual(
            pf.flow_order(self.pipe, "galaxy"),
            [
                "cluster:g.root",
                "cluster:g.structure",
                "cluster:g.population",
                "unit:g.system",
                "unit:g.range_query",
                "cluster:g.mega",
            ],
        )
        self.assertEqual(
            pf.unit_of(self.pipe, "g.density"), "cluster:g.structure"
        )  # nested in g.fields

    def test_packing_fills_each_band_to_the_width(self) -> None:
        sizes = {unit: (480.0, 300.0) for unit in "abcde"}
        self.assertEqual(pf.pack("abcde", sizes, 1000), [["a", "b"], ["c", "d"], ["e"]])
        self.assertEqual(pf.pack("abcde", sizes, 100), [["a"], ["b"], ["c"], ["d"], ["e"]])
        self.assertEqual(pf.packed_height(pf.pack("abcde", sizes, 1000), sizes), 3 * 372)

    def test_the_width_aims_at_the_shape(self) -> None:
        sizes = {str(n): (480.0, 300.0) for n in range(16)}
        width = pf.choose_width([list(sizes)], sizes)
        bands = pf.pack(list(sizes), sizes, width)
        shape = max(sum(sizes[u][0] + 20 for u in band) for band in bands) / pf.packed_height(
            bands, sizes
        )
        self.assertLess(abs(shape / pf.ASPECT - 1), 0.35)

    def test_long_edges_are_read_back_from_the_layout(self) -> None:
        layout = {
            "objects": [
                {"name": "a", "pos": "0,0"},
                {"name": "b", "pos": "100,0"},
                {"name": "c", "pos": "0,2000"},
            ],
            "edges": [
                {"tail": 0, "head": 1, "pf_edge": "4"},
                {"tail": 0, "head": 2, "pf_edge": "7"},
                {"tail": 1, "head": 2},
            ],
        }
        self.assertEqual(pf.long_edges(layout), {7})


class Sections(unittest.TestCase):
    """What each stage's section holds."""

    pipe: pf.Pipeline
    numbers: dict

    @classmethod
    def setUpClass(cls) -> None:
        cls.pipe = pf.load(VALID)[0]
        cls.numbers = pf.step_numbers(cls.pipe)

    def section(
        self,
        stage: str,
        cuts: Iterable[int] = (),
        detail: str = "standard",
        boxed: bool = False,
        bands: Sequence[Sequence[str]] = (),
    ) -> pf.Section:
        return pf.build_section(
            self.pipe, self.numbers, stage, detail, False, set(cuts), boxed, bands
        )

    @staticmethod
    def box_of(section: pf.Section, key: str) -> str:
        def find(box: pf.Box) -> str:
            if key in box.items:
                return box.key
            return next((found for child in box.boxes if (found := find(child))), "")

        return find(section.root)

    def test_a_section_holds_the_steps_of_its_stage(self) -> None:
        for stage, parts in (
            ("galaxy", {"galaxy"}),
            ("stars", {"stars"}),
            ("planets", {"planets-systems", "planets-worlds"}),
        ):
            section = self.section(stage)
            steps = {key[2:] for key in section.items if key.startswith("n:")}
            self.assertEqual(
                steps, {n.id for n in self.pipe.nodes.values() if n.part in parts}, stage
            )
            self.assertEqual(section.nodes, len(steps))

    def test_an_input_from_another_stage_gets_a_connector_for_each_remote_step_and_cluster(
        self,
    ) -> None:
        section = self.section("stars")
        inputs = [
            key
            for key, item in section.items.items()
            if item.look == "connector" and key.startswith("in:")
        ]
        self.assertEqual(
            inputs,
            [
                "in:g.system@n:s.stream",
                "in:g.imf@c:s.primary",
                "in:g.sfh@c:s.primary",
                "in:g.metallicity@c:s.evolution",
            ],
        )
        self.assertEqual(self.box_of(section, "in:g.imf@c:s.primary"), "cluster:s.primary")
        label = section.items["in:g.imf@c:s.primary"].label
        self.assertIn("from Galactic Properties &amp; Megastructures:", label)
        self.assertIn("<B>G9  Initial mass function</B>", label)

    def test_a_steps_edges_to_another_stage_end_at_one_connector_listing_them(self) -> None:
        section = self.section("stars")
        outputs = [key for key, item in section.items.items() if key.startswith("out:")]
        self.assertEqual(
            outputs, ["out:s.system@planets", "out:s.star@planets", "out:s.flare@planets"]
        )
        label = section.items["out:s.star@planets"].label
        for target in (
            "to Planets:",
            "P2  Disc mass",
            "P3  Snow line",
            "P10  Equilibrium temperature",
        ):
            self.assertIn(target, label)
        self.assertEqual(
            [link.label for link in section.links if link.dst == "out:s.star@planets"], [""]
        )
        self.assertEqual(section.connectors, 7)
        # One edge for each kind: galaxy's metallicity feeds stars' tracks as data, and so on, while
        # the bar's density leaves only as data; a kind is never merged away.
        galaxy = self.section("galaxy")
        kinds = {
            (link.src, link.dst): link.kind for link in galaxy.links if link.dst.startswith("out:")
        }
        self.assertEqual(kinds[("n:g.sfh", "out:g.sfh@stars")], "lookup")
        self.assertEqual(kinds[("n:g.system", "out:g.system@stars")], "seed")
        many = [(f"P{n}", self.pipe.nodes["pw.teq"]) for n in range(7)]
        self.assertIn("and 3 more", pf.outputs_label("planets", many))

    def test_a_cut_edge_starts_at_a_stub_beside_its_consumer(self) -> None:
        section = self.section("galaxy", pf.hub_cuts(self.pipe, "galaxy"))
        stubs = [key for key, item in section.items.items() if item.look == "stub"]
        self.assertEqual(
            stubs,
            [
                "ref:g.params@c:g.structure",
                "ref:g.params@c:g.fields",
                "ref:g.params@c:g.population",
            ],
        )
        self.assertEqual(self.box_of(section, "ref:g.params@c:g.fields"), "cluster:g.fields")
        self.assertEqual(
            sorted(link.dst for link in section.links if link.src == "ref:g.params@c:g.structure"),
            ["n:g.arms", "n:g.potential"],
        )
        # The hub says where its stubs are, so that it can be followed forward.
        self.assertIn("↻ stubs at G4 · G5 · G6 · G7 · G8", section.items["n:g.params"].label)
        stub = section.items["ref:g.params@c:g.structure"]
        self.assertIn("↻ G2", stub.label)
        self.assertIn("Galaxy parameters", stub.label)
        self.assertEqual(stub.family, "inputs")
        self.assertFalse(
            any(link.src == "n:g.params" and link.dst == "n:g.potential" for link in section.links)
        )
        self.assertEqual(section.stubs, 3)

    def test_every_edge_is_drawn_whatever_is_cut(self) -> None:
        for stage in pf.STAGE_NAMES:
            everything = set(range(len(self.pipe.edges)))
            for cuts in (set(), pf.hub_cuts(self.pipe, stage), everything):
                self.assertEqual(
                    undrawn(self.pipe, self.section(stage, cuts)), [], (stage, len(cuts))
                )

    def test_every_edge_of_the_real_data_is_drawn(self) -> None:
        pipe, found = pf.load(DATA)
        if found or not pipe.nodes:
            self.skipTest(f"{pf.display_path(DATA)} does not validate yet")
        numbers = pf.step_numbers(pipe)
        for stage in pf.chart_stages(pipe, pf.COMPLETE):
            layout = pf.SectionLayout(pipe, numbers, stage, "standard", False, False, 600)
            for cuts in (layout.hubs, layout.hubs | layout.between):
                section = pf.build_section(pipe, numbers, stage, "standard", False, cuts, False)
                self.assertEqual(undrawn(pipe, section), [], stage)

    def test_backslashes_reach_the_chart_as_written(self) -> None:
        # Graphviz reads \N, \G and \E in HTML-like labels and in tooltips as names.
        self.assertEqual(pf.esc("C:\\New\\Graph"), "C:\\\\New\\\\Graph")
        self.assertEqual(pf.tooltip("a\\N"), "a\\\\N")
        self.assertEqual(pf.quote(pf.tooltip("a\\N")), '"a\\\\\\\\N"')

    def test_feedback_is_a_constraint_that_closes_a_cycle(self) -> None:
        links = {(link.src, link.dst): link for link in self.section("planets").links}
        self.assertTrue(links[("n:ps.stable", "n:ps.spacing")].feedback)  # redraw until Hill-stable
        self.assertFalse(
            links[("n:pw.atmosphere", "n:pw.surface")].feedback
        )  # a constraint, forward
        galaxy = {(link.src, link.dst): link for link in self.section("galaxy").links}
        self.assertFalse(galaxy[("n:g.potential", "n:g.density")].feedback)

    def test_bands_rank_below_one_another_and_hang_from_their_anchors(self) -> None:
        order = pf.flow_order(self.pipe, "galaxy")
        bands = [order[:2], order[2:]]
        section = self.section("galaxy", bands=bands)
        self.assertEqual(
            [key for key, item in section.items.items() if item.look == "gate"][:2],
            ["gate:galaxy:0", "anchor:cluster:g.root"],
        )
        # A band's anchors are ranked together, one for each unit, in its box, in the band's order.
        self.assertEqual(
            section.aligned[0], ["anchor:cluster:g.root", "anchor:cluster:g.structure"]
        )
        self.assertEqual(self.box_of(section, "anchor:cluster:g.structure"), "cluster:g.structure")
        into = [link for link in section.links if link.dst == "gate:galaxy:1"]
        out = [link for link in section.links if link.src == "gate:galaxy:1"]
        self.assertTrue(into and out)
        self.assertTrue(all(link.invisible and link.weight == 0 for link in into + out))
        self.assertEqual({link.dst for link in out}, set(section.aligned[1]))
        # An anchor pulls its unit's first steps, references among them, up to the band's top edge.
        pulled = {link.dst for link in section.links if link.src == "anchor:cluster:g.mega"}
        self.assertIn("in:pw.life@c:g.mega", pulled)
        self.assertTrue(
            all(link.weight == 1 for link in section.links if link.src.startswith("anchor:"))
        )

    def test_the_complete_charts_sections_are_each_in_a_stage_box(self) -> None:
        section = self.section("planets", boxed=True, detail="full")
        self.assertEqual([box.key for box in section.root.boxes], ["cluster_stage_planets"])
        self.assertEqual(
            [box.label for box in section.root.boxes[0].boxes],
            [
                "Protoplanetary disc",
                "Architecture",
                "Bulk properties",
                "Atmosphere",
                "Surface & life",
            ],
        )

    def test_detail_levels(self) -> None:
        imf = self.pipe.nodes["g.imf"]
        brief, standard, full = (pf.node_label(imf, level, False, "G9") for level in pf.DETAILS)
        for label in (brief, standard, full):
            self.assertIn("<B>Initial mass function</B>", label)
            self.assertIn("Broken power law, 0.08–150 M☉", label)
            self.assertIn("<B>G9</B>", label)
        self.assertNotIn("inverse CDF", brief)
        self.assertIn("inverse CDF", standard)
        self.assertNotIn("Kroupa", standard)
        self.assertNotIn("imf.rs", standard)
        self.assertIn("<I>Kroupa 2001</I>", full)
        self.assertIn('FACE="DejaVu Sans Mono', full)
        self.assertIn("crates/hyperion-sim/src/galaxy/imf.rs", full)
        self.assertIn("g.imf", full)
        self.assertEqual(pf.DEFAULT_DETAIL[pf.COMPLETE], "full")
        self.assertEqual(pf.DEFAULT_DETAIL["stars"], "standard")

    def test_status_shows_in_the_label_as_well_as_the_outline(self) -> None:
        self.assertIn("◐ PARTIAL", pf.node_label(self.pipe.nodes["g.arms"], "brief", False))
        self.assertIn("○ PLANNED", pf.node_label(self.pipe.nodes["g.dyson"], "brief", False))
        self.assertNotIn("BUILT", pf.node_label(self.pipe.nodes["g.imf"], "brief", False))

    def test_label_text_is_escaped(self) -> None:
        tricky = pf.Node("g.x", 'A & B <i> "C"', "model", "x < y > z & w", "built", "galaxy")
        label = pf.node_label(tricky, "brief", False)
        self.assertIn("A &amp; B &lt;i&gt; &quot;C&quot;", label)
        self.assertIn("x &lt; y &gt; z &amp; w", label)
        self.assertNotIn("<i>", label)
        self.assertEqual(pf.esc('&<>"'), "&amp;&lt;&gt;&quot;")
        self.assertIn("A &amp; B &lt;i&gt;", pf.stub_label("G1", tricky))

    def test_the_subtitle_counts_without_a_timestamp(self) -> None:
        self.assertEqual(
            pf.subtitle(self.pipe, "stars", [self.section("stars")], "standard"),
            "HYPERION  ·  snapshot a6d3f6dd  ·  14 nodes: 11 built, 2 partial, 1 planned  ·  "
            "7 connectors to other stages, 0 reference stubs  ·  detail: standard",
        )
        sections = [self.section(stage, detail="full", boxed=True) for stage in pf.STAGE_NAMES]
        self.assertIn(
            "snapshots a6d3f6dd (galaxy, stars, planets-systems), c3b34c96 (planets-worlds)",
            pf.subtitle(self.pipe, pf.COMPLETE, sections, "full"),
        )

    def test_placeholder_stubs_sit_beside_their_steps_when_allowed(self) -> None:
        pipe = pf.load(PLACEHOLDERS)[0]
        numbers = pf.step_numbers(pipe)
        stars = pf.build_section(pipe, numbers, "stars", "standard", True, set(), False)
        stubs = {
            key: item.tooltip for key, item in stars.items.items() if item.look == "placeholder"
        }
        self.assertEqual(
            stubs,
            {
                "ph:1@n:s.track": "@galaxy:local [Fe/H] at position",
                "ph:2@n:s.star": "@planets-systems:disc mass draw",
            },
        )
        galaxy = pf.build_section(pipe, numbers, "galaxy", "standard", True, set(), False)
        self.assertIn("bulge &amp; bar density", galaxy.items["ph:1@n:g.system"].label)
        self.assertEqual(galaxy.placeholders, 1)
        refused = pf.build_section(pipe, numbers, "stars", "standard", False, set(), False)
        self.assertFalse(any(item.look == "placeholder" for item in refused.items.values()))

    def test_the_legend_shows_every_kind_status_reference_and_edge_kind(self) -> None:
        legend, items, links = pf.build_legend("stars", "standard", False)
        for kind in pf.NODE_KINDS:
            self.assertEqual(items[f"legend:k:{kind}"].look, kind)
        for status in pf.STATUSES:
            self.assertEqual(items[f"legend:s:{status}"].status, status)
        for kind in pf.EDGE_KINDS:
            self.assertIn(
                pf.Link(
                    f"legend:e:{kind}", f"legend:e:{kind}:meaning", kind, "", minlen=3, weight=10
                ),
                links,
            )
        self.assertEqual(items["legend:r:stub"].look, "stub")
        self.assertEqual(items["legend:r:connector"].look, "connector")
        self.assertNotIn("legend:r:placeholder", legend.items)
        self.assertIn("legend:r:placeholder", pf.build_legend("stars", "standard", True)[0].items)


class Dot(unittest.TestCase):
    """The DOT written for Graphviz, and the canvas the pieces are stacked on."""

    pipe: pf.Pipeline

    @classmethod
    def setUpClass(cls) -> None:
        cls.pipe = pf.load(VALID)[0]

    def dot(self, stage: str) -> str:
        section = pf.build_section(
            self.pipe,
            pf.step_numbers(self.pipe),
            stage,
            "standard",
            False,
            pf.hub_cuts(self.pipe, stage),
            False,
        )
        return pf.emit_section(
            section, {key: (120.0, 40.0) for key in pf.fitted_labels(section.items)}
        )

    def test_the_same_data_writes_the_same_dot(self) -> None:
        for stage in pf.STAGE_NAMES:
            self.assertEqual(self.dot(stage), self.dot(stage))

    def test_feedback_is_written_backwards_never_unconstrained(self) -> None:
        text = self.dot("planets")
        self.assertNotIn("constraint=false", text)
        line = next(
            line
            for line in text.splitlines()
            if line.startswith('  "n:ps.spacing" -> "n:ps.stable"') and "dir=" in line
        )
        self.assertIn('dir="back"', line)
        self.assertIn('arrowtail="tee"', line)
        self.assertIn('arrowhead="none"', line)
        self.assertIn("weight=0", line)

    def test_direct_edges_carry_their_index_for_the_long_edge_pass(self) -> None:
        text = self.dot("galaxy")
        index = next(
            i
            for i, edge in enumerate(self.pipe.edges)
            if (edge.src, edge.dst) == ("g.seed", "g.arms")
        )
        self.assertIn(f"pf_edge={index}", text)
        self.assertIn('"ref:g.params@c:g.structure" -> "n:g.potential"', text)
        self.assertIn('shape="plain", label=<<TABLE STYLE="rounded"', text)

    def test_quoting(self) -> None:
        self.assertEqual(pf.quote('a"b\\c'), '"a\\"b\\\\c"')
        self.assertEqual(
            pf.attrs(
                {"label": pf.Html("<B>x</B>"), "width": 1.5, "fixedsize": True, "shape": "box"}
            ),
            '[label=<<B>x</B>>, width=1.5, fixedsize=true, shape="box"]',
        )
        self.assertEqual(pf.attrs({"tooltip": "<not markup>"}), '[tooltip="<not markup>"]')

    def test_fitted_shapes_hold_their_labels(self) -> None:
        for width, height in ((60.0, 14.0), (180.0, 40.0), (330.0, 110.0), (240.0, 160.0)):
            for rule, inside in (
                ("ellipse", in_ellipse),
                ("diamond", in_diamond),
                ("parallelogram", in_parallelogram),
                ("pentagon", in_pentagon),
            ):
                size = pf.fit_shape(rule, width, height)
                outer = (float(size["width"]) * 72, float(size["height"]) * 72)
                self.assertTrue(size["fixedsize"])
                for x, y in corners(width, height):
                    self.assertTrue(
                        inside(x, y, outer, size),
                        f"{rule} {width}x{height}: ({x}, {y}) outside {outer}",
                    )
        self.assertLessEqual(float(pf.fit_shape("parallelogram", 20.0, 200.0)["skew"]), 0.5)

    def test_png_resolution_is_capped_for_large_charts(self) -> None:
        self.assertEqual(pf.png_dpi('graph [bb="0,0,720,360"];', 96), 96)
        dpi = pf.png_dpi(b'graph [bb="0,0,38322,16152"];', 96)
        width, height = 38322 / 72 * dpi, 16152 / 72 * dpi
        self.assertLessEqual(max(width, height), pf.PNG_MAX_SIDE)
        self.assertLessEqual(width * height, pf.PNG_MAX_PIXELS)
        self.assertGreater(width * height, 0.9 * pf.PNG_MAX_PIXELS)

    def test_translation_moves_every_position(self) -> None:
        text = 'graph [bb="0,0,100,50", lp="50,10"]; "a" [pos="10,20"]; "a" -> "b" [pos="e,1,2 3.5,4", xlp="5,6"];'
        self.assertEqual(
            pf.translate(text, 10, 100),
            'graph [bb="10.00,100.00,110.00,150.00", lp="60.00,110.00"]; "a" [pos="20.00,120.00"]; '
            '"a" -> "b" [pos="e,11.00,102.00 13.50,104.00", xlp="15.00,106.00"];',
        )

    def test_pieces_stack_top_to_bottom(self) -> None:
        title = 'digraph t {\n\tgraph [bb="0,0,200,40"];\n\t"title"\t[pos="100,20"];\n}\n'
        body = 'digraph s {\n\tgraph [bb="0,0,600,300"];\n\t"n:a"\t[pos="300,150"];\n}\n'
        canvas = pf.compose("x", [pf.Piece(title, "left"), pf.Piece(body, "centre")])
        self.assertEqual(pf.bounds(canvas), (0.0, 0.0, 600.0, 340 + pf.GAP))
        self.assertIn('"title"\t[pos="100.00,', canvas)
        self.assertIn('"n:a"\t[pos="300.00,150.00"]', canvas)
        self.assertEqual(canvas.count("subgraph"), 2)
        self.assertAlmostEqual(pf.stacked_aspect([title, body]), 600 / 340)

    def test_the_canvas_draws_labels_above_every_edge_and_drops_invisible_edges(self) -> None:
        label = pf.link_attrs(pf.Link("n:a", "n:b", "data", "[Fe/H] at site"))["label"]
        body = (
            'digraph s {\n\tgraph [bb="0,0,600,300"];\n\t"n:a"\t[pos="300,250"];\n\t"n:b"\t[pos="300,50"];\n'
            f'\t"n:a" -> "n:b"\t[color="#52514e",\n\t\tlabel=<{label}>,\n\t\tlp="330,150",\n\t\tpos="e,300,70 300,230"];\n'
            '\t"n:a" -> "gate:s:1"\t[pos="e,900,0 300,230",\n\t\tstyle=invis];\n}\n'
        )
        canvas = pf.compose("x", [pf.Piece(body, "centre")])
        head, first = canvas.splitlines()[1:3]
        self.assertIn('outputorder="edgesfirst"', head)  # the root's: Graphviz ignores a subgraph's
        # The label is a node of its own, declared before every step, so drawn after every edge.
        self.assertTrue(first.startswith('  "label 1: n:a -> n:b" [shape="plain"'), first)
        self.assertIn('pos="330.00,150.00"', first)
        self.assertIn("[Fe/H] at site", first)
        self.assertEqual(canvas.count("[Fe/H] at site"), 1)
        self.assertNotIn("lp=", canvas)
        self.assertIn('"n:a" -> "n:b"\t[color="#52514e",', canvas)
        self.assertNotIn("gate:s:1", canvas)


def corners(width: float, height: float) -> Iterable[tuple[float, float]]:
    return ((sx * width / 2, sy * height / 2) for sx in (-1, 1) for sy in (-1, 1))


def in_ellipse(x: float, y: float, outer: tuple[float, float], _: dict) -> bool:
    return (x / (outer[0] / 2)) ** 2 + (y / (outer[1] / 2)) ** 2 <= 1


def in_diamond(x: float, y: float, outer: tuple[float, float], _: dict) -> bool:
    return abs(x) / (outer[0] / 2) + abs(y) / (outer[1] / 2) <= 1


def in_parallelogram(x: float, y: float, outer: tuple[float, float], size: dict) -> bool:
    # The top edge shifted right by d = W·s / (2·√(1 + s²) + s) (measured from Graphviz 16).
    skew = float(size["skew"])
    width, height = outer
    d = width * skew / (2 * (1 + skew * skew) ** 0.5 + skew)
    t = (y + height / 2) / height  # 0 at the bottom, 1 at the top
    left, right = -width / 2 + d * t, width / 2 - d * (1 - t)
    return left <= x <= right and abs(y) <= height / 2


def in_pentagon(x: float, y: float, outer: tuple[float, float], _: dict) -> bool:
    # `invhouse`: the rectangle from 0.0955 H to 0.6545 H below the top, then the point (Graphviz 16).
    width, height = outer
    below_top = height / 2 - y
    if below_top < 0.0955 * height:
        return False
    if below_top <= 0.6545 * height:
        return abs(x) <= width / 2
    return abs(x) <= width / 2 * (height - below_top) / (0.3455 * height)


class Palette(unittest.TestCase):
    """The colours: documented, legible, and never the only cue."""

    def fills(self) -> dict:
        fills = {f"{f.name} built": f.fill for f in pf.FAMILIES.values()}
        fills.update({f"{f.name} planned": f.planned_fill for f in pf.FAMILIES.values()})
        fills.update(
            {
                "connector": pf.NEUTRAL,
                "placeholder": pf.mix(pf.SERIOUS, pf.SURFACE, 0.14),
                "surface": pf.SURFACE,
            }
        )
        fills.update(
            {
                f"cluster {depth}": pf.mix(pf.HAIRLINE, pf.SURFACE, amount)
                for depth, amount in enumerate(pf.CLUSTER_FILLS)
            }
        )
        return fills

    def test_every_hue_is_a_documented_slot(self) -> None:
        used = {family.hue for family in pf.FAMILIES.values()}
        used |= {style.colour for style in pf.EDGE_KINDS.values()}
        used |= {
            pf.SURFACE,
            pf.INK,
            pf.INK_SECONDARY,
            pf.MUTED,
            pf.HAIRLINE,
            pf.BASELINE,
            pf.NEUTRAL,
            pf.SERIOUS,
        }
        self.assertLessEqual(used, DOCUMENTED)

    def test_text_reads_at_4_5_to_1_on_every_fill(self) -> None:
        for name, fill in self.fills().items():
            for ink in (pf.INK, pf.INK_SECONDARY):
                self.assertGreaterEqual(contrast(ink, fill), 4.5, f"{ink} on {name} ({fill})")

    def test_the_families_have_distinct_hues_within_the_categorical_cap(self) -> None:
        hues = [family.hue for family in pf.FAMILIES.values()]
        self.assertEqual(len(set(hues)), len(hues))
        self.assertLessEqual(len(hues), 8)
        self.assertNotIn(pf.EDGE_KINDS["constraint"].colour, hues)
        self.assertEqual(set(pf.FAMILIES), {style.family for style in pf.NODE_KINDS.values()})

    def test_each_kind_has_a_shape_of_its_own(self) -> None:
        shapes = [(style.shape, style.rounded) for style in pf.NODE_KINDS.values()]
        shapes += [(pf.look_style("connector")[0], False)]
        self.assertEqual(len(set(shapes)), len(shapes))

    def test_each_edge_kind_differs_in_line_colour_and_arrowhead(self) -> None:
        styles = list(pf.EDGE_KINDS.values())
        for attribute in ("style", "colour", "arrowhead"):
            values = [getattr(style, attribute) for style in styles]
            self.assertEqual(len(set(values)), len(values), attribute)

    def test_mix(self) -> None:
        self.assertEqual(pf.mix("#000000", "#ffffff", 0.5), "#808080")
        self.assertEqual(pf.mix("#2a78d6", "#fcfcfb", 0), "#fcfcfb")
        self.assertEqual(pf.mix("#2a78d6", "#fcfcfb", 1), "#2a78d6")


@unittest.skipUnless(GRAPHVIZ, "Graphviz's dot and neato are not installed")
class Rendering(unittest.TestCase):
    """The charts laid out and drawn by Graphviz, as `just flowcharts` draws them."""

    def draw(self, out: Path, *extra: str) -> tuple[int, str]:
        status, _, err = run(["--data", str(VALID), "--out", str(out), *extra])
        return status, err

    def test_the_same_data_draws_byte_identical_files(self) -> None:
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
            for out in (a, b):
                status, err = self.draw(
                    Path(out), "--format", "svg,pdf", "--keep-dot", "--only", "stars,complete"
                )
                self.assertEqual(status, 0, err)
            names = sorted(path.name for path in Path(a).iterdir())
            self.assertEqual(
                names,
                [
                    "complete.dot",
                    "complete.pdf",
                    "complete.svg",
                    "stars.dot",
                    "stars.pdf",
                    "stars.svg",
                ],
            )
            for name in names:
                self.assertEqual((Path(a) / name).read_bytes(), (Path(b) / name).read_bytes(), name)
            # The kept DOT is the positioned canvas, drawn as it is.
            canvas = (Path(a) / "stars.dot").read_text(encoding="utf-8")
            self.assertNotIn("\\\n", canvas)
            self.assertEqual(
                pf.run_graphviz(["neato", "-n2", "-Tsvg"], canvas, 60),
                (Path(a) / "stars.svg").read_bytes(),
            )

    def test_only_and_format_choose_what_is_written(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            status, err = self.draw(
                Path(tmp), "--only", "galaxy", "--format", "png", "--detail", "brief"
            )
            self.assertEqual(status, 0, err)
            self.assertEqual([path.name for path in Path(tmp).iterdir()], ["galaxy.png"])
            self.assertEqual((Path(tmp) / "galaxy.png").read_bytes()[:8], b"\x89PNG\r\n\x1a\n")
            self.assertIn("galaxy: 16 nodes, 28 edges (", err)

    def test_the_svg_carries_the_text_with_its_glyphs_and_references(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            status, err = self.draw(Path(tmp), "--only", "planets")
            self.assertEqual(status, 0, err)
            svg = (Path(tmp) / "planets.svg").read_text(encoding="utf-8")
        self.assertIn("Procedural Generation Flowchart — Planets", svg)
        self.assertIn("Does it keep an atmosphere at t?", svg)
        self.assertIn("R ∝ M^0.28 up to 2 M⊕", svg)
        self.assertIn("from Star Systems:", svg)
        self.assertRegex(
            svg, r"S11(\s|&#160;)+Star record"
        )  # its two spaces, however the SVG writes them
        self.assertIn('font-family="DejaVu Sans,Verdana,sans-serif"', svg)
        self.assertNotIn("Times", svg)
        # Drawn in order: every edge, then every edge's label, then the steps.
        groups = re.findall(r'<g id="[^"]+" class="(edge|node)">\n<title>([^<]*)</title>', svg)
        kinds = [
            "edge" if kind == "edge" else "label" if title.startswith("label ") else "node"
            for kind, title in groups
        ]
        self.assertIn("label", kinds)
        self.assertEqual(kinds, sorted(kinds, key=["edge", "label", "node"].index))

    def test_the_complete_chart_stacks_its_stages_in_one_column(self) -> None:
        pipe = pf.load(VALID)[0]
        layout = pf.lay_out_chart(pipe, pf.COMPLETE, "full", False)
        assert layout is not None
        canvas = layout.positioned
        tops = [
            pf.bounds(canvas[canvas.index(f"cluster_stage_{stage}") :])[3]
            for stage in pf.STAGE_NAMES
        ]
        self.assertEqual(tops, sorted(tops, reverse=True), "galaxy above stars above planets")
        x0, y0, x1, y1 = pf.bounds(canvas)
        self.assertLess((x1 - x0) / (y1 - y0), 2.5)
        self.assertEqual([section.stage for section in layout.chart.sections], list(pf.STAGE_NAMES))

    def test_backslashes_are_drawn_as_written(self) -> None:
        with tempfile.TemporaryDirectory() as data, tempfile.TemporaryDirectory() as out:
            write_files(
                Path(data),
                {
                    "galaxy.toml": META
                    + node(
                        "g.a",
                        "C:\\\\New\\\\Graph \\\\E",
                        extra=('source = ["C:\\\\path\\\\to\\\\"]\n'),
                    )
                    + node("g.b")
                    + '[[edge]]\nfrom = "g.a"\nto = "g.b"\n'
                },
            )
            status, _, err = run(["--data", data, "--out", out, "--only", "galaxy"])
            self.assertEqual(status, 0, err)
            svg = (Path(out) / "galaxy.svg").read_text(encoding="utf-8")
        self.assertIn(">C:\\New\\Graph \\E</text>", svg)
        self.assertIn("source: C:\\path\\to\\", svg)

    def test_problems_draw_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            status, _, err = run(["--data", str(INVALID), "--out", tmp])
            self.assertEqual(status, 1)
            self.assertTrue(err.rstrip().endswith("nothing drawn"))
            self.assertEqual(list(Path(tmp).iterdir()), [])

    def test_placeholders_draw_as_stubs_when_allowed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            status, _, err = run(
                ["--data", str(PLACEHOLDERS), "--out", tmp, "--allow-placeholders"]
            )
            self.assertEqual(status, 0, err)
            self.assertIn("planets: no data files for this chart: skipped", err)
            svg = (Path(tmp) / "stars.svg").read_text(encoding="utf-8")
        self.assertIn("⚠ UNRESOLVED", svg)
        self.assertIn("local [Fe/H] at position", svg)


class CommandLine(unittest.TestCase):
    def test_what_cannot_be_done_exits_2(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            afile = Path(tmp) / "afile"
            afile.write_text("", encoding="utf-8")
            empty = Path(tmp) / "empty"
            empty.mkdir()
            for argv in (
                ["--data", str(VALID), "--out", str(afile)],
                ["--check", "--data", str(empty)],
                ["--check", "--data", str(afile)],
            ):
                with self.subTest(argv=argv):
                    status, _, err = run(argv)
                    self.assertEqual(status, 2, err)
                    self.assertTrue(err.startswith("error: "), err)
                    self.assertNotIn("Traceback", err)
        for argv in (["--dpi", "0"], ["--dpi", "-96"], ["--timeout", "0"], ["--timeout", "nan"]):
            with self.subTest(argv=argv), contextlib.redirect_stderr(io.StringIO()) as stderr:
                with self.assertRaises(SystemExit) as raised:
                    pf.parse_args(argv)
                self.assertEqual(raised.exception.code, 2)
                self.assertIn("must be a positive number", stderr.getvalue())

    def test_unknown_charts_and_formats_are_refused(self) -> None:
        for argv in (["--only", "moons"], ["--format", "gif"], ["--detail", "most"]):
            with self.subTest(argv=argv), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as raised:
                    pf.parse_args(argv)
                self.assertEqual(raised.exception.code, 2)

    def test_charts_and_formats_come_in_a_fixed_order(self) -> None:
        args = pf.parse_args(["--only", "complete,galaxy", "--format", "pdf,svg"])
        self.assertEqual((args.only, args.format), (["galaxy", "complete"], ["svg", "pdf"]))

    def test_the_default_output_is_gitignored(self) -> None:
        self.assertEqual(pf.OUT_DIR.relative_to(pf.ROOT).parts[0], "out")
        self.assertIn("out/", (pf.ROOT / ".gitignore").read_text(encoding="utf-8").splitlines())


if __name__ == "__main__":
    unittest.main(verbosity=1)
