/**
 * The physical constants the client shares with the simulation, each its one copy, with no imports
 * of its own so that a worker can take one without the wire's adapters.
 */

/**
 * The Newtonian constant of gravitation, m³ kg⁻¹ s⁻²: the simulation's
 * `hyperion_base::units::consts::GRAVITATIONAL_CONSTANT`, 6.674 30 × 10⁻¹¹ (CODATA 2018, unchanged
 * in CODATA 2022), by which the wire's kilograms and μ become masses and GM.
 */
export const GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 = 6.674_3e-11;
