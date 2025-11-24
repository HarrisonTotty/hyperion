# Kinetic Weapons & Ammo

The following directory contains the YAML file specifications for the various kinetic weapons in the game. Kinetic weapons are weapons such as cannons and railguns which fire physical projectiles. The damage of kinetic weapons is not baked into the weapon itself but rather the ammunition being fired. Available ammo types can be found in the `ammo/` subdirectory.

A kinetic weapon has the following specification:

```yaml
# (str) The name, model, kind, and manufacturer of the weapon. The
# `name` is usually the same as the model, and is the only value that needs to
# be unique. The rest are for lore/rp purposes and will be displayed on the
# science screen when scanning a vessel.
name: 200mm Cannon
model: Mark II "Brute" Cannon
kind: Cannon
manufacturer: VelocityTech Idustries Inc.

# (str) A brief description of the weapon.
desc: >-
  A standard 200mm cannon capable of bombarding enemy ships with sustained
  firepower.

# (int) The build cost of the weapon.
cost: 100

# The compatible ammunition type and size.
ammo:
  type: shell
  size: 200mm

# (list) A list of weapon tags associated with the weapon.
tags: ["Single-Fire", "Automatic"]

# (float) The number of seconds it takes to reload the weapon at 100% power.
reload_time: 2.0

# (int) The number of projectiles per volley of fire.
num_projectiles: 1

# (int) The amount of ammo consumed per volley of file.
ammo_consumption: 1

# (float) The accuracy of the weapon out to its effective range.
accuracy: 0.9

# (int) The effective range of the weapon, in meters.
effective_range: 300
```

Likewise, ammunition has the following specification:

```yaml
# (str) Basic information about the name, type, and size of the round.
name: ST 200mm Shell
type: shell
size: 200mm

# (str) A description of the 
desc: >-
  A crate of standard 200mm rounds.

# (int) The build cost of the round.
cost: 10

# (int) The weight of the ammo, per round, in kg.
weight: 10

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