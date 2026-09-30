# SSE reference vectors (plan 06, P06.T12.a)

These files are output of the published SSE code, run once, offline, for the comparison tests of
P06.T12.b (`crates/hyperion-sim/tests/sse_reference.rs`). Only its output is here. No SSE source is
committed, and none of it is copied into the generator.

## Source

- **Code:** SSE, the single-star evolution code of Hurley, Pols and Tout (2000, MNRAS 315, 543),
  from `sse.tar.gz` on Jarrod Hurley's page (`http://astronomy.swin.edu.au/~jhurley/sse.tar.gz`).
  The archive is dated 2006-11-24. It was retrieved on 2026-09-21 through the Internet Archive's
  capture of 2025-04-20, and its sha256 is
  `61c02d333943226c3a78b9bb832208ceaa853b16faa7f466d85c21b3b73ebcfe`.
- **Build:** `x86_64-w64-mingw32-gfortran` 16.2.0, `-static`, run under Wine 11.17 on 2026-09-23.
- **Driver:**
  - A small program calls SSE's `evolv1` for each star of the plan's grid:
    - m = 0.1, 0.3, 0.5, 0.8, 1, 1.5, 2, 3, 5, 8, 10, 15, 20, 40, 60 and 100 M☉;
    - Z = 0.0001, 0.001, 0.004, 0.02 and 0.03.
  - Each star starts on the zero-age main sequence, with no pre-main sequence, and runs to an end
    time of 10⁸ Myr.
  - `evolv1` was changed in one place only. It writes every step's state in double precision, and
    it stops at the first step of a remnant.

## Options

The options are those of SSE's distributed `evolve.in`, with three changed as ruling 26 asks, so
that the run uses the recipes of `WindRecipe::Hurley2000` and `RemnantRecipe::Hurley2000`:

| Option                 | Distributed      | Here                       | Meaning                                                                                   |
| ---------------------- | ---------------- | -------------------------- | ----------------------------------------------------------------------------------------- |
| `neta`                 | 0.5              | 0.5                        | Reimers' η                                                                                |
| `bwind`                | 0.0              | 0.0                        | no enhanced wind (binaries only)                                                          |
| `hewind`               | 0.5              | **1.0**                    | the helium-star wind at the paper's strength                                              |
| `sigma`                | 190              | 190                        | the kick dispersion, km/s (no effect on a single star's evolution)                        |
| `ifflag`               | 0                | 0                          | the paper's initial–final mass relation                                                   |
| `wdflag`               | 1                | 1                          | Hurley and Shara's (2003) white-dwarf cooling (white-dwarf luminosities are not compared) |
| `bhflag`               | 0                | 0                          | no black-hole kicks                                                                       |
| `nsflag`               | 1                | **0**                      | the paper's neutron-star and black-hole masses (HPT eq. 92)                               |
| `mxns`                 | 3.0              | **1.8**                    | the paper's largest neutron-star mass                                                     |
| `pts1`, `pts2`, `pts3` | 0.05, 0.01, 0.02 | **0.0005, 0.0001, 0.0002** | the time-step factors                                                                     |

**The steps are a hundred times finer than SSE ships with** (ruling 40 of 2026-09-22). With the
distributed steps, SSE's masses are off by up to 2.2% against its own converged result, and a
60 M☉ star at Z = 0.02 ends as a neutron star instead of a black hole. T12 tests the formulae, not
SSE's step size. A second run with steps ten times finer than distributed (`pts` × 0.1) is used
only to choose the samples below. Neither run is committed, only what is taken from the finer one.

## The files

There is one file per metallicity, `z<Z>.csv`. Each starts with a comment header giving the version,
the options and the date, then a line of column names. The columns are:

- `kind`: `phase` for the first logged step of one of SSE's stellar types, and `sample` for a step
  inside one;
- `m0`: the initial mass, M☉;
- `kw`: SSE's stellar type. 0 and 1 are the main sequence (0 deeply convective), 2 the
  Hertzsprung gap, 3 the first giant branch, 4 core helium burning, and 5 and 6 the early and
  thermally pulsing AGB. 7 to 9 are the naked helium main sequence, Hertzsprung gap and giant
  branch. 10 to 12 are helium, carbon–oxygen and oxygen–neon white dwarfs, 13 a neutron star, 14
  a black hole and 15 no remnant;
- `fraction`: the step's share of its phase's time, from the phase's first step to the next
  phase's (0 on a `phase` row);
- `age_myr`: Myr from the zero-age main sequence;
- `mass` and `core_mass`: M☉, the current mass and SSE's core mass;
- `log_l` and `log_r`: log₁₀ of L in L☉ and R in R☉.

The `phase` row of a remnant's type is the star's death: its age is the lifetime and its mass the
remnant's.

## How the samples were chosen

Up to 20 samples per star were chosen from SSE's own steps, with no reference to this generator:

1. **Candidates.** The steps nearest 0.05, 0.10, … 0.95 of each living phase that lasts at least
   10⁻³ of the star's lifetime.
2. **Smooth.** A candidate is kept where log L and log R move by at most 0.05 dex across ±1% of the
   phase. A difference of 10⁻³ in the phase's timing between the two codes then moves them by a few
   thousandths of a dex at most.
3. **Converged.** A candidate is kept where the run with ten-times-coarser steps agrees with this
   one to 0.005 dex in log L and log R at the same fraction of the phase. The reference is then
   converged in its own step there.
4. **Taken** round-robin over the phases, each phase from its middle outwards, until there are 20.

This removes the places where the state moves steeply with time, such as the thermally pulsing
AGB's end and the small-envelope regime of massive stars. There a sample would test SSE's step
size, not the formulae: at 20 M☉ and Z = 0.03 on the early AGB, SSE's radius moves by 0.075 dex
between the two step settings.

Of 6,327 candidates, 6,013 are smooth and 5,988 are also converged. Every star has 20 samples.
