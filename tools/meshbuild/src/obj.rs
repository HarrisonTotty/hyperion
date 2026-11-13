//! The builder's input format: Wavefront OBJ's geometry, the subset authoring tools all export.
//!
//! It reads `v`, `vt`, `vn` and `f` (polygons fanned into triangles from their first corner,
//! indices 1-based or negative, as `v`, `v/vt`, `v//vn` or `v/vt/vn`) and skips every other
//! statement. Each distinct corner (position, UV, normal) becomes one vertex, in the order the
//! faces first use it. The mesh keeps UVs only if every corner has one, and normals likewise;
//! positions are in metres. `str::parse` rounds each number correctly, so the mesh read is the
//! same bits on every platform.

use std::collections::HashMap;

use thiserror::Error;

use crate::mesh::{MAX_ELEMENTS, NewSourceMeshError, SourceMesh};
use crate::num::u32_of;

/// Why an OBJ file could not be read.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ReadObjError {
    /// A statement is malformed.
    #[error("line {line}: {what}")]
    Syntax {
        /// Its line, from 1.
        line: usize,
        /// What was wrong.
        what: &'static str,
    },
    /// The mesh it describes was refused.
    #[error("the mesh was refused")]
    Mesh(#[from] NewSourceMeshError),
}

type Corner = (usize, Option<usize>, Option<usize>);

/// Reads an OBJ file's text (see the module's documentation).
///
/// # Errors
///
/// [`ReadObjError::Syntax`] for a malformed statement or an index out of range, and
/// [`ReadObjError::Mesh`] if the mesh is refused.
pub fn read_obj(text: &str) -> Result<SourceMesh, ReadObjError> {
    let mut positions: Vec<[f64; 3]> = Vec::new();
    let mut uvs: Vec<[f64; 2]> = Vec::new();
    let mut normals: Vec<[f64; 3]> = Vec::new();
    let mut corners: Vec<Corner> = Vec::new();
    let mut index: HashMap<Corner, u32> = HashMap::new();
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = n + 1;
        let syntax = |what| ReadObjError::Syntax { line, what };
        let mut tokens = raw.split('#').next().unwrap_or_default().split_whitespace();
        match tokens.next() {
            Some("v") => positions
                .push(numbers(&mut tokens).ok_or_else(|| syntax("a v needs three numbers"))?),
            Some("vt") => {
                let mut uv = [0.0; 2];
                let first: [f64; 1] =
                    numbers(&mut tokens).ok_or_else(|| syntax("a vt needs a number"))?;
                uv[0] = first[0];
                if let Some(v) = tokens.next() {
                    uv[1] = v.parse().map_err(|_| syntax("a vt's v is not a number"))?;
                }
                uvs.push(uv);
            }
            Some("vn") => normals
                .push(numbers(&mut tokens).ok_or_else(|| syntax("a vn needs three numbers"))?),
            Some("f") => {
                let face: Vec<u32> = tokens
                    .map(|token| {
                        let corner = parse_corner(token, positions.len(), uvs.len(), normals.len())
                            .ok_or_else(|| {
                                syntax("a face's corner is malformed or out of range")
                            })?;
                        if corners.len() >= MAX_ELEMENTS && !index.contains_key(&corner) {
                            return Err(ReadObjError::Mesh(NewSourceMeshError::TooLarge));
                        }
                        Ok(*index.entry(corner).or_insert_with(|| {
                            corners.push(corner);
                            u32_of(corners.len() - 1)
                        }))
                    })
                    .collect::<Result<_, ReadObjError>>()?;
                if face.len() < 3 {
                    return Err(syntax("a face needs three corners"));
                }
                for k in 1..face.len() - 1 {
                    triangles.push([face[0], face[k], face[k + 1]]);
                }
            }
            _ => {}
        }
    }
    let vertex_positions = corners.iter().map(|c| positions[c.0]).collect();
    let vertex_uvs = corners
        .iter()
        .map(|c| c.1.map(|t| uvs[t]))
        .collect::<Option<Vec<_>>>();
    let vertex_normals = corners
        .iter()
        .map(|c| c.2.map(|t| normals[t]))
        .collect::<Option<Vec<_>>>();
    let mut mesh = SourceMesh::new(vertex_positions, triangles)?;
    if let Some(normals) = vertex_normals {
        mesh = mesh.with_normals(normals)?;
    }
    if let Some(uvs) = vertex_uvs {
        mesh = mesh.with_uvs(uvs)?;
    }
    Ok(mesh)
}

fn numbers<const N: usize>(tokens: &mut std::str::SplitWhitespace<'_>) -> Option<[f64; N]> {
    let mut out = [0.0; N];
    for slot in &mut out {
        *slot = tokens.next()?.parse().ok()?;
    }
    Some(out)
}

/// A corner's (position, UV, normal), each 0-based, from `v`, `v/vt`, `v//vn` or `v/vt/vn`.
fn parse_corner(token: &str, positions: usize, uvs: usize, normals: usize) -> Option<Corner> {
    let mut parts = token.split('/');
    let position = resolve(parts.next()?, positions)?;
    let uv = match parts.next() {
        None | Some("") => None,
        Some(t) => Some(resolve(t, uvs)?),
    };
    let normal = match parts.next() {
        None => None,
        Some(n) => Some(resolve(n, normals)?),
    };
    parts.next().is_none().then_some((position, uv, normal))
}

/// A 1-based or negative (counting back from the last) index, 0-based, if in range.
fn resolve(token: &str, count: usize) -> Option<usize> {
    let i: i64 = token.parse().ok()?;
    let count = i64::try_from(count).ok()?;
    let zero_based = if i < 0 { count + i } else { i - 1 };
    (0..count)
        .contains(&zero_based)
        .then(|| usize::try_from(zero_based).ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quad_fans_into_two_triangles_with_its_attributes() {
        let text = "# a quad\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nvn 0 0 1\nf 1/1/1 2/2/1 3/3/1 -1/-1/-1\n";
        let mesh = read_obj(text).unwrap();
        assert_eq!(mesh.triangles(), &[[0, 1, 2], [0, 2, 3]]);
        assert_eq!(mesh.positions()[3], [0.0, 1.0, 0.0]);
    }

    #[test]
    fn an_index_out_of_range_names_its_line() {
        let refused = read_obj("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 4\n");
        assert_eq!(
            refused,
            Err(ReadObjError::Syntax {
                line: 4,
                what: "a face's corner is malformed or out of range"
            })
        );
    }
}
