//! Galaxy messages: the drawn parameters, the density map and the systems within range of a point.
//!
//! These are wire types only. The server converts the simulation's types to and from them and
//! validates every field it receives; nothing here depends on the simulation crate. Quantities
//! carry their unit in the field name or in a [`Unit`], and every unit is one the bridge client
//! can display without converting.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::primitives::{GalacticPosition, SeedHex, SystemIdHex, UniverseIdHex, UniverseTime};

/// Asks for a universe's galaxy parameters (`galaxy_parameters`), answered with
/// [`GalaxyParameters`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GalaxyParametersRequest {
    /// The universe whose galaxy is described.
    pub universe: UniverseIdHex,
}

/// The seed and the structural parameters of a universe's galaxy, as a grouped list.
///
/// The list mirrors no simulation struct: parameters are added as the model grows, and the client
/// shows them as a table with labels from its own glossary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GalaxyParameters {
    /// The universe described.
    pub universe: UniverseIdHex,
    /// Its seed.
    pub seed: SeedHex,
    /// The generator version the parameters were drawn under.
    pub generator_version: u32,
    /// The parameters, in the order the server lists them.
    pub groups: Vec<ParameterGroup>,
}

/// A named group of parameters, such as `mass` or `discs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ParameterGroup {
    /// The group's key, `snake_case`, never renamed once shipped.
    pub key: String,
    /// The group's parameters, in display order.
    pub parameters: Vec<Parameter>,
}

/// One galaxy parameter: its key, where its value came from, and the value with its unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Parameter {
    /// Dotted `snake_case` key, such as `disc.thin.scale_length`: unique within a response and
    /// never renamed once shipped.
    pub key: String,
    /// Whether the value was drawn from the seed, derived from drawn values, or fixed.
    pub origin: ParameterOrigin,
    /// The value.
    pub value: ParameterValue,
}

/// Where a parameter's value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ParameterOrigin {
    /// Drawn from the seed.
    Drawn,
    /// Computed from drawn values, such as the system count or the bar's pattern speed.
    Derived,
    /// Fixed by the generator version, such as the mass function.
    Fixed,
}

/// A parameter's value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ParameterValue {
    /// A number in the stated unit, already converted for display.
    Number {
        /// The value in `unit`.
        value: f64,
        /// The unit of `value`.
        unit: Unit,
    },
    /// A value that is a name, such as the mass function's.
    Text {
        /// The text.
        value: String,
    },
}

/// The unit of a numeric parameter.
///
/// The wire carries only units that the bridge client displays, so it formats and never converts:
/// lengths in light-years, masses in solar masses, times in megayears or gigayears, speeds in
/// kilometres per second, angles in degrees and densities per cubic light-year. The kiloparsec is
/// never sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Unit {
    /// Dimensionless, such as a share or a concentration.
    None,
    /// A number of objects, such as the system count.
    Count,
    /// Solar masses, M☉.
    Msun,
    /// Light-years, ly.
    Ly,
    /// Megayears, Myr: 10⁶ Julian years.
    Myr,
    /// Gigayears, Gyr: 10⁹ Julian years.
    Gyr,
    /// Kilometres per second, km/s.
    KmPerS,
    /// Degrees per megayear, °/Myr, for the bar's pattern speed: 38 km/s per kpc is 2.23 °/Myr.
    DegPerMyr,
    /// Degrees of arc, °.
    Deg,
    /// Per cubic light-year, ly⁻³, for number densities.
    PerLy3,
}

/// The direction from which a density map is seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MapView {
    /// Seen from galactic north (+z), with +x to the right and +y up, so that the galaxy rotates
    /// counter-clockwise on screen.
    FaceOn,
    /// Looking along +y, with +x to the right and +z (galactic north) up.
    EdgeOn,
}

/// Which systems a density map counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MapPopulation {
    /// Every population.
    All,
    /// The young thin disc only, which is where the spiral arms show.
    Young,
}

/// Asks for a column-density map of a universe's galaxy (`density_map`), answered with a
/// [`DensityMap`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DensityMapRequest {
    /// The universe whose galaxy is mapped.
    pub universe: UniverseIdHex,
    /// The direction the map is seen from.
    pub view: MapView,
    /// Which systems are counted.
    pub population: MapPopulation,
    /// The map's width in pixels: 128, 256, 512 or 1,024.
    pub resolution: u16,
    /// Bits per pixel code: 8 or 16.
    pub bits: u8,
}

