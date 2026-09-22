//! The layer table: which cell size and which band of primary initial mass each stellar layer owns.

use crate::galaxy::imf::{MASS_LIMIT_HI, MASS_LIMIT_LO, MassBand};
use crate::id::Layer;
use crate::units::SolarMasses;

/// One row of the layer table: a stellar layer, its generation cell edge and its mass band.
///
/// The rows are [`STELLAR_LAYERS`]; nothing else builds one. The cell edge is the layer's
/// [`Layer::cell_size_ly`] and the band plan 02's [`MassBand`], so the table restates no figure
/// (brainstorm, "Sizing the layers").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerSpec {
    layer: Layer,
    cell_ly: u32,
    band: MassBand,
}

impl LayerSpec {
    /// The row of the layer that owns `band`.
    #[must_use]
    const fn of(band: MassBand) -> Self {
        let layer = band.layer();
        Self {
            layer,
            cell_ly: layer.cell_size_ly(),
            band,
        }
    }

    /// The layer.
    #[must_use]
    pub const fn layer(&self) -> Layer {
        self.layer
    }

    /// The generation cell's edge, light-years: 8 for layer A to 128 for layer E.
    #[must_use]
    pub const fn cell_ly(&self) -> u32 {
        self.cell_ly
    }

    /// The band of primary initial mass the layer owns.
    #[must_use]
    pub const fn band(&self) -> MassBand {
        self.band
    }
}

/// The five stellar layers, coarsest (E, 128 ly) to finest (A, 8 ly): the order the range query
/// walks them in (brainstorm, "The range query").
///
/// | Layer | Cell   | Primary initial mass |
/// | ----- | ------ | -------------------- |
/// | E     | 128 ly | 8–150 M☉             |
/// | D     | 64 ly  | 2.5–8 M☉             |
/// | C     | 32 ly  | 0.75–2.5 M☉          |
/// | B     | 16 ly  | 0.5–0.75 M☉          |
/// | A     | 8 ly   | 0.08–0.5 M☉          |
///
/// The substellar layers are not here: plan 13 places them.
pub const STELLAR_LAYERS: [LayerSpec; 5] = [
    LayerSpec::of(MassBand::E),
    LayerSpec::of(MassBand::D),
    LayerSpec::of(MassBand::C),
    LayerSpec::of(MassBand::B),
    LayerSpec::of(MassBand::A),
];

/// [`STELLAR_LAYERS`] at a fixed address, so that [`layer_spec`] can lend a row for `'static`.
static STELLAR_LAYER_TABLE: [LayerSpec; 5] = STELLAR_LAYERS;

/// The row of a stellar layer, or `None` for the brown-dwarf and rogue-planet layers.
#[must_use]
pub fn layer_spec(layer: Layer) -> Option<&'static LayerSpec> {
    STELLAR_LAYER_TABLE.iter().find(|spec| spec.layer == layer)
}

/// The stellar layer whose band holds a primary initial mass, or `None` outside 0.08–150 M☉.
///
/// Each band is closed below and open above, `[lo, hi)`, except layer E's, which keeps the upper
/// mass limit itself, `[8, 150]` M☉. A NaN mass has no layer.
///
/// This names the layer a mass would be placed in. It does not name a placed system's layer,
/// which is always its ID's: plan 02's `MassFunction::sample_in_band` draws a primary from the
/// closed band, so a layer-B primary can be exactly 0.75 M☉, where this function answers C.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::placement::layer_for_initial_mass;
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::units::SolarMasses;
///
/// // The Sun's primary is a layer-C system; 0.5 M☉ opens layer B.
/// assert_eq!(layer_for_initial_mass(SolarMasses::new(1.0)), Some(Layer::C));
/// assert_eq!(layer_for_initial_mass(SolarMasses::new(0.5)), Some(Layer::B));
/// assert_eq!(layer_for_initial_mass(SolarMasses::new(0.07)), None);
/// ```
#[must_use]
pub fn layer_for_initial_mass(mass: SolarMasses) -> Option<Layer> {
    let m = mass.value();
    if !(MASS_LIMIT_LO..=MASS_LIMIT_HI).contains(&m) {
        return None;
    }
    // The heaviest band whose lower edge the mass reaches.
    MassBand::ALL
        .into_iter()
        .rev()
        .find(|band| band.lo() <= m)
        .map(MassBand::layer)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::imf::MASS_BAND_EDGES;

    #[test]
    fn the_table_runs_from_e_to_a_with_the_brainstorms_cells_and_bands() {
        let expected = [
            (Layer::E, 128, 8.0, 150.0),
            (Layer::D, 64, 2.5, 8.0),
            (Layer::C, 32, 0.75, 2.5),
            (Layer::B, 16, 0.5, 0.75),
            (Layer::A, 8, 0.08, 0.5),
        ];
        for (spec, (layer, cell_ly, lo, hi)) in STELLAR_LAYERS.iter().zip(expected) {
            assert_eq!(spec.layer(), layer);
            assert_eq!(spec.cell_ly(), cell_ly, "{layer:?}");
            assert_eq!(spec.cell_ly(), layer.cell_size_ly(), "{layer:?}");
            assert_eq!(spec.band(), MassBand::try_from(layer).unwrap());
            assert_same_bits(spec.band().lo(), lo);
            assert_same_bits(spec.band().hi(), hi);
        }
    }

    #[test]
    fn layer_spec_finds_each_stellar_layer_and_no_substellar_one() {
        for spec in &STELLAR_LAYERS {
            assert_eq!(layer_spec(spec.layer()), Some(spec));
        }
        assert_eq!(layer_spec(Layer::BrownDwarf), None);
        assert_eq!(layer_spec(Layer::RoguePlanet), None);
    }

    #[test]
    fn layer_for_initial_mass_takes_each_lower_edge_into_its_own_band() {
        for band in MassBand::ALL {
            let layer = band.layer();
            assert_eq!(
                layer_for_initial_mass(SolarMasses::new(band.lo())),
                Some(layer),
                "lower edge of {layer:?}"
            );
            let just_below_hi = band.hi() - band.hi() * f64::EPSILON;
            assert_eq!(
                layer_for_initial_mass(SolarMasses::new(just_below_hi)),
                Some(layer),
                "just below the upper edge of {layer:?}"
            );
            let middle = f64::midpoint(band.lo(), band.hi());
            assert_eq!(
                layer_for_initial_mass(SolarMasses::new(middle)),
                Some(layer)
            );
        }
        // The inner edges belong to the band above.
        for (edge, layer) in
            MASS_BAND_EDGES[1..5]
                .iter()
                .zip([Layer::B, Layer::C, Layer::D, Layer::E])
        {
            assert_eq!(layer_for_initial_mass(SolarMasses::new(*edge)), Some(layer));
        }
    }

    #[test]
    fn layer_for_initial_mass_keeps_the_upper_limit_and_nothing_outside() {
        assert_eq!(
            layer_for_initial_mass(SolarMasses::new(MASS_LIMIT_HI)),
            Some(Layer::E)
        );
        let above = MASS_LIMIT_HI + MASS_LIMIT_HI * f64::EPSILON;
        assert_eq!(layer_for_initial_mass(SolarMasses::new(above)), None);
        let below = MASS_LIMIT_LO - MASS_LIMIT_LO * f64::EPSILON;
        assert_eq!(layer_for_initial_mass(SolarMasses::new(below)), None);
        for outside in [
            0.0,
            -1.0,
            1_000.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            assert_eq!(
                layer_for_initial_mass(SolarMasses::new(outside)),
                None,
                "{outside}"
            );
        }
    }
}
