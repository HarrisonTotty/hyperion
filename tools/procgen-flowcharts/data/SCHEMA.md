# Flowchart data schema

Each `*.toml` file in this directory describes one part of HYPERION's procedural-generation
pipeline as a graph. `procgen_flowcharts.py` renders each stage on its own and all of them stitched
together. Every file has the same shape.

```toml
[meta]
stage = "galaxy"                # galaxy | stars | planets: the chart the nodes appear on
part = "galaxy"                 # file stem; a stage may span several files (planets does)
prefix = "g"                    # every id in the file starts with "<prefix>."
title = "Galactic Properties & Megastructures"
snapshot = "a6d3f6dd"           # commit of rendering-and-planets the data was read from

[[cluster]]                     # a subsystem box; clusters may nest via `parent`
id = "g.disc"
label = "Stellar disc"
parent = ""                     # optional: id of the enclosing cluster

[[node]]
id = "g.imf"                    # "<prefix>.<snake_case>", unique across all files
label = "Initial mass function" # <= 32 characters: the node's title
kind = "draw"                   # see "Node kinds"
cluster = "g.disc"              # optional: id of a cluster in this file
summary = "Broken power law, 0.08-150 Msun"     # <= 60 characters, one line
detail = [                      # optional, <= 6 lines of <= 60 characters each
  "alpha = 1.3 below 0.5 Msun, 2.3 above",
  "mass drawn by inverse CDF from one stream word",
]
cite = ["Kroupa 2001"]          # optional: literature the step follows
source = ["crates/hyperion-sim/src/galaxy/imf.rs", "P06.T3"]  # code paths and/or plan task IDs
status = "built"                # built | partial | planned

[[edge]]
from = "g.seed"
to = "g.imf"
kind = "seed"                   # see "Edge kinds"; default "data"
label = "stream STAR_MASS"      # optional, <= 28 characters: what passes along the edge
```

## Node kinds

| kind       | meaning                                                                  |
| ---------- | ------------------------------------------------------------------------ |
| `seed`     | the root u64 seed, a derived key, or an RNG stream (hash + domain tag)   |
| `param`    | an input: galaxy parameter, configuration value or fixed constant set    |
| `table`    | fitted or tabulated data (offline fits, evolution tracks, opacity grids) |
| `draw`     | a stochastic step: consumes stream words to sample a distribution        |
| `model`    | a deterministic physics or derivation step (no randomness)               |
| `decision` | a branch; label the outgoing edges with the condition                    |
| `entity`   | a generated record or object (star, system, planet, atmosphere record)   |
| `enrich`   | LLM text enrichment of generated content                                 |
| `query`    | a runtime access path that evaluates the pipeline lazily on demand       |

## Edge kinds

| kind         | meaning                                                         |
| ------------ | --------------------------------------------------------------- |
| `data`       | a value flows from one step to the next (the default)           |
| `seed`       | seed or stream derivation: which key feeds which random step    |
| `lookup`     | a step reads a table                                            |
| `constraint` | conditioning, bounds, rejection, or feedback to an earlier step |

## Cross-file references

An edge may name a node in another file. Use its real id when you know it; otherwise use a
placeholder of the form `@<part>:<description>`, for example `"@galaxy:local [Fe/H] at position"`.
Placeholders are resolved to real ids when the files are reconciled, and the renderer refuses to
draw any chart while one remains (`--allow-placeholders` previews them as marked stubs).

## Status

`built` means the step exists in code at the snapshot commit. `partial` means some of it does.
`planned` means it is specified in a plan or brainstorm but not yet in code. Planned nodes are drawn
with a dashed outline.

## Validation

`just flowcharts --check` holds every file to this schema and prints each problem as
`file: id: problem` (`../README.md`, "Editing the data"). What it checks beyond the notes above:

- Keys marked optional, and `source`, may be left out; every other key is required. A key this
  schema does not name is an error, so that a typo is caught.
- Node ids are unique among nodes and cluster ids among clusters. A cluster and a node may share an
  id: an edge always joins nodes, and a node's `cluster` always names a cluster.
- Two edges with the same `from`, `to` and `kind` are an error, as is an edge from a node to itself.
- `cite` and `source` entries have no length limit; the renderer wraps long ones.
- A file needs at least one `[[node]]`. An edge needs a node at one end at least: two placeholders
  are an error.
- A placeholder may name a part that is not written yet. Its stage is read from the part's name, so
  `@planets-systems:...` belongs to the planets stage.