/// A column-density map: systems per square light-year along each line of sight, computed from the
/// galaxy's fields alone, as quantised logarithms.
///
/// **Encoding.** `data_base64` is the standard base64 alphabet with padding. Decoded, it holds
/// `width_px × height_px` codes of `bits` bits each: one byte per code at 8 bits, two bytes
/// little-endian at 16 bits. Rows run top to bottom and pixels left to right within a row, so the
/// code of column `i` and row `j` is the `(j × width_px + i)`-th.
///
/// **Orientation.** Face-on, the map is seen from galactic north with +x to the right and +y up,
/// so the galaxy's rotation is counter-clockwise on screen. Edge-on, it looks along +y with +x to
/// the right and +z up. Axes are those of the `GALACTIC` frame (see [`GalacticPosition`]).
///
/// **Geometry.** Pixels are squares `ly_per_px` light-years across, and `centre_ly` is the point at
/// the centre of the raster as (horizontal, vertical): (x, y) face-on, (x, z) edge-on. The centre
/// of the pixel in column `i` and row `j` therefore lies at
///
/// ```text
/// horizontal_ly = centre_ly[0] + (i + 0.5 − width_px ÷ 2) × ly_per_px
/// vertical_ly   = centre_ly[1] + (height_px ÷ 2 − j − 0.5) × ly_per_px
/// ```
///
/// In the first milestone the extent is fixed: face-on a square 131,072 ly across centred on the
/// galactic centre, edge-on 131,072 ly wide and 65,536 ly tall, with `height_px` half of
/// `width_px`.
///
/// **Codes.** With `max = 2^bits − 1`, code 0 means "at or below the floor, or empty", and codes 1
/// to `max` span `floor_log10_per_ly2` to `ceiling_log10_per_ly2` linearly in log₁₀ of systems per
/// square light-year:
///
/// ```text
/// log10_per_ly2 = floor + (code − 1) × (ceiling − floor) ÷ (max − 1)
/// ```
///
/// so code 1 is the floor and code `max` the ceiling. The ceiling is the largest value in the map
/// and the floor lies 5 dex below it face-on and 7 dex below it edge-on. A map with no systems at
/// all has floor and ceiling 0 and every code 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DensityMap {
    /// The universe mapped.
    pub universe: UniverseIdHex,
    /// The direction the map is seen from.
    pub view: MapView,
    /// Which systems are counted.
    pub population: MapPopulation,
    /// Pixels per row.
    pub width_px: u16,
    /// Rows.
    pub height_px: u16,
    /// The centre of the raster in light-years: (x, y) face-on, (x, z) edge-on.
    pub centre_ly: [f64; 2],
    /// The width and height of one pixel in light-years.
    pub ly_per_px: f64,
    /// Bits per code: 8 or 16.
    pub bits: u8,
    /// log₁₀ of the column density, in systems per square light-year, that code 1 stands for.
    pub floor_log10_per_ly2: f64,
    /// log₁₀ of the column density, in systems per square light-year, that code `2^bits − 1`
    /// stands for.
    pub ceiling_log10_per_ly2: f64,
    /// The codes, base64-encoded with the standard alphabet and padding.
    pub data_base64: String,
}

/// A mass layer of the placement grid: a band of primary initial mass with its own cell size.
///
/// The bands, from the brainstorm's "Sizing the layers", are A 0.08–0.5 M☉ (8 ly cells), B
/// 0.5–0.75 M☉ (16 ly), C 0.75–2.5 M☉ (32 ly), D 2.5–8 M☉ (64 ly) and E 8–150 M☉ (128 ly). The
/// census carries the exact edges with each answer. The bands are of initial mass, so a coarse
/// layer's system may by now be a remnant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MassLayer {
    /// 0.08–0.5 M☉.
    A,
    /// 0.5–0.75 M☉.
    B,
    /// 0.75–2.5 M☉.
    C,
    /// 2.5–8 M☉.
    D,
    /// 8–150 M☉.
    E,
}

/// The stellar population a system was placed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Population {
    /// The young thin disc, under about 100 Myr old and bound to the spiral arms.
    YoungThinDisc,
    /// The old thin disc, 0.1–10 Gyr.
    OldThinDisc,
    /// The thick disc, 10–12 Gyr.
    ThickDisc,
    /// The boxy bulge, 8–12 Gyr.
    Bulge,
    /// The long bar, 6–10 Gyr.
    LongBar,
    /// The nuclear disc at the galactic centre, mostly over 8 Gyr.
    NuclearDisc,
    /// The stellar halo, 10–13 Gyr.
    Halo,
}

