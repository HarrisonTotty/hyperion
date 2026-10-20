//! The perennial ices (R09.T13.d and R09.T13.f; decision-r09-t13d-normalisation §1,
//! decision-r09-t13f-cold-traps §1.5 and §5): each ice of the inputs placed to plan 14's area for
//! its substance (`surface_ices`), its zonal part on the cells coldest against its own frost point
//! by their annual radiative mean, and its cold-trapped part ([`CoarseInputs::cold_traps`]) in each
//! cell's cold-trap fraction, both in one placement ([`place_ices`]).
//!
//! # The zonal parts (R09.T13.d)
//!
//! - **The key** of field cell i is `K_i`, its annual radiative mean surface temperature, ⟨T⁴⟩^¼
//!   over the year, after the normalisation and the lapse term ([`IceKey`]); ties go by its
//!   warmest month's surface temperature `T̂_i`, then by its index. The radiative mean ranks cells by
//!   the annual energy a frost-pinned ice's mass balance turns on (Leighton and Murray 1966,
//!   Science 153, 136, as the ruling read it), and it is the kind of mean plan 14's own profile is.
//!   The warmest month ices a dry world's tropics at Earth's tilt and a Mars-like ring near 60° N;
//!   the arithmetic mean ices the poles of a warm dry world at 90° and of a Pluto (the ruling's
//!   §1.3).
//! - **The ices placed** are the inputs' ([`CoarseInputs::ices`]) with a positive area, each its
//!   zonal part here: its share less its cold-trapped part, which the cold traps place (below).
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
//! # The cold-trapped parts (R09.T13.f)
//!
//! After every zonal part, each ice with a cold-trapped share `A_s` above zero, in the same order
//! (decision-r09-t13f-cold-traps §5.3):
//!
//! - **The fractions** ([`trap_fractions`]). One `ColdTrapGround` a body, plan 14's (P14.T51.f),
//!   built from the inputs' spin obliquity, host flux and seasonal orbit's eccentricity (0 without
//!   one), the cold-trap inputs' star angular radius and bare-ground Bond albedo
//!   ([`CoarseInputs::cold_trap_inputs`]), the heat flow and the crater contract, whose terrain is
//!   plan 14's `terrain_of`; each trapped substance's threshold is plan 14's `trap_threshold`. Each
//!   field cell's fraction `f_s,i` of every trapped substance comes from one `fractions_at` call at
//!   its centre's latitude, asin z, a share of its ground: plan 14's closed form where the zonal
//!   annual maximum there reaches the threshold, and 0 under the zonal cap. The model is
//!   symmetric about the equator. A body whose sky the closed form refuses has none anywhere.
//! - **The scale** `k_s` = `A_s` Σ Ω ÷ Σ Ω `f_s`, the sums over every field cell in index order,
//!   held to at most 1, and each cell's share `w_s,i` = `k_s` `f_s,i`, so that no share exceeds its
//!   fraction. Where plan 14 and the field agree, `k_s` is plan 14's exposed fraction `f_x` up to
//!   the cells' quadrature. Where it is held, the rest of `A_s` is `short`.
//! - **Eligibility.** A cell with no ice takes a share; a cell holding s's own zonal part adds s's
//!   share, to at most 1, the rest `undrawn`; a cell holding another ice keeps it, and every share
//!   there is `undrawn`, never pushed onto the free cells.
//! - **One ice a cell.** A free cell takes the trapped ice of largest share, ties in the ices'
//!   order, and the others' shares there are `undrawn`. A colder species' cold-trapping ground lies
//!   inside a warmer one's (`fractions_at` does not fall as its threshold rises), so they are
//!   nested in it; moving them to other cells would put ice where the model has none.
//! - **The quantisation.** Each ice's shares on the free cells it takes are written as whole codes
//!   of the field's ice byte, c ÷ [`ICE_CODES`], the cells taken coldest first by the zonal key:
//!   each cell's code is the one nearest its share plus the rounding carried so far, and at most
//!   the code below its fraction. So each drawn total matches its shares to under one cell, a
//!   share under half a code is carried to the next cell rather than rounded away, and no written
//!   share exceeds its fraction. A share added to the ice's own zonal cell is written as it is,
//!   for the quantiser to round with the zonal part. A trapped ice is never placed coldest-first by
//!   whole cells.
//! - **The diagnostics** ([`PlacedTrap`]), no output: each part's target, the field's mean
//!   fraction, `k_s`, `placed` (Σ Ω over what it wrote, ÷ Σ Ω), `undrawn`, `short` and its cells,
//!   with placed + undrawn + short = `A_s` to the quantisation's rounding.
//!
//! R09.T14.d, which re-applies the lapse term and the mean after erosion, calls [`place_ices`]
//! again with its own keys: a placement first takes back every ice it places, both of its parts,
//! and leaves every other cell's ice alone, so that a second call rebuilds both.

