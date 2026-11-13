"""The phase integral q of a hazy envelope's column (plan 14, P14.T24.h.b; the lane
`fix-dark-sub-neptune-2`), the source of `photometry::BRIGHT_HAZE_PHASE_INTEGRAL` and of its doc
comment's check on a mixed slab. Prints, at 450, 550 and 650 nm, each case's geometric albedo p,
spherical albedo A_s and q = A_s / p.

Run from the repository root, with NumPy:

    uv run --with numpy python3 -I crates/hyperion-fit/data/nicholls_2025_dayspring/phase_integral.py

The particles are Mie spheres (Bohren and Huffman's BHMIE, its amplitude functions S1 and S2 on
2,001 cosines) over Hansen and Travis's (1974) gamma distribution of effective variance 0.05, at
the client's index files (`apps/hyperion/src/renderer/src/view/atmosphere/materials/`): Khare et
al.'s (1984) `tholin.json` at an effective radius of 0.1 um, the bright haze's, and `soot.json` at
50 nm, Gao et al.'s (2020) soot's. The column is a homogeneous slab of the haze (or hazes, mixed
by their scattering optical depths) of the given tau(550) over a semi-infinite Rayleigh gas of
single-scattering albedo 0.9, plane-parallel, lit as a sphere is (mu_0 = sqrt(u), so that the
escaping weight is the sphere's spherical albedo), each photon's scattering towards opposition
added to p with its attenuation along the way out (peel-off), unpolarised (scalar), 150,000
photons a band from a fixed seed. The checks: an isotropic conservative semi-infinite atmosphere's
p, 0.6897 exact (Chandrasekhar 1950), and an isotropic dark one's q, near a Lommel-Seeliger
sphere's 16 (1 - ln 2) / 3 = 1.6366.
"""
import json
import math
import pathlib
import sys

import numpy as np

REPO = pathlib.Path(__file__).resolve().parents[4]
MAT = REPO / "apps/hyperion/src/renderer/src/view/atmosphere/materials"
VEFF = 0.05
MUS = np.linspace(-1.0, 1.0, 2001)
PHOTONS = 150_000
GAS_OMEGA = 0.9
TAU_MAX = 300.0


def index(name, lam_nm):
    """The file's n + ik at a wavelength it lists."""
    d = json.load(open(MAT / name))
    i = d["wavelengthsNm"].index(lam_nm)
    return complex(d["n"][i], d["k"][i])


def bhmie(x, m):
    """Q_ext, Q_sca and S11(mu) on MUS of a sphere of size parameter x and index m."""
    y = m * x
    nstop = int(round(x + 4.05 * x ** (1 / 3) + 2))
    nmx = int(round(max(nstop, abs(y)) + 15))
    d = np.zeros(nmx + 1, dtype=complex)
    for n in range(nmx - 1, 0, -1):
        t = (n + 1) / y
        d[n] = t - 1 / (d[n + 1] + t)
    psi0, psi1 = math.cos(x), math.sin(x)
    chi0, chi1 = -math.sin(x), math.cos(x)
    xi1 = complex(psi1, -chi1)
    qsca = qext = 0.0
    s1 = np.zeros(MUS.size, dtype=complex)
    s2 = np.zeros(MUS.size, dtype=complex)
    pi0, pi1 = np.zeros(MUS.size), np.ones(MUS.size)
    for n in range(1, nstop + 1):
        fn = (2 * n + 1) / (n * (n + 1))
        psi = (2 * n - 1) * psi1 / x - psi0
        chi = (2 * n - 1) * chi1 / x - chi0
        xi = complex(psi, -chi)
        an = ((d[n] / m + n / x) * psi - psi1) / ((d[n] / m + n / x) * xi - xi1)
        bn = ((d[n] * m + n / x) * psi - psi1) / ((d[n] * m + n / x) * xi - xi1)
        qsca += (2 * n + 1) * (abs(an) ** 2 + abs(bn) ** 2)
        qext += (2 * n + 1) * (an.real + bn.real)
        tau = n * MUS * pi1 - (n + 1) * pi0
        s1 += fn * (an * pi1 + bn * tau)
        s2 += fn * (an * tau + bn * pi1)
        pi0, pi1 = pi1, ((2 * n + 1) * MUS * pi1 - (n + 1) * pi0) / n
        psi0, psi1, chi0, chi1, xi1 = psi1, psi, chi1, chi, xi
    s11 = 0.5 * (np.abs(s1) ** 2 + np.abs(s2) ** 2)
    return 2 * qext / x**2, 2 * qsca / x**2, s11


def polydisperse(r_eff, lam_nm, m, nr=400):
    """Extinction cross section, omega and the phase function P(mu), normalised to a mean of 1 over
    mu, of the gamma distribution of effective radius r_eff (m) at wavelength lam_nm."""
    lam = lam_nm * 1e-9
    rmin = r_eff * max(1e-3, 1 - 12 * math.sqrt(VEFF)) * 0.05
    rmax = r_eff * (1 + 12 * math.sqrt(VEFF))
    ext = sca = 0.0
    phase = np.zeros(MUS.size)
    for i, r in enumerate(np.linspace(rmin, rmax, nr + 1)):
        w = 0.5 if i in (0, nr) else 1.0
        n = r ** ((1 - 3 * VEFF) / VEFF) * math.exp(-r / (r_eff * VEFF))
        qe, qs, s11 = bhmie(2 * math.pi * r / lam, m)
        area = math.pi * r * r
        ext += w * n * area * qe
        sca += w * n * area * qs
        phase += w * n * s11
    phase *= 2 / np.trapezoid(phase, MUS)
    return ext, sca / ext, phase


