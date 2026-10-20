"""P14.T49.e's reference for condensate_extinction_a_phase_table_meets_the_client_s_quadrature
(science-r08-nonspherical section 6.4): a mode's Q_ext and omega at 550 nm from a committed phase
table, by the client's own route (nonSpherical.ts's tableModeOptics and sumsOverNode,
sizeDistribution.ts's sizeRange and logNumberPerLnRadius), in pure Python: Gauss-Legendre panels of
order 6, at most 0.1 in ln r, their ends at the distribution's cuts and every table node inside; the
cross-sections, V and A log-linear in ln r between the table's sizes; below the smallest the
volume-equivalent sphere (TAMUdust2020) or Grenfell and Warren's V/A spheres (Yang 2013) by Mie at
the particles' own index; the floor's and Yang's absorption scaled by k / kTable; the bracketing
wavelength nodes' per-particle sums blended linearly in wavelength.

An independent route from the fit's (a trapezoid on its fine grid of step 0.002 in ln r).

Usage, from the repository's root:
python3 -I crates/hyperion-fit/src/tasks/condensate_extinction/client_table.py
    apps/hyperion/src/renderer/src/view/atmosphere/materials <index file> [refuse|spheres]
    <gamma|logNormal> <r_eff um> <v_eff> [...]
Prints Q_ext, omega, the table's omega as made (the floor rule's input), whether a node is floored,
and the tail's share.
"""
import json
import math
import os
import sys

GL6_X = [-0.9324695142031521, -0.6612093864662645, -0.2386191860831969,
         0.2386191860831969, 0.6612093864662645, 0.9324695142031521]
GL6_W = [0.1713244923791704, 0.3607615730481386, 0.4679139345726910,
         0.4679139345726910, 0.3607615730481386, 0.1713244923791704]
SIZE_PANEL_LN = 0.1
FLOOR = 1e-6
WAVELENGTH_NM = 550.0


def mie(x, n, k):
    """Q_ext and Q_sca of a sphere of size parameter x and index n + ik (Wiscombe 1980's structure)."""
    m = complex(n, -k)
    nstop = int(x + 4.05 * x ** (1 / 3) + 2)
    z = m * x
    N = nstop
    w = 1 / z
    f = (N + 1) * w
    c = f
    d = 0
    for j in range(2, 10 ** 7):
        a = (-1) ** (j + 1) * (2 * N + 2 * j - 1) * w
        d = 1 / (a + d)
        c = a + 1 / c
        delta = c * d
        f *= delta
        if abs(delta - 1) < 1e-15:
            break
    D = [0j] * (N + 1)
    D[N] = f
    for j in range(N, 0, -1):
        D[j - 1] = j * w - 1 / (D[j] + j * w)
    psi0, chi0 = math.sin(x), math.cos(x)
    if x < 0.1:
        # psi1 by its series, as the client's PSI1_SERIES_BELOW
        x2 = x * x
        psi1 = x2 / 3 * (1 - x2 / 10 * (1 - x2 / 28 * (1 - x2 / 54)))
    else:
        psi1 = math.sin(x) / x - math.cos(x)
    chi1 = math.cos(x) / x + math.sin(x)
    qext = 0.0
    qsca = 0.0
    for j in range(1, N + 1):
        if j > 1:
            psi_n = (2 * j - 1) / x * psi1 - psi0
            chi_n = (2 * j - 1) / x * chi1 - chi0
            psi0, psi1 = psi1, psi_n
            chi0, chi1 = chi1, chi_n
        xi = complex(psi1, chi1)
        xim = complex(psi0, chi0)
        ta = D[j] / m + j / x
        tb = m * D[j] + j / x
        an = (ta * psi1 - psi0) / (ta * xi - xim)
        bn = (tb * psi1 - psi0) / (tb * xi - xim)
        qext += (2 * j + 1) * (an.real + bn.real)
        qsca += (2 * j + 1) * (abs(an) ** 2 + abs(bn) ** 2)
    return qext * 2 / x ** 2, qsca * 2 / x ** 2


def index_at(f, w):
    g, n, k = f["wavelengthsNm"], f["n"], f["k"]
    for i in range(len(g) - 1):
        if g[i] <= w <= g[i + 1]:
            t = (w - g[i]) / (g[i + 1] - g[i])
            k0, k1 = k[i], k[i + 1]
            kk = k0 * (k1 / k0) ** t if k0 > 0 and k1 > 0 else k0 + t * (k1 - k0)
            return n[i] + t * (n[i + 1] - n[i]), kk
    raise ValueError("off grid")


