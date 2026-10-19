//! The perennial ices (R09.T13.d; decision-r09-t13d-normalisation §1): each ice of the inputs
//! placed to plan 14's area for its substance (`surface_ices`) on the cells coldest against its
//! own frost point by their annual radiative mean.
//!
//! # The rule ([`place_ices`])
//!
//! - **The key** of field cell i is `K_i`, its annual radiative mean surface temperature, ⟨T⁴⟩^¼
//!   over the year, after the normalisation and the lapse term ([`IceKey`]); ties go by its
//!   warmest month's surface temperature `T̂_i`, then by its index. The radiative mean ranks cells by
//!   the annual energy a frost-pinned ice's mass balance turns on (Leighton and Murray 1966,
//!   Science 153, 136, as the ruling read it), and it is the kind of mean plan 14's own profile is.
//!   The warmest month ices a dry world's tropics at Earth's tilt and a Mars-like ring near 60° N;
//!   the arithmetic mean ices the poles of a warm dry world at 90° and of a Pluto (the ruling's
//!   §1.3).
//! - **The ices placed** are the inputs' ([`CoarseInputs::ices`]) with a positive area, less the
//!   cold-trapped ones that R09.T13.f places in each cell's cold-trap fraction (`cold_trapped`;
//!   none until T13.f).
//! - **Their order**: ascending frost point ([`frost_point_k`](super::frost::frost_point_k)), ties
//!   by key, the most volatile first, which alone passes the Mars test, since a joint ranking of
//!   (ice, cell) pairs gives water the coldest cells (research-d §2.2). An ice of a row without a
//!   phase diagram is ranked against its palette entry's transition, its first melt.
//! - **Each in turn** takes the cells no ice holds yet in ascending (`K_i` − `T_f`, `T̂_i`, cell index),
//!   whole cells while the next one still fits inside its area, then the next cell the remainder
//!   as its fraction. Each cell holds one ice. The margin `K_i` − `T_f` sorts as `K_i` while `T_f` is
//!   a body constant; it is kept so that a frost point by cell (the ruling's D2) enters with no
//!   change to the rule.
//! - **Above its frost point**: plan 14's area is placed even where the field is warmer than
//!   `T_f` (Design note 3); a diagnostic counts those cells, which is no output.
//!
//! R09.T14.d, which re-applies the lapse term and the mean after erosion, calls [`place_ices`]
//! again with its own keys: a placement first takes back the ices it places, and leaves every
//! other cell's ice, a cold trap's, alone.

use hyperion_surface::field::PaletteRole;
use hyperion_surface::substance_key::SubstanceKey;

use super::super::CellState;
use super::finite_k;
use super::frost::frost_point_k;
use crate::planetary::surface::inputs::CoarseInputs;

/// A field cell's temperatures the ices are ranked by (the [module documentation](self)), K.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct IceKey {
    /// `K_i`, its annual radiative mean surface temperature, ⟨T⁴⟩^¼ over the year.
    pub(crate) radiative_mean_k: f64,
    /// `T̂_i`, its warmest month's surface temperature, the first tie-break.
    pub(crate) warmest_k: f64,
}

impl IceKey {
    /// The key of a cell whose month k's surface temperature is `months_k[k]`, each month's share
    /// of the year `shares[k]` (the arithmetic models': `K_i` = (Σₖ wₖ `T_ik`⁴)^¼, which a one-month
    /// year's one month gives as its own temperature).
    #[must_use]
    pub(crate) fn of_months(months_k: &[f64], shares: &[f64]) -> Self {
        if let [only] = months_k {
            return Self {
                radiative_mean_k: *only,
                warmest_k: *only,
            };
        }
        let (fourth, warmest) = months_k.iter().zip(shares).fold(
            (0.0, f64::NEG_INFINITY),
            |(sum, warmest): (f64, f64), (&t, &w)| {
                (
                    sum + w * t * t * t * t,
                    if t > warmest { t } else { warmest },
                )
            },
        );
        // By comparison, so that a NaN reaches the keys' finiteness check.
        let fourth = if fourth < 0.0 { 0.0 } else { fourth };
        Self {
            radiative_mean_k: fourth.sqrt().sqrt(),
            warmest_k: warmest,
        }
    }
}