use hyperion_surface::field::PaletteRole;
use hyperion_surface::substance_key::SubstanceKey;

use super::super::CellState;
use super::finite_k;
use super::frost::frost_point_k;
use crate::math;
use crate::planetary::hooks::cold_traps::{ColdTrapGround, ColdTrapGroundParts, trap_threshold};
use crate::planetary::surface::inputs::CoarseInputs;
use crate::substance::by_key;
use crate::units::{Kelvin, Radians};

/// The steps of a cell's ice share in the field: 255, the ice byte's 1 ÷ 255
/// (`hyperion_surface::field::SynthesisCell::quantise_ice`, Design note 17), in which the cold
/// traps write their shares whole.
pub(crate) const ICE_CODES: f64 = 255.0;

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

/// One ice's zonal part a placement placed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlacedIce {
    /// The substance.
    pub(crate) substance: SubstanceKey,
    /// Its palette entry.
    pub(crate) entry: u8,
    /// Its frost point, K.
    pub(crate) frost_point_k: f64,
    /// The share of the surface its zonal part was given: plan 14's, less its cold-trapped part.
    pub(crate) target: f64,
    /// The share of the surface it covers, Σ Ω f ÷ Σ Ω.
    pub(crate) placed: f64,
    /// The cells it lies on, whole or in part.
    pub(crate) cells: u32,
    /// Of those, the cells whose key lies above its frost point.
    pub(crate) above_frost_point: u32,
}

/// One ice's cold-trapped part a placement placed (R09.T13.f), a diagnostic that is no output
/// (decision-r09-t13f-cold-traps §5.3 item 6). Its shares of the surface obey placed + undrawn +
/// short = target to the quantisation's rounding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlacedTrap {
    /// The substance.
    pub(crate) substance: SubstanceKey,
    /// Its palette entry, its zonal part's.
    pub(crate) entry: u8,
    /// Its cold-trap threshold, K (plan 14's `trap_threshold`).
    pub(crate) threshold_k: f64,
    /// `A_s`, the share of the surface its cold-trapped part was given, plan 14's.
    pub(crate) target: f64,
    /// The field's mean cold-trap fraction of it, Σ Ω f ÷ Σ Ω over every cell.
    pub(crate) fraction: f64,
    /// `k_s`, the scale from each cell's fraction to its share, 0 to 1.
    pub(crate) scale: f64,
    /// The share of the surface it covers as written, Σ Ω over its cells' written shares ÷ Σ Ω:
    /// the codes on the free cells it takes, and what it adds to its own zonal cells.
    pub(crate) placed: f64,
    /// The share of the surface it lies on but does not draw: inside another ice on that ice's
    /// cells, inside a trapped ice of larger share on a free cell, or over 1 on its own zonal
    /// cells.
    pub(crate) undrawn: f64,
    /// The share the scale's hold at 1 leaves unplaced, `A_s` − Σ Ω f ÷ Σ Ω, where the field's
    /// fractions cannot hold its target; 0 otherwise.
    pub(crate) short: f64,
    /// The cells it was written on.
    pub(crate) cells: u32,
}

/// What a placement placed: each ice's zonal part and each cold-trapped part, in their order.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PlacedIces {
    /// The zonal parts, one for each ice with a positive area.
    pub(crate) zonal: Vec<PlacedIce>,
    /// The cold-trapped parts, one for each ice with a cold-trapped share above zero.
    pub(crate) traps: Vec<PlacedTrap>,
}

