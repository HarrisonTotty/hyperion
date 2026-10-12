# Procedural-generation flowcharts

`procgen_flowcharts.py` draws HYPERION's procedural-generation pipeline as flowcharts: how a `u64`
seed and the galaxy parameters become a galaxy, its stars, planets, moons, atmospheres, surfaces and
life. The pipeline is described as data in `data/*.toml`, the single source of truth, written to the
schema in [`data/SCHEMA.md`](data/SCHEMA.md). The script validates every file, then lays the charts
out with Graphviz.

## The charts

| `--only`   | Title                                                                  | Holds                                                    | Detail   |
| ---------- | ---------------------------------------------------------------------- | -------------------------------------------------------- | -------- |
| `galaxy`   | Procedural Generation Flowchart — Galactic Properties & Megastructures | the files whose `meta.stage` is `galaxy`                 | standard |
| `stars`    | Procedural Generation Flowchart — Star Systems                         | `stage = "stars"`                                        | standard |
| `planets`  | Procedural Generation Flowchart — Planets                              | `stage = "planets"` (planets-systems and planets-worlds) | standard |
| `complete` | Procedural Generation Flowchart — Complete Pipeline                    | every stage's section, stacked in one column             | full     |

Every step is numbered, in every chart alike: G12 is the twelfth step of the galaxy stage in the
files' order, and S and P number the star-system and planet stages. An edge that would run across
the chart is drawn instead from a **reference** beside the step it feeds:

- **An off-page connector** for a step in another stage, such as "from _Star Systems_: _S7 Primary
  mass_". There is one connector for each remote step and each cluster that uses it, placed in that
  cluster. A step's edges to another stage end at one connector, "to _Planets_:", that lists the
  steps it feeds.
- **A stub** for a step of the same stage, such as "↻ G2 Galaxy parameters". These come from a hub,
  meaning a step feeding five edges or more, when its edge goes into another cluster, and from any
  edge that would still run far once laid out.

To trace a reference, find the step with its number. The complete chart stacks the three stages'
sections in pipeline order, its cross-stage edges drawn as connector pairs that name each other's
numbers.

Detail levels (`--detail` overrides the default):

- `brief`: each node's title and summary.
- `standard`: adds its detail lines. This is the default for stage charts.
- `full`: adds its citations, its sources and its id. This is the default for the complete chart.

Each chart has a title block and a legend. The title block's subtitle gives the data's snapshot
commit (from `meta.snapshot`), the node count with its built, partial and planned tallies, and the
numbers of connectors and stubs. The legend shows every node kind, the statuses and step numbers, the
references (stub and connector), the boxes (a subsystem, a box in a box, and in the complete chart a
stage), and the edge kinds, each drawn as the chart draws it.

## Usage

```sh
just flowcharts                                  # every chart, SVG, into out/procgen-flowcharts/
just flowcharts --check                          # validate only; exit 1 on any problem
just flowcharts --only stars,complete --format svg,png,pdf
just flowcharts --allow-placeholders --keep-dot  # preview data still being reconciled
just flowcharts --out docs/diagrams/procgen      # anywhere else
```

| Option                 | Effect                                                                          |
| ---------------------- | ------------------------------------------------------------------------------- |
| `--check`              | validate and draw nothing                                                       |
| `--only CHARTS`        | some of `galaxy,stars,planets,complete` (default all)                           |
| `--format FORMATS`     | some of `svg,png,pdf` (default `svg`)                                           |
| `--detail LEVEL`       | `brief`, `standard` or `full`, for every chart drawn                            |
| `--out DIR`            | where to write (default `out/procgen-flowcharts/`)                              |
| `--data DIR`           | the TOML files (default `data/` beside the script)                              |
| `--keep-dot`           | also write each chart's laid-out `.dot`, which `neato -n2 -Tsvg` draws as it is |
| `--allow-placeholders` | draw unresolved `@part:...` references as marked stubs, rather than refusing    |
| `--dpi DPI`            | PNG resolution (default 96), lowered automatically for large charts             |
| `--timeout SECONDS`    | the limit for each Graphviz run (default 600)                                   |

