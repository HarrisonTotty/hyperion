"""Reduce the narrow hazes' particle tables onto the sim's twelve optics bins (plan 14, P14.T24.h.b,
the lane `fix-dark-sub-neptune`), writing
`crates/hyperion-sim/src/planetary/derive/photometry/envelope_bond/narrow.rs`.

Run from the repository root, with NumPy:

    uv run --with numpy python3 -I crates/hyperion-fit/data/nicholls_2025_dayspring/reduce_narrow.py
    cargo fmt -p hyperion-sim

`reduce.py`'s particle tables are Mie spheres of a sigma_g 2 log-normal (v_eff 0.62), the decks'
width. A haze the record draws as Hansen and Travis's (1974) gamma distribution of v_eff 0.05 (Gao
et al. 2020's soot) is far narrower: at 50 nm its single-scattering albedo is some 30-50% below the
wide table's across the visible. This writes, for each material in NARROW, the same table as
`reduce.py`'s (extinction over its 550 nm value, omega and g, by effective radius and bin, at the
same radii, bins, wavelengths and index files) over that gamma distribution instead. It reads only
the client's index files, so it runs without the Dayspring cache (`reduce.py` parses it as it
loads): the few of `reduce.py`'s helpers it needs are restated here, and `--check` first rebuilds
each material's wide table by them and `reduce.averaged`'s log-normal and compares it with the
committed `opacity.rs`'s, figure for figure.
"""
import hashlib
import json
import math
import pathlib
import re
import sys

import numpy as np

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import mie  # noqa: E402

REPO = str(HERE.parents[3])
OUT = REPO + "/crates/hyperion-sim/src/planetary/derive/photometry/envelope_bond/narrow.rs"
OPACITY = REPO + "/crates/hyperion-sim/src/planetary/derive/photometry/envelope_bond/opacity.rs"
MAT = REPO + "/apps/hyperion/src/renderer/src/view/atmosphere/materials/"
FIT = REPO + "/crates/hyperion-fit/data/"
# `reduce.py`'s: the optics' bin edges (nm), the radii (m), and the materials' rows it reads here
# (key, client file, infrared source, large-particle g): soot with no infrared set, and tholin with
# Khare et al.'s (1984) own index beyond 780 nm (`khare_1984_tholin/nk.csv`, which the regolith's
# infrared reads), as `reduce.index_at` takes an infrared set: n held at the client file's last, k
# linear in wavelength.
EDGES = [300, 390, 495, 595, 700, 780, 1000, 1300, 1700, 2200, 2800, 3500, 5000]
NB = 12
RADII = [1e-8, 3e-8, 1e-7, 3e-7, 1e-6, 3e-6, 1e-5, 3e-5, 1e-4]
MATERIALS = [("soot", "soot.json", None, None), ("tholin", "tholin.json", "tholin", None)]
INFRARED = {"tholin": "khare_1984_tholin/nk.csv"}
WIDE_VEFF = math.exp(math.log(2.0) ** 2) - 1  # sigma_g 2 log-normal, `reduce.VEFF`


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def ir_k(src):
    """`reduce.ir_k` for the infrared sources here: (lambda nm, k) of the set's rows."""
    if src is None:
        return None
    d = np.loadtxt(FIT + INFRARED[src], delimiter=",", skiprows=1)
    return d[:, 0] * 1000, d[:, 2]


def index_at(fname, lam, ir=None):
    """`reduce.index_at`: the file's n and k, linear, and beyond its last row its last n and its
    last k, or with an infrared set `ir` that set's k, linear, from its first row on."""
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


def fmt(x):
    if x == 0:
        return "0.0"
    m, e = f"{x:.5e}".split("e")
    return f"{m}e{int(e)}"


def arr(v):
    if isinstance(v, (list, tuple, np.ndarray)):
        return "[" + ", ".join(arr(x) for x in v) + "]"
    return fmt(float(v))