/// One ice a placement placed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlacedIce {
    /// The substance.
    pub(crate) substance: SubstanceKey,
    /// Its palette entry.
    pub(crate) entry: u8,
    /// Its frost point, K.
    pub(crate) frost_point_k: f64,
    /// The share of the surface it was given, plan 14's.
    pub(crate) target: f64,
    /// The share of the surface it covers, Σ Ω f ÷ Σ Ω.
    pub(crate) placed: f64,
    /// The cells it lies on, whole or in part.
    pub(crate) cells: u32,
    /// Of those, the cells whose key lies above its frost point.
    pub(crate) above_frost_point: u32,
}

/// Places every perennial ice of `inputs` but `cold_trapped` over the field cells `cells` of solid
/// angles `solid_angles_sr`, by their keys `keys` (the [module documentation](self)): each placed
/// cell's `ice_fraction` and `ice_entry`, the ice's [`PaletteRole::Ice`] entry. What it placed,
/// in the order it placed it.
///
/// A cell that names one of the ices it places is cleared first; a cell that names any other ice
/// keeps it and takes none.
///
/// # Panics
///
/// If a key is not finite, or an ice has no `Ice` entry in the palette, which the inputs' builder
/// refuses.
pub(crate) fn place_ices(
    inputs: &CoarseInputs,
    solid_angles_sr: &[f64],
    keys: &[IceKey],
    cold_trapped: &[SubstanceKey],
    cells: &mut [CellState],
) -> Vec<PlacedIce> {
    let palette = inputs.palette();
    let mut ices: Vec<(f64, SubstanceKey, u8, f64)> = inputs
        .ices()
        .iter()
        .filter(|ice| ice.area_fraction > 0.0 && !cold_trapped.contains(&ice.substance))
        .map(|ice| {
            let entry = palette
                .find(PaletteRole::Ice, ice.substance)
                .expect("the inputs' builder gives every ice an Ice entry");
            let frost = frost_point_k(inputs, ice.substance).unwrap_or_else(|| {
                palette
                    .get(entry)
                    .expect("a palette entry that `find` names is in the palette")
                    .transition
                    .value()
            });
            (finite_k(frost), ice.substance, entry, ice.area_fraction)
        })
        .collect();
    ices.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for cell in cells.iter_mut() {
        if cell
            .ice_entry
            .is_some_and(|entry| ices.iter().any(|ice| ice.2 == entry))
        {
            cell.ice_entry = None;
            cell.ice_fraction = 0.0;
        }
    }
    for key in keys {
        let _finite = (finite_k(key.radiative_mean_k), finite_k(key.warmest_k));
    }
    let total = solid_angles_sr.iter().fold(0.0, |sum, omega| sum + omega);
    let mut placed = Vec::with_capacity(ices.len());
    for (frost, substance, entry, area) in ices {
        let mut order: Vec<(f64, f64, usize)> = cells
            .iter()
            .zip(keys)
            .enumerate()
            .filter(|(_, (cell, _))| cell.ice_entry.is_none())
            .map(|(i, (_, key))| (key.radiative_mean_k - frost, key.warmest_k, i))
            .collect();
        order.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(a.1.total_cmp(&b.1))
                .then(a.2.cmp(&b.2))
        });
        let target = area * total;
        let (mut held, mut count, mut above) = (0.0, 0_u32, 0_u32);
        for (margin, _, i) in order {
            let omega = solid_angles_sr[i];
            let fraction = if held + omega <= target {
                1.0
            } else {
                (target - held) / omega
            };
            if fraction.is_nan() || fraction <= 0.0 {
                break;
            }
            cells[i].ice_fraction = fraction;
            cells[i].ice_entry = Some(entry);
            held += fraction * omega;
            count += 1;
            if margin > 0.0 {
                above += 1;
            }
            if fraction < 1.0 {
                break;
            }
        }
        placed.push(PlacedIce {
            substance,
            entry,
            frost_point_k: frost,
            target: area,
            placed: held / total,
            cells: count,
            above_frost_point: above,
        });
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::surface::grid::CellGraph;
    use crate::planetary::surface::inputs::AreaShare;
    use crate::planetary::surface::reference;
    use crate::planetary::surface::steps::climate::tests::builder_of;
    use crate::planetary::surface::steps::{Pass, Working};
    use hyperion_surface::field::CoarseLevel;
    use hyperion_testkit::float::assert_same_bits;

    fn key(k: &str) -> SubstanceKey {
        SubstanceKey::new(k).unwrap()
    }

    /// A placement of water on six cells of equal area, carbon dioxide held out as a cold trap's
    /// (as R09.T13.f's would be): the cell naming carbon dioxide keeps it and takes no water; the
    /// cells naming water from an earlier placement are cleared first; and among the cells of one
    /// key the coldest warmest month goes first, then the cell index. Water's 2.25 cells of area
    /// take cells 2 and 4 (one key, the same warmest month, by index), then a quarter of cell 1 (a
    /// warmer month); cell 3 (warmer still) and cell 5 (a warmer key) are left clear.
    #[test]
    fn a_placement_clears_its_own_ice_keeps_another_and_breaks_ties_by_the_warmest_month() {
        let mars = reference::mars_like();
        let inputs = builder_of(&mars)
            .ices([
                AreaShare {
                    substance: key("H2O"),
                    area_fraction: 0.375,
                },
                AreaShare {
                    substance: key("CO2"),
                    area_fraction: 0.1,
                },
            ])
            .build()
            .unwrap();
        let palette = inputs.palette();
        let (water, co2) = (
            palette.find(PaletteRole::Ice, key("H2O")).unwrap(),
            palette.find(PaletteRole::Ice, key("CO2")).unwrap(),
        );
        let level = CoarseLevel::new(5).unwrap();
        let (field, climate) = (CellGraph::for_field(level), CellGraph::for_climate(level));
        let pass = Pass::new(reference::SEED, &inputs, &field, &climate);
        let mut cells = Working::initial(&pass).cells[..6].to_vec();
        for (cell, held) in cells.iter_mut().zip([
            Some((co2, 1.0)),
            Some((water, 0.3)),
            None,
            Some((water, 0.7)),
            None,
            Some((water, 1.0)),
        ]) {
            if let Some((entry, fraction)) = held {
                cell.ice_entry = Some(entry);
                cell.ice_fraction = fraction;
            }
        }
        let keys = [
            (200.0, 250.0),
            (150.0, 220.0),
            (150.0, 210.0),
            (150.0, 230.0),
            (150.0, 210.0),
            (300.0, 310.0),
        ]
        .map(|(radiative_mean_k, warmest_k)| IceKey {
            radiative_mean_k,
            warmest_k,
        });
        let placed = place_ices(&inputs, &[1.0; 6], &keys, &[key("CO2")], &mut cells);
        assert_eq!(placed.len(), 1, "{placed:?}");
        assert_eq!(placed[0].substance, key("H2O"));
        assert_eq!(placed[0].cells, 3);
        assert_eq!(placed[0].above_frost_point, 0);
        assert_same_bits(placed[0].placed, 0.375);
        let expected = [
            (Some(co2), 1.0),
            (Some(water), 0.25),
            (Some(water), 1.0),
            (None, 0.0),
            (Some(water), 1.0),
            (None, 0.0),
        ];
        for (n, (cell, (entry, fraction))) in cells.iter().zip(expected).enumerate() {
            assert_eq!(cell.ice_entry, entry, "cell {n}");
            assert_same_bits(cell.ice_fraction, fraction);
        }
    }
}
