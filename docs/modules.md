# Ship Module System Rework

The chief idea behind ship modules is the following:

1. A ship is comprised of multiple ship modules which are selected during ship creation.
2. Each entry defined in `data/module-slots/*.yaml` defines the various ship module _slots_. Users select one of these to add a module slot of that type to their ship.
3. Some module slots don't require any additional selection by the user. For instance, deflector plating modules simply add armor to the ship and have no variants.
4. Most module slots require further selection to determine the specific variant to install. When adding an "impulse engine" module slot for instance, the player must select which specific impulse engine to install from those available in `data/modules/impulse-engine/`.
5. Each module variant is defined in its own YAML file typically within an appropriate subdirectory of `data/modules/`, however the layout of these files underneath `data/modules/` is arbitrary. The `type` field within each module variant file determines which module slot it fits into.
6. Ammunition for torpedo tubes, missile launchers, and kinetic weapons is defined in `data/ammo/`.

## Module Slot Specification

Each `data/module-slots/*.yaml` file has the following specification:

```yaml
# (str) A unique identifier associated with the ship module slot. Module
# varients reference this identifier in the `type` key, as well as the Rust code.
id: power-core

# (str) The display name of the module slot.
name: "Power Core"
    
# (str) A brief description of the module slot.
desc: >-
  Provides the ship with power generation and capacity.

# (str) An extended description of the module slot.
extended_desc: >-
  Different power cores provide varying levels of power output and
  capacity. The power core is essential for running all ship systems,
  and its performance directly affects the ship's overall capabilities.
    
# (list[str]) A arbitrary list of "groups" associated with the modules slot. These groups are used for filtering in the user interface. And for certain ship class bonuses that target certain module groups.
groups: ["Essential", "Power", "Support"]

# (bool) Whether at least one module of this type is required on a ship.
required: true

# (bool) Whether this module slot has different varients.
has_varients: true
    
# (int) The base cost in build points to add this module slot to a ship.
base_cost: 10

# (int) The maximum number of slots of this type allowed on a ship.
max_slots: 2

# (int) The base hit points allocated to a module of this type.
base_hp: 10

# (float) The base power consumption of the module at 100% power, per second.
base_power_consumption: 0.0

# (float) The base heat generation of the module at 100% power, per second.
base_heat_generation: 5.0

# (int) The base weight of the module slot, in kg.
base_weight: 100
```


## Module Varient Specification

All relevant ship module variants have at minimum the following specification format. Additional fields are added depending on the `type` of module.

```yaml
# (str) A unique identifier associated with the ship module.
id: mk2-fusion-reactor

# (str) The kind of module slot this variant fits into. This must match one of
# the module types defined in `data/module-slots/*.yaml`.
type: power-core

# (str) The name of the module. This is the "title" of the module displayed to
# the user. This is usually the same as the model.
name: "Mark II Fusion Reactor"

# (str) The model of the module, for lore. This is displayed when viewing the
# extended details of the module.
model: "Xenon Mk2 Fusion Reactor"

# (str) The manufacturer of the module, for lore. This is displayed when
# viewing the extended details of the module.
manufacturer: "Xenon Dynamics"

# (str) A brief description of the module.
desc: >-
  A compact fusion reactor providing reliable power output for medium-sized ships.

# (str) An extended description of the module for lore purposes. This is
# displayed when viewing the extended details of the module.
lore: >-
  The Mark II Fusion Reactor by Xenon Dynamics is renowned for its efficiency
  and durability. It utilizes advanced fusion technology to deliver a steady
  power output, making it ideal for ships that require consistent energy
  levels during extended missions.

# (int) The cost of the module in build points.
cost: 15

# (int) The additional module hit points granted by this varient.
additional_hp: 20

# (float) The additional power consumption of the module at 100% power, per second.
additional_power_consumption: 0.0

# (float) The additonal heat generation of the module at 100% power, per second.
additional_heat_generation: 2.0

# (int) The additional weight of the module, in kg.
additional_weight: 20
```

While all modules underneath `data/modules/**/*.yaml` are loaded, it is common to organize modules by slot `type`.

## Module Slot Types

### Auxiliary Support System (`aux-support-system`)

Auxiliary support systems provide additional support functions for the ship, such as emergency power or shields. From a gameplay perspective they are akin to a "health or mana potion" from traditional RPGs. They have a limited number of uses until recharged at a station. `type: aux-support-system` modules have the following additional fields in their YAML spec:

```yaml
# (int) The amount of HP regained when "using" the module.
hp_regained: 50

