// The cluster path's stable compaction (plan R14, R14.T2.a; Design note 5): the ordered pass's
// marks (`clusterCull.wgsl`'s, one a cluster, its template or NO_MARK) written into that pass's
// buckets in the draw set's order, instance by instance and, within an instance, cluster by
// cluster, so that a colour pass draws its clusters in the order the lit mesh drew its patches. Three
// dispatches of this one kernel, by `params.counts.w`, integer throughout, so that the result is the
// same on every path and from run to run (`bit-exact`):
//
// - _Count_ (phase 0), a workgroup an instance and a thread a cluster: each template's kept
//   clusters of the instance, into `tallies` (TEMPLATES words an instance).
// - _Scan_ (phase 1), one workgroup: each template's exclusive prefix sum over the instances, in
//   place, each thread over its own run of instances, the runs' sums scanned in order by one thread
//   a template; each template's total is its bucket's instance count in `args`.
// - _Scatter_ (phase 2), a workgroup an instance and a thread a cluster: each kept cluster's entry,
//   (instance << 8) | cluster, at its template's list offset in the pass, plus its instance's
//   prefix, plus its rank among the instance's earlier clusters of its template.
//
// The TypeScript twin is `clusters/compact.ts`' `compactTwin`. CompactParams: `counts` (instances,
// clusters a page, templates, phase), `place` (the pass's first bucket and its first list entry),
// then each template's first list entry within the pass, four to a `vec4u`.

struct CompactParams {
  counts : vec4u,
  place : vec4u,
  listOffsetsLow : vec4u,
  listOffsetsHigh : vec4u,
}

@group(0) @binding(0) var<uniform> params : CompactParams;
@group(0) @binding(1) var<storage, read> marks : array<u32>;
@group(0) @binding(2) var<storage, read_write> tallies : array<u32>;
@group(0) @binding(3) var<storage, read_write> lists : array<u32>;
@group(0) @binding(4) var<storage, read_write> args : array<u32>;

// `compact.ts`' COMPACT_TEMPLATES, NO_MARK and COMPACT_WORKGROUP.
const TEMPLATES : u32 = 8u;
const NO_MARK : u32 = 0xffffffffu;
const GROUP : u32 = 128u;

var<workgroup> tally : array<atomic<u32>, 8>;
var<workgroup> runs : array<u32, 1024>;
var<workgroup> held : array<u32, 128>;

fn template_offset(t : u32) -> u32 {
  if (t < 4u) {
    return params.listOffsetsLow[t];
  }
  return params.listOffsetsHigh[t - 4u];
}

// The instance's cluster `local`'s mark, NO_MARK past the page's clusters.
fn mark_of(instance : u32, local : u32) -> u32 {
  let clusters = params.counts.y;
  if (local >= clusters) {
    return NO_MARK;
  }
  return marks[instance * clusters + local];
}

//COUNT//

fn scan(local : u32) {
  let n = params.counts.x;
  let templates = min(params.counts.z, TEMPLATES);
  let chunk = (n + GROUP - 1u) / GROUP;
  let first = min(local * chunk, n);
  let last = min(first + chunk, n);
  for (var t = 0u; t < TEMPLATES; t = t + 1u) {
    var sum = 0u;
    for (var i = first; i < last; i = i + 1u) {
      sum = sum + tallies[i * TEMPLATES + t];
    }
    runs[t * GROUP + local] = sum;
  }
  workgroupBarrier();
  if (local < templates) {
    var running = 0u;
    for (var k = 0u; k < GROUP; k = k + 1u) {
      let v = runs[local * GROUP + k];
      runs[local * GROUP + k] = running;
      running = running + v;
    }
    args[(params.place.x + local) * 5u + 1u] = running;
  }
  workgroupBarrier();
  for (var t = 0u; t < templates; t = t + 1u) {
    var running = runs[t * GROUP + local];
    for (var i = first; i < last; i = i + 1u) {
      let v = tallies[i * TEMPLATES + t];
      tallies[i * TEMPLATES + t] = running;
      running = running + v;
    }
  }
}

fn scatter(instance : u32, local : u32) {
  let mark = mark_of(instance, local);
  held[local] = mark;
  workgroupBarrier();
  if (mark < min(params.counts.z, TEMPLATES)) {
    var rank = 0u;
    for (var j = 0u; j < local; j = j + 1u) {
      if (held[j] == mark) {
        rank = rank + 1u;
      }
    }
    let at = params.place.y + template_offset(mark) + tallies[instance * TEMPLATES + mark] + rank;
    lists[at] = (instance << 8u) | local;
  }
}

@compute @workgroup_size(128)
fn main(@builtin(workgroup_id) group : vec3u, @builtin(local_invocation_index) local : u32 //LANE//) {
  switch params.counts.w {
    case 0u: {
      if (group.x < params.counts.x) {
        count(group.x, local //LANEARG//);
      }
    }
    case 1u: {
      scan(local);
    }
    default: {
      if (group.x < params.counts.x) {
        scatter(group.x, local);
      }
    }
  }
}
