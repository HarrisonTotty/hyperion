//! The DAG's checks, on the pages as read back: what the client will draw.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use meshbuild::cut::{CutView, cut, in_uniform_cut};
use meshbuild::page::{self, MAX_PAGE_BYTES, MAX_PAGE_CLUSTERS, MeshFile, PageCluster, PageVertex};
use meshbuild::sphere::Sphere;
use meshbuild::{MAX_CLUSTER_TRIANGLES, MAX_CLUSTER_VERTICES, SourceMesh, build};
use sha2::{Digest, Sha256};

pub type Point = [u32; 3];
pub type Edge = (Point, Point);

/// A built mesh: its file's bytes and the file read back.
pub struct Built {
    pub bytes: Vec<u8>,
    pub file: MeshFile,
}

/// Builds `source` twice, and checks that both builds are the same bytes.
pub fn build_twice(source: &SourceMesh) -> Built {
    let write =
        || page::write_mesh(&build(source).expect("the test mesh builds")).expect("it fits");
    let bytes = write();
    assert_eq!(
        sha256(&write()),
        sha256(&bytes),
        "two builds of one mesh give one file"
    );
    let file = page::read_mesh(&bytes).expect("the pages read back");
    Built { bytes, file }
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut hex, b| {
            write!(hex, "{b:02x}").expect("writing to a string succeeds");
            hex
        })
}

pub fn clusters(file: &MeshFile) -> Vec<&PageCluster> {
    cut(file, |_| true)
}

fn corners(c: &PageCluster) -> impl Iterator<Item = [Point; 3]> + '_ {
    c.triangles()
        .iter()
        .map(|t| t.map(|i| c.vertices()[usize::from(i)].grid()))
}

/// Every directed edge of `clusters`' triangles, sorted.
pub fn directed_edges(clusters: &[&PageCluster]) -> Vec<Edge> {
    let mut edges: Vec<Edge> = clusters
        .iter()
        .flat_map(|c| corners(c))
        .flat_map(|[a, b, c]| [(a, b), (b, c), (c, a)])
        .collect();
    edges.sort_unstable();
    edges
}

/// The grid points on the leaves' open boundary (none for a closed mesh), sorted.
pub fn open_points(file: &MeshFile) -> Vec<Point> {
    let leaves: Vec<&PageCluster> = cut(file, |c| c.generating_group().is_none());
    let edges = directed_edges(&leaves);
    let mut open: Vec<Point> = edges
        .iter()
        .filter(|&&(a, b)| edges.binary_search(&(b, a)).is_err())
        .flat_map(|&(a, b)| [a, b])
        .collect();
    open.sort_unstable();
    open.dedup();
    open
}

fn on_boundary(open: &[Point], (a, b): Edge) -> bool {
    open.binary_search(&a).is_ok() && open.binary_search(&b).is_ok()
}

/// Checks that `drawn` is one watertight cover: no triangle degenerate, no directed edge twice,
/// and every edge matched by its reverse but on the mesh's open boundary.
pub fn assert_watertight(drawn: &[&PageCluster], open: &[Point], what: &str) {
    assert!(!drawn.is_empty(), "{what}: the cut is empty");
    for c in drawn {
        for [a, b, d] in corners(c) {
            assert!(
                a != b && b != d && d != a,
                "{what}: a degenerate triangle {a:?} {b:?} {d:?}"
            );
        }
    }
    let edges = directed_edges(drawn);
    if let Some(&[twice, _]) = edges.windows(2).find(|w| w[0] == w[1]) {
        let holders: Vec<_> = drawn
            .iter()
            .filter(|c| corners(c).any(|[a, b, d]| [(a, b), (b, d), (d, a)].contains(&twice)))
            .map(|c| (c.generating_group(), c.consuming_group()))
            .collect();
        panic!("{what}: the edge {twice:?} is drawn twice (an overlap), by {holders:?}");
    }
    let unmatched: Vec<&Edge> = edges
        .iter()
        .filter(|&&(a, b)| edges.binary_search(&(b, a)).is_err())
        .filter(|&&e| !on_boundary(open, e))
        .collect();
    assert!(
        unmatched.is_empty(),
        "{what}: {} edges have no neighbour (a crack), the first {:?}",
        unmatched.len(),
        unmatched.first()
    );
}

