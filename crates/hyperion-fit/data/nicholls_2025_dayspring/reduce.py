"""Reduce the envelope albedo's opacities onto the sim's twelve optics bins (plan 14, P14.T24.h.b
part 5), writing `crates/hyperion-sim/src/planetary/derive/photometry/envelope_bond/opacity.rs`.

Run from the repository root, with NumPy:

    uv run --with numpy python3 -I crates/hyperion-fit/data/nicholls_2025_dayspring/reduce.py
    cargo fmt -p hyperion-sim

Inputs, each named with its SHA-256 in the module it writes:

- the client's `absorbers/crossSections.json`: Karkoschka and Tomasko 2010's CH4 (380-800 nm, 0.25
  nm, 100/198/296 K, their Eq. 8 in temperature over 50-300 K, held beyond);
- the client's `absorbers/alkaliPairs.json`: the Na and K doublets' far wings with H2 and He;
- the SOCRATES spectral files `Dayspring` (this directory's PROVENANCE.toml), from
  `crates/hyperion-fit/data/cache/nicholls_2025_dayspring/{48,256}/`: CH4 from the 256-band file
  over 851-1,024 nm and the 48-band file beyond, H2O and CO and H2-H2's pairs from the 48-band file;
- the client's material index files (380-780 nm) and hyperion-fit's infrared sets (water ice,
  CH4 ice, NH3 ice, ZnS, MgSiO3 and Mg2SiO4), for the particles' Mie tables (`mie.py`, BHMIE).

Each bin's sub-elements (0.25 nm rows; each band's k-terms by its overlap with the bin; the bin's
uncovered rest at k 0) are sorted by k at a reference state (1 bar and 200 K for CH4 and the pairs,
1 bar and 1,600 K for H2O, CO and the alkalis), and their k averaged, weighted, into eight
g-points of widths G_W at every pressure and temperature node.
"""
import hashlib
import json
import math
import pathlib
import sys

import numpy as np

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from socrates import parse_sf, parse_sfk  # noqa: E402
import mie  # noqa: E402

REPO = str(HERE.parents[3])
OUT = REPO + "/crates/hyperion-sim/src/planetary/derive/photometry/envelope_bond/opacity.rs"
CACHE = HERE.parent / "cache" / "nicholls_2025_dayspring"
DS48 = str(CACHE / "48" / "Dayspring")
DS256 = str(CACHE / "256" / "Dayspring")
EDGES = [300, 390, 495, 595, 700, 780, 1000, 1300, 1700, 2200, 2800, 3500, 5000]
NB = 12
G_EDGES = [0.0, 0.25, 0.5, 0.7, 0.85, 0.93, 0.97, 0.99, 1.0]
G_W = [G_EDGES[i + 1] - G_EDGES[i] for i in range(8)]
P_NODES = [1e2, 1e3, 1e4, 1e5, 1e6, 1e7]
T_NODES = [100.0, 200.0, 400.0, 800.0, 1600.0, 2400.0]
NA = 6.02214076e23
M_CH4 = 16.0425e-3  # kg/mol (NIST, 2018 atomic weights)
SPLIT_256_48 = 1024.421176  # nm, the /48 file's band 12 lower edge


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


# ---------------------------------------------------------------- sub-elements
def group(elems, ref_key):
    """elems: list of (weight, values dict {(p,T): k}); sorted by ref_key's k; returns per g-point the
    weighted mean of each node's k (members split at the g boundaries)."""
    tot = sum(w for w, _ in elems)
    if tot <= 0:
        return [[0.0] * (len(P_NODES) * len(T_NODES)) for _ in range(8)]
    order = sorted(elems, key=lambda e: e[1][ref_key])
    out = [np.zeros(len(P_NODES) * len(T_NODES)) for _ in range(8)]
    acc = [0.0] * 8
    c = 0.0
    for w, vals in order:
        lo, hi = c / tot, (c + w) / tot
        c += w
        vec = np.array([vals[(p, t)] for p in P_NODES for t in T_NODES])
        for j in range(8):
            a, b = max(lo, G_EDGES[j]), min(hi, G_EDGES[j + 1])
            if b > a:
                out[j] += (b - a) * vec
                acc[j] += b - a
    return [list(out[j] / acc[j]) if acc[j] > 0 else [0.0] * len(out[j]) for j in range(8)]


