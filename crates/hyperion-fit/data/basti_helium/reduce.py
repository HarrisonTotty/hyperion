#!/usr/bin/env python3
"""Reduce BaSTI's helium-enhanced grid to `turnoff.csv` and `zahb.csv` (plan 15, P15.T7.b).

The raw files are T7.a's 88, listed with their SHA-256s in `raw.sha256` beside this script, in
`crates/hyperion-fit/data/cache/basti_helium/` (git-ignored) under the site's paths below
`basti-old/BASTI/`. Each is checked against its hash before it is read. The sets: BaSTI's
alpha-enhanced canonical models (Pietrinferni, Cassisi, Salaris and Castelli 2006, ApJ 642, 797;
Reimers eta = 0.2) at Z = 0.0001, 0.0003, 0.001, 0.002, 0.004, 0.008 and 0.0198, and the database's
helium-enhanced extension at Y = 0.30, 0.35 and 0.40 at the same [M/H] (only 0.35 and 0.40 at the
solar row), from the site's readme of the extension ("EVOLUTIONARY TRACKS & ISOCHRONES:
alpha-helium enhanced MODELS", last modified 2009-02-26) and of the canonical tracks.

What is read, and from where (the readmes' column lists):

- Tracks, the members `MMMMz<lmn>y<ghi>aes[_eta02].nor_c04ae` of `ms*.tar.gz`: column 1, log10 of
  the age in years from the zero-age main sequence. The initial mass is MMMM / 100 M_sun. Z is
  l.m x 10^-n and Y is 0.ghi, both from the member's name: the file headers print Z to four
  decimals, and every track of `HE_EN/Z13/msz13y350aes_c04ae.tar.gz` says Y = 0.300 in its header
  where its name, its archive and its ages say 0.35. Each track is resampled to the same key points
  on the same rows; the readme's table puts key point 4, "Maximum in logL for high-mass or Xc=0.0
  for low-mass stars", on row 320 (Pietrinferni et al. 2004, ApJ 612, 168, Table 2). For the
  0.6-0.9 M_sun tracks, none of which develops a convective core, that is the exhaustion of
  central hydrogen. It stands for the end of the main sequence that plan 06's tracks give, Hurley,
  Pols and Tout's (2000, eqs. 5-6) t_MS = x t_BGB with x = 0.95-0.99, which in these tracks falls
  0.5-2% later in age, with the same sensitivity to helium. Tracks that never leave the main
  sequence within the models' age limit (0.6 M_sun, and 0.7 at the higher Z, in the canonical
  sets) end there, so it is the one point every track of the range has.
- Zero-age horizontal branches, `zahb*_c04ae`: column 1, the mass M_HB (M_sun); 2, log10 L/L_sun;
  3, log10 T_eff (K).
- The core mass that sets each branch's envelope, M_env = M_HB - M_c: for a canonical set, the
  summary table `newtablow*aes_eta02.dat`'s "Mc_He" at the "TIP RGB" of the branch's progenitor,
  whose mass the horizontal-branch tracks name (`hb*.tar.gz`, members `<M_HB>m<pq>z...`: 0.8 M_sun,
  0.9 at Z = 0.008 and 1.0 at 0.0198). The helium-enhanced sets have no summary table, so their
  core mass is the branch's lowest mass less the median, over the seven canonical sets, of the
  canonical branches' lowest mass less their tip core mass (0.0005-0.0012 M_sun; median 0.0007).
- The solar row's canonical composition, which its names leave as `zsunysun`: the summary table's
  header, Z = 0.0198 and Y = 0.2734.

The output, LF line ends, one header line and rows sorted by their leading columns:

- `turnoff.csv`: `z_row,z,y,mass,log_age_ms_end`. `z_row` is the canonical Z of the set's [M/H]
  row; `log_age_ms_end` is log10 of the age at key point 4 (yr), to eight decimals; masses from
  0.6 to 0.9 M_sun, the ones each set has.
- `zahb.csv`: `z_row,z,y,core_mass,hb_mass,envelope_mass,log_l,log_teff`, every point of every
  branch, masses to four decimals and logarithms to six, as the files print them.

Decimal arithmetic throughout, so that the bytes are the same on every platform. Run it as
`python3 -I crates/hyperion-fit/data/basti_helium/reduce.py`; it uses the standard library only.
"""
import hashlib
import io
import pathlib
import re
import tarfile
from decimal import Decimal

