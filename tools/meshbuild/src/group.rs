//! Grouping a level's clusters for simplification.
//!
//! Groups are grown by rounds of heavy-edge matching on the clusters' adjacency, weighted by the
//! edges two clusters share: each round merges disjoint pairs of neighbouring groups, the pairs
//! sharing the most edges first, while the merged group holds at most [`MAX_GROUP_CLUSTERS`]
//! clusters. Sharing the most edges keeps a group's border short, so that few of its vertices are
//! locked, and it favours merging across the last level's group borders, which simplification left
//! dense, so that those borders are simplified at the next level rather than locked again (Karis
//! 2021, on alternating group borders). Ties fall to the lower cluster indices.

use crate::num::{ix, u32_of};

/// The most clusters a group holds.
pub(crate) const MAX_GROUP_CLUSTERS: usize = 8;

/// The clusters' groups: each a sorted list of indices into `clusters`, in the order of their
/// first cluster.
#[must_use]
pub(crate) fn group(clusters: &[&[[u32; 3]]], position_of: impl Fn(u32) -> u32) -> Vec<Vec<usize>> {
    let adjacency = adjacency(clusters, position_of);
    let mut group_of: Vec<usize> = (0..clusters.len()).collect();
    let mut members: Vec<Vec<usize>> = (0..clusters.len()).map(|c| vec![c]).collect();
    loop {
        let mut pairs: Vec<(usize, usize, u32)> = adjacency
            .iter()
            .filter_map(|&(a, b, w)| {
                let (ga, gb) = (group_of[ix(a)], group_of[ix(b)]);
                (ga != gb).then(|| (ga.min(gb), ga.max(gb), w))
            })
            .collect();
        pairs.sort_unstable();
        let mut weighted: Vec<(usize, usize, u32)> = Vec::with_capacity(pairs.len());
        for (ga, gb, w) in pairs {
            match weighted.last_mut() {
                Some(last) if last.0 == ga && last.1 == gb => last.2 += w,
                _ => weighted.push((ga, gb, w)),
            }
        }
        weighted.retain(|&(ga, gb, _)| members[ga].len() + members[gb].len() <= MAX_GROUP_CLUSTERS);
        weighted.sort_unstable_by(|x, y| y.2.cmp(&x.2).then((x.0, x.1).cmp(&(y.0, y.1))));
        let mut matched = vec![false; members.len()];
        let mut merged = false;
        for (ga, gb, _) in weighted {
            if !matched[ga] && !matched[gb] {
                matched[ga] = true;
                matched[gb] = true;
                let moved = std::mem::take(&mut members[gb]);
                members[ga].extend(moved);
                merged = true;
            }
        }
        if !merged {
            break;
        }
        members.retain(|m| !m.is_empty());
        for m in &mut members {
            m.sort_unstable();
        }
        members.sort_unstable_by_key(|m| m[0]);
        for (g, m) in members.iter().enumerate() {
            for &c in m {
                group_of[c] = g;
            }
        }
    }
    members
}

/// Each pair of clusters that share edges, as (lower, higher, shared edges), sorted.
fn adjacency(clusters: &[&[[u32; 3]]], position_of: impl Fn(u32) -> u32) -> Vec<(u32, u32, u32)> {
    let mut edges: Vec<(u64, u32)> =
        Vec::with_capacity(3 * clusters.iter().map(|c| c.len()).sum::<usize>());
    for (c, triangles) in clusters.iter().enumerate() {
        let c = u32_of(c);
        for t in *triangles {
            let p = t.map(&position_of);
            for k in 0..3 {
                let (a, b) = (p[k], p[(k + 1) % 3]);
                edges.push(((u64::from(a.min(b)) << 32) | u64::from(a.max(b)), c));
            }
        }
    }
    edges.sort_unstable();
    let mut pairs = Vec::new();
    let mut start = 0;
    while start < edges.len() {
        let mut end = start + 1;
        while end < edges.len() && edges[end].0 == edges[start].0 {
            end += 1;
        }
        let run = &edges[start..end];
        for (i, &(_, a)) in run.iter().enumerate() {
            for &(_, b) in &run[i + 1..] {
                if a != b && !run[..i].iter().any(|&(_, x)| x == a) {
                    pairs.push((a, b));
                }
            }
        }
        start = end;
    }
    pairs.sort_unstable();
    let mut weighted: Vec<(u32, u32, u32)> = Vec::new();
    for (a, b) in pairs {
        match weighted.last_mut() {
            Some(last) if last.0 == a && last.1 == b => last.2 += 1,
            _ => weighted.push((a, b, 1)),
        }
    }
    weighted
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip of `n` clusters of two triangles each, cluster i on the quad from x = i to i + 1.
    fn strip(n: u32) -> Vec<Vec<[u32; 3]>> {
        (0..n)
            .map(|i| {
                let (low_left, low_right) = (2 * i, 2 * i + 2);
                let (high_left, high_right) = (2 * i + 1, 2 * i + 3);
                vec![
                    [low_left, low_right, high_right],
                    [low_left, high_right, high_left],
                ]
            })
            .collect()
    }

    #[test]
    fn neighbours_share_their_edges() {
        let clusters = strip(3);
        let lists: Vec<&[[u32; 3]]> = clusters.iter().map(Vec::as_slice).collect();
        let adjacency = adjacency(&lists, |v| v);
        assert_eq!(adjacency, vec![(0, 1, 1), (1, 2, 1)]);
    }

    #[test]
    fn a_strip_groups_into_runs_of_at_most_the_limit() {
        let clusters = strip(20);
        let lists: Vec<&[[u32; 3]]> = clusters.iter().map(Vec::as_slice).collect();
        let groups = group(&lists, |v| v);
        assert!(groups.iter().all(|g| g.len() <= MAX_GROUP_CLUSTERS));
        let mut all: Vec<usize> = groups.iter().flatten().copied().collect();
        all.sort_unstable();
        assert_eq!(all, (0..20).collect::<Vec<_>>());
        for g in &groups {
            assert!(
                g.windows(2).all(|w| w[1] == w[0] + 1),
                "a group of a strip is a run: {g:?}"
            );
        }
        assert!(groups.len() <= 5, "{groups:?}");
    }
}