/// One ice of the inputs as a placement takes it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ice {
    /// Its frost point, K, by which the ices are ordered.
    frost_k: f64,
    /// The substance.
    substance: SubstanceKey,
    /// Its `Ice` palette entry.
    entry: u8,
    /// Its zonal part's share of the surface: its share less its cold-trapped part.
    zonal: f64,
    /// Its cold-trapped part's share of the surface.
    trapped: f64,
}

/// The field cells a placement reads: their solid angles, sr, their centres, unit vectors in the
/// body-fixed frame, and their keys, each in cell-index order, and their total solid angle, sr.
#[derive(Debug, Clone, Copy)]
struct Field<'a> {
    solid_angles_sr: &'a [f64],
    centres: &'a [[f64; 3]],
    keys: &'a [IceKey],
    total_sr: f64,
}

/// Places every perennial ice of `inputs` over the field cells `cells` of solid angles
/// `solid_angles_sr`, centres `centres` (unit vectors in the body-fixed frame) and keys `keys`, all
/// in cell-index order (the [module documentation](self)): each ice's zonal part, then each
/// cold-trapped part, writing each placed cell's `ice_fraction` and `ice_entry`, the ice's
/// [`PaletteRole::Ice`] entry. What it placed, in the order it placed it.
///
/// A cell that names one of the ices it places is cleared first, whichever part put it there; a
/// cell that names any other ice keeps it and takes none.
///
/// # Panics
///
/// If a key is not finite, the slices differ in length, an ice has no `Ice` entry in the palette,
/// or a cold-trapped ice no cold-trap threshold, which the inputs' builder refuses.
pub(crate) fn place_ices(
    inputs: &CoarseInputs,
    solid_angles_sr: &[f64],
    centres: &[[f64; 3]],
    keys: &[IceKey],
    cells: &mut [CellState],
) -> PlacedIces {
    assert!(
        solid_angles_sr.len() == cells.len()
            && centres.len() == cells.len()
            && keys.len() == cells.len(),
        "a placement reads one solid angle, centre and key a cell"
    );
    let ices = ices_of(inputs);
    for cell in cells.iter_mut() {
        if cell
            .ice_entry
            .is_some_and(|entry| ices.iter().any(|ice| ice.entry == entry))
        {
            cell.ice_entry = None;
            cell.ice_fraction = 0.0;
        }
    }
    for key in keys {
        let _finite = (finite_k(key.radiative_mean_k), finite_k(key.warmest_k));
    }
    let field = Field {
        solid_angles_sr,
        centres,
        keys,
        total_sr: solid_angles_sr.iter().fold(0.0, |sum, omega| sum + omega),
    };
    let zonal = ices
        .iter()
        .map(|ice| place_zonal(ice, &field, cells))
        .collect();
    let traps = place_traps(inputs, &ices, &field, cells);
    PlacedIces { zonal, traps }
}

/// The ices of `inputs` with a positive area, in their placement's order: ascending frost point,
/// ties by key, each with its zonal and cold-trapped parts.
///
/// # Panics
///
/// If an ice has no `Ice` entry in the palette, which the inputs' builder refuses.
#[must_use]
fn ices_of(inputs: &CoarseInputs) -> Vec<Ice> {
    let palette = inputs.palette();
    let mut ices: Vec<Ice> = inputs
        .ices()
        .iter()
        .filter(|ice| ice.area_fraction > 0.0)
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
            let trapped = inputs
                .cold_traps()
                .iter()
                .find(|trap| trap.substance == ice.substance)
                .map_or(0.0, |trap| trap.area_fraction);
            // The builder holds a cold-trapped part to at most its ice's share.
            let zonal = ice.area_fraction - trapped;
            Ice {
                frost_k: finite_k(frost),
                substance: ice.substance,
                entry,
                zonal: if zonal > 0.0 { zonal } else { 0.0 },
                trapped,
            }
        })
        .collect();
    ices.sort_by(|a, b| {
        a.frost_k
            .total_cmp(&b.frost_k)
            .then(a.substance.cmp(&b.substance))
    });
    ices
}

