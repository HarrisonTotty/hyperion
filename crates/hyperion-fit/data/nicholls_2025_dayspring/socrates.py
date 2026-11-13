"""The SOCRATES spectral files' reader for `reduce.py`: parse a spectral file (.sf) and its k-table (.sf_k): band limits, per (band, gas) the
k-term weights, and k(p, T) per term; continuum k(T) per (band, continuum)."""
import numpy as np


def parse_sf(path):
    lines = open(path).read().split("\n")
    out = {"bands": [], "terms": {}, "cont_terms": {}}
    i = 0
    while i < len(lines):
        l = lines[i]
        if l.startswith("*BLOCK: TYPE =    1"):
            i += 4
            while not lines[i].startswith("*END"):
                p = lines[i].split()
                out["bands"].append((float(p[1]), float(p[2])))
                i += 1
        elif l.startswith("*BLOCK: TYPE =    5"):
            i += 5
            while not lines[i].startswith("*END"):
                p = lines[i].split()
                band, gas, n = int(p[0]), int(p[1]), int(p[2])
                i += 1  # the line with pressure/temperature indices or ref p/T
                i += 1
                ks, ws = [], []
                for _ in range(n):
                    q = lines[i].split()
                    ks.append(float(q[0]))
                    ws.append(float(q[1]))
                    i += 1
                out["terms"][(band, gas)] = np.array(ws)
            continue
        elif l.startswith("*BLOCK: TYPE =   19"):
            i += 5
            while not lines[i].startswith("*END"):
                p = lines[i].split()
                band, cont, n = int(p[0]), int(p[1]), int(p[2])
                i += 1
                ws = []
                for _ in range(n):
                    q = lines[i].split()
                    ws.append(float(q[1]))
                    i += 1
                out["cont_terms"][(band, cont)] = np.array(ws)
            continue
        i += 1
    return out


def parse_sfk(path):
    f = open(path)
    lines = f.read().split("\n")
    i = 0
    assert lines[0].startswith("*BLOCK: k-table")
    np_, nt = [int(x) for x in lines[2].replace(",", " ").split() if x.isdigit()][:2]
    i = 3
    vals = []
    while len(vals) < np_ * (nt + 1):
        vals += [float(x) for x in lines[i].split()]
        i += 1
    grid = np.array(vals).reshape(np_, nt + 1)
    pressures = grid[:, 0]
    temps = grid[:, 1:]
    k = {}
    while i < len(lines):
        l = lines[i]
        if l.startswith("Band:"):
            p = l.replace(",", " ").replace(":", " ").split()
            band, gas, n = int(p[1]), int(p[3]), int(p[5])
            i += 1
            vals = []
            need = n * np_ * nt
            while len(vals) < need:
                vals += [float(x) for x in lines[i].split()]
                i += 1
            k[(band, gas)] = np.array(vals).reshape(n, np_, nt)
            continue
        if l.startswith("*BLOCK: sub-band") or l.startswith("*BLOCK: continuum"):
            break
        i += 1
    # continuum
    cont = {}
    while i < len(lines) and not lines[i].startswith("*BLOCK: continuum"):
        i += 1
    if i < len(lines):
        i += 2
        nt_c = int(lines[i].split()[2]) if "temperatures" in lines[i] else nt
        i += 1
        tv = []
        while len(tv) < nt_c:
            tv += [float(x) for x in lines[i].split()]
            i += 1
        cont["temps"] = np.array(tv)
        while i < len(lines):
            l = lines[i]
            if l.startswith("Band:"):
                p = l.replace(",", " ").replace(":", " ").split()
                band, c, n = int(p[1]), int(p[3]), int(p[5])
                i += 1
                vals = []
                while len(vals) < n * nt_c:
                    vals += [float(x) for x in lines[i].split()]
                    i += 1
                cont[(band, c)] = np.array(vals).reshape(n, nt_c)
                continue
            i += 1
    return pressures, temps, k, cont


if __name__ == "__main__":
    import sys
    sf = parse_sf(sys.argv[1] + ".sf")
    p, t, k, cont = parse_sfk(sys.argv[1] + ".sf_k")
    print(len(sf["bands"]), "bands;", p[:3], p[-1], "Pa;", t[0][:4])
    print("cont temps", cont.get("temps"))
    for key in sorted(k)[:5]:
        print(key, k[key].shape, sf["terms"].get(key))
    print([key for key in cont if key != "temps"][:10])