# (int) The amount of energy restored when "using" the module.
energy_restored: 0

# (int) The amount of heat dissipated when "using" the module.
heat_dissipated: 0

# (int) The number of times the module can be "used".
num_uses: 10
```

### Cargo Bay (`cargo-bay`) - No Varients

The cargo bay module increases the ship's cargo capacity, allowing it to store more goods and resources. This is essential for long missions where resupply opportunities are scarce. Adding a cargo bay simply increases the maximum weight capacity.

### Communications System (`comms-system`)

The communications system is a required module. When added, players select the model of communication system they wish to add to their ship. The model of communication system selected affects:

1. The range at which the ship can communicate with other ships and stations.
2. The types of communications that can be sent and received (e.g., standard, encrypted, distress signals).

`type: comm-system` modules have the following additional fields in their YAML spec:

```yaml
# (int) The effective range of the communications system at 100% power, in meters.
comm_range: 1000000

# (int) The level of encryption offered by the communications system, 1-10.
encryption_lvl: 4
```

### Cooling System (`cooling-system`)

Adding a cooling system to the ship helps mitigate heat generation by other modules. Each cooling system generates and stores a certain amount of "coolant" that can be allocated to other ship systems to help reduce their heat levels, similar to allocating power. `type: cooling-system` modules add the following additional fields to their YAML spec:

```yaml
# (int) The maximum amount of "coolant" (cooling points) capable of being
# allocated to other systems when at 100% power.
maximum_coolant: 1000

# (int) The amount of coolant (cooling points) generated per second when at
# 100% power.
generated_cooling: 50
```

### Countermeasure System (`countermeasure-system`) - NOT YET IMPLEMENTED

Countermeasure systems provide the ship with defensive countermeasures against incoming missiles. Players select the specific countermeasure to install after adding the module. The countermeasure's effectiveness and characteristics depend on the model selected. Includes chaff, flares, radar jammers, and point defense systems.

The additional YAML fields associated with `type: countermeasure-system` modules have not yet been defined.

### Deflector Plating (`deflector-plating`) - No Varients

Each deflector plating module increases the ship's overall armor rating. Each module reduces incoming damage by a certain percentage, defined in `data/game.yaml`.

### Directed Energy Weapon Port (`de-weapon`)

Directed energy weapon ports allow the ship to mount energy-based weapons, such as lasers or particle beams. Players select the specific weapon to mount when adding this module. `type: de-weapon` modules have the following additional fields in their specification:

```yaml
# (int) The damage done by the weapon when at 100% power.
damage: 5

# (float) The number of seconds the weapon takes to recharge when at 100%
# power.
recharge_time: 5.0

# (int) The maximum range of the weapon, in meters.
max_range: 100000

# (int) The speed of the projectile or beam, in m/s.
projectile_speed: 10000