/// Asks for the systems within `radius_ly` of `centre` at `time` (`systems_in_range`), answered
/// with [`SystemsInRange`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemsInRangeRequest {
    /// The universe to search.
    pub universe: UniverseIdHex,
    /// The centre of the sphere, in the `GALACTIC` frame, inside the root cube.
    pub centre: GalacticPosition,
    /// The sphere's radius in light-years: finite, above zero and at most 131,072.
    pub radius_ly: f64,
    /// The instant at which positions are tested and ages given, within 1,000 years of the epoch.
    pub time: UniverseTime,
    /// The lightest layer wanted: `a` asks for every layer, `c` for layers C, D and E only.
    pub min_layer: MassLayer,
    /// The most systems the census may expect to return, from 1 to 20,000.
    pub limit: u32,
}

/// The systems within range of a point at a time, with the census that says what was left out.
///
/// `centre`, `radius_ly` and `time` echo the request. The systems come in the server's order and
/// clients sort for themselves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemsInRange {
    /// The universe searched.
    pub universe: UniverseIdHex,
    /// The centre of the sphere, in the `GALACTIC` frame.
    pub centre: GalacticPosition,
    /// The sphere's radius in light-years.
    pub radius_ly: f64,
    /// The instant at which positions were tested and ages given.
    pub time: UniverseTime,
    /// Which layers are complete, and why the others were left out.
    pub census: Census,
    /// Every system of the included layers inside the sphere at `time`.
    pub systems: Vec<SystemRecord>,
}

/// What a range query returned, layer by layer.
///
/// Each layer is returned whole or not at all, decided from its expected count before anything is
/// generated. Finding that nothing fits is a valid answer with no systems, not an error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Census {
    /// The request's limit on the expected number of systems.
    pub limit: u32,
    /// The lower mass edge, in M☉ of primary initial mass, above which the result is complete; the
    /// lower edge of the lightest included layer. `null` when no layer fits.
    pub complete_above_msun: Option<f64>,
    /// All five layers, A to E.
    pub layers: Vec<LayerCensus>,
}

/// One layer's line in the census.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LayerCensus {
    /// The layer.
    pub layer: MassLayer,
    /// The lower edge of the layer's band of primary initial mass, in M☉.
    pub mass_min_msun: f64,
    /// The upper edge of the layer's band of primary initial mass, in M☉.
    pub mass_max_msun: f64,
    /// The expected number of the layer's systems in the sphere.
    pub expected: f64,
    /// The number of the layer's systems returned: 0 unless the layer is included.
    pub returned: u32,
    /// Whether the layer is included, and why not otherwise.
    pub status: LayerStatus,
}

/// Whether a layer is in a range query's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LayerStatus {
    /// Every system of the layer in the sphere is returned.
    Included,
    /// Left out because the expected count would have passed the limit.
    OverLimit,
    /// Left out because it is lighter than the requested `min_layer`.
    BelowMassFloor,
    /// Left out because the query would have visited too many cells.
    OverCellBudget,
}