def kt_elements():
    d = json.load(open(REPO + "/apps/hyperion/src/renderer/src/view/atmosphere/absorbers/crossSections.json"))
    t = [x for x in d["tables"] if x["species"] == "CH4"][0]
    rows = np.array(t["rows"])
    temps = t["temperaturesK"]
    lam = rows[:, 0]
    sig = rows[:, 1:]

    def at(i, T):
        T = min(max(T, 50.0), 300.0)
        s = sig[i]
        if (s > 0).all():
            t0, t1, t2 = temps
            ls = np.log(s)
            l0 = (T - t1) * (T - t2) / ((t0 - t1) * (t0 - t2))
            l1 = (T - t0) * (T - t2) / ((t1 - t0) * (t1 - t2))
            l2 = (T - t0) * (T - t1) / ((t2 - t0) * (t2 - t1))
            return math.exp(l0 * ls[0] + l1 * ls[1] + l2 * ls[2])
        return float(np.interp(T, temps, s))

    out = []
    for i in range(len(lam)):
        vals = {(p, T): at(i, T) * NA / M_CH4 for p in P_NODES for T in T_NODES}
        out.append((float(lam[i]), vals))
    return out, t


def interp_pt(table, pressures, temps, p, T):
    lp = np.log(pressures)
    x = min(max(math.log(p), lp[0]), lp[-1])
    i = max(min(np.searchsorted(lp, x) - 1, len(lp) - 2), 0)
    fx = (x - lp[i]) / (lp[i + 1] - lp[i])
    tg = temps[i]
    y = min(max(T, tg[0]), tg[-1])
    j = max(min(np.searchsorted(tg, y) - 1, len(tg) - 2), 0)
    fy = (y - tg[j]) / (tg[j + 1] - tg[j])
    lk = np.log(np.maximum(table, 1e-300))
    v = (1 - fx) * ((1 - fy) * lk[:, i, j] + fy * lk[:, i, j + 1]) + fx * ((1 - fy) * lk[:, i + 1, j] + fy * lk[:, i + 1, j + 1])
    o = np.exp(v)
    o[o < 1e-250] = 0.0
    return o


SF48 = parse_sf(DS48 + ".sf"); P48, T48, K48, C48 = parse_sfk(DS48 + ".sf_k")
SF256 = parse_sf(DS256 + ".sf"); P256, T256, K256, C256 = parse_sfk(DS256 + ".sf_k")
GAS = {"H2O": 1, "CO": 4, "CH4": 5}


def socrates_elements(b, gas, use256_below=None):
    """Sub-elements (weight in nm, vals) of `gas` in bin b from the /48 file, and for CH4 from /256
    below SPLIT_256_48."""
    lo, hi = EDGES[b], EDGES[b + 1]
    el = []
    covered = 0.0
    if gas == "CH4":
        for band, (l1, l2) in enumerate(SF256["bands"], start=1):
            a, c = l1 * 1e9, min(l2 * 1e9, SPLIT_256_48)
            ov = min(hi, c) - max(lo, a)
            if ov > 0 and (band, 5) in K256:
                ws = SF256["terms"][(band, 5)]
                ks = {(p, T): interp_pt(K256[(band, 5)], P256, T256, p, T) for p in P_NODES for T in T_NODES}
                for t, w in enumerate(ws):
                    el.append((ov * w, {key: float(v[t]) for key, v in ks.items()}))
                covered += ov
        floor48 = SPLIT_256_48
    else:
        floor48 = 0.0
    for band, (l1, l2) in enumerate(SF48["bands"], start=1):
        a, c = max(l1 * 1e9, floor48), l2 * 1e9
        ov = min(hi, c) - max(lo, a)
        if ov > 0 and (band, GAS[gas]) in K48:
            ws = SF48["terms"][(band, GAS[gas])]
            ks = {(p, T): interp_pt(K48[(band, GAS[gas])], P48, T48, p, T) for p in P_NODES for T in T_NODES}
            for t, w in enumerate(ws):
                el.append((ov * w, {key: float(v[t]) for key, v in ks.items()}))
            covered += ov
    return el, covered


