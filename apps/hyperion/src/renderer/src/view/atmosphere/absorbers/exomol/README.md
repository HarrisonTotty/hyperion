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
- `kDistributions/<key>.json` and its parts `<key>-<n>.bin` for the hot molecules (plan R08,
  R08.T4.f): their k-distributions per 1 nm cell, reduced by super-lines on the grid and in the
  16-bit format each sidecar states, from these lists, their files fetched from the database's
  archive of each dataset on Zenodo (each record's DOI in its sidecar), whose records state CC BY
  4.0 (read 2026-10-29), the database's own licence quoted above:
  - `FeH`: MoLLIST's FeH as ExoMol carries it (M. Dulick et al., Astrophys. J. 594 (2003) 651; S.
    Wende, A. Reiners, A. Seifahrt and P. F. Bernath, Astron. Astrophys. 523 (2010) A58; P. F.
    Bernath, J. Quant. Spectrosc. Radiat. Transf. 240 (2020) 106687).
  - `TiO`: Toto's five Ti isotopologues (L. K. McKemmish et al., Mon. Not. R. Astron. Soc. 488
    (2019) 2836, DOI 10.1093/mnras/stz1818).
  - `N2`: WCCRMT's ¹⁴N₂, its triplet system (C. M. Western, L. Carter-Blatchford, P. Crozet, A. J.
    Ross, J. Morville and D. W. Tokaryk, J. Quant. Spectrosc. Radiat. Transf. 219 (2018) 127, DOI
    10.1016/j.jqsrt.2018.07.017), its partition function from P. S. Barklem and R. Collet, Astron.
    Astrophys. 588 (2016) A96.
  - `CO`: Li2015's ¹²C¹⁶O, its bands from v′ − v″ = 8 left out and its intensities scaled by 2.05
    to HITRAN2024's 7–0 band (G. Li, I. E. Gordon, L. S. Rothman, Y. Tan, S.-M. Hu, S. Kassi, A.
    Campargue and E. S. Medvedev, Astrophys. J. Suppl. Ser. 216 (2015) 15, DOI
    10.1088/0067-0049/216/1/15).
  - `SiO`: SiOUVenIR's ²⁸Si¹⁶O (S. N. Yurchenko et al., Mon. Not. R. Astron. Soc. 510 (2022) 903,
    DOI 10.1093/mnras/stab3267).
  - `H2S`: AYT2's H₂³²S, its transitions of 13,000–27,000 cm⁻¹ (A. A. A. Azzam, J. Tennyson, S. N.
    Yurchenko and O. V. Naumenko, Mon. Not. R. Astron. Soc. 460 (2016) 4063, DOI
    10.1093/mnras/stw1133).
  - `HCN`: Harris's H¹²C¹⁴N (G. J. Harris et al., Mon. Not. R. Astron. Soc. 367 (2006) 400, DOI
    10.1111/j.1365-2966.2005.09960.x; R. J. Barber et al., Mon. Not. R. Astron. Soc. 437 (2014)
    1828, DOI 10.1093/mnras/stt2011).
  - `VO`: HyVO's ⁵¹V¹⁶O (C. A. Bowesman, Q. Qu, L. K. McKemmish, S. N. Yurchenko and J. Tennyson,
    Mon. Not. R. Astron. Soc. 529 (2024) 1321, DOI 10.1093/mnras/stae542), its strong lines as P. E.
    Cubillos's REPACK holds them (Repacked ExoMol Opacity Line Lists, Zenodo record 14266247,
    version 3, DOI 10.5281/zenodo.14266247, whose record states CC BY 4.0, read 2026-10-30; the
    method P. E. Cubillos, Astrophys. J. 850 (2017) 32), with HyVO's `.def` and `.pf` from ExoMol's
    own site, under the database's licence quoted above.
- `lineReferencesNH3.json` and `lineReferencesN2O.json` (plan R08, R08.T4.g), and
  `lineReferencesHot.json` (plan R08, R08.T4.f): named cells' curves of growth at three node
  pressures and three midpoints, computed from the same lists, beside the reduction's own, which
  the tests read.
- `n2oMerge.json` (plan R08, R08.T4.g): HITRAN2024's N₂O bands against TYM's of the same
  assignment, and each merged cell's bands by list and its mean cross-sections, which the tests
  read.
- None of the line lists' files (`.def`, `.def.json`, `.states`, `.trans`, `.pf`, `.broad`,
  REPACK's `_lbl.dat`), nor any decompressed, filtered or binned copy of them, is committed; they are fetched by URL and
  checked against the SHA-256 the sidecars and `src/tools/exomolCli.ts` name.
- This adapted material is offered under CC BY-SA 4.0. It is kept in files of its own, never
  merged into `crossSections.json` or into code (decision-composition §5), and `NOTICE` carries
  its attribution. ExoMol asks that the line lists' papers be cited; the database's release paper
  (J. Tennyson et al., J. Quant. Spectrosc. Radiat. Transf. 326 (2024) 109083, DOI
  10.1016/j.jqsrt.2024.109083) is cited beside them.
- Provided as is, without warranties (CC BY-SA 4.0, Section 5).
