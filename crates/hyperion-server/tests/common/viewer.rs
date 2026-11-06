//! The planet viewer's planets over the wire (rendering plan R15), shared by the integration tests
//! that generate one: the viewer's own test and the sky's, which leaves a viewer system out.

use hyperion_protocol::{
    PlanetInputsDto, RequestBody, ResponseBody, SeedHex, UniverseIdHex, ViewerPlanetDto,
    ViewerPlanetRequest,
};

use super::TestClient;

/// An Earth about a Sun, its day and tilt given, everything else drawn.
pub fn earth() -> PlanetInputsDto {
    PlanetInputsDto {
        seed: SeedHex::from_u64(0x0e15_0002),
        star_mass_msun: 1.0,
        fe_h_dex: 0.0,
        age_gyr: 4.57,
        mass_mearth: 1.0,
        semi_major_axis_au: 1.0,
        eccentricity: 0.0167,
        formed_at_au: None,
        day_hours: Some(24.0),
        tilt_deg: Some(23.4),
        water_rank: None,
        carbon_rank: None,
        nitrogen_rank: None,
    }
}

/// The planet `client`'s server generates from `inputs` in `universe`, which it must serve.
pub async fn generated(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    inputs: PlanetInputsDto,
) -> ViewerPlanetDto {
    match client
        .request(RequestBody::ViewerPlanet(ViewerPlanetRequest {
            universe: universe.clone(),
            inputs,
        }))
        .await
    {
        Ok(ResponseBody::ViewerPlanet(planet)) => *planet,
        other => panic!("expected the planet, got {other:?}"),
    }
}