HERE = pathlib.Path(__file__).resolve().parent
CACHE = HERE.parent / "cache" / "basti_helium"

# The grid's [M/H] rows: the directory, the summary table's code, and the canonical Z and Y.
ROWS = [
    ("Z14", "z14", "0.0001", "0.245"),
    ("Z34", "z34", "0.0003", "0.245"),
    ("Z13", "z13", "0.001", "0.246"),
    ("Z23", "z23", "0.002", "0.248"),
    ("Z43", "z43", "0.004", "0.251"),
    ("Z83", "z83", "0.008", "0.256"),
    ("ZSUN", "z22", "0.0198", "0.2734"),
]

# The turn-off's key point (the readme's key point 4) and the masses read, M_sun.
MS_END_ROW = 320
MASS_RANGE = (Decimal("0.6"), Decimal("0.9"))

TRACK = re.compile(r"^(\d{4})z(\d{3}|sun)y(\d{3}|sun)aes(?:_eta02)?\.nor_c04ae$")
BRANCH = re.compile(r"^\d+m(\d+)z")


def recorded_hashes() -> dict:
    """The raw files' SHA-256s, from `raw.sha256`'s `<name> <sha256>` lines."""
    hashes = {}
    for line in (HERE / "raw.sha256").read_text(encoding="utf-8").splitlines():
        name, digest = line.split(" ")
        hashes[name] = digest
    return hashes


HASHES = recorded_hashes()


def raw(name: str) -> bytes:
    """The raw file `name`'s bytes, checked against its recorded SHA-256."""
    data = (CACHE / name).read_bytes()
    found = hashlib.sha256(data).hexdigest()
    if found != HASHES[name]:
        raise SystemExit(f"{name}: SHA-256 {found}, not the recorded {HASHES[name]}")
    return data


def table_rows(text: str) -> list:
    """The data rows of a BaSTI file: every non-blank line not opened by `#`, split."""
    return [l.split() for l in text.splitlines() if l.strip() and not l.lstrip().startswith("#")]


def code_z(code: str) -> Decimal:
    """Z from a name's three digits l, m and n: l.m x 10^-n."""
    return (Decimal(code[:2]) * Decimal(10) ** -(int(code[2]) + 1)).normalize()


def tar_members(name: str):
    """Each file member of the tar archive `name`, in name order: its base name and its text."""
    with tarfile.open(fileobj=io.BytesIO(raw(name))) as tar:
        for member in sorted(tar.getmembers(), key=lambda m: m.name):
            if member.isfile():
                text = tar.extractfile(member).read().decode("latin-1")
                yield pathlib.PurePosixPath(member.name).name, text


def fixed(value: str, places: int) -> Decimal:
    """`value` as a decimal with `places` places, rounded half-even."""
    return Decimal(value).quantize(Decimal(1).scaleb(-places))


def composition(code_z_part: str, code_y_part: str, row: tuple) -> tuple:
    """A track's Z and Y from its name, the solar row's from the summary table."""
    if code_z_part == "sun":
        return Decimal(row[2]), fixed(row[3], 4)
    return code_z(code_z_part), fixed(str(Decimal(code_y_part) / 1000), 4)


def turnoff_rows() -> list:
    """`turnoff.csv`'s rows: each set's 0.6-0.9 M_sun tracks at key point 4."""
    out = []
    for row in ROWS:
        directory = row[0]
        archives = [f"DATA_AE_S/{directory}/ms{row[1]}aes_eta02_c04ae.tar.gz"]
        archives += sorted(n for n in HASHES if re.fullmatch(
            rf"DATA_AE_S/HE_EN/{directory}/ms\w+y\d{{3}}aes_c04ae\.tar\.gz", n))
        for archive in archives:
            for member, text in tar_members(archive):
                m = TRACK.match(member)
                if not m:
                    raise SystemExit(f"{archive}: unexpected member {member}")
                mass = Decimal(m.group(1)).scaleb(-2)
                if not MASS_RANGE[0] <= mass <= MASS_RANGE[1]:
                    continue
                z, y = composition(m.group(2), m.group(3), row)
                rows = table_rows(text)
                if len(rows) < MS_END_ROW:
                    raise SystemExit(f"{archive}/{member}: {len(rows)} rows, short of key point 4")
                age = fixed(rows[MS_END_ROW - 1][0], 8)
                out.append((Decimal(row[2]), z, y, mass, age))
    out.sort()
    return out