# The gamma particles' effective variance, every photochemical haze rule's (`photochem::gamma`).
VEFF = 0.05
# The materials whose gamma modes read a narrow table: soot, and from the lane
# `fix-dark-sub-neptune-2` Khare et al.'s tholin, the warm envelopes' bright haze, which has no wide
# table (`reduce.py` reads no tholin). The hazes fitted on the cold
# giants' measured albedos and colours (the chromophore, the ammonia and ice giants' hazes) keep
# the wide table their optical depths were fitted with.
NARROW = ["soot", "tholin"]
# The narrow tables' effective radii, m: eight a decade from 10 nm to 100 um. A narrow small
# particle's omega rises about as r^3, so the wide table's half-decade steps, read linear in ln r,
# would put 50 nm soot's V-band omega some 40% high; at eight a decade the error is a few per cent.
NARROW_RADII = [1e-8 * 10 ** (k / 8) for k in range(33)]
# The gamma's radii, linear in r over [0.05, 3] r_eff: its mean is 0.9 r_eff and its deviation
# 0.21 r_eff at v_eff 0.05, so the range holds it to four deviations below and ten above (its
# weight below is under 1e-16).
GAMMA_POINTS = 96


def weighted(lam_nm, r, area, m, g_large):
    """`reduce.averaged`'s area-weighted Q_ext, omega and g over radii `r` of weights `area`:
    BHMIE up to x 150, beyond it the geometric limit (Q_ext 2, Q_abs = 1 - exp(-alpha 4r/3), g held
    at its Mie value at x 150 or the crystal's g_large)."""
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


def log_normal_averaged(lam_nm, reff, m, g_large, npts=24):
    """`reduce.averaged`: the sigma_g 2 log-normal of effective radius reff."""
    rg, ls = mie.lognormal_from_eff(reff, WIDE_VEFF)
    lnr = np.linspace(math.log(rg) - 3.5 * ls, math.log(rg) + 3.5 * ls, npts)
    nr = np.exp(-((lnr - math.log(rg)) ** 2) / (2 * ls * ls))
    r = np.exp(lnr)
    return weighted(lam_nm, r, nr * r * r, m, g_large)


def gamma_averaged(lam_nm, reff, m, g_large):
    """Area-weighted Q_ext, omega, g of Hansen and Travis's gamma, n(r) ~ r^((1 - 3b)/b)
    exp(-r/(a b)) with a = reff and b = VEFF, as `reduce.averaged` takes its log-normal: BHMIE up
    to x 150, beyond it the geometric limit."""
    a, b = reff, VEFF
    r = np.linspace(0.05 * a, 3.0 * a, GAMMA_POINTS)
    ln_n = ((1 - 3 * b) / b) * np.log(r / a) - r / (a * b)
    nr = np.exp(ln_n - ln_n.max())
    return weighted(lam_nm, r, nr * r * r, m, g_large)


def tables(averaged, radii):
    """`reduce.particle_tables` over the distribution `averaged` gives at the effective radii
    `radii`, for NARROW's materials."""
    out = {}
    for key, fname, irsrc, g_large in MATERIALS:
        if key not in NARROW:
            continue
        ir = ir_k(irsrc)
        m550 = index_at(fname, 550.0, ir)
        rows = []
        for r in radii:
            q550, _, _ = averaged(550.0, r, m550, g_large)
            per_bin = []
            for b in range(NB):
                acc = [0.0, 0.0, 0.0]
                lams = bin_lams(b, 3 if r > 3e-6 else 5)
                for lam in lams:
                    m = index_at(fname, lam, ir)
                    q, w, g = averaged(lam, r, m, g_large)
                    acc[0] += q
                    acc[1] += q * w
                    acc[2] += q * w * g
                q = acc[0] / len(lams)
                w = acc[1] / acc[0]
                g = acc[2] / acc[1] if acc[1] > 0 else 0.0
                per_bin.append((q / q550, w, g))
            rows.append(per_bin)
        out[key] = rows
    return out