**Requirements.** uv, which runs the tool in the Python 3.14 that `pyproject.toml` asks for and
fetches it where the machine's `python3` is older (macOS's lacks `tomllib`, standard since 3.11). At
run time the tool needs only the standard library; `mypy` and `ruff` are its dev dependencies (see
"Tests"). Graphviz (`dot`
and `neato`, tested with 16.1). The DejaVu Sans and DejaVu Sans Mono fonts. `fc-match` and
`fc-query` are used, when present, to warn about characters the font lacks.

**Output.** Each chart is written as `<chart>.<format>`. The default directory is `out/`, which is
gitignored, for three reasons:

- The charts are derived from the TOML, so a committed copy would drift from it, and nothing in `ci`
  would redraw or check it.
- The complete chart's SVG is over a megabyte, and every PNG is several, while pre-commit's
  `check-added-large-files` allows 500 KB.
- smart-ci knows no rule for non-Markdown files under `docs/`, so committing a chart would run the
  full `ci`.

Pass `--out` to put the charts elsewhere.

**Formats.** SVG and PDF are the formats to read the large charts in, since they zoom. `dot` lays
each chart out (see "Layout" below), and `neato -n2` then renders every format from that one layout.
A PNG keeps to
16,000 px a side and 60 megapixels: its resolution drops below `--dpi` where the chart is larger.
That makes the complete chart's PNG an overview, not a page to read. A PDF over 200 inches a side,
the limit viewers accept, is scaled to fit.

**Byte-identical output.** The same data draws the same files. Nothing depends on the time, every
statement is written in the files' order, and PDFs are stamped with `SOURCE_DATE_EPOCH=0`.

**Timings.** Measured on the reconciled data of 2026-10-12 (310 nodes and 659 edges) on a quiet
machine:

- The complete chart takes 2.8 s, from reading the data to writing its SVG.
- All four charts as SVG, PNG and PDF take 20 s in all, most of it drawing the PNGs.
- A stage's layout, every pass of it, takes 1 to 3 s. Each section runs its own `dot`, in parallel.

## The visual language

Every cue is carried twice. A node's kind is told by its shape, its colour and the kind written
above its title. Its status is told by its outline and fill and by a marker beside the kind. An
edge's kind is told by its line, its colour and its arrowhead. Text is always drawn in ink, never in
a hue.

### Reading a node

```
G9  ·  DRAW  ·  ◐ PARTIAL  ·  g.imf     number, kind, status (unless built), id (full detail)
Initial mass function                   title (bold, at most 32 characters)
Broken power law, 0.08–150 M☉           summary (at most 60 characters)
α = 1.3 below 0.5 M☉, 2.3 above         detail lines (standard and full; at most 6 of 60)
Kroupa 2001                             citations (italic; full)
crates/hyperion-sim/src/galaxy/imf.rs   sources (monospace; full)
```

Hovering over a node in an SVG shows its id, kind, status and sources.

### Node kinds: shape for the kind, colour for its family

The shapes follow ISO 5807's flowchart symbols where it has one. The colours group the kinds into
five families: a hue for each family, a shape for each kind. Graphviz would inflate the oval,
diamond, parallelogram and pentagon around a label, so the script sizes those itself, from the
label's measured size.

| Kind       | Shape                      | Family     | Hue    | Meaning                                       |
| ---------- | -------------------------- | ---------- | ------ | --------------------------------------------- |
| `seed`     | oval (ISO terminal)        | randomness | red    | the root seed, a derived key or an RNG stream |
| `draw`     | rounded rectangle          | randomness | red    | a stochastic step that consumes stream words  |
| `param`    | parallelogram (ISO data)   | inputs     | green  | a parameter, setting or constant set          |
| `table`    | cylinder (ISO stored data) | inputs     | green  | fitted or tabulated data                      |
| `model`    | rectangle (ISO process)    | derivation | blue   | a deterministic physics or derivation step    |
| `decision` | diamond (ISO decision)     | derivation | blue   | a branch; its edges' labels name the cases    |
| `entity`   | 3-D box                    | products   | yellow | a generated record or object                  |
| `query`    | pennant (arrow-ended box)  | runtime    | aqua   | a runtime access path, evaluated lazily       |
| `enrich`   | document (folded corner)   | runtime    | aqua   | LLM text enrichment                           |

