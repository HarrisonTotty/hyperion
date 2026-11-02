//! `meshbuild`: builds a mesh's cluster DAG and writes its pages (plan R14, T5.a), as
//! `meshbuild <in.obj> <out>`, or one of the procedural test meshes as
//! `meshbuild procedural:<icosphere|torus-knot|terrain> <out>` (T5.b).

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use meshbuild::procedural::TestMesh;
use meshbuild::{build, obj, page};

/// Builds a mesh's cluster DAG from an OBJ file and writes it as mesh pages.
#[derive(Debug, Parser)]
#[command(version)]
struct Cli {
    /// The source mesh, Wavefront OBJ, positions in metres; or `procedural:<name>`, a test mesh.
    input: PathBuf,
    /// The mesh file to write.
    output: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let named = cli
        .input
        .to_str()
        .and_then(|input| input.strip_prefix("procedural:"));
    let source = if let Some(name) = named {
        TestMesh::from_name(name)
            .with_context(|| {
                format!("no procedural test mesh {name}: icosphere, torus-knot or terrain")
            })?
            .source()
    } else {
        let text = std::fs::read_to_string(&cli.input)
            .with_context(|| format!("reading {}", cli.input.display()))?;
        obj::read_obj(&text).with_context(|| format!("reading {}", cli.input.display()))?
    };
    let dag = build(&source).context("building the DAG")?;
    let bytes = page::write_mesh(&dag).context("writing the pages")?;
    std::fs::write(&cli.output, &bytes)
        .with_context(|| format!("writing {}", cli.output.display()))?;
    let file = page::read_mesh(&bytes).context("reading the pages back")?;
    let roots = dag
        .clusters()
        .iter()
        .filter(|c| c.consuming_group().is_none())
        .count();
    println!(
        "{} triangles: {} clusters in {} groups over {} levels, {} roots; {} pages ({} root), {} bytes",
        dag.leaf_triangles(),
        dag.clusters().len(),
        dag.groups().len(),
        dag.levels(),
        roots,
        file.pages().len(),
        file.root_pages(),
        bytes.len(),
    );
    Ok(())
}