class Phase:
    """A tabulated phase function on MUS: its value and a sampler of the scattering cosine."""

    def __init__(self, p):
        self.p = p
        cdf = np.concatenate([[0.0], np.cumsum(0.5 * (p[1:] + p[:-1]) * np.diff(MUS))])
        self.cdf = cdf / cdf[-1]

    def at(self, mu):
        return np.interp(mu, MUS, self.p)

    def sample(self, u):
        return np.interp(u, self.cdf, MUS)


ISOTROPIC = Phase(np.ones(MUS.size))
RAYLEIGH = Phase(0.75 * (1 + MUS * MUS))


def turn(d, ct, phi):
    """The directions d (N x 3) turned by the scattering cosines ct at the azimuths phi."""
    st = np.sqrt(np.maximum(0.0, 1 - ct * ct))
    cp, sp = np.cos(phi), np.sin(phi)
    ux, uy, uz = d[:, 0], d[:, 1], d[:, 2]
    s = np.sqrt(np.maximum(1e-30, 1 - uz * uz))
    out = np.empty_like(d)
    out[:, 0] = st * (ux * uz * cp - uy * sp) / s + ux * ct
    out[:, 1] = st * (uy * uz * cp + ux * sp) / s + uy * ct
    out[:, 2] = -st * cp * s + uz * ct
    polar = np.abs(uz) > 0.99999
    out[polar, 0] = (st * cp)[polar]
    out[polar, 1] = (st * sp)[polar]
    out[polar, 2] = (ct * np.sign(uz))[polar]
    return out


def run(tau_haze, omega_haze, phase_haze, omega_gas, phase_gas, rng):
    """p, A_s and q of a haze slab over a semi-infinite gas (the module's Monte Carlo)."""
    n = PHOTONS
    mu0 = np.sqrt(rng.random(n))
    s0 = np.sqrt(1 - mu0 * mu0)
    d = np.stack([s0, np.zeros(n), -mu0], axis=1)
    o = np.stack([-s0, np.zeros(n), mu0], axis=1)
    tau = np.zeros(n)
    w = np.ones(n)
    alive = np.ones(n, dtype=bool)
    p_sum = a_sum = 0.0
    while alive.any():
        idx = np.flatnonzero(alive)
        tau[idx] += -np.log(rng.random(idx.size)) * (-d[idx, 2])
        out = tau[idx] < 0
        a_sum += w[idx][out].sum()
        alive[idx[out]] = False
        alive[idx[tau[idx] > TAU_MAX]] = False
        idx = np.flatnonzero(alive)
        if idx.size == 0:
            break
        in_haze = tau[idx] < tau_haze
        omega = np.where(in_haze, omega_haze, omega_gas)
        ct = np.einsum("ij,ij->i", d[idx], o[idx])
        p_val = np.where(in_haze, phase_haze.at(ct), phase_gas.at(ct))
        p_sum += (w[idx] * omega * p_val * np.exp(-tau[idx] / mu0[idx])).sum()
        w[idx] *= omega
        low = w[idx] < 1e-3
        survive = rng.random(idx.size) < 0.1
        w[idx[low & survive]] *= 10
        alive[idx[low & ~survive]] = False
        idx = np.flatnonzero(alive)
        if idx.size == 0:
            break
        in_haze = tau[idx] < tau_haze
        u = rng.random(idx.size)
        ct = np.where(in_haze, phase_haze.sample(u), phase_gas.sample(u))
        d[idx] = turn(d[idx], ct, 2 * math.pi * rng.random(idx.size))
    p = p_sum / (4 * n)
    a = a_sum / n
    return p, a, a / p


def column(hazes, lam_nm, rng):
    """The light of a well-mixed slab of `hazes`, (file, r_eff m, tau(550)), over the gas."""
    taus, scas, phases = [], [], []
    for name, r_eff, tau550 in hazes:
        ext550, _, _ = polydisperse(r_eff, 550, index(name, 550))
        ext, omega, phase = polydisperse(r_eff, lam_nm, index(name, lam_nm))
        tau = tau550 * ext / ext550
        taus.append(tau)
        scas.append(tau * omega)
        phases.append(phase)
    tau, sca = sum(taus), sum(scas)
    phase = Phase(sum(s * ph for s, ph in zip(scas, phases)) / sca)
    shares = [s / sca for s in scas]
    return run(tau, sca / tau, phase, GAS_OMEGA, RAYLEIGH, rng), shares


def main():
    rng = np.random.Generator(np.random.PCG64(12345))
    p, a, q = run(0.0, 1.0, ISOTROPIC, 1.0, ISOTROPIC, rng)
    print(f"check: isotropic conservative, p {p:.4f} (0.6897 exact), A_s {a:.4f}, q {q:.3f}")
    p, a, q = run(0.0, 1.0, ISOTROPIC, 0.1, ISOTROPIC, rng)
    print(f"check: isotropic omega 0.1, p {p:.5f}, A_s {a:.5f}, q {q:.3f} (1.6366 single)")
    cases = [
        ("the bright haze, tau(550) 10", [("tholin.json", 1e-7, 10.0)]),
        ("the bright haze, tau(550) 3", [("tholin.json", 1e-7, 3.0)]),
        (
            "the owner's draw: soot 2.35 with the bright haze 2.80",
            [("soot.json", 5e-8, 2.35), ("tholin.json", 1e-7, 2.80)],
        ),
    ]
    for label, hazes in cases:
        for lam in (450, 550, 650):
            (p, a, q), shares = column(hazes, lam, rng)
            share = shares[-1]
            print(f"{label}: {lam} nm p {p:.4f} A_s {a:.4f} q {q:.3f}; the last haze's share of the scattering {share:.3f}")
    sys.stdout.flush()


if __name__ == "__main__":
    main()