/// The edges of `set`'s boundary that are not on the mesh's open boundary, sorted.
fn inner_boundary(set: &[&PageCluster], open: &[Point]) -> Vec<Edge> {
    let edges = directed_edges(set);
    edges
        .iter()
        .copied()
        .filter(|&(a, b)| edges.binary_search(&(b, a)).is_err())
        .filter(|&e| !on_boundary(open, e))
        .collect()
}

/// Checks that each group's clusters and those made from it have one border: every cut that
/// replaces one by the other is watertight wherever the other was.
pub fn assert_group_borders_kept(file: &MeshFile, open: &[Point]) {
    let mut inputs: BTreeMap<u32, Vec<&PageCluster>> = BTreeMap::new();
    let mut outputs: BTreeMap<u32, Vec<&PageCluster>> = BTreeMap::new();
    for c in clusters(file) {
        if let Some(g) = c.consuming_group() {
            inputs.entry(g).or_default().push(c);
        }
        if let Some(g) = c.generating_group() {
            outputs.entry(g).or_default().push(c);
        }
    }
    assert_eq!(
        inputs.len(),
        usize::try_from(file.group_count()).unwrap(),
        "every group has inputs"
    );
    assert_eq!(outputs.len(), inputs.len(), "every group has outputs");
    for (g, made) in &outputs {
        assert_eq!(
            inner_boundary(&inputs[g], open),
            inner_boundary(made, open),
            "group {g}'s border moved"
        );
    }
}

fn sphere_bits(s: &Sphere) -> [u32; 4] {
    let [x, y, z] = s.centre_m();
    [x, y, z, s.radius_m()].map(f32::to_bits)
}

/// Checks the errors and spheres: own ≤ parent; a group's error and sphere the same bits in
/// every record that holds them; parent spheres holding own spheres, and own spheres the
/// clusters' vertices; the clusters and pages within their limits.
pub fn assert_monotone_and_nested(file: &MeshFile) {
    let mut seen: BTreeMap<u32, (u32, [u32; 4])> = BTreeMap::new();
    for p in file.pages() {
        assert!(
            p.clusters().len() <= MAX_PAGE_CLUSTERS,
            "page {} holds too many",
            p.index()
        );
        assert!(
            usize::try_from(p.bytes()).unwrap() <= MAX_PAGE_BYTES,
            "page {} too large",
            p.index()
        );
        let step = f64::from(p.step_m());
        for c in p.clusters() {
            assert!(c.triangles().len() <= MAX_CLUSTER_TRIANGLES);
            assert!(c.vertices().len() <= MAX_CLUSTER_VERTICES);
            assert!(
                c.own_error_m() <= c.parent_error_m(),
                "own {} > parent {}",
                c.own_error_m(),
                c.parent_error_m()
            );
            assert!(
                c.parent_sphere().contains(c.own_sphere(), 0.0),
                "{:?} outside {:?}",
                c.own_sphere(),
                c.parent_sphere()
            );
            for v in c.vertices().iter().map(PageVertex::grid) {
                let x = v.map(|q| f64::from(q) * step);
                assert!(
                    c.own_sphere().contains_point(x, 0.0),
                    "{x:?} outside {:?}",
                    c.own_sphere()
                );
            }
            let held = [
                c.generating_group()
                    .map(|g| (g, c.own_error_m(), c.own_sphere())),
                c.consuming_group()
                    .map(|g| (g, c.parent_error_m(), c.parent_sphere())),
            ];
            for (g, error, sphere) in held.into_iter().flatten() {
                let record = (error.to_bits(), sphere_bits(sphere));
                let first = *seen.entry(g).or_insert(record);
                assert_eq!(
                    first, record,
                    "group {g}'s error and sphere in every record that holds them"
                );
            }
        }
    }
    let root_pages = usize::try_from(file.root_pages()).unwrap();
    assert!(root_pages >= 1, "a root page");
    for (i, p) in file.pages().iter().enumerate() {
        let roots = p.clusters().iter().any(|c| c.consuming_group().is_none());
        assert!(
            !roots || i < root_pages,
            "a root on page {i}, beyond the {root_pages} root pages"
        );
    }
}