Each node is filled with an 18% tint of its hue over the surface and outlined in the hue itself.

### Status

| Status    | Outline | Fill                                                            | Marker      |
| --------- | ------- | --------------------------------------------------------------- | ----------- |
| `built`   | solid   | the family's tint                                               | none        |
| `partial` | solid   | split down the middle: the tint on the left, muted on the right | `◐ PARTIAL` |
| `planned` | dashed  | muted: a 6% tint, nearly the surface                            | `○ PLANNED` |

### References

| Reference              | Shape                                 | Colour                                      | Text                                                      |
| ---------------------- | ------------------------------------- | ------------------------------------------- | --------------------------------------------------------- |
| stub                   | small rounded chip, one line          | the muted tint and hue of the step it names | "↻ G2 Galaxy parameters": the number and title there      |
| off-page connector     | pentagon pointing down (ISO off-page) | neutral gray, gray outline                  | "from _stage_:" or "to _stage_:", then numbers and titles |
| unresolved placeholder | rectangle, thick dotted outline       | the status palette's "serious" orange       | `⚠ UNRESOLVED · @part` and its description                |

A stub is drawn inside the cluster of the step it feeds, so its edge stays short, and it wears the
colours of the step it names. That step, in turn, says where its stubs are: "↻ stubs at G15 · G20",
in step order, so a hub can be followed forward as well as a stub back. An edge from a stub or an
input connector keeps its kind, colour and label. A step's edges to another stage reach its output
connector as one edge for each kind, labelled where every edge of that kind carries the same label.
The connector lists its targets: five at most, or four and "and N more" when there are six or more.

Placeholders are drawn only with `--allow-placeholders`. Each one is a fault in the data, so it
takes a status colour, which no kind uses.

### Edges

| Kind         | Line        | Colour    | Arrowhead         | Meaning                                     |
| ------------ | ----------- | --------- | ----------------- | ------------------------------------------- |
| `data`       | thin solid  | dark gray | filled triangle ▶ | a value flows from one step to the next     |
| `seed`       | dashed      | red       | open triangle ▷   | which key or stream feeds which random step |
| `lookup`     | dotted      | green     | open square □     | a step reads a table                        |
| `constraint` | thick solid | violet    | bar ⊣             | conditioning, bounds, rejection or feedback |

The `seed` and `lookup` edges take the hue of the family they come from. `constraint` takes a hue
no node uses. An edge's label sits on a patch of the surface, so that a line passing behind it does
not strike it through.

**Feedback.** A constraint that closes a cycle (redraw until stable) is drawn backwards for layout,
so it neither drags its target down nor stretches its cluster. It is written head to tail with
`dir=back` at weight 0, rather than with `constraint=false`, which makes dot fail with "trouble in
init_rank" on charts of this size.

### Layout

A chart is laid out in pieces, stacked top to bottom on one canvas: the title block, the legend,
then one section for each stage. Each section is its own `dot` graph, so a stage chart's section and
the complete chart's are laid out the same way, and the complete chart's sections take one width.
Each section is laid out in passes:

1. **Measure.** The section is laid out with every edge between top-level clusters cut, which
   measures each cluster on its own. A cluster here is a unit; a step in no cluster is a unit of its
   own.