def log_number_per_ln_r(kind, r_eff, v_eff):
    """ln(r n(r)) up to a constant, of y = ln(r / um)."""
    if kind == "gamma":
        b = v_eff
        return lambda y: (1 - 2 * b) / b * y - math.exp(y) / (r_eff * b)
    s2 = math.log1p(v_eff)
    ln_rg = math.log(r_eff) - 2.5 * s2
    return lambda y: -(y - ln_rg) ** 2 / (2 * s2)


def size_range(kind, r_eff, v_eff):
    """Where r^2 n(r) falls to 1e-6 of its peak below, and r^6 n(r) above (sizeRange)."""
    lw = log_number_per_ln_r(kind, r_eff, v_eff)

    def cut(power, side):
        f = lambda y: lw(y) + (power - 1) * y
        # the peak, by golden-section on a wide bracket
        lo, hi = math.log(r_eff) - 30, math.log(r_eff) + 10
        for _ in range(300):
            a = lo + (hi - lo) * 0.382
            b = lo + (hi - lo) * 0.618
            if f(a) < f(b):
                lo = a
            else:
                hi = b
        peak_y = 0.5 * (lo + hi)
        peak = f(peak_y)
        inner, outer = peak_y, peak_y + side
        while f(outer) - peak > math.log(FLOOR):
            outer += side * (abs(outer - peak_y) + 1)
        for _ in range(200):
            mid = 0.5 * (inner + outer)
            if f(mid) - peak > math.log(FLOOR):
                inner = mid
            else:
                outer = mid
        return 0.5 * (inner + outer)

    return cut(2, -1), cut(6, 1)


def node_sums(table, node, index, kind, r_eff, v_eff, yang, beyond="refuse"):
    length_um = WAVELENGTH_NM / 1000 / (2 * math.pi)
    r_tilde = [0.75 * v / a for v, a in zip(node["volumes"], node["areas"])]
    ln_r = [math.log(r * length_um) for r in r_tilde]
    lw = log_number_per_ln_r(kind, r_eff, v_eff)
    ln_from, ln_to = size_range(kind, r_eff, v_eff)
    top = ln_r[-1]
    if ln_to > top + 1e-9 and beyond == "refuse":
        raise ValueError(f"the distribution reaches {math.exp(ln_to)} um, past the table's {math.exp(top)} um")
    v0, r0 = node["volumes"][0], r_tilde[0]
    if yang:
        sphere_over_va, per_particle = 1.0, node["areas"][0] / (math.pi * r0 * r0)
    else:
        sphere_over_va, per_particle = (3 * v0 / (4 * math.pi)) ** (1 / 3) / r0, 1.0
    ends = [ln_from] + [l for l in ln_r if ln_from < l < ln_to] + [ln_to]
    panels = [ends[0]]
    for a, b in zip(ends, ends[1:]):
        steps = max(1, math.ceil((b - a) / SIZE_PANEL_LN))
        for s in range(1, steps + 1):
            panels.append(a + (b - a) * s / steps)
    pts = []
    for a, b in zip(panels, panels[1:]):
        half = 0.5 * (b - a)
        for x, w in zip(GL6_X, GL6_W):
            y = a + half * (1 + x)
            pts.append((y, math.log(half * w) + lw(y)))
    peak = max(p[1] for p in pts)
    S = dict(number=0.0, area=0.0, ext=0.0, sca=0.0, tail_ext=0.0, tail_sca=0.0, top_ext=0.0, top_sca=0.0)
    um2 = length_um * length_um
    for y, lwt in pts:
        w = math.exp(lwt - peak)
        if not w > 0:
            continue
        r_um = math.exp(y)
        S["number"] += w
        if y < ln_r[0]:
            ws = w * per_particle
            rs = sphere_over_va * r_um
            qe, qs = mie(rs / length_um, *index)
            g = math.pi * rs * rs
            S["area"] += ws * g
            S["ext"] += ws * qe * g
            S["sca"] += ws * qs * g
            S["tail_ext"] += ws * qe * g
            S["tail_sca"] += ws * qs * g
            continue
        if y > top and beyond == "spheres":
            # Grenfell and Warren's V/A spheres of the top's A / (pi r^2) per particle.
            ratio = node["areas"][-1] / (math.pi * r_tilde[-1] ** 2)
            qe, qs = mie(r_um / length_um, *index)
            g = ratio * math.pi * r_um * r_um
            S["area"] += w * g
            S["ext"] += w * qe * g
            S["sca"] += w * qs * g
            S["top_ext"] += w * qe * g
            S["top_sca"] += w * qs * g
            continue
        s = 0
        lo, hi = 0, len(ln_r) - 1
        while hi - lo > 1:
            mid = (lo + hi) // 2
            if ln_r[mid] <= y:
                lo = mid
            else:
                hi = mid
        s = lo
        t = min(max((y - ln_r[s]) / (ln_r[s + 1] - ln_r[s]), 0), 1)
        ll = lambda vals: math.exp(math.log(vals[s]) + t * (math.log(vals[s + 1]) - math.log(vals[s])))
        S["area"] += w * ll(node["areas"]) * um2
        S["ext"] += w * ll(node["extinction"]) * um2
        S["sca"] += w * ll(node["scattering"]) * um2
    per = 1 / S["number"]
    return {k: v * per for k, v in S.items()}