/// The uniform cuts at up to `samples` of the DAG's errors, and the view-dependent cuts from
/// `eyes` eyes at four tolerances, each checked watertight.
pub fn assert_cuts_watertight(file: &MeshFile, open: &[Point], samples: usize, eyes: usize) {
    let all = clusters(file);
    let mut errors: Vec<f32> = all.iter().map(|c| c.own_error_m()).collect();
    errors.sort_unstable_by(f32::total_cmp);
    errors.dedup();
    let step = errors.len().div_ceil(samples).max(1);
    let mut thresholds: Vec<f64> = errors.iter().step_by(step).map(|&e| f64::from(e)).collect();
    thresholds.push(f64::from(*errors.last().unwrap()));
    for t in thresholds {
        let drawn = cut(file, |c| in_uniform_cut(c, t));
        assert_watertight(&drawn, open, &format!("the uniform cut at {t} m"));
    }
    let (centre, radius) = roots_extent(&all);
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let mut unit = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        f64::from(u32::try_from(seed >> 40).unwrap()) / f64::from(1_u32 << 24)
    };
    for e in 0..eyes {
        let direction = [unit() - 0.5, unit() - 0.5, unit() - 0.5];
        let n = direction.iter().map(|d| d * d).sum::<f64>().sqrt();
        let distance = radius * (0.2 + 6.0 * unit() * unit());
        let eye = std::array::from_fn(|k| centre[k] + direction[k] / n * distance);
        let view = CutView::new(eye, 1920.0, 60_f64.to_radians());
        for tau in [0.5, 1.0, 2.0, 8.0] {
            let drawn = cut(file, |c| view.draws(c, tau));
            assert_watertight(
                &drawn,
                open,
                &format!("the cut from eye {e} at {eye:?}, τ {tau} px"),
            );
        }
    }
}

/// The centre and radius of a sphere holding the roots' own spheres, about the page's origin, m.
pub fn roots_extent(all: &[&PageCluster]) -> ([f64; 3], f64) {
    let roots: Vec<&&PageCluster> = all
        .iter()
        .filter(|c| c.consuming_group().is_none())
        .collect();
    let count = f64::from(u32::try_from(roots.len()).unwrap());
    let mut centre = [0.0; 3];
    for r in &roots {
        let c = r.own_sphere().centre_f64();
        for k in 0..3 {
            centre[k] += c[k] / count;
        }
    }
    let mut radius: f64 = 0.0;
    for r in &roots {
        let c = r.own_sphere().centre_f64();
        let d = (0..3)
            .map(|k| (c[k] - centre[k]) * (c[k] - centre[k]))
            .sum::<f64>()
            .sqrt();
        radius = radius.max(d + f64::from(r.own_sphere().radius_m()));
    }
    (centre, radius)
}

/// The triangles of a cut.
#[allow(dead_code, reason = "each test binary takes the helpers it needs")]
pub fn triangles(drawn: &[&PageCluster]) -> usize {
    drawn.iter().map(|c| c.triangles().len()).sum()
}

/// A summary line for the test's output.
pub fn summary(built: &Built) -> String {
    let all = clusters(&built.file);
    let roots: Vec<_> = all
        .iter()
        .filter(|c| c.consuming_group().is_none())
        .collect();
    let root_triangles: usize = roots.iter().map(|c| c.triangles().len()).sum();
    let root_bytes: u32 = built.file.pages()[..usize::try_from(built.file.root_pages()).unwrap()]
        .iter()
        .map(page::Page::bytes)
        .sum();
    format!(
        "{} clusters, {} groups, {} levels, {} roots of {root_triangles} triangles, {} pages \
         ({} root, {root_bytes} bytes), {} bytes",
        all.len(),
        built.file.group_count(),
        built.file.levels(),
        roots.len(),
        built.file.pages().len(),
        built.file.root_pages(),
        built.bytes.len()
    )
}
