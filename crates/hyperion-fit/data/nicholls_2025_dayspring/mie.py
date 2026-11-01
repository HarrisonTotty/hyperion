"""BHMIE (Bohren and Huffman 1983, Absorption and Scattering of Light by Small Particles, App. A)
and its average over a log-normal, for `reduce.py`."""
import cmath, math
import numpy as np


def bhmie(x, m):
    nstop = int(x + 4.05 * x ** (1 / 3) + 2)
    y = m * x
    nmx = int(max(nstop, abs(y)) + 15)
    d = [0j] * (nmx + 1)
    for n in range(nmx, 0, -1):
        d[n - 1] = n / y - 1 / (d[n] + n / y)
    psi0, psi1 = math.cos(x), math.sin(x)
    chi0, chi1 = -math.sin(x), math.cos(x)
    xi1 = complex(psi1, -chi1)
    qsca = qext = gsum = 0.0
    an1 = bn1 = 0j
    for n in range(1, nstop + 1):
        fn = (2 * n + 1) / (n * (n + 1))
        psi = (2 * n - 1) * psi1 / x - psi0
        chi = (2 * n - 1) * chi1 / x - chi0
        xi = complex(psi, -chi)
        an = ((d[n] / m + n / x) * psi - psi1) / ((d[n] / m + n / x) * xi - xi1)
        bn = ((m * d[n] + n / x) * psi - psi1) / ((m * d[n] + n / x) * xi - xi1)
        qsca += (2 * n + 1) * (abs(an) ** 2 + abs(bn) ** 2)
        qext += (2 * n + 1) * (an.real + bn.real)
        gsum += fn * (an * bn.conjugate()).real
        if n > 1:
            gsum += ((n - 1) * (n + 1) / n) * (an1 * an.conjugate() + bn1 * bn.conjugate()).real
        psi0, psi1 = psi1, psi
        chi0, chi1 = chi1, chi
        xi1 = complex(psi1, -chi1)
        an1, bn1 = an, bn
    qsca *= 2 / x ** 2
    qext *= 2 / x ** 2
    g = 4 * gsum / (qsca * x ** 2)
    return qext, qsca, g


def lognormal_from_eff(reff, veff):
    s2 = math.log(1 + veff)  # ln^2 sigma
    rg = reff / math.exp(2.5 * s2)
    return rg, math.sqrt(s2)


def averaged(wavelength_m, reff, veff, m, kind="lognormal", npts=48):
    """Area-weighted Q_ext, omega, g over the distribution of effective radius reff."""
    if kind == "lognormal":
        rg, ls = lognormal_from_eff(reff, veff)
        lo, hi = math.log(rg) - 4 * ls, math.log(rg) + 4 * ls
        lnr = np.linspace(lo, hi, npts)
        nr = np.exp(-((lnr - math.log(rg)) ** 2) / (2 * ls * ls))
    else:
        raise ValueError(kind)
    r = np.exp(lnr)
    area = nr * r * r
    cext = csca = cg = 0.0
    a_tot = 0.0
    for ri, ai in zip(r, area):
        x = 2 * math.pi * ri / wavelength_m
        if x > 2000:
            qe, qs, g = 2.0, 2.0 * (1 if m.imag == 0 else 0.53), 0.85
        else:
            qe, qs, g = bhmie(x, m)
        cext += ai * qe
        csca += ai * qs
        cg += ai * qs * g
        a_tot += ai
    return cext / a_tot, csca / cext, cg / csca