def gas_tables():
    kt, kt_meta = kt_elements()
    tables = {}
    for gas in ("CH4", "H2O", "CO"):
        per_bin = []
        for b in range(NB):
            lo, hi = EDGES[b], EDGES[b + 1]
            el = []
            covered = 0.0
            if gas == "CH4":
                for lam, vals in kt:
                    if lo <= lam < hi:
                        el.append((0.25, vals))
                        covered += 0.25
            s_el, s_cov = socrates_elements(b, gas)
            el += s_el
            covered += s_cov
            gap = (hi - lo) - covered
            if gap > 1e-9:
                el.append((gap, {(p, T): 0.0 for p in P_NODES for T in T_NODES}))
            ref = (1e5, 200.0) if gas == "CH4" else (1e5, 1600.0)
            per_bin.append(group(el, ref))
        tables[gas] = per_bin
    return tables, kt_meta


def cia_table():
    """H2-H2 (continuum 3 of /48), m5 kg-2: per bin per g-point at T_NODES."""
    temps = C48["temps"]
    per_bin = []
    for b in range(NB):
        lo, hi = EDGES[b], EDGES[b + 1]
        el = []
        covered = 0.0
        for band, (l1, l2) in enumerate(SF48["bands"], start=1):
            ov = min(hi, l2 * 1e9) - max(lo, l1 * 1e9)
            if ov > 0 and (band, 3) in C48:
                tab = C48[(band, 3)]
                ws = SF48["cont_terms"][(band, 3)]
                for t, w in enumerate(ws):
                    vals = {(p, T): float(np.interp(T, temps, tab[t])) for p in P_NODES for T in T_NODES}
                    el.append((ov * w, vals))
                covered += ov
        gap = (hi - lo) - covered
        if gap > 1e-9:
            el.append((gap, {(p, T): 0.0 for p in P_NODES for T in T_NODES}))
        g = group(el, (1e5, 200.0))
        # p-independent: keep the first pressure's T row
        per_bin.append([row[: len(T_NODES)] for row in g])
    return per_bin


def alkali_tables():
    d = json.load(open(REPO + "/apps/hyperion/src/renderer/src/view/atmosphere/absorbers/alkaliPairs.json"))
    out = {}
    for pair in d["pairs"]:
        rows = np.array(pair["rows"])
        temps = pair["temperaturesK"]
        per_bin = []
        for b in range(NB):
            lo, hi = EDGES[b], EDGES[b + 1]
            el = []
            covered = 0.0
            for r in rows:
                lam = r[0]
                # the cell [lam - 0.5, lam + 0.5)
                a, c = lam - 0.5, lam + 0.5
                ov = min(hi, c) - max(lo, a)
                if ov > 0:
                    vals = {(p, T): float(np.interp(T, temps, r[1:])) for p in P_NODES for T in T_NODES}
                    el.append((ov, vals))
                    covered += ov
            gap = (hi - lo) - covered
            if gap > 1e-9:
                el.append((gap, {(p, T): 0.0 for p in P_NODES for T in T_NODES}))
            g = group(el, (1e5, 1600.0))
            per_bin.append([row[: len(T_NODES)] for row in g])
        out[pair["pair"]] = per_bin
    return out, d


