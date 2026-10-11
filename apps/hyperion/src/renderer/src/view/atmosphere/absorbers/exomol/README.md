# ExoMol-derived data

The files in this directory are adapted material of the ExoMol database (J. Tennyson et al.,
<https://www.exomol.com>), whose data are licensed under the Creative Commons
Attribution-ShareAlike 4.0 International licence (CC BY-SA 4.0,
<https://creativecommons.org/licenses/by-sa/4.0/>): "All data in ExoMol is released under the
Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0) licence"
(<https://www.exomol.com/data/licence/>, read 2026-10-10 and 2026-10-11).

- `bounds.json` (plan R08, R08.T4.c; `decision-r08-t4b-t12c.md`, follow-ups 3 and 4): for each
  pending absorber, its 1 nm cell-mean cross-sections over 380–760 nm at the line list's
  temperature nodes, reduced from the line lists each entry names (C2018, AYT2, SiOUVenIR, Li2015,
  TYM, Harris, Toto, HyVO, MoLLIST's FeH and POKAZATEL), with each list's paper, its dataset
  DOI where ExoMol gives one, the files' SHA-256 and the date fetched. The values were computed
  from the line lists, by direct sums or through ExoMol's cross-section service, a modification
  of them; CO's are scaled to HITRAN2024's 7–0 band, as each entry's `method` states.
- This adapted material is offered under CC BY-SA 4.0. It is kept in a file of its own, never
  merged into `crossSections.json` or into code (decision-composition §5), and `NOTICE` carries
  its attribution.
- Provided as is, without warranties (CC BY-SA 4.0, Section 5).
