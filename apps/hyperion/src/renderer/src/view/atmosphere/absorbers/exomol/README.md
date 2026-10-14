# ExoMol-derived data

The files in this directory are adapted material of the ExoMol database (J. Tennyson et al.,
<https://www.exomol.com>), whose data are licensed under the Creative Commons
Attribution-ShareAlike 4.0 International licence (CC BY-SA 4.0,
<https://creativecommons.org/licenses/by-sa/4.0/>): "All data in ExoMol is released under the
Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0) licence"
(<https://www.exomol.com/data/licence/>, read 2026-10-10, 2026-10-11 and 2026-10-13).

- `bounds.json` (plan R08, R08.T4.c; `decision-r08-t4b-t12c.md`, follow-ups 3 and 4): for each
  pending absorber, its 1 nm cell-mean cross-sections over 380–760 nm at the line list's
  temperature nodes, reduced from the line lists each entry names (C2018, AYT2, SiOUVenIR, Li2015,
  TYM, Harris, Toto, HyVO, MoLLIST's FeH and POKAZATEL), with each list's paper, its dataset
  DOI where ExoMol gives one, the files' SHA-256 and the date fetched. The values were computed
  from the line lists, by direct sums or through ExoMol's cross-section service, a modification
  of them; CO's are scaled to HITRAN2024's 7–0 band, as each entry's `method` states.
- `kDistributions/NH3.json` and its parts `NH3-<n>.bin` (plan R08, R08.T4.g): NH₃'s
  k-distributions per 1 nm cell, reduced from ExoMol's C2018 list (P. A. Coles, R. I. Ovsyannikov,
  O. L. Polyansky, S. N. Yurchenko and J. Tennyson, J. Quant. Spectrosc. Radiat. Transf. 219 (2018)
  199, DOI 10.1016/j.jqsrt.2018.07.022) and its H₂ and He `.broad` files, by super-lines on the
  grid and in the 16-bit format the sidecar states.
- `kDistributions/N2O.json` and its parts `N2O-<n>.bin` (plan R08, R08.T4.g): N₂O's
  k-distributions per 1 nm cell, reduced from ExoMol's TYM list (S. N. Yurchenko, T. M. Mellor and
  J. Tennyson, Mon. Not. R. Astron. Soc. 534 (2024) 1364, DOI 10.1093/mnras/stae2201) merged with
  HITRAN2024's four N₂O bands (I. E. Gordon et al., J. Quant. Spectrosc. Radiat. Transf. 353
  (2026) 109807), each band from one list only. The whole table is offered under CC BY-SA 4.0, its
  HITRAN2024 part with HITRAN's citation.
- `lineReferencesNH3.json` and `lineReferencesN2O.json` (plan R08, R08.T4.g): named cells' curves
  of growth at three node pressures and three midpoints, computed from the same lists, beside the
  reduction's own, which the tests read.
- `n2oMerge.json` (plan R08, R08.T4.g): HITRAN2024's N₂O bands against TYM's of the same
  assignment, and each merged cell's bands by list and its mean cross-sections, which the tests
  read.
- None of the line lists' files (`.def`, `.def.json`, `.states`, `.trans`, `.pf`, `.broad`), nor
  any decompressed, filtered or binned copy of them, is committed; they are fetched by URL and
  checked against the SHA-256 the sidecars and `src/tools/exomolCli.ts` name.
- This adapted material is offered under CC BY-SA 4.0. It is kept in files of its own, never
  merged into `crossSections.json` or into code (decision-composition §5), and `NOTICE` carries
  its attribution. ExoMol asks that the line lists' papers be cited; the database's release paper
  (J. Tennyson et al., J. Quant. Spectrosc. Radiat. Transf. 326 (2024) 109083, DOI
  10.1016/j.jqsrt.2024.109083) is cited beside them.
- Provided as is, without warranties (CC BY-SA 4.0, Section 5).