# ---------------------------------------------------------------- particles
MAT = REPO + "/apps/hyperion/src/renderer/src/view/atmosphere/materials/"
FIT = REPO + "/crates/hyperion-fit/data/"
MATERIALS = [
    # key, client file, infrared (alpha source), large-particle g (None: Mie)
    ("H2O", "water.json", "water_ice", None),
    ("H2O_ice", "water-ice.json", "water_ice", 0.75),
    ("NH3", "ammonia-ice.json", "nh3", 0.75),
    ("NH4SH", "ammonium-hydrosulphide.json", "nh3", 0.75),
    ("CH4", "methane-ice.json", "ch4", 0.75),
    ("H2S", "hydrogen-sulphide-ice.json", None, 0.75),
    ("KCl", "potassium-chloride.json", None, None),
    ("Na2S", "sodium-sulphide.json", None, None),
    ("ZnS", "zinc-sulphide.json", "zns", None),
    ("MnS", "manganese-sulphide.json", None, None),
    ("Cr", "chromium.json", None, None),
    ("Fe", "iron.json", None, None),
    ("MgSiO3", "enstatite-glass.json", "mgsio3", None),
    ("Mg2SiO4", "forsterite-amorphous.json", "mg2sio4", None),
    ("Al2O3", "aluminium-oxide.json", None, None),
    ("CaTiO3", "calcium-titanate.json", None, None),
    ("chromophore", "jovian-chromophore.json", None, None),
    ("ice_giant_haze", "ice-giant-haze.json", None, None),
    ("soot", "soot.json", None, None),
    ("S8", "sulphur.json", None, None),
]
RADII = [1e-8, 3e-8, 1e-7, 3e-7, 1e-6, 3e-6, 1e-5, 3e-5, 1e-4]
VEFF = math.exp(math.log(2.0) ** 2) - 1  # sigma_g 2 log-normal


def ir_k(src):
    """(lambda nm, k) beyond 780 nm."""
    if src == "water_ice":
        rows = []
        on = False
        for line in open(FIT + "warren_brandt_2008_h2o/Warren-2008.yml"):
            s = line.strip()
            if s.startswith("data:"):
                on = True
                continue
            if on:
                p = s.split()
                if len(p) == 3:
                    try:
                        rows.append([float(x) for x in p])
                    except ValueError:
                        break
                else:
                    break
        d = np.array(rows)
        return d[:, 0] * 1000, d[:, 2]
    if src == "ch4":
        d = np.loadtxt(FIT + "grundy_2002_ch4/alpha_40k.csv", delimiter=",", skiprows=1)
        return d[:, 0] * 1000, d[:, 1] * 100 * d[:, 0] * 1e-6 / (4 * math.pi)
    if src == "nh3":
        d = np.loadtxt(FIT + "martonchik_1984_nh3/nk.csv", delimiter=",", skiprows=1)
        return d[:, 0] * 1000, d[:, 2]
    for key, fname in (("zns", "palik_1985_zns"), ("mgsio3", "dorschner_1995_mgsio3"), ("mg2sio4", "jager_2003_mg2sio4")):
        if src == key:
            import glob
            f = sorted(glob.glob(FIT + fname + "/*.csv"))[0]
            d = np.genfromtxt(f, delimiter=",", names=True)
            cols = d.dtype.names
            lam = d[cols[0]] * (1000 if d[cols[0]].max() < 100 else 1)
            return lam, d[cols[-1]]
    return None


def index_at(key, fname, ir, lam):
    d = json.load(open(MAT + fname))
    w = np.array(d["wavelengthsNm"]); n = np.array(d["n"]); k = np.array(d["k"])
    if lam <= w[-1]:
        return complex(np.interp(lam, w, n), np.interp(lam, w, k))
    kk = k[-1]
    if ir is not None:
        lw, lk = ir
        if lam >= lw[0]:
            kk = float(np.interp(lam, lw, lk))
    return complex(n[-1], kk)


def bin_lams(b, n=5):
    lo, hi = EDGES[b], EDGES[b + 1]
    return [lo + (hi - lo) * (i + 0.5) / n for i in range(n)]


