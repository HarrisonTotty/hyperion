//! The index-headroom check: no cell of any stellar layer may draw more candidates than its
//! layer's index field can number (plan 03, Design note 6).
//!
//! A cell's candidate count is a Poisson draw ([`candidate_count`](super::candidate_count)), so
//! nothing but a bound on its mean keeps it inside the layer's index capacity. A count above the
//! capacity would be clamped, and a clamped cell is silently short of systems in release builds,
//! which is exactly the failure mode the brainstorm's bound discussion warns about. So a galaxy is
//! checked once, when it is built for play, and refused if any layer's densest possible cell comes
//! within eight standard deviations of its capacity.
//!
//! The largest mean needs no search. It is [`Fields::layer_bound`] over one root octant,
//! `CellBox::new([0, 0, 0], 65_536)`, times the layer's cell volume: that box's nearest corner is
//! the origin, where every envelope peaks, and its ranges of radius and of arm phase contain every
//! cell's, in any octant, so its bound is at least every cell's bound (plan 02, R17). It is not the
//! sum of the components' density peaks, because plan 02's bound multiplies each envelope by an arm
//! factor's bound over the cell.

use super::cell::CellKey;
use super::layers::STELLAR_LAYERS;
use super::{ExceedIndexCapacityError, LayerSpec};
use crate::coords::ROOT_HALF_WIDTH_LY;
use crate::galaxy::Galaxy;
use crate::galaxy::bounds::CellBox;
#[cfg(doc)]
use crate::galaxy::fields::Fields;

/// How many standard deviations of a cell's Poisson draw must fit between the largest mean and the
/// index capacity: eight, about one chance in 10¹⁵ of a draw above the capacity per cell (plan 03,
/// Design note 6).
const HEADROOM_SIGMAS: f64 = 8.0;

/// Checks that no cell of `galaxy` can draw more candidates than its layer's IDs can number.
///
/// Whoever builds a [`Galaxy`] for play calls this once (plan 04's `GalaxyCache`, in
/// `compute/galaxies.rs`; the universe registry never builds one). Nothing on
/// the hot path calls it: [`candidate_count`](super::candidate_count) clamps instead, and this check
/// exists so that no galaxy which could reach the clamp is ever played.
///
/// # Errors
///
/// [`ExceedIndexCapacityError::LayerTooDense`] naming the first layer, coarsest to finest, whose
/// largest possible candidate count plus eight standard deviations reaches its index capacity. A
/// bound that is not finite fails the same way.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::check_index_headroom;
///
/// // Every galaxy a seed can draw has room; the fullest layer-A cell expects some 6,000
/// // candidates of 65,536.
/// check_index_headroom(&Galaxy::new(Seed::new(1)))?;
/// # Ok::<(), hyperion_sim::galaxy::placement::ExceedIndexCapacityError>(())
/// ```
pub fn check_index_headroom(galaxy: &Galaxy) -> Result<(), ExceedIndexCapacityError> {
    let octant = root_octant();
    for spec in STELLAR_LAYERS {
        let largest_mean = largest_cell_mean(galaxy, spec, &octant);
        let capacity = capacity_of(spec);
        if !fits_capacity(largest_mean, capacity) {
            return Err(ExceedIndexCapacityError::LayerTooDense {
                layer: spec.layer(),
                largest_mean,
                capacity,
            });
        }
    }
    Ok(())
}

/// One octant of the root cube, the box whose bound is at least every cell's (module
/// documentation).
///
/// # Panics
///
/// Never: the edge is a power of two, the box is the octant `[0, 65_536]³` of the root cube, and it
/// touches the axis planes without crossing one.
#[must_use]
fn root_octant() -> CellBox {
    CellBox::new([0, 0, 0], ROOT_HALF_WIDTH_LY.unsigned_abs())
        .expect("a root octant is a power-of-two box inside the cube that straddles no plane")
}

/// The largest number of candidates any cell of the layer can expect: the bound over `octant` times
/// the layer's cell volume.
#[must_use]
fn largest_cell_mean(galaxy: &Galaxy, spec: LayerSpec, octant: &CellBox) -> f64 {
    let bound = galaxy
        .fields()
        .layer_bound(galaxy.shares(), spec.band(), octant);
    bound * cell_of(spec).volume_ly3()
}