# (list[str]) The weapon tags associated with the weapon.
weapon_tags: ["Beam", "Photon"]
```

### Impulse Engines (`impulse-engine`)

The impulse engines module determines the thrust characteristics of the ship. Players select the specific impulse engine to install after adding the module to the ship. `type: impulse-engine` modules have the following additional fields in their specification:

```yaml
# (int) The maximum thrust output of the impulse engine at 100% power, in Newtons.
max_thrust: 50000
```

### Kinetic Weapon Port (`kinetic-weapon`)

Kinetic weapon ports allow the ship to mount projectile-based weapons, such as railguns or autocannons. Players select the specific weapon to mount when adding this module. The potential ammunition types can be found in `data/ammo/kinetic/*.yaml`. `type: kinetic-weapon` modules have the following additional fields in their specification:

```yaml
# (str) The type of ammo compatible with the weapon.
ammo_type: shell

# (str) The size of ammo compatible with the weapon.
ammo_size: 200mm

# (float) The reload time (time between volleys) of the weapon when at 100%
# power, in seconds.
reload_time: 5.0

# (int) The number of projectiles fired each volley.
num_projectiles: 1

# (int) The amount of ammo consumed each volley.
ammo_consumed: 1

# (float) The accuracy of the weapon at its effective range.
accuracy: 0.9

# (int) The effective range of the weapon, in meters.
effective_range: 1000
```

### Maneuvering Thrusters (`maneuvering-thruster`)

Maneuvering thrusters provide the ship with lateral and rotational thrust capabilities. Players select the specific thruster model to install after adding the module to the ship. `type: maneuvering-thruster` modules have the following additional fields in their YAML specification:

```yaml
# (int) The amount of angular thrust provided by the thruster when at 100% power, in Newtons.
angular_thrust: 100000
```

### Missile Launchers (`missile-launcher`)

Missile launchers allow the ship to launch guided missile weapons. Players select the specific missile launcher to install when adding this module. The potential missile types can be found in `data/ammo/missiles/*.yaml`. `type: missile-launcher` modules have the following additional fields in their specification:

```yaml
# (float) The time it takes to reload the missile launcher when at 100% power, in seconds.
reload_time: 10.0

# (float) The minimum firing delay between missile volleys when at 100% power, in seconds.
missile_volley_delay: 1.0

# (int) The number of missiles launched per volley.
num_launched: 1

# (int) The maximum number of missiles that can be loaded in the launcher.
ammo_capacity: 4
```

### Power Core (`power-core`)

Adds a power core to the ship. Players select the type of power core to install after adding the module. The power core determines the total power capacity and power production of the ship, which can then be allocated to other modules by the engineering officer to improve their capabilities.`type: power-core` modules have the following additional fields in their YAML specification:

```yaml
# (int) The amount of energy produced by the module per second.
energy_production: 1000

# (int) The total energy capacity of the power core.
energy_capacity: 5000
```

### Radial Emission System (`radial-emission-system`) - NOT YET IMPLEMENTED

Radial emission systems provide things like EMP pulses that eminate from all directions around the ship. Players select the specific radial emission system to install after adding the module. `type: radial-emission-system` modules have the following additional fields in their YAML specification:

```yaml
# (int) The maximum range of the pulse when at 100% power, in meters.
max_pulse_range: 10000

# (int) The speed of the generated pulse when at 100% power, in m/s.
pulse_speed: 1000
```

How the effect of the pulse itself is defined in YAML has not yet been determined.

### Sensor Array (`sensor-array`)

The sensor array module determines the ship's sensor capabilities. Players select the specific sensor array to install after adding the module. The sensor array affects the ship's ability to detect other ships, celestial bodies, and anomalies in space. `type: sensor-array` modules have the following additional fields in their YAML specifications:

```yaml
# (int) The scan range of the sensor array when provided 100% power, in meters.
scan_range: 1000000

# (int) (1-10) The level of detail provided by the sensor when scanning individual objects, with higher values revealing more detailed information.
detail_level: 5

# (float) The number of seconds it takes to scan individual objects when at 100% power.
scan_time: 3.0
```

### Shield Generator (`shield-generator`)

Shield generators provide the ship with energy shields that can absorb incoming damage. Players select the specific shield generator to install after adding the module. The shield generator's strength and recharge rate depend on the model selected. `type: shield-generator` modules have the following additional fields in their YAML specifications:

```yaml
# (int) The maximum shield strength provided by the generator.
max_shield_strength: 10000

# (int) The rate at which the shield recharges when not taking damage at 100%
# power, in shield points per second.
shield_recharge_rate: 500
```

### Stealth System (`stealth-system`)

Stealth systems allow the ship to reduce its detectability by enemy sensors. Players select the specific stealth system to install after adding the module. The characteristics and effectiveness of the stealth system depends on the model selected. `type: stealth-system` modules have the following additional fields in their YAML specifications:

```yaml
# (float) The reduction in detectability provided by the stealth system, as a
# percentage. Only used by AI systems.
detectability_reduction: 0.5

# (float) Increases the scan time of enemy sensors when at 100% power, in seconds.
scan_time_increase: 2.0
```

### Torpedo Tube (`torpedo-tube`) - No Varients

Adds a new torpedo tube to the ship. Each torpedo tube allows the ship to launch one torpedo at a time. Players select the type of torpedo to load into the tube when launching. The potential torpedo types are defined in `data/ammo/torpedos/*.yaml`.

### Warp/Jump Core (`warp-jump-core`)

Warp/jump cores enable faster-than-light travel by allowing the ship to enter a warp or jump state. Players select the specific warp/jump core to install after adding the module. The core determines the ship's FTL capabilities, including maximum speed or jump range. `type: warp-jump-core` modules have the following additional fields in their YAML specifications:

```yaml
# (warp|jump) The type of FTL travel provided by the module. If `warp`, the
# ship will accelerate to FTL. If `jump`, the ship will instantaneously
# "teleport" some distance ahead.
warp_type: warp

# (float) The amount of time it takes to engage the drive at 100% power, in
# seconds.
warp_delay: 5.0

# (int) If `warp_type` is `jump`, the maximum distance jumped by the warp drive
# when at 100% power, in km.
jump_distance: 10
```

## Ammunition - `data/ammo/**/*.yaml`

All ammunition types have the following base fields:

```yaml
# (str) A unique identifier associated with the ammunition type.
id: shell-200mm-st

