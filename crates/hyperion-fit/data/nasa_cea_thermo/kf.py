"""P14.T49.d's asserted values, from an independent evaluation of NASA CEA's raw thermo.inp.

Usage: python3 -I kf.py <cea.py> <thermo.inp>

log10 K_f at 1 bar from the elements in their reference states, by NIST-JANAF's convention (Chase
1998, Introduction section 5, pp. 13-14): H, N, O, Cl and the noble gases as their gases at every temperature; carbon
as graphite throughout; every other element its condensed Ref-Elm records up to where their G per
atom meets the gas form's at 1 bar, then that gas (monatomic for the metals, S2 for sulphur, P2 for
phosphorus); the electron gas for charge. Intervals above 6,000 K are not used. Never extrapolated.
G / RT is McBride, Zehe and Gordon 2002's eqs. (2) and (3) (p. 2), log10 K their eq. (9) (p. 3).

Run alone, it prints the references' switches, each row's range and log10 K_f, the NIST-JANAF
comparisons and the two cross-checks; asserted.py imports it to print the Rust test's table.
"""
import importlib.util
import math
import sys

spec = importlib.util.spec_from_file_location("cea", sys.argv[1])
cea = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cea)
RECORDS, _ = cea.read(sys.argv[2])
TOP = 6000.0

BY_NAME = {}
for r in RECORDS:
    BY_NAME.setdefault(r.name, []).append(r)


def gas(name):
    hits = [r for r in BY_NAME[name] if r.phase == 0]
    assert len(hits) == 1, name
    return hits[0]


class Piecewise:
    """Intervals from one or more records, in temperature order, degenerate ones and those above
    6,000 K dropped, each with its record's divisor (atoms per formula unit)."""

    def __init__(self, records, per=1.0):
        self.ivs = []
        for r in records:
            for iv in r.intervals:
                if iv[0] < iv[1] and iv[0] < TOP:
                    self.ivs.append(iv)
        self.ivs.sort(key=lambda iv: iv[0])
        for a, b in zip(self.ivs, self.ivs[1:]):
            assert a[1] == b[0], (records[0].name, a[:2], b[:2])
        self.per = per

    def lo(self):
        return self.ivs[0][0]

    def hi(self):
        return self.ivs[-1][1]

    def g(self, t):
        for iv in self.ivs:
            if iv[0] <= t <= iv[1]:
                _, _, a, b1, b2 = iv
                lt = math.log(t)
                h = (-a[0] / t ** 2 + a[1] * lt / t + a[2] + a[3] * t / 2 + a[4] * t ** 2 / 3
                     + a[5] * t ** 3 / 4 + a[6] * t ** 4 / 5 + b1 / t)
                s = (-a[0] / (2 * t ** 2) - a[1] / t + a[2] * lt + a[3] * t + a[4] * t ** 2 / 2
                     + a[5] * t ** 3 / 3 + a[6] * t ** 4 / 4 + b2)
                return (h - s) / self.per
        return None

    def cp(self, t):
        for iv in self.ivs:
            if iv[0] <= t <= iv[1]:
                a = iv[2]
                return (a[0] / t ** 2 + a[1] / t + a[2] + a[3] * t + a[4] * t ** 2
                        + a[5] * t ** 3 + a[6] * t ** 4) / self.per
        return None


def condensed(names):
    out = []
    for n in names:
        out += [r for r in BY_NAME[n] if r.phase != 0]
    return out


# Gaseous at 298 K: the gas at every temperature (atoms per formula unit).
GAS_REF = {"H": ("H2", 2), "N": ("N2", 2), "O": ("O2", 2), "CL": ("CL2", 2), "HE": ("He", 1),
           "NE": ("Ne", 1), "AR": ("Ar", 1), "KR": ("Kr", 1), "XE": ("Xe", 1), "E": ("e-", 1)}