2. **Pack.** The units are put in flow order, each after the units that feed it (cycles are broken
   by the files' order). They are packed left to right into bands no wider than a width chosen to
   make the section, or the complete chart's stack of sections, about 1.6 times as wide as it is tall.
3. **Lay out in bands.** Each unit has an invisible anchor at its top. A band's anchors share one
   rank (`rank=same`), chained left to right in flow order, and each pulls its unit's first steps up
   to it, so that the band's clusters hang from one top edge. An invisible gate ranks each band's
   anchors below every step of the band before. Edges between units stay drawn only from one band
   into the next. Edges within a band, edges that run back up, and edges that skip a band become
   stubs, as do a hub's edges into other clusters.
4. **Cut long edges.** Any direct edge whose ends lie more than 1,500 pt apart becomes a stub, and
   the section is laid out again, at most twice.
5. **Reshape.** If the result misses the shape aimed at by more than a quarter, the bands are widened
   or narrowed once and laid out again, and the closer result is kept.

The gates' edges weigh nothing, so they set ranks and pull nothing sideways: weighted edges also
steer dot's horizontal placement, and from one gate they pulled the bands into a staircase. An
anchor's pull, of weight 1, stays within its own box. A reference's edge to its step weighs 3, to
keep the stub beside the step. Feedback edges are counted in the direction they are written, head to
tail, when the first steps of a band are found; otherwise a step fed only by feedback would float
free of its band.

The pieces' coordinates are shifted onto one canvas, and `neato -n2` draws the result. Graphviz
breaks long strings in its DOT output with a backslash and a newline, even between the two numbers
of a point; these are joined before anything is moved. On the canvas each edge's label becomes a
node at its place, drawn after every edge and before every step (`outputorder=edgesfirst`, which
Graphviz reads from the root graph only), since a label drawn with its edge is struck through by
every edge drawn after it. The gates' invisible edges are dropped, since their splines widened the
canvas.

Settings: `splines=spline`, `nodesep=0.28`, `ranksep=0.5`, `newrank=true`, `mclimit=0.3`.
`splines=ortho` was not used: it is unreliable with clusters and HTML labels in dot, and slow at this
size. Graphviz's `newrank` keeps clusters whole but ignores `rank=min` and `rank=max`, which is why
the gates are invisible edges. `mclimit=0.3` runs fewer rounds of crossing reduction than the
default, which keeps more of the bands' flow order. On the 2026-10-12 data it put every pair of units
in flow order in the stars and planets stages, against 9 of 9 and 70 of 76 at the default.

The table gives the sizes on the real data of 2026-10-12. "Before" is a single graph with gathered
connector rows; "after" is this layout.

| Chart    | Before (in) | After (in) | Stubs | Connectors |
| -------- | ----------- | ---------- | ----- | ---------- |
| galaxy   | 219 × 69    | 102 × 66   | 70    | 24         |
| stars    | 162 × 73    | 90 × 66    | 42    | 31         |
| planets  | 171 × 163   | 143 × 93   | 121   | 27         |
| complete | 601 × 365   | 228 × 173  | 239   | 82         |

### Colour

The hues and the chrome are the data-viz skill's reference palette, light mode: categorical slots
blue `#2a78d6`, red `#e34948`, green `#008300`, yellow `#eda100` and aqua `#1baf7a`, with violet
`#6250d6` for constraints only. Ink is `#0b0b0b` and `#52514e`, on the surface `#fcfcfb`.

On a flowchart any two kinds can sit side by side, so the families were checked as all pairs.
Among every five-slot subset of the palette, these five separate best. The skill's validator
(`validate_palette.js --mode light --pairs all`) gives:

- **Node families** pass. The worst CVD pair is aqua against red, ΔE 6.9: in the 6–8 band, which is
  legal only with secondary encoding, and every kind has a shape and a written name. The worst
  normal-vision pair is aqua against green, ΔE 15.6, above the floor of 15. Yellow and aqua sit below
  3:1 against the surface, which is relieved by the visible label on every node.
- **Edge colours** (red, green, violet) pass. The worst CVD pair is ΔE 7.2, so each edge kind also
  has its own line and arrowhead. All three edge colours clear 3:1.

Text is ink only. Primary ink is at least 14.9:1 and secondary ink at least 6.0:1 on every fill:
tints, muted fills, connectors, placeholders and clusters. The tests hold both to 4.5:1.

There is no dark theme. The same five hues stepped for the dark surface fail the all-pairs
normal-vision floor (ΔE 11.9), and the skill allows a dark mode only once it is validated.

### Fonts

DejaVu Sans, with DejaVu Sans Mono for sources. It draws every symbol the data uses: Greek,
☉ ⊕ ≤ ≥ ≈ × → ± ° ² ³ and the markers ◐ ○ ⚠. Noto Sans lacks the mathematical and astronomical
symbols. SVGs name Verdana and then the generic family as fallbacks for viewers without DejaVu.
The script warns, through fontconfig, about any character in the data that DejaVu Sans does not
draw.

## Editing the data

The schema, with every key, kind and limit, is [`data/SCHEMA.md`](data/SCHEMA.md). The renderer
holds the data to it:

- **Text limits are the layout's contract.** A label is at most 32 characters, a summary at most
  60, detail at most 6 lines of 60, and an edge label at most 28. One character over is an error
  naming the field and its length. Citations and sources have no limit and wrap.
- **Ids.** Node ids are `<prefix>.<snake_case>` and unique across all files. Cluster ids follow the
  same pattern. A cluster and a node may share an id: a node's `cluster` always names a cluster,
  and an edge always joins nodes.
- **Keys.** A key the schema does not name is an error, which catches typos such as `detial`.
- **Edges.** An edge's ends must be nodes, in any file, or `@part:description` placeholders. Two
  edges with the same ends and kind are an error, and so is an edge from a node to itself.
- **Placeholders.** A placeholder names a part that may not be written yet. Its stage is taken from
  the part's name: `@planets-systems:...` belongs to the planets stage. The data is not reconciled
  while any placeholder remains, and drawing refuses it until then (`--allow-placeholders`
  previews).
- **Decisions.** Label a decision's outgoing edges with their cases (`yes`, `no: past the last
EEP`).

The workflow: edit the TOML, run `just flowcharts --check`, then `just flowcharts` and open the
SVG. Every problem prints as `file: id: problem`:

```
data/stars.toml: s.mass: 'summary' is 63 characters, over the limit of 60: "…"
data/stars.toml: edge s.mass -> s.track: dangling 'to': no node 's.track' in any file
```

## Tests

```sh
uv run --project tools/procgen-flowcharts python -B tools/procgen-flowcharts/test_procgen_flowcharts.py
```

The code is linted, formatted (at 100 columns, the repo's width) and type-checked by the project's
dev tools, from this directory:

```sh
uv run ruff check . && uv run ruff format --check . && uv run mypy
```

Standard library only, about 6 s. The tests use the fixtures in `testdata/`. One test also checks
that every edge of `data/` is drawn, and skips while `data/` does not validate:

- `valid/`: a small pipeline shaped like the real one, with a hub and with text at the limits.
- `invalid/`: each check the validator makes, failed at least once (unknown kinds and keys, ids,
  duplicates, cycles, types, text limits, empty and control characters, edges to nothing,
  placeholders, a missing `[meta]`, a part with no steps). The tests add a file that is not TOML,
  since pre-commit's `check-toml` would refuse to commit one.
- `placeholders/`: unresolved references.

Where Graphviz is installed they also lay the charts out, check that the complete chart stacks its
stages in one column, and draw everything twice to compare the files byte for byte.
They are not part of `just ci`.

## Known limitations

- **Order within a band.** Bands follow the flow top to bottom, and left to right as far as dot's
  crossing reduction allows: in the galaxy stage 16 of 22 pairs of units are in flow order.
- **References.** Stubs and connectors keep edges short at the cost of looking a number up: the
  planets stage has about 120 stubs. Some edges inside a tall cluster still run its whole height,
  such as a stream key feeding steps at several depths.
- **Placement.** A cluster's interior is Graphviz's choice. A band is as tall as its tallest
  cluster, so short clusters leave space below them.
- **Edge labels.** Two labels on edges into the same step can sit side by side and touch, since dot
  places them independently.
- **The complete PNG.** It is an overview: at the capped resolution its text is a few pixels high.
  Read the SVG or the PDF.
- **Sizing.** The fitted shapes depend on measured Graphviz geometry (the `invhouse` proportions and
  the skew of a parallelogram, from 16.1). A Graphviz that changed them would let text touch those
  outlines; `test_fitted_shapes_hold_their_labels` pins the formulas.
- **Glyph warnings.** These need fontconfig. Without it the check is skipped.