def scaled(S, factor):
    """The table's particles' absorption scaled; the tail's (and the spheres' above) kept."""
    table_abs = (S["ext"] - S["tail_ext"] - S["top_ext"]) - (S["sca"] - S["tail_sca"] - S["top_sca"])
    sphere_abs = (S["tail_ext"] - S["tail_sca"]) + (S["top_ext"] - S["top_sca"])
    out = dict(S)
    out["ext"] = S["sca"] + table_abs * factor + sphere_abs
    return out


def mode_optics(mdir, name, kind, r_eff, v_eff, beyond="refuse"):
    index_file = json.load(open(os.path.join(mdir, name)))
    table = json.load(open(os.path.join(mdir, "phase", name)))
    index = index_at(index_file, WAVELENGTH_NM)
    yang = table["model"] == "yang2013"
    nodes = table["nodes"]
    last = len(nodes) - 1
    i = 0
    while i < last - 1 and nodes[i + 1]["wavelengthNm"] <= WAVELENGTH_NM:
        i += 1
    lower, upper = nodes[i], nodes[min(i + 1, last)]
    t = 0.0 if upper is lower else min(max((WAVELENGTH_NM - lower["wavelengthNm"]) / (upper["wavelengthNm"] - lower["wavelengthNm"]), 0), 1)

    def sums(node):
        raw = node_sums(table, node, index, kind, r_eff, v_eff, yang, beyond)
        scales = (node["kTable"] > node["k"] or yang) and node["kTable"] > 0
        return raw, scaled(raw, index[1] / node["kTable"]) if scales else raw

    a_raw, a = sums(lower)
    b_raw, b = sums(upper) if t > 0 else (a_raw, a)
    mix = lambda p, q: {key: (1 - t) * p[key] + t * q[key] for key in p}
    S, R = mix(a, b), mix(a_raw, b_raw)
    table_ext = R["ext"] - R["tail_ext"] - R["top_ext"]
    table_albedo = (R["sca"] - R["tail_sca"] - R["top_sca"]) / table_ext if table_ext > 0 else R["sca"] / R["ext"]
    floored = lower["kTable"] > lower["k"] or (t > 0 and upper["kTable"] > upper["k"])
    return dict(q_ext=S["ext"] / S["area"], omega=S["sca"] / S["ext"], table_albedo=table_albedo,
                floored=floored, tail_share=S["tail_sca"] / S["sca"], top_share=S["top_sca"] / S["sca"],
                area_um2=S["area"], index=index, t=t)


if __name__ == "__main__":
    mdir, name = sys.argv[1], sys.argv[2]
    args = sys.argv[3:]
    beyond = "refuse"
    if args and args[0] in ("refuse", "spheres"):
        beyond, args = args[0], args[1:]
    for j in range(0, len(args), 3):
        kind, r_eff, v_eff = args[j], float(args[j + 1]), float(args[j + 2])
        o = mode_optics(mdir, name, kind, r_eff, v_eff, beyond)
        print(f"{name} {kind} r_eff {r_eff} um v_eff {v_eff}: Q_ext {o['q_ext']:.10g} omega {o['omega']:.10g} "
              f"table omega {o['table_albedo']:.6g} floored {o['floored']} tail {o['tail_share']:.3g} top {o['top_share']:.3g} "
              f"(index {o['index'][0]:.6g} + {o['index'][1]:.4g}i, t {o['t']:.4f})")