# Condensed below the switch: the Ref-Elm records, then the gas form.
COND_REF = {
    "NA": (["Na(cr)", "Na(L)"], "Na", 1), "K": (["K(cr)", "K(L)"], "K", 1),
    "MG": (["Mg(cr)", "Mg(L)"], "Mg", 1), "AL": (["AL(cr)", "AL(L)"], "AL", 1),
    "SI": (["Si(cr)", "Si(L)"], "Si", 1), "P": (["P(cr)", "P(L)"], "P2", 2),
    "S": (["S(a)", "S(b)", "S(L)"], "S2", 2), "CA": (["Ca(a)", "Ca(b)", "Ca(L)"], "Ca", 1),
    "TI": (["Ti(a)", "Ti(b)", "Ti(L)"], "Ti", 1), "V": (["V(cr)", "V(L)"], "V", 1),
    "CR": (["Cr(cr)", "Cr(L)"], "Cr", 1),
    "MN": (["Mn(a)", "Mn(b)", "Mn(c)", "Mn(d)", "Mn(L)"], "Mn", 1),
    "FE": (["Fe(a)", "Fe(c)", "Fe(d)", "Fe(L)"], "Fe", 1), "NI": (["Ni(cr)", "Ni(L)"], "Ni", 1),
    "ZN": (["Zn(cr)", "Zn(L)"], "Zn", 1),
}


def bisect(f, lo, hi, n=200):
    flo = f(lo)
    for _ in range(n):
        mid = 0.5 * (lo + hi)
        fm = f(mid)
        if (fm > 0) == (flo > 0):
            lo, flo = mid, fm
        else:
            hi = mid
    return 0.5 * (lo + hi)


class Reference:
    def __init__(self, el):
        self.el = el
        if el in GAS_REF:
            name, per = GAS_REF[el]
            self.cond = None
            self.gas = Piecewise([gas(name)], per)
            self.switch = None
        elif el == "C":
            self.cond = Piecewise(condensed(["C(gr)"]))
            self.gas = None
            self.switch = None
        else:
            names, g, per = COND_REF[el]
            self.cond = Piecewise(condensed(names))
            self.gas = Piecewise([gas(g)], per)
            lo = max(self.cond.lo(), self.gas.lo())
            hi = min(self.cond.hi(), self.gas.hi())
            # The crossing: condensed G per atom minus the gas form's, which falls through zero.
            d = lambda t: self.cond.g(t) - self.gas.g(t)
            assert d(lo) < 0 < d(hi), el
            self.switch = bisect(d, lo, hi)

    def lo(self):
        return (self.cond or self.gas).lo()

    def hi(self):
        return self.gas.hi() if self.gas else self.cond.hi()

    def g(self, t):
        if self.cond is None:
            return self.gas.g(t)
        if self.switch is None or t <= self.switch:
            return self.cond.g(t)
        return self.gas.g(t)


REFS = {el: Reference(el) for el in list(GAS_REF) + ["C"] + list(COND_REF)}

# Registry gas rows (key, CEA name), in registry order; FeH has no record.
ROWS = [
    ("H2", "H2"), ("He", "He"), ("H2O", "H2O"), ("CH4", "CH4"), ("NH3", "NH3"), ("N2", "N2"),
    ("O2", "O2"), ("CO2", "CO2"), ("Ar", "Ar"), ("Ne", "Ne"), ("Kr", "Kr"), ("Xe", "Xe"),
    ("N2O", "N2O"), ("CO", "CO"), ("SO2", "SO2"), ("H2S", "H2S"), ("O3", "O3"), ("HCN", "HCN"),
    ("C2H2", "C2H2,acetylene"), ("C2H4", "C2H4"), ("C2H6", "C2H6"), ("PH3", "PH3"),
    ("CH3OH", "CH3OH"), ("H", "H"), ("O", "O"), ("e-", "e-"), ("H-", "H-"), ("Na", "Na"),
    ("K", "K"), ("Fe", "Fe"), ("Mg", "Mg"), ("Si", "Si"), ("Ca", "Ca"), ("Ti", "Ti"),
    ("SiO", "SiO"), ("TiO", "TiO"), ("VO", "VO"), ("Al", "AL"), ("V", "V"), ("Mn", "Mn"),
    ("Ni", "Ni"), ("Zn", "Zn"), ("Na+", "Na+"), ("Mg+", "Mg+"), ("Al+", "AL+"), ("Si+", "Si+"),
    ("K+", "K+"), ("Ca+", "Ca+"), ("Ti+", "Ti+"), ("V+", "V+"), ("Cr+", "Cr+"), ("Mn+", "Mn+"),
    ("Fe+", "Fe+"), ("Ni+", "Ni+"), ("Zn+", "Zn+"), ("HCl", "HCL"), ("Cr", "Cr"), ("S", "S"),
    ("N", "N"), ("C", "C"),
]