def committed_keys():
    """The committed `opacity.rs`'s PARTICLE_KEYS."""
    text = open(OPACITY).read()
    keys = re.search(r"PARTICLE_KEYS: \[&str; \d+\] = \[([^\]]*)\]", text).group(1)
    return [k.strip().strip('"') for k in keys.split(",") if k.strip()]


def committed_wide(key):
    """The committed `opacity.rs`'s PARTICLES table of `key`, as its figures print."""
    text = open(OPACITY).read()
    keys = re.search(r"PARTICLE_KEYS: \[&str; \d+\] = \[([^\]]*)\]", text).group(1)
    keys = [k.strip().strip('"') for k in keys.split(",") if k.strip()]
    body = text[text.index("pub(crate) static PARTICLES"):]
    body = body[body.index("= [") + 2:]
    numbers = [float(x) for x in re.findall(r"-?\d+\.\d+(?:e-?\d+)?", body)]
    per = len(RADII) * NB * 3
    i = keys.index(key)
    return numbers[i * per:(i + 1) * per]


if __name__ == "__main__":
    if "--check" in sys.argv:
        wide = tables(log_normal_averaged, RADII)
        for key in NARROW:
            if key not in committed_keys():
                print(f"{key}: no wide table in opacity.rs to check against")
                continue
            ours = [float(fmt(x)) for r in wide[key] for b in r for x in b]
            theirs = committed_wide(key)
            assert ours == theirs, f"{key}: the restated helpers miss reduce.py's committed table"
            print(f"checked {key}: {len(ours)} figures as reduce.py's")
    narrow = tables(gamma_averaged, NARROW_RADII)
    files = [(key, fname) for key, fname, _, _ in MATERIALS if key in NARROW]
    infrared = [INFRARED[src] for key, _, src, _ in MATERIALS if key in NARROW and src is not None]
    W = lambda f, s: f.write(s + "\n")
    with open(OUT, "w") as f:
        W(f, "//! The narrow hazes' particle tables on the optics' twelve bins (P14.T24.h.b, from generator")
        W(f, "//! version 32), reduced by `crates/hyperion-fit/data/nicholls_2025_dayspring/reduce_narrow.py`")
        W(f, "//! from these inputs (SHA-256):")
        W(f, "//!")
        for key, fname in files:
            W(f, f"//! - `apps/hyperion/src/renderer/src/view/atmosphere/materials/{fname}` `{sha(MAT + fname)}`")
        for path in infrared:
            W(f, f"//! - `crates/hyperion-fit/data/{path}` `{sha(FIT + path)}`")
        W(f, "//!")
        W(f, "//! Each is [`super::opacity::PARTICLES`]' table over Hansen and Travis's (1974) gamma")
        W(f, f"//! distribution of effective variance {VEFF} in place of its `σ_g` 2 log-normal, at radii eight a")
        W(f, "//! decade ([`NARROW_RADII_M`]); what it holds and its use are [`super`]'s documentation. Not")
        W(f, "//! edited by hand.")
        W(f, "")
        W(f, "/// The effective variance of the narrow tables' gamma distribution.")
        W(f, f"pub(crate) const NARROW_EFFECTIVE_VARIANCE: f64 = {VEFF};")
        W(f, "/// The effective radii of the narrow tables, m: eight a decade from 10 nm to 100 µm.")
        W(f, f"pub(crate) const NARROW_RADII_M: [f64; {len(NARROW_RADII)}] = {arr(NARROW_RADII)};")
        W(f, "/// The narrow tables' materials, by registry key.")
        W(f, f"pub(crate) const NARROW_KEYS: [&str; {len(files)}] = [" + ", ".join(f'"{k}"' for k, _ in files) + "];")
        W(f, "/// Each material's [extinction over its 550 nm value, ω, g] by radius and bin.")
        W(f, f"pub(crate) static NARROW_PARTICLES: [[[[f64; 3]; 12]; {len(NARROW_RADII)}]; {len(files)}] = [" + ", ".join(arr(narrow[k]) for k, _ in files) + "];")
    print("written", OUT)