/// One star system found by a range query, as it is at the query's time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemRecord {
    /// The system's ID.
    pub id: SystemIdHex,
    /// Its catalogue designation, derived from the ID.
    pub designation: String,
    /// Its position at the query's time, in the `GALACTIC` frame.
    pub position: GalacticPosition,
    /// The layer it was placed in.
    pub layer: MassLayer,
    /// The initial mass of its primary star, in M☉.
    pub initial_mass_msun: f64,
    /// Its age at the query's time, in megayears.
    pub age_myr: f64,
    /// The population it was placed from.
    pub population: Population,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::envelope::{RequestBody, ResponseBody};
    use crate::testing::{assert_wire_form, assert_wire_strings};

    const UNIVERSE: u64 = 0x0123_4567_89ab_cdef;

    fn universe() -> UniverseIdHex {
        UniverseIdHex::from_u64(UNIVERSE)
    }

    #[test]
    fn galaxy_parameters_request_wire_form() {
        assert_wire_form(
            &RequestBody::GalaxyParameters(GalaxyParametersRequest {
                universe: universe(),
            }),
            json!({ "kind": "galaxy_parameters", "universe": "0123456789abcdef" }),
        );
    }

    #[test]
    fn galaxy_parameters_response_wire_form() {
        assert_wire_form(
            &ResponseBody::GalaxyParameters(GalaxyParameters {
                universe: universe(),
                seed: SeedHex::from_u64(1234),
                generator_version: 2,
                groups: vec![
                    ParameterGroup {
                        key: "identity".to_owned(),
                        parameters: vec![Parameter {
                            key: "identity.mass_function".to_owned(),
                            origin: ParameterOrigin::Fixed,
                            value: ParameterValue::Text {
                                value: "Kroupa (2001)".to_owned(),
                            },
                        }],
                    },
                    ParameterGroup {
                        key: "discs".to_owned(),
                        parameters: vec![Parameter {
                            key: "disc.thin.scale_length".to_owned(),
                            origin: ParameterOrigin::Drawn,
                            value: ParameterValue::Number {
                                value: 8_500.0,
                                unit: Unit::Ly,
                            },
                        }],
                    },
                ],
            }),
            json!({
                "kind": "galaxy_parameters",
                "universe": "0123456789abcdef",
                "seed": "00000000000004d2",
                "generator_version": 2,
                "groups": [
                    {
                        "key": "identity",
                        "parameters": [{
                            "key": "identity.mass_function",
                            "origin": "fixed",
                            "value": { "type": "text", "value": "Kroupa (2001)" },
                        }],
                    },
                    {
                        "key": "discs",
                        "parameters": [{
                            "key": "disc.thin.scale_length",
                            "origin": "drawn",
                            "value": { "type": "number", "value": 8_500.0, "unit": "ly" },
                        }],
                    },
                ],
            }),
        );
    }

    #[test]
    fn unit_strings() {
        assert_wire_strings(&[
            (Unit::None, "none"),
            (Unit::Count, "count"),
            (Unit::Msun, "msun"),
            (Unit::Ly, "ly"),
            (Unit::Myr, "myr"),
            (Unit::Gyr, "gyr"),
            (Unit::KmPerS, "km_per_s"),
            (Unit::DegPerMyr, "deg_per_myr"),
            (Unit::Deg, "deg"),
            (Unit::PerLy3, "per_ly3"),
        ]);
    }

    #[test]
    fn parameter_origin_strings() {
        assert_wire_strings(&[
            (ParameterOrigin::Drawn, "drawn"),
            (ParameterOrigin::Derived, "derived"),
            (ParameterOrigin::Fixed, "fixed"),
        ]);
    }

    #[test]
    fn density_map_request_wire_form() {
        assert_wire_form(
            &RequestBody::DensityMap(DensityMapRequest {
                universe: universe(),
                view: MapView::EdgeOn,
                population: MapPopulation::Young,
                resolution: 512,
                bits: 16,
            }),
            json!({
                "kind": "density_map",
                "universe": "0123456789abcdef",
                "view": "edge_on",
                "population": "young",
                "resolution": 512,
                "bits": 16,
            }),
        );
    }

    #[test]
    fn density_map_response_wire_form() {
        assert_wire_form(
            &ResponseBody::DensityMap(DensityMap {
                universe: universe(),
                view: MapView::FaceOn,
                population: MapPopulation::All,
                width_px: 4,
                height_px: 2,
                centre_ly: [0.0, -0.5],
                ly_per_px: 32_768.0,
                bits: 8,
                floor_log10_per_ly2: -3.25,
                ceiling_log10_per_ly2: 1.75,
                data_base64: "AAH/gAECAwQ=".to_owned(),
            }),
            json!({
                "kind": "density_map",
                "universe": "0123456789abcdef",
                "view": "face_on",
                "population": "all",
                "width_px": 4,
                "height_px": 2,
                "centre_ly": [0.0, -0.5],
                "ly_per_px": 32_768.0,
                "bits": 8,
                "floor_log10_per_ly2": -3.25,
                "ceiling_log10_per_ly2": 1.75,
                "data_base64": "AAH/gAECAwQ=",
            }),
        );
    }

    #[test]
    fn map_view_strings() {
        assert_wire_strings(&[(MapView::FaceOn, "face_on"), (MapView::EdgeOn, "edge_on")]);
    }

    #[test]
    fn map_population_strings() {
        assert_wire_strings(&[(MapPopulation::All, "all"), (MapPopulation::Young, "young")]);
    }

    fn chart_centre() -> GalacticPosition {
        GalacticPosition {
            cell_ly: [26_000, 0, -1],
            offset_m: [0.0, 0.0, 4_730_365_236_290_400.0],
        }
    }

    fn epoch_plus_a_century() -> UniverseTime {
        UniverseTime {
            seconds: 3_155_760_000,
            nanos: 0,
        }
    }

    #[test]
    fn systems_in_range_request_wire_form() {
        assert_wire_form(
            &RequestBody::SystemsInRange(SystemsInRangeRequest {
                universe: universe(),
                centre: chart_centre(),
                radius_ly: 50.0,
                time: epoch_plus_a_century(),
                min_layer: MassLayer::A,
                limit: 5_000,
            }),
            json!({
                "kind": "systems_in_range",
                "universe": "0123456789abcdef",
                "centre": {
                    "cell_ly": [26_000, 0, -1],
                    "offset_m": [0.0, 0.0, 4_730_365_236_290_400.0],
                },
                "radius_ly": 50.0,
                "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
                "min_layer": "a",
                "limit": 5_000,
            }),
        );
    }

    #[test]
    fn systems_in_range_request_accepts_whole_numbers_for_floats() {
        // JavaScript writes 50.0 as `50`, so every float field must accept an integer literal.
        let text = r#"{"kind":"systems_in_range","universe":"0123456789abcdef",
            "centre":{"cell_ly":[26000,0,-1],"offset_m":[0,0,4730365236290400]},
            "radius_ly":50,"time":{"seconds":3155760000,"nanos":0},"min_layer":"a","limit":5000}"#;
        let body: RequestBody = serde_json::from_str(text).unwrap();
        assert_eq!(
            body,
            RequestBody::SystemsInRange(SystemsInRangeRequest {
                universe: universe(),
                centre: chart_centre(),
                radius_ly: 50.0,
                time: epoch_plus_a_century(),
                min_layer: MassLayer::A,
                limit: 5_000,
            })
        );
    }

    fn census_line(
        layer: MassLayer,
        [mass_min_msun, mass_max_msun]: [f64; 2],
        expected: f64,
        returned: u32,
        status: LayerStatus,
    ) -> LayerCensus {
        LayerCensus {
            layer,
            mass_min_msun,
            mass_max_msun,
            expected,
            returned,
            status,
        }
    }

    fn census_line_json(
        layer: &str,
        [mass_min_msun, mass_max_msun]: [f64; 2],
        expected: f64,
        returned: u32,
        status: &str,
    ) -> serde_json::Value {
        json!({
            "layer": layer,
            "mass_min_msun": mass_min_msun,
            "mass_max_msun": mass_max_msun,
            "expected": expected,
            "returned": returned,
            "status": status,
        })
    }

    #[test]
    fn systems_in_range_response_wire_form() {
        let records = vec![
            SystemRecord {
                id: SystemIdHex::from_u64(0x0200_0800_2000_0000),
                designation: "Vorth AB-C e4-17".to_owned(),
                position: GalacticPosition {
                    cell_ly: [26_012, -3, -2],
                    offset_m: [1.5e15, 0.0, 9.0e15],
                },
                layer: MassLayer::E,
                initial_mass_msun: 11.25,
                age_myr: 7_250.5,
                population: Population::OldThinDisc,
            },
            SystemRecord {
                id: SystemIdHex::from_u64(0x6000_0000_0000_0001),
                designation: "Vorth AB-C b17-2".to_owned(),
                position: GalacticPosition {
                    cell_ly: [25_990, 20, 0],
                    offset_m: [0.0, 2.0e15, 3.0e15],
                },
                layer: MassLayer::B,
                initial_mass_msun: 0.625,
                age_myr: 45.0,
                population: Population::YoungThinDisc,
            },
        ];
        let census = Census {
            limit: 5_000,
            complete_above_msun: Some(0.5),
            layers: vec![
                census_line(
                    MassLayer::A,
                    [0.08, 0.5],
                    1_202.5,
                    0,
                    LayerStatus::OverLimit,
                ),
                census_line(MassLayer::B, [0.5, 0.75], 155.0, 1, LayerStatus::Included),
                census_line(MassLayer::C, [0.75, 2.5], 174.0, 0, LayerStatus::Included),
                census_line(MassLayer::D, [2.5, 8.0], 36.5, 0, LayerStatus::Included),
                census_line(MassLayer::E, [8.0, 150.0], 10.125, 1, LayerStatus::Included),
            ],
        };
        assert_wire_form(
            &ResponseBody::SystemsInRange(SystemsInRange {
                universe: universe(),
                centre: chart_centre(),
                radius_ly: 50.0,
                time: epoch_plus_a_century(),
                census,
                systems: records,
            }),
            json!({
                "kind": "systems_in_range",
                "universe": "0123456789abcdef",
                "centre": {
                    "cell_ly": [26_000, 0, -1],
                    "offset_m": [0.0, 0.0, 4_730_365_236_290_400.0],
                },
                "radius_ly": 50.0,
                "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
                "census": {
                    "limit": 5_000,
                    "complete_above_msun": 0.5,
                    "layers": [
                        census_line_json("a", [0.08, 0.5], 1_202.5, 0, "over_limit"),
                        census_line_json("b", [0.5, 0.75], 155.0, 1, "included"),
                        census_line_json("c", [0.75, 2.5], 174.0, 0, "included"),
                        census_line_json("d", [2.5, 8.0], 36.5, 0, "included"),
                        census_line_json("e", [8.0, 150.0], 10.125, 1, "included"),
                    ],
                },
                "systems": [
                    {
                        "id": "0200080020000000",
                        "designation": "Vorth AB-C e4-17",
                        "position": {
                            "cell_ly": [26_012, -3, -2],
                            "offset_m": [1.5e15, 0.0, 9.0e15],
                        },
                        "layer": "e",
                        "initial_mass_msun": 11.25,
                        "age_myr": 7_250.5,
                        "population": "old_thin_disc",
                    },
                    {
                        "id": "6000000000000001",
                        "designation": "Vorth AB-C b17-2",
                        "position": {
                            "cell_ly": [25_990, 20, 0],
                            "offset_m": [0.0, 2.0e15, 3.0e15],
                        },
                        "layer": "b",
                        "initial_mass_msun": 0.625,
                        "age_myr": 45.0,
                        "population": "young_thin_disc",
                    },
                ],
            }),
        );
    }

    #[test]
    fn systems_in_range_response_wire_form_when_nothing_fits() {
        let bands = [
            (MassLayer::A, "a", [0.08, 0.5]),
            (MassLayer::B, "b", [0.5, 0.75]),
            (MassLayer::C, "c", [0.75, 2.5]),
            (MassLayer::D, "d", [2.5, 8.0]),
            (MassLayer::E, "e", [8.0, 150.0]),
        ];
        let layers = bands
            .iter()
            .map(|&(layer, _, band)| census_line(layer, band, 40_000.0, 0, LayerStatus::OverLimit))
            .collect();
        let layers_json: Vec<_> = bands
            .iter()
            .map(|&(_, text, band)| census_line_json(text, band, 40_000.0, 0, "over_limit"))
            .collect();
        assert_wire_form(
            &ResponseBody::SystemsInRange(SystemsInRange {
                universe: universe(),
                centre: GalacticPosition::default(),
                radius_ly: 50.0,
                time: UniverseTime::default(),
                census: Census {
                    limit: 100,
                    complete_above_msun: None,
                    layers,
                },
                systems: Vec::new(),
            }),
            json!({
                "kind": "systems_in_range",
                "universe": "0123456789abcdef",
                "centre": { "cell_ly": [0, 0, 0], "offset_m": [0.0, 0.0, 0.0] },
                "radius_ly": 50.0,
                "time": { "seconds": 0, "nanos": 0 },
                "census": {
                    "limit": 100,
                    "complete_above_msun": null,
                    "layers": layers_json,
                },
                "systems": [],
            }),
        );
    }

    #[test]
    fn mass_layer_strings() {
        assert_wire_strings(&[
            (MassLayer::A, "a"),
            (MassLayer::B, "b"),
            (MassLayer::C, "c"),
            (MassLayer::D, "d"),
            (MassLayer::E, "e"),
        ]);
    }

    #[test]
    fn population_strings() {
        assert_wire_strings(&[
            (Population::YoungThinDisc, "young_thin_disc"),
            (Population::OldThinDisc, "old_thin_disc"),
            (Population::ThickDisc, "thick_disc"),
            (Population::Bulge, "bulge"),
            (Population::LongBar, "long_bar"),
            (Population::NuclearDisc, "nuclear_disc"),
            (Population::Halo, "halo"),
        ]);
    }

    #[test]
    fn layer_status_strings() {
        assert_wire_strings(&[
            (LayerStatus::Included, "included"),
            (LayerStatus::OverLimit, "over_limit"),
            (LayerStatus::BelowMassFloor, "below_mass_floor"),
            (LayerStatus::OverCellBudget, "over_cell_budget"),
        ]);
    }
}