/// The layer's index capacity.
#[must_use]
fn capacity_of(spec: LayerSpec) -> u32 {
    cell_of(spec).index_capacity()
}

/// A cell of the layer, for its volume and its index capacity, which every cell of a layer shares.
///
/// # Panics
///
/// Never: a row of the layer table names a stellar layer, and cell `(0, 0, 0)` lies in the root
/// cube at every size.
#[must_use]
fn cell_of(spec: LayerSpec) -> CellKey {
    CellKey::new(spec.layer(), [0, 0, 0]).expect("cell (0, 0, 0) of a stellar layer is a key")
}

/// Whether a layer whose densest cell expects `largest_mean` candidates keeps eight standard
/// deviations of headroom below `capacity`.
///
/// A mean that is NaN or infinite fits nothing, so a galaxy whose bound is not finite is refused.
#[must_use]
fn fits_capacity(largest_mean: f64, capacity: u32) -> bool {
    largest_mean + HEADROOM_SIGMAS * largest_mean.sqrt() <= f64::from(capacity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::{GalaxyParams, GalaxyParamsBuilder};
    use crate::galaxy::placement::cell::layer_bound;
    use crate::id::Layer;
    use crate::rng::Seed;
    use crate::units::{LightYears, SolarMasses};

    /// The seed of the fixture galaxy, and the base of the sweep's seeds.
    const SEED: u64 = 0x0300_11ea_0000_0000;

    fn milky_way() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
    }

    #[test]
    fn headroom_holds_for_the_milky_way_fixture() {
        assert_eq!(check_index_headroom(&milky_way()), Ok(()));
    }

    #[test]
    fn headroom_holds_under_both_mass_functions() {
        for kind in [MassFunctionKind::Kroupa, MassFunctionKind::Chabrier] {
            let galaxy = Galaxy::with_mass_function(Seed::new(SEED | 1), kind);
            assert_eq!(check_index_headroom(&galaxy), Ok(()), "{kind:?}");
        }
    }

    /// The fullest layer-A cell expects about 6,000 candidates of 65,536 at Milky Way values (plan
    /// 03, Design note 6), and every layer keeps a wide margin.
    #[test]
    fn the_fullest_cell_of_each_layer_stays_well_inside_its_capacity() {
        let galaxy = milky_way();
        let octant = root_octant();
        for spec in STELLAR_LAYERS {
            let mean = largest_cell_mean(&galaxy, spec, &octant);
            let capacity = f64::from(capacity_of(spec));
            assert!(
                mean > 0.0 && mean < 0.5 * capacity,
                "layer {} expects {mean} candidates of {capacity}",
                spec.layer().letter()
            );
        }
        let layer_a = STELLAR_LAYERS
            .into_iter()
            .find(|spec| spec.layer() == Layer::A)
            .expect("the table has layer A");
        let mean = largest_cell_mean(&galaxy, layer_a, &octant);
        assert!(
            (3_000.0..12_000.0).contains(&mean),
            "the fullest layer-A cell expects {mean} candidates, not about 6,000"
        );
    }

    /// The octant's bound is at least every cell's, in every layer and every octant (module
    /// documentation): the claim that lets the check skip a search. The densest cells are the eight
    /// that touch the origin, whose bounds come within 0.4% of the octant's; the sweep reaches out
    /// geometrically to the cube's faces along every axis and diagonal.
    #[test]
    fn the_octant_bounds_every_cell_in_every_layer() {
        let galaxy = milky_way();
        let octant = root_octant();
        assert_eq!((octant.min_corner(), octant.edge()), ([0, 0, 0], 65_536));
        for spec in STELLAR_LAYERS {
            let layer = spec.layer();
            let largest_mean = largest_cell_mean(&galaxy, spec, &octant);
            let edge = 65_536 / i32::try_from(layer.cell_size_ly()).unwrap();
            // 0 to 3, then doubling to the last cell before the face.
            let mut steps = vec![0, 1, 2, 3];
            while let Some(&last) = steps.last().filter(|&&last| last < edge - 1) {
                steps.push((2 * last + 1).min(edge - 1));
            }
            let mut densest = 0.0_f64;
            for &sx in &steps {
                for &sy in &steps {
                    for &sz in &steps {
                        for signs in 0..8_u8 {
                            // A step of s is cell s on the positive side and cell −s − 1 on the
                            // negative, so the eight cells touching the origin are all visited.
                            let side =
                                |s: i32, bit: u8| if signs >> bit & 1 == 0 { s } else { -s - 1 };
                            let key = CellKey::new(layer, [side(sx, 0), side(sy, 1), side(sz, 2)])
                                .unwrap();
                            let mean = layer_bound(&galaxy, key) * key.volume_ly3();
                            assert!(
                                mean <= largest_mean,
                                "layer {} cell {:?} expects {mean}, above the octant's {largest_mean}",
                                layer.letter(),
                                key.gen_cell().to_array()
                            );
                            densest = densest.max(mean);
                        }
                    }
                }
            }
            // The sweep reached the densest cells, or it would prove little.
            assert!(
                densest > 0.99 * largest_mean,
                "layer {}: the densest cell swept expects {densest} of the octant's {largest_mean}",
                layer.letter()
            );
        }
    }

    #[test]
    fn the_comparison_keeps_eight_standard_deviations_of_headroom() {
        // m + 8√m = 65,536 at m ≈ 63,519.5, so that is where layer A's headroom runs out.
        assert!(fits_capacity(63_519.0, 65_536));
        assert!(!fits_capacity(63_520.0, 65_536));
        assert!(fits_capacity(0.0, 1));
        assert!(!fits_capacity(1.0, 1));
        assert!(!fits_capacity(f64::from(u32::MAX), 65_536));
    }

    /// The nuclear disc at its densest: the check refuses a galaxy whose layer-A cells could draw
    /// more candidates than an ID can number (plan 03, Design note 6).
    ///
    /// The parameters are the densest the builder allows at the centre — the heaviest galaxy, the
    /// nuclear disc's largest share in its smallest and flattest form, and Kroupa's function, which
    /// makes the most systems per solar mass. No seed draws them all together, which is what the
    /// sweep over 200 seeds below shows.
    #[test]
    fn headroom_fails_for_the_densest_centre_the_builder_allows() {
        let params = GalaxyParamsBuilder::new()
            .mass_function(MassFunctionKind::Kroupa)
            .stellar_mass(SolarMasses::new(1.0e11))
            .nuclear_disc_share(0.025)
            .nuclear_length(LightYears::new(200.0))
            .nuclear_height_ratio(0.3)
            .build()
            .expect("every value is inside its range");
        let galaxy = Galaxy::from_params(Seed::new(SEED | 2), params);
        let error = check_index_headroom(&galaxy).unwrap_err();
        let ExceedIndexCapacityError::LayerTooDense {
            layer,
            largest_mean,
            capacity,
        } = error;
        assert_eq!(layer, Layer::A, "{error}");
        assert_eq!(capacity, 65_536);
        assert!(largest_mean > 0.0, "{error}");
    }

    #[test]
    fn a_mean_that_is_not_finite_fits_nothing() {
        for mean in [f64::NAN, f64::INFINITY] {
            assert!(!fits_capacity(mean, 1 << 28), "{mean}");
        }
    }

    #[test]
    fn a_layer_over_its_capacity_is_named_with_its_mean() {
        let error = ExceedIndexCapacityError::LayerTooDense {
            layer: Layer::A,
            largest_mean: 64_000.0,
            capacity: 65_536,
        };
        assert_eq!(
            error.to_string(),
            "layer A's densest cell expects 64000 candidates, within eight standard deviations of \
             its index capacity of 65536"
        );
    }

    #[test]
    #[ignore = "slow: builds the fields of 200 galaxies"]
    fn headroom_holds_over_two_hundred_seeds() {
        for n in 0..200_u64 {
            let galaxy = Galaxy::new(Seed::new(SEED | n));
            assert_eq!(check_index_headroom(&galaxy), Ok(()), "seed {n}");
        }
    }
}