def particle_tables():
    out = {}
    for key, fname, irsrc, g_large in MATERIALS:
        ir = ir_k(irsrc) if irsrc else None
        m550 = index_at(key, fname, ir, 550.0)
        rows = []
        for r in RADII:
            q550, _, _ = averaged(550.0, r, m550, g_large)
            per_bin = []
            for b in range(NB):
                acc = [0.0, 0.0, 0.0]
                lams = bin_lams(b, 3 if r > 3e-6 else 5)
                for lam in lams:
                    m = index_at(key, fname, ir, lam)
                    q, w, g = averaged(lam, r, m, g_large)
                    acc[0] += q; acc[1] += q * w; acc[2] += q * w * g
                q = acc[0] / len(lams)
                w = acc[1] / acc[0]
                g = acc[2] / acc[1] if acc[1] > 0 else 0.0
                per_bin.append((q / q550, w, g))
            rows.append(per_bin)
        out[key] = rows
    return out


def averaged(lam_nm, reff, m, g_large, npts=24):
    """Area-weighted Q_ext, omega, g of a sigma_g 2 log-normal of effective radius reff: BHMIE
    up to x 150, beyond it the geometric limit (Q_ext 2, Q_abs = 1 - exp(-alpha 4r/3) from the
    index, g held at its Mie value at x 150 or the crystal's g_large)."""
    rg, ls = mie.lognormal_from_eff(reff, VEFF)
    lnr = np.linspace(math.log(rg) - 3.5 * ls, math.log(rg) + 3.5 * ls, npts)
    nr = np.exp(-((lnr - math.log(rg)) ** 2) / (2 * ls * ls))
    r = np.exp(lnr)
    area = nr * r * r
    cext = csca = cg = 0.0
    lam = lam_nm * 1e-9
    g150 = None
    for ri, ai in zip(r, area):
        x = 2 * math.pi * ri / lam
        if x <= 150:
            qe, qs, g = mie.bhmie(x, m)
            if g_large is not None and x > 20:
                g = g_large
        else:
            if g150 is None:
                _, _, g150 = mie.bhmie(150.0, m)
            alpha = 4 * math.pi * m.imag / lam
            qabs = 1 - math.exp(-alpha * 4 * ri / 3)
            qe, qs = 2.0, 2.0 - qabs
            g = g_large if g_large is not None else g150
        cext += ai * qe
        csca += ai * qs
        cg += ai * qs * g
    return cext / area.sum(), csca / cext, cg / csca


# ---------------------------------------------------------------- emit
def fmt(x):
    if x == 0:
        return "0.0"
    m, e = f"{x:.5e}".split("e")
    return f"{m}e{int(e)}"


def arr(v):
    if isinstance(v, (list, tuple, np.ndarray)):
        return "[" + ", ".join(arr(x) for x in v) + "]"
    return fmt(float(v))


