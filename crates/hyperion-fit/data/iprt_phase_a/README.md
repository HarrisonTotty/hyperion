# IPRT Phase A: selected results, reformatted

IPRT polarized radiative transfer model intercomparison, Phase A (C. Emde et al., J. Quant.
Spectrosc. Radiat. Transfer 164 (2015) 8–36, DOI: 10.1016/j.jqsrt.2015.05.007), results and case
A4's optical properties from <https://www.libradtran.org/iprt/>, licensed under CC BY-SA 3.0
(<https://creativecommons.org/licenses/by-sa/3.0/>). The values in this directory are selected
rows of those results and case A4's phase matrix less its Legendre moments, reformatted; that
adapted material is offered under CC BY-SA 3.0.

They are the values the reference tracer's benchmarks assert (rendering plan R08, R08.T12.c,
`crates/hyperion-fit/src/atmosphere/benchmarks.rs`; ruled in `decision-r08-licences.md`, row 6d).
They are a test input of `hyperion-fit` and are not shipped in the app. `PROVENANCE.toml` records
each file's SHA-256, which the tests check before they read it.

## Files

- `a1_rayleigh.txt`: case A1 (Rayleigh scattering, one layer of optical thickness 0.5, at
  depolarisation factors 0, 0.03 and 0.1), 82 rows of the model PSTAR's results.
- `a2_lambert.txt`: case A2 (Rayleigh scattering, optical thickness 0.1, depolarisation 0.03,
  over a Lambertian surface of albedo 0.3), 33 rows of PSTAR's results.
- `a4_spheroid.txt`: case A4 (prolate spheroidal aerosol, optical thickness 0.2), 34 rows of
  PSTAR's results.
- `a4_spheroid_matrix.txt`: case A4's phase matrix, P1–P6 at each of the 1,801 scattering angles
  from 0° to 180°, one row per angle in ascending order, the values as printed; the file's
  Legendre moments are not kept.

Each results row is `depol altitude sza saa vza vaa I Q U V`, as IPRT's files have it: the
altitude 0 at the bottom of the layer and 1 at its top, the angles in degrees, and the Stokes
vector divided by the extraterrestrial irradiance. The slanted rows were chosen to avoid the
sun's direction by 15° or more. The zenith and nadir rows are kept at every sun, so A1's two
bottom rows with the sun overhead look at it; their values, like every row's, are of the diffuse
light alone. Nothing else in the rows is changed.

## Sources

Fetched on 2026-10-09 from `https://www.libradtran.org/iprt/lib/exe/fetch.php?media=` followed by
the media name:

| Media name                                           | SHA-256                                                            |
| ---------------------------------------------------- | ------------------------------------------------------------------ |
| `intercomparisons:phase_a:a1:iprt_case_a1_pstar.dat` | `27c77b0d3f61cad6c1eededbb52b8f46dcadd5d1f888420a0ee71a2ede78171b` |
| `intercomparisons:phase_a:a2:iprt_case_a2_pstar.dat` | `c74d0386339d464ef925d6a8c72913061c3b2fb2155705a0f89404c8826f8f13` |
| `intercomparisons:phase_a:a4:iprt_case_a4_pstar.dat` | `4af12f0b5ac46896922f16851d71945aea8b3048673fa33a66054da9d9f6ba04` |
| `intercomparisons:phase_a:sizedistr_spheroid.dat`    | `5e02e1fc248e49f96499bcaca87953fad605634b42942e750a9003f4498ca7c5` |

The same pages' IPOL and MYSTIC results were read to check PSTAR's. IPOL's and SHDOM's files carry
the opposite signs of U and V to PSTAR's and MYSTIC's (one difference of handedness): with those
negated, IPOL agrees with PSTAR at the rows used to 1.1 × 10⁻⁶ of I in every component in A1, to
6.9 × 10⁻⁶ in A2, and in A4 to 1.8 × 10⁻⁴ in I and 3 × 10⁻⁶ in Q, U and V; PSTAR's and MYSTIC's
V in A4, which reaches 1.6 × 10⁻⁴ of I, agree to 1.4 × 10⁻⁶ of I (`iprt_case_a1_ipol.dat`,
SHA-256 `939621ba0f9643c7f108749227f14e03a8e799355411dfb8626a056afbb18791`;
`iprt_case_a2_ipol.dat`, `c1a8e6ed4c85f097ed586e028e8d961481cae9297df488991995c5715373dcaf`;
`iprt_case_a4_ipol.dat`, `82f8d50b3084ecfc32aa34974a1afa83e44990e72a8594a14cb3bd4fb149df61`;
`iprt_case_a4_mystic.dat`, `247fd3a8f8de19dd3e1554f47c11cf417aa2361eb67c1fa438c597401105dd43`).
PSTAR's are the values asserted, each component held to its own spread.