def canonical_tip_core(row: tuple, progenitor: Decimal) -> Decimal:
    """The summary table's TIP RGB Mc_He of the canonical `progenitor` (M_sun) of `row`."""
    text = raw(f"DATA_AE_S/newtablow{row[1]}aes_eta02.dat").decode("latin-1")
    for line in text.splitlines():
        cells = line.split()
        if len(cells) == 20 and re.fullmatch(r"\d*\.\d+", cells[0]):
            if Decimal(cells[0]) == progenitor:
                return Decimal(cells[10])
    raise SystemExit(f"newtablow{row[1]}: no {progenitor} M_sun row")


def progenitor_of(archive: str) -> Decimal:
    """The progenitor mass that the horizontal-branch tracks of `archive` name: m<pq>, p.q M_sun."""
    masses = set()
    for member, _ in tar_members(archive):
        digits = BRANCH.match(member).group(1)
        masses.add(Decimal(digits[0] + "." + digits[1:]))
    if len(masses) != 1:
        raise SystemExit(f"{archive}: progenitors {sorted(masses)}")
    return masses.pop()


def branch(name: str) -> list:
    """The ZAHB file `name`'s points: (M_HB, log L, log T_eff), to four, six and six places."""
    rows = table_rows(raw(name).decode("latin-1"))
    return [(fixed(r[0], 4), fixed(r[1], 6), fixed(r[2], 6)) for r in rows]


def zahb_rows() -> list:
    """`zahb.csv`'s rows: every point of every set's zero-age horizontal branch."""
    canonical = []
    for row in ROWS:
        directory = row[0]
        hb = f"DATA_AE_S/{directory}/HB/"
        name = next(n for n in HASHES if re.fullmatch(rf"{hb}zahb\w+", n))
        archive = next(n for n in HASHES if re.fullmatch(rf"{hb}hb\w+\.tar\.gz", n))
        points = branch(name)
        core = canonical_tip_core(row, progenitor_of(archive))
        canonical.append((row, Decimal(row[2]), fixed(row[3], 4), core, points))
    # The median of the seven canonical branches' lowest mass less their tip core mass.
    offsets = sorted(points[0][0] - core for _, _, _, core, points in canonical)
    offset = offsets[len(offsets) // 2]
    sets = list(canonical)
    for row in ROWS:
        directory = row[0]
        for name in sorted(n for n in HASHES if re.fullmatch(
                rf"DATA_AE_S/HE_EN/{directory}/HB/zahb\w+", n)):
            archive = name.replace("/zahb", "/hb") + ".tar.gz"
            member = next(m for m, _ in tar_members(archive))
            m = re.match(r"^\d+m\d+z(\d{3})y(\d{3})aes", member)
            points = branch(name)
            core = points[0][0] - offset
            y = fixed(str(Decimal(m.group(2)) / 1000), 4)
            sets.append((row, code_z(m.group(1)), y, core, points))
    out = []
    for row, z, y, core, points in sets:
        for mass, log_l, log_teff in points:
            out.append((Decimal(row[2]), z, y, core, mass, mass - core, log_l, log_teff))
    out.sort()
    return out


def write(name: str, header: str, rows: list) -> None:
    """Writes `rows` under `header` to `name` beside this script, comma-separated, LF-ended."""
    lines = [header] + [",".join(str(c) for c in r) for r in rows]
    with open(HERE / name, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")


def main() -> None:
    write("turnoff.csv", "z_row,z,y,mass,log_age_ms_end", turnoff_rows())
    write("zahb.csv", "z_row,z,y,core_mass,hb_mass,envelope_mass,log_l,log_teff", zahb_rows())


if __name__ == "__main__":
    main()
