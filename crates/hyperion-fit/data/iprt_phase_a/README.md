# IPRT Phase A: selected results, reformatted

IPRT polarized radiative transfer model intercomparison, Phase A (C. Emde et al., J. Quant.
Spectrosc. Radiat. Transfer 164 (2015) 8–36, DOI: 10.1016/j.jqsrt.2015.05.007), results and the
optical properties of cases A3, A4 and A5 from <https://www.libradtran.org/iprt/>, licensed under
CC BY-SA 3.0 (<https://creativecommons.org/licenses/by-sa/3.0/>). The values in this directory are
selected rows of those results, case A4's phase matrix less its Legendre moments, and the phase
matrices of A3 and A5 at selected angles, reformatted; that adapted material is offered under
CC BY-SA 3.0.

They are the values the reference tracer's benchmarks assert (rendering plan R08, R08.T12.c and
R08.T12.e, `crates/hyperion-fit/src/atmosphere/benchmarks.rs`; ruled in `decision-r08-licences.md`,
row 6d), and the matrices the client's Mie is checked against (R08.T12.e,
`apps/hyperion/src/renderer/src/test/mieMatrices.test.ts`). They are test inputs and are not
shipped in the app. `PROVENANCE.toml` records each file's SHA-256, which the Rust tests check
before they read it.

## Files

- `a1_rayleigh.txt`: case A1 (Rayleigh scattering, one layer of optical thickness 0.5, at
  depolarisation factors 0, 0.03 and 0.1), 82 rows of the model PSTAR's results.
- `a2_lambert.txt`: case A2 (Rayleigh scattering, optical thickness 0.1, depolarisation 0.03,
  over a Lambertian surface of albedo 0.3), 33 rows of PSTAR's results.
- `a3_sphere.txt`: case A3 (spherical water-soluble aerosol at 350 nm, optical thickness 0.2),
  34 rows of PSTAR's results, the rows A4's file keeps.
- `a3_sphere_matrix.txt`: case A3's phase matrix, P1–P4 at each of the file's 68 scattering
  angles, one row per angle in ascending order, the values as printed.
- `a4_spheroid.txt`: case A4 (prolate spheroidal aerosol, optical thickness 0.2), 34 rows of
  PSTAR's results.
- `a4_spheroid_matrix.txt`: case A4's phase matrix, P1–P6 at each of the 1,801 scattering angles
  from 0° to 180°, one row per angle in ascending order, the values as printed; the file's
  Legendre moments are not kept.
- `a5_cloud.txt`: case A5 (a liquid water cloud at 800 nm, optical thickness 5), 62 rows of
  PSTAR's results: 35 in the principal plane (the cloudbow's at the top, at a relative azimuth of
  180° and 6°–14° from nadir, every 2°) and 27 in the almucantar (the bow's at the top, 115°–135°
  in azimuth, every 5°). Loaded and counted by the fast tests, and asserted by the benchmark of
  R08.T12.f, which traces them with the tracer's variance reduction for thick clouds.
- `a5_cloud_matrix.txt`: case A5's phase matrix, P1–P4 at every fifth of the file's 450
  scattering angles from 0° and at 180°, one row per angle in ascending order, the values as
  printed.

Each results row is `depol altitude sza saa vza vaa I Q U V`, as IPRT's files have it: the
altitude 0 at the bottom of the layer and 1 at its top, the angles in degrees, and the Stokes
vector divided by the extraterrestrial irradiance. The slanted rows were chosen to avoid the
sun's direction by 15° or more. The zenith and nadir rows are kept at every sun, so A1's two
bottom rows with the sun overhead look at it; their values, like every row's, are of the diffuse
light alone. Nothing else in the rows is changed. Each matrix row is `theta_deg P1 P2 P3 P4`,
with P1 averaging 1 over the sphere and, for spheres, P = [[P1, P2, 0, 0], [P2, P1, 0, 0],
[0, 0, P3, P4], [0, 0, −P4, P3]].

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

Fetched on 2026-10-10 from the same address (R08.T12.e):

| Media name                                              | SHA-256                                                            |
| ------------------------------------------------------- | ------------------------------------------------------------------ |
| `intercomparisons:phase_a:a3:iprt_case_a3_pstar.dat`    | `bf4581a367d799104cf94e7cac91e15a792085905e6dd5accd03e38ebcf979b5` |
| `intercomparisons:phase_a:a5:iprt_case_a5-pr_pstar.dat` | `3124178169c6c379f1588fb837e5895adf08e17da0110e5c7a690ec27d2cee9c` |
| `intercomparisons:phase_a:a5:iprt_case_a5-al_pstar.dat` | `33b4e8e772d44aad0da4e9f4dd704854d8bc39c32cc24f314d94f8a8dd4c64f5` |
| `intercomparisons:phase_a:waso.mie.dat`                 | `094f5b1d04ea987488e5042cec99e643df58870081e875f77164584310d801e8` |
| `intercomparisons:phase_a:watercloud.mie.dat`           | `22d1053e255c78dd9a428f0d34b1890767325b51900d71dca4f6af9095db3f88` |

The client's Mie writes A3's and A5's matrices itself (`mieMatrices.ts`) from the parameters of
libRadtran's Mie tool that IPRT's netCDF copies of the same optical properties record, which the
wiki's text and Emde et al. do not all state: `waso.mie.cdf` (SHA-256
`484bf106547a83ad01914aeec193aa44a035db19f79e1b9ecaa4a512bf00fed2`) a log-normal of radius
parameter 0.0262 µm, `n_r_max` 5 and `dx_max` 0.03 at 350 nm, index 1.422 − 0.002649i; and
`watercloud.mie.cdf` (`b4e86515fdcdabe92c1b71938c781d98ea126c10d8a63168e265202e41a2c99d`) a gamma
distribution of α 7 and effective radius 10 µm, `n_r_max` 5 and `dx_max` 0.003 at 800 nm, index
1.3258698 − 1.250319 × 10⁻⁷i. Neither netCDF file is committed. The wiki's page for A3 says 500 nm;
both files and Emde et al. (§3.2.3) say 350 nm. `waso.mie.cdf` records a width parameter of 2.0,
but only Emde et al.'s 2.24 reproduces the file's matrix.

The same pages' IPOL results, U and V negated, agree with PSTAR's at the rows used to 9.7 × 10⁻⁵
of I in I, 4.7 × 10⁻⁵ in Q, 1.1 × 10⁻⁴ in U and 3.6 × 10⁻⁶ in V in A3, and to 6.5 × 10⁻⁴,
3.8 × 10⁻⁴, 2.2 × 10⁻⁵ and 5.2 × 10⁻⁷ in A5; MYSTIC's are noisier (2.5 × 10⁻³ of I in A3's I)
(`iprt_case_a3_ipol.dat`, `dbdaa70452d1b7d8675dcfe2cfa9b4eeb58d9876061533a9067ebf99e3042aa3`;
`iprt_case_a3_mystic.dat`, `041c0bd3016453cc6f8a3530b441e9bb543e37b7ee8a8afcf4f723e5293e11e5`;
`iprt_case_a5_pp_ipol_2014-08-20.dat`,
`0b69f3164baecdc31e37baa445869bfcd7162b17278881801fee543aef9b12f6`;
`iprt_case_a5_al_ipol_2014-08-17.dat`,
`a351d8c87f5b020c068253193dea122b000ac14f67c859ee7fde2feb1b2c13ee`).