# (str) The name of the ammunition type.
name: ST 200mm Shell

# (str) A brief description of the ammunition type.
desc: >-
  Standard 200mm shell for cannons.

# (int) The cost of the ammo.
cost: 1

# (int) The weight of the ammo, in kg.
weight: 2
```

### Kinetic Ammunition - `data/ammo/kinetic/*.yaml`

Kinetic weapon ammunition includes shells for cannons and slugs for railguns. Cannon shells come in ST (Standard), AP (Armor Piercing), and HE (High Explosive) variants. Kinetic weapon ammunition has the following additional fields:

```yaml
# (int) The impact damage of the round.
impact_damage: 10

# (int) The blast radius of the round, in (m).
blast_radius: 5

# (int) The blast damage of the round.
blast_damage: 1

# (int) The velocity of the round (m/s).
velocity: 600

# (int) The level of armor penetration of the round (1-10).
armor_penetration: 5
```

### Missiles - `data/ammo/missiles/*.yaml`

Missile ammunition includes various types of guided missiles. Missiles can have different guidance systems, warhead types, and speeds. Missile ammunition has the following additional fields:

```yaml
# (str) The guidance system of the missile (e.g., "heat-seeking", "radar-guided").
guidance_system: heat-seeking

# (int) The impact damage of the missile.
impact_damage: 100

# (int) The blast radius of the missile, in (m).
blast_radius: 5

# (int) The blast damage of the missile.
blast_damage: 1

# (float) The acceleration of the missile's engine, in m/s^2.
acceleration: 0.5

# (int) The maximum speed of the missile, in m/s.
max_speed: 800

# (float) The maximum turn rate of the missile, in deg/s.
max_turn_rate: 1.0

# (int) The maximum lifetime of the missile before automatically detonating, in
# seconds.
lifetime: 10

# (list[str]) Weapon tags associated with the missile.
weapon_tags: ["Missile", "Tachyon"]
```

### Torpedos - `data/ammo/torpedos/*.yaml`

Torpedos act like large, unguided missiles. They are typically slower but can travel further distances and deal much more damage. Torpedos have the following additional fields in their specification:

```yaml
# (int) The impact damage of the torpedo.
impact_damage: 100

# (int) The blast radius of the torpedo, in (m).
blast_radius: 500

# (int) The blast damage of the torpedo.
blast_damage: 1000

# (float) The acceleration of the torpedo's engine, in m/s^2.
acceleration: 0.5

# (int) The maximum speed of the torpedo, in m/s.
max_speed: 300

# (int) The maximum lifetime of the torpedo before automatically detonating, in
# seconds.
lifetime: 100

# (list[str]) Weapon tags associated with the torpedo.
weapon_tags: ["Torpedo", "Graviton"]
```

## Module Design Guidelines

1. Stick to the established format for module slot and module varient specifications.
2. Ensure all required fields are present for each module slot and varient.
3. Use consistent naming conventions for IDs, names, and models.
4. Provide clear and concise descriptions for each module slot and varient.
5. See `docs/game.md` for a list of weapon tags.

## API Endpoints (Not Exhaustive)

The ship modules system should expose the following API endpoints for use during ship construction.

### `GET /v1/catalog/module-slots`

Lists the various ship module slots, returning a list of module slot ids.

### `GET /v1/catalog/module-slots/<slot id>`

Example: `/v1/catalog/module-slots/impulse-engine`

Gets detailed information about the specified module slot. This should return all the fields specified in the YAML file for that module slot.

### `GET /v1/catalog/modules/<slot id>`

Example: `/v1/catalog/modules/impulse-engine`

Lists all available module varients for the specified module slot. This should return a list of module IDs.

### `GET /v1/catalog/modules/<slot id>/<module id>`

Example: `/v1/catalog/modules/impulse-engine/ion-engines`

Gets detailed infomration about the specified module varient.

### `GET /v1/catalog/ammo`

Returns the list of all available ammunition categories (`kinetic`, `missiles`, or `torpedos`).

### `GET /v1/catalog/ammo/<ammo category>`

Example: `/v1/catalog/ammo/kinetic`

Lists all of the types of ammo for the specified ammo category, returning a list of IDs.

### `GET /v1/catalog/ammo/<ammo category>/<ammo id>`

Example: `/v1/catalog/ammo/kinetic/shell-100mm-ap`

Gets detailed information about the specified ammunition type.