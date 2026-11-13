//! The planet viewer's wire forms (rendering plan R15): a planet's inputs both ways, what was
//! drawn, the answers and the refusals.

use hyperion_protocol::{
    BodyIdHex, ErrorCode, PlanetInputsDto, RequestError, SeedHex, SystemIdHex, UniverseIdHex,
    ViewerDrawnDto, ViewerPlanetDto,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::planetary::viewer::{
    DrawnInputs, GenerateViewerError, InputField, InvalidInputError, PlanetInputs, RawPlanetInputs,
    VIEWER_PLACE_LY, ViewerSystem,
};

use super::galactic_position;

/// The inputs of `dto`, as plain values, not yet checked.
#[must_use]
pub(crate) fn raw_inputs(dto: &PlanetInputsDto) -> RawPlanetInputs {
    RawPlanetInputs {
        seed: dto.seed.to_u64(),
        star_mass_msun: dto.star_mass_msun,
        fe_h_dex: dto.fe_h_dex,
        age_gyr: dto.age_gyr,
        mass_mearth: dto.mass_mearth,
        semi_major_axis_au: dto.semi_major_axis_au,
        eccentricity: dto.eccentricity,
        formed_at_au: dto.formed_at_au,
        day_hours: dto.day_hours,
        tilt_deg: dto.tilt_deg,
        water_rank: dto.water_rank,
        carbon_rank: dto.carbon_rank,
        nitrogen_rank: dto.nitrogen_rank,
    }
}

/// `raw` as the wire carries it.
#[must_use]
pub(crate) fn inputs_dto(raw: &RawPlanetInputs) -> PlanetInputsDto {
    PlanetInputsDto {
        seed: SeedHex::from_u64(raw.seed),
        star_mass_msun: raw.star_mass_msun,
        fe_h_dex: raw.fe_h_dex,
        age_gyr: raw.age_gyr,
        mass_mearth: raw.mass_mearth,
        semi_major_axis_au: raw.semi_major_axis_au,
        eccentricity: raw.eccentricity,
        formed_at_au: raw.formed_at_au,
        day_hours: raw.day_hours,
        tilt_deg: raw.tilt_deg,
        water_rank: raw.water_rank,
        carbon_rank: raw.carbon_rank,
        nitrogen_rank: raw.nitrogen_rank,
    }
}

/// The request's field an input is: `inputs.` and its own name.
#[must_use]
const fn input_field(field: InputField) -> &'static str {
    match field {
        InputField::Seed => "inputs.seed",
        InputField::StarMass => "inputs.star_mass_msun",
        InputField::FeH => "inputs.fe_h_dex",
        InputField::Age => "inputs.age_gyr",
        InputField::Mass => "inputs.mass_mearth",
        InputField::SemiMajorAxis => "inputs.semi_major_axis_au",
        InputField::Eccentricity => "inputs.eccentricity",
        InputField::FormedAt => "inputs.formed_at_au",
        InputField::Day => "inputs.day_hours",
        InputField::Tilt => "inputs.tilt_deg",
        InputField::Water => "inputs.water_rank",
        InputField::Carbon => "inputs.carbon_rank",
        InputField::Nitrogen => "inputs.nitrogen_rank",
    }
}

/// The inputs of `dto`, checked.
///
/// # Errors
///
/// `bad_request` naming the first input outside its range, `inputs.` and its name.
pub(crate) fn planet_inputs(dto: &PlanetInputsDto) -> Result<PlanetInputs, RequestError> {
    PlanetInputs::try_from(raw_inputs(dto)).map_err(|error: InvalidInputError| RequestError {
        code: ErrorCode::BadRequest,
        message: error.to_string(),
        field: Some(input_field(error.field()).to_owned()),
    })
}

/// Why inputs within their ranges make no system, as the wire says it: `bad_request` naming the
/// input it is of.
#[must_use]
pub(crate) fn generate_refusal(error: &GenerateViewerError) -> RequestError {
    RequestError {
        code: ErrorCode::BadRequest,
        message: error.to_string(),
        field: Some(input_field(error.field()).to_owned()),
    }
}

/// What was drawn, as the wire carries it.
#[must_use]
fn drawn_dto(drawn: &DrawnInputs) -> ViewerDrawnDto {
    ViewerDrawnDto {
        formed_at_au: drawn.formed_at_au,
        day_hours: drawn.day_hours,
        tilt_deg: drawn.tilt_deg,
        water_rank: drawn.water_rank,
        carbon_rank: drawn.carbon_rank,
        nitrogen_rank: drawn.nitrogen_rank,
    }
}

/// The answer to `viewer_planet` for `system`, generated in `universe`.
///
/// # Panics
///
/// Never: the viewer's place lies inside the root cube.
#[must_use]
pub(crate) fn viewer_planet(universe: UniverseIdHex, system: &ViewerSystem) -> ViewerPlanetDto {
    let place = GalacticPosition::from_light_years(VIEWER_PLACE_LY)
        .expect("the Sun-like point lies inside the root cube");
    let planet = system.planet();
    ViewerPlanetDto {
        universe,
        system: SystemIdHex::from_u64(system.id().raw()),
        body: BodyIdHex::from_parts(planet.system().raw(), planet.body_index()),
        barycentre: galactic_position(&place),
        inputs: inputs_dto(system.inputs().raw()),
        drawn: drawn_dto(&system.drawn()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earth() -> PlanetInputsDto {
        PlanetInputsDto {
            seed: SeedHex::from_u64(7),
            star_mass_msun: 1.0,
            fe_h_dex: -0.0,
            age_gyr: 4.57,
            mass_mearth: 1.0,
            semi_major_axis_au: 1.0,
            eccentricity: 0.0167,
            formed_at_au: None,
            day_hours: Some(24.0),
            tilt_deg: None,
            water_rank: Some(0.5),
            carbon_rank: None,
            nitrogen_rank: None,
        }
    }

    #[test]
    fn inputs_cross_the_wire_unchanged_but_for_a_negative_zero() {
        let inputs = planet_inputs(&earth()).expect("inputs within their ranges");
        let back = inputs_dto(inputs.raw());
        assert_eq!(back.fe_h_dex.to_bits(), 0.0_f64.to_bits());
        assert_eq!(
            PlanetInputsDto {
                fe_h_dex: 0.0,
                ..earth()
            },
            back
        );
    }

    #[test]
    fn an_input_out_of_range_is_a_bad_request_naming_it() {
        let error = planet_inputs(&PlanetInputsDto {
            tilt_deg: Some(200.0),
            ..earth()
        })
        .expect_err("out of range");
        assert_eq!(error.code, ErrorCode::BadRequest);
        assert_eq!(error.field.as_deref(), Some("inputs.tilt_deg"));
    }

    #[test]
    fn an_orbit_its_system_cannot_hold_is_a_bad_request_naming_the_axis() {
        let inputs = planet_inputs(&PlanetInputsDto {
            semi_major_axis_au: 0.002,
            ..earth()
        })
        .expect("within its range");
        let error = ViewerSystem::generate(inputs).expect_err("inside the star");
        let refusal = generate_refusal(&error);
        assert_eq!(refusal.code, ErrorCode::BadRequest);
        assert_eq!(refusal.field.as_deref(), Some("inputs.semi_major_axis_au"));
    }
}