/// Places the zonal part of `ice` on the free cells of `cells` by T13.d's rule (the [module
/// documentation](self)): whole cells coldest first against its frost point, then the remainder.
fn place_zonal(ice: &Ice, field: &Field<'_>, cells: &mut [CellState]) -> PlacedIce {
    let mut order: Vec<(f64, f64, usize)> = cells
        .iter()
        .zip(field.keys)
        .enumerate()
        .filter(|(_, (cell, _))| cell.ice_entry.is_none())
        .map(|(i, (_, key))| (key.radiative_mean_k - ice.frost_k, key.warmest_k, i))
        .collect();
    order.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then(a.1.total_cmp(&b.1))
            .then(a.2.cmp(&b.2))
    });
    let target = ice.zonal * field.total_sr;
    let (mut held, mut count, mut above) = (0.0, 0_u32, 0_u32);
    for (margin, _, i) in order {
        let omega = field.solid_angles_sr[i];
        let fraction = if held + omega <= target {
            1.0
        } else {
            (target - held) / omega
        };
        if fraction.is_nan() || fraction <= 0.0 {
            break;
        }
        cells[i].ice_fraction = fraction;
        cells[i].ice_entry = Some(ice.entry);
        held += fraction * omega;
        count += 1;
        if margin > 0.0 {
            above += 1;
        }
        if fraction < 1.0 {
            break;
        }
    }
    PlacedIce {
        substance: ice.substance,
        entry: ice.entry,
        frost_point_k: ice.frost_k,
        target: ice.zonal,
        placed: held / field.total_sr,
        cells: count,
        above_frost_point: above,
    }
}

/// The cold-trap threshold of the substance `substance`, K (plan 14's `trap_threshold`).
///
/// # Panics
///
/// If its row has none, which the inputs' builder refuses for a cold-trapped ice.
#[must_use]
fn threshold_of(substance: SubstanceKey) -> Kelvin {
    by_key(substance.as_str())
        .and_then(trap_threshold)
        .expect("the inputs' builder refuses a cold-trapped ice with no cold-trap threshold")
}

/// The body's cold-trap ground from `inputs` (decision-r09-t13f-cold-traps §3.2, "R09's side"):
/// plan 14's `ColdTrapGround` of its spin's obliquity, its host flux, its seasonal orbit's
/// eccentricity (0 without one), its cold-trap inputs' star angular radius and Bond albedo, its
/// heat flow and its crater contract. `None` without cold-trap inputs, or where the closed form
/// refuses the sky (no light, a star that fills it, an albedo of 1).
#[must_use]
fn ground_of(inputs: &CoarseInputs) -> Option<ColdTrapGround> {
    let group = inputs.cold_trap_inputs()?;
    ColdTrapGround::new(&ColdTrapGroundParts {
        obliquity: inputs.spin().obliquity,
        orbit_mean_flux: inputs.host_flux(),
        eccentricity: inputs
            .seasonal_orbit()
            .map_or(0.0, |orbit| orbit.eccentricity),
        star_radius: group.star_angular_radius,
        bond_albedo: group.bond_albedo,
        heat_flow: inputs.heat_flow(),
        craters: inputs.craters(),
    })
}

/// Each field cell's cold-trap fraction of each threshold of `thresholds` on the body of `inputs`,
/// row by row in cell-index order, each row the thresholds' fractions in their order: one call of
/// plan 14's `ColdTrapGround::fractions_at` at each centre of `centres` (unit vectors in the
/// body-fixed frame), at its latitude asin z, a share of the cell's ground, 0 to 1. Every fraction
/// is 0 on a body with no ground ([`ground_of`]'s `None`).
///
/// # Panics
///
/// If a threshold is not finite and positive.
#[must_use]
pub(super) fn trap_fractions(
    inputs: &CoarseInputs,
    centres: &[[f64; 3]],
    thresholds: &[Kelvin],
) -> Vec<f64> {
    let n = thresholds.len();
    let mut fractions = vec![0.0; centres.len() * n];
    if n == 0 {
        return fractions;
    }
    let Some(ground) = ground_of(inputs) else {
        return fractions;
    };
    for (row, centre) in fractions.chunks_exact_mut(n).zip(centres) {
        let latitude = Radians::new(math::asin(centre[2].clamp(-1.0, 1.0)));
        ground.fractions_at(latitude, thresholds, row);
    }
    fractions
}