if __name__ == "__main__":
    gases, kt_meta = gas_tables()
    cia = cia_table()
    alk, alk_meta = alkali_tables()
    parts = particle_tables()
    inputs = [
        ("apps/hyperion/src/renderer/src/view/atmosphere/absorbers/crossSections.json", sha(REPO + "/apps/hyperion/src/renderer/src/view/atmosphere/absorbers/crossSections.json")),
        ("apps/hyperion/src/renderer/src/view/atmosphere/absorbers/alkaliPairs.json", sha(REPO + "/apps/hyperion/src/renderer/src/view/atmosphere/absorbers/alkaliPairs.json")),
        ("Dayspring/48 Dayspring.sf", sha(DS48 + ".sf")), ("Dayspring/48 Dayspring.sf_k", sha(DS48 + ".sf_k")),
        ("Dayspring/256 Dayspring.sf", sha(DS256 + ".sf")), ("Dayspring/256 Dayspring.sf_k", sha(DS256 + ".sf_k")),
    ]
    for key, fname, _, _ in MATERIALS:
        inputs.append(("apps/hyperion/src/renderer/src/view/atmosphere/materials/" + fname, sha(MAT + fname)))
    for d in ("warren_brandt_2008_h2o/Warren-2008.yml", "grundy_2002_ch4/alpha_40k.csv", "martonchik_1984_nh3/nk.csv",
              "palik_1985_zns/nk.csv", "dorschner_1995_mgsio3/nk.csv", "jager_2003_mg2sio4/nk.csv"):
        inputs.append(("crates/hyperion-fit/data/" + d, sha(FIT + d)))
    def reshape(t):
        return [[[row[i * len(T_NODES):(i + 1) * len(T_NODES)] for i in range(len(P_NODES))] for row in b] for b in t]
    W = lambda f, s: f.write(s + "\n")
    with open(OUT, "w") as f:
        W(f, "//! The envelope albedo's opacities on the optics' twelve bins (P14.T24.h.b part 5), reduced by")
        W(f, "//! `crates/hyperion-fit/data/envelope_opacity/reduce.py` from these inputs (SHA-256):")
        W(f, "//!")
        for k, v in inputs:
            W(f, f"//! - `{k}` `{v}`")
        W(f, "//!")
        W(f, "//! What each table holds, and its sources, is [`super`]'s documentation. Not edited by hand.")
        W(f, "")
        W(f, "/// The weights of a bin's g-points, in ascending order of absorption: their widths in g.")
        W(f, f"pub(crate) const G_WEIGHTS: [f64; 8] = {arr(G_W)};")
        W(f, "/// The pressures of the lines' tables, Pa.")
        W(f, f"pub(crate) const PRESSURES_PA: [f64; 6] = {arr(P_NODES)};")
        W(f, "/// The temperatures of every gas table, K.")
        W(f, f"pub(crate) const TEMPERATURES_K: [f64; 6] = {arr(T_NODES)};")
        lnz = lambda t: [[[[(math.log(v) if v > 0 else -700.0) for v in row] for row in g] for g in b] for b in t]
        for name, gas, what in (("METHANE", "CH4", "CH₄'s lines"), ("WATER", "H2O", "H₂O's lines"), ("CARBON_MONOXIDE", "CO", "CO's lines")):
            W(f, f"/// ln of {what}' mass absorption coefficient (m² kg⁻¹), by bin, g-point, pressure and temperature;")
            W(f, "/// −700 for none (e^−700, below every float a column multiplies it to).")
            W(f, f"pub(crate) static {name}: [[[[f64; 6]; 6]; 8]; 12] = {arr(lnz(reshape(gases[gas])))};")
        W(f, "/// H₂–H₂'s collision-induced absorption, m⁵ kg⁻², by bin, g-point and temperature.")
        W(f, f"pub(crate) const HYDROGEN_PAIRS: [[[f64; 6]; 8]; 12] = {arr(cia)};")
        for name, pair in (("SODIUM_HYDROGEN", "Na+H2"), ("SODIUM_HELIUM", "Na+He"), ("POTASSIUM_HYDROGEN", "K+H2"), ("POTASSIUM_HELIUM", "K+He")):
            W(f, f"/// The {pair} far wings' pair cross-section, m⁵ a pair, by bin, g-point and temperature.")
            W(f, f"pub(crate) const {name}: [[[f64; 6]; 8]; 12] = {arr(alk[pair])};")
        W(f, "/// The effective radii of the particle tables, m (`σ_g` 2 log-normals).")
        W(f, f"pub(crate) const RADII_M: [f64; {len(RADII)}] = {arr(RADII)};")
        W(f, "/// The particle tables' materials, by registry key (`H2O_ice` for water's solid).")
        W(f, f"pub(crate) const PARTICLE_KEYS: [&str; {len(MATERIALS)}] = [" + ", ".join(f'"{m[0]}"' for m in MATERIALS) + "];")
        W(f, "/// Each material's [extinction over its 550 nm value, ω, g] by radius and bin.")
        W(f, f"pub(crate) static PARTICLES: [[[[f64; 3]; 12]; {len(RADII)}]; {len(MATERIALS)}] = [" + ", ".join(arr(parts[m[0]]) for m in MATERIALS) + "];")
    print("written", OUT)