class Species:
    def __init__(self, pw, formula):
        self.pw = pw
        self.formula = formula

    def range(self):
        lo, hi = self.pw.lo(), self.pw.hi()
        for el, _ in self.formula:
            lo, hi = max(lo, REFS[el].lo()), min(hi, REFS[el].hi())
        return lo, hi

    def log_kf(self, t):
        lo, hi = self.range()
        if not lo <= t <= hi:
            return None
        total = self.pw.g(t)
        for el, n in self.formula:
            total -= n * REFS[el].g(t)
        return -total / cea.LN10


def species_of(name, cond=False):
    recs = [r for r in BY_NAME[name] if (r.phase != 0) == cond]
    return Species(Piecewise(recs), recs[0].formula)


def merged_condensed(names):
    recs = condensed(names)
    return Species(Piecewise(recs), recs[0].formula)


if __name__ == "__main__":
    print("references:")
    for el, ref in REFS.items():
        sw = f"{ref.switch:.4f}" if ref.switch else "-"
        print(f"  {el:3s} [{ref.lo():g}, {ref.hi():g}] gas from {sw}")
    temps = (500.0, 2500.0, 5000.0)
    print("rows: key, range, log10 K_f at", temps)
    for key, name in ROWS:
        s = species_of(name)
        lo, hi = s.range()
        vals = [s.log_kf(t) for t in temps]
        print(f"  {key:6s} [{lo:g}, {hi:g}] " + " ".join(f"{v!r}" for v in vals))
    print("JANAF comparisons:")
    for key, t, janaf in [("H2O", 298.15, 40.047), ("H2O", 1000.0, 10.060), ("CO", 1000.0, 10.461),
                          ("CH4", 1000.0, -1.018)]:
        v = species_of(dict(ROWS)[key]).log_kf(t)
        print(f"  {key} {t}: CEA {v!r} JANAF {janaf} diff {v - janaf:+.5f}")

    # MgSO4's open-system decomposition at 1 bar in all (the registry's arithmetic).
    mgso4 = merged_condensed(["MgSO4(II)", "MgSO4(I)"])
    mgo = merged_condensed(["MgO(cr)"])
    so3, so2 = species_of("SO3"), species_of("SO2")

    def mgso4_total(t):
        p_so3 = 10 ** (mgo.log_kf(t) + so3.log_kf(t) - mgso4.log_kf(t))
        k2 = 10 ** (mgo.log_kf(t) + so2.log_kf(t) - mgso4.log_kf(t))
        p_so2 = (math.sqrt(2) * k2) ** (2 / 3)
        return p_so3 + p_so2 + p_so2 / 2

    t1 = bisect(lambda t: math.log10(mgso4_total(t)), 1000.0, 1400.0)
    print(f"MgSO4: total 1 bar at {t1!r} K; ranges mgso4 {mgso4.range()} mgo {mgo.range()} so3 {so3.range()}")
    # As the registry interpolated JANAF's 100 K rows: linear in 1/T between 1,300 and 1,400 K.
    f = lambda t: math.log10(mgso4_total(t))
    a, b = 1300.0, 1400.0
    x = 1 / a + (1 / b - 1 / a) * (0 - f(a)) / (f(b) - f(a))
    print(f"   interpolated between 1300 and 1400 K rows: {1 / x!r}")
    cs = [species_of(n) for n in ["C", "C2", "C3", "C4", "C5"]]
    total_c = lambda t: sum(10 ** c.log_kf(t) for c in cs)
    t2 = bisect(lambda t: math.log10(total_c(t)), 3500.0, 4500.0)
    print(f"graphite: C1-C5 1 bar at {t2!r} K")
    a, b = 3900.0, 4000.0
    f = lambda t: math.log10(total_c(t))
    x = 1 / a + (1 / b - 1 / a) * (0 - f(a)) / (f(b) - f(a))
    print(f"   interpolated between 3900 and 4000 K rows: {1 / x!r}")
    for c, n in zip(cs, ["C", "C2", "C3", "C4", "C5"]):
        print(f"   {n} p {10 ** c.log_kf(t2):.4f} bar")