/// One cold-trapped part as its placement proceeds.
#[derive(Debug, Clone, PartialEq)]
struct Trapping {
    /// The ice.
    ice: Ice,
    /// Its cold-trap threshold, K.
    threshold_k: f64,
    /// Σ Ω f over every cell, sr.
    fraction_sr: f64,
    /// `k_s`.
    scale: f64,
    /// The share of the surface the scale's hold leaves unplaced.
    short: f64,
    /// What it wrote, Σ Ω over its written shares, sr.
    placed_sr: f64,
    /// What it lies on but does not draw, sr.
    undrawn_sr: f64,
    /// The cells it was written on.
    cells: u32,
    /// The free cells it takes, by index, before their quantisation.
    drawn: Vec<usize>,
}

/// Places the cold-trapped part of every ice of `ices` over `cells`, after their zonal parts
/// (the [module documentation](self)'s "The cold-trapped parts").
fn place_traps(
    inputs: &CoarseInputs,
    ices: &[Ice],
    field: &Field<'_>,
    cells: &mut [CellState],
) -> Vec<PlacedTrap> {
    let trapped: Vec<Ice> = ices
        .iter()
        .filter(|ice| ice.trapped > 0.0)
        .copied()
        .collect();
    if trapped.is_empty() {
        return Vec::new();
    }
    let thresholds: Vec<Kelvin> = trapped
        .iter()
        .map(|ice| threshold_of(ice.substance))
        .collect();
    let n = trapped.len();
    let fractions = trap_fractions(inputs, field.centres, &thresholds);
    let mut sums = vec![0.0; n];
    for (row, &omega) in fractions.chunks_exact(n).zip(field.solid_angles_sr) {
        for (sum, &f) in sums.iter_mut().zip(row) {
            *sum += omega * f;
        }
    }
    let mut traps: Vec<Trapping> = trapped
        .iter()
        .zip(&thresholds)
        .zip(&sums)
        .map(|((&ice, threshold), &sum)| {
            let wanted = ice.trapped * field.total_sr;
            let (scale, short) = if sum > wanted {
                (wanted / sum, 0.0)
            } else {
                let rest = ice.trapped - sum / field.total_sr;
                (1.0, if rest > 0.0 { rest } else { 0.0 })
            };
            Trapping {
                ice,
                threshold_k: threshold.value(),
                fraction_sr: sum,
                scale,
                short,
                placed_sr: 0.0,
                undrawn_sr: 0.0,
                cells: 0,
                drawn: Vec::new(),
            }
        })
        .collect();
    share_out(&mut traps, &fractions, field, cells);
    for (s, trap) in traps.iter_mut().enumerate() {
        quantise_trap(trap, s, n, &fractions, field, cells);
    }
    traps
        .into_iter()
        .map(|trap| PlacedTrap {
            substance: trap.ice.substance,
            entry: trap.ice.entry,
            threshold_k: trap.threshold_k,
            target: trap.ice.trapped,
            fraction: trap.fraction_sr / field.total_sr,
            scale: trap.scale,
            placed: trap.placed_sr / field.total_sr,
            undrawn: trap.undrawn_sr / field.total_sr,
            short: trap.short,
            cells: trap.cells,
        })
        .collect()
}

