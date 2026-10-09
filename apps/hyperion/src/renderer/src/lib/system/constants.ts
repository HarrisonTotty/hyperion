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

/**
 * The atomic mass constant m_u, kg: the simulation's
 * `hyperion_sim::planetary::derive::atmosphere::ATOMIC_MASS_CONSTANT_KG`, 1.660 539 068 92 × 10⁻²⁷
 * (CODATA 2022), in which molecular masses are counted: a molecule of molar mass M g mol⁻¹ weighs
 * M m_u.
 */
export const ATOMIC_MASS_CONSTANT_KG = 1.660_539_068_92e-27;