/// Shares each field cell's cold-trapped ice among `traps`, whose fractions `fractions` holds row
/// by row (the [module documentation](self)'s "Eligibility" and "One ice a cell"): on a cell that
/// holds a trapped ice's own zonal part its share is added there, to at most 1, and every other
/// share is undrawn; on a cell that holds another ice every share is undrawn; and a free cell is
/// taken by the trapped ice of largest share, ties to the first, the others undrawn there.
fn share_out(
    traps: &mut [Trapping],
    fractions: &[f64],
    field: &Field<'_>,
    cells: &mut [CellState],
) {
    let n = traps.len();
    for (i, row) in fractions.chunks_exact(n).enumerate() {
        let omega = field.solid_angles_sr[i];
        if let Some(entry) = cells[i].ice_entry {
            // A zonal part's cell: its own trapped part adds to it, and every other share there
            // lies inside it.
            let own = traps.iter().position(|trap| trap.ice.entry == entry);
            for (slot, (trap, &fraction)) in traps.iter_mut().zip(row).enumerate() {
                let share = trap.scale * fraction;
                if share.is_nan() || share <= 0.0 {
                    continue;
                }
                if own == Some(slot) {
                    let zonal = cells[i].ice_fraction;
                    let total = if zonal + share < 1.0 {
                        zonal + share
                    } else {
                        1.0
                    };
                    let added = total - zonal;
                    cells[i].ice_fraction = total;
                    trap.placed_sr += omega * added;
                    trap.undrawn_sr += omega * (share - added);
                    if added > 0.0 {
                        trap.cells += 1;
                    }
                } else {
                    trap.undrawn_sr += omega * share;
                }
            }
        } else {
            // A free cell: the trapped ice of largest share, ties in the ices' order.
            let mut best: Option<(usize, f64)> = None;
            for (slot, (trap, &fraction)) in traps.iter().zip(row).enumerate() {
                let share = trap.scale * fraction;
                if share > 0.0 && best.is_none_or(|(_, most)| share > most) {
                    best = Some((slot, share));
                }
            }
            for (slot, (trap, &fraction)) in traps.iter_mut().zip(row).enumerate() {
                let share = trap.scale * fraction;
                if share.is_nan() || share <= 0.0 {
                    continue;
                }
                if best.is_some_and(|(chosen, _)| chosen == slot) {
                    trap.drawn.push(i);
                } else {
                    trap.undrawn_sr += omega * share;
                }
            }
        }
    }
}

/// Writes the shares of `trap`, the `s`-th of `n` trapped ices whose fractions `fractions` holds
/// row by row, on the free cells it takes, coldest first by the zonal key, as whole codes of the
/// ice byte (the [module documentation](self)'s "The quantisation"): each the code nearest its
/// share plus the rounding carried so far, from 0 to the code at or below its fraction.
fn quantise_trap(
    trap: &mut Trapping,
    s: usize,
    n: usize,
    fractions: &[f64],
    field: &Field<'_>,
    cells: &mut [CellState],
) {
    let mut order: Vec<(f64, f64, usize)> = trap
        .drawn
        .iter()
        .map(|&i| {
            let key = field.keys[i];
            (key.radiative_mean_k - trap.ice.frost_k, key.warmest_k, i)
        })
        .collect();
    order.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then(a.1.total_cmp(&b.1))
            .then(a.2.cmp(&b.2))
    });
    let mut carry_sr = 0.0;
    for (_, _, i) in order {
        let omega = field.solid_angles_sr[i];
        let f = fractions[i * n + s];
        let w = trap.scale * f;
        let most = (ICE_CODES * f).floor();
        let nearest = (ICE_CODES * (w + carry_sr / omega)).round();
        let code = if nearest < 0.0 {
            0.0
        } else if nearest > most {
            most
        } else {
            nearest
        };
        let share = code / ICE_CODES;
        carry_sr += omega * (w - share);
        if code > 0.0 {
            cells[i].ice_fraction = share;
            cells[i].ice_entry = Some(trap.ice.entry);
            trap.placed_sr += omega * share;
            trap.cells += 1;
        }
    }
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

    /// A placement of water on six cells of equal area, carbon dioxide named by a cell but not
    /// among the ices this placement places (as another step's ice would be): the cell naming
    /// carbon dioxide keeps it and takes no water; the cells naming water from an earlier placement
    /// are cleared first; and among the cells of one key the coldest warmest month goes first, then
    /// the cell index. Water's 2.25 cells of area take cells 2 and 4 (one key, the same warmest
    /// month, by index), then a quarter of cell 1 (a warmer month); cell 3 (warmer still) and cell
    /// 5 (a warmer key) are left clear.
    #[test]
    fn a_placement_clears_its_own_ice_keeps_another_and_breaks_ties_by_the_warmest_month() {
        let mars = reference::mars_like();
        let inputs = builder_of(&mars)
            .ices([AreaShare {
                substance: key("H2O"),
                area_fraction: 0.375,
            }])
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
        let centres = &field.centres()[..6];
        let placed = place_ices(&inputs, &[1.0; 6], centres, &keys, &mut cells);
        assert!(placed.traps.is_empty(), "{placed:?}");
        let placed = placed.zonal;
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
