# Missile Weapons

The following directory contains the YAML files for missile weapons. Each missile has the following structure:

```yaml
# (str) The name, model, kind, and manufacturer of the weapon. The
# `name` is usually the same as the model, and is the only value that needs to
# be unique. The rest are for lore/rp purposes and will be displayed on the
# science screen when scanning a vessel.
name: Type 3 Positron Missile
model: Type 3 Positron Missile
kind: Missile
manufacturer: SteelArrow Inc.

# (str) A breif description of the thruster.
desc: >-
  A missile armed with a positron warhead, dealing less damage than the MK 11
  but allowing some of the damage to bypass shields.

# (int) The cost of the missile.
cost: 10

# (int) The weight of the missile, in kg.
weight: 1

# (int) The amount of forward thrust provided by the missile's engine.
forward_thrust: 400

# (float) The maximum turn rate of the missile, in deg/s.
max_turn_rate: 1.0

# (float) The time it takes to load/prepare the missile when provided 100% power, in seconds.
load_time: 5.0

# (list) A list of weapon tags associated with the missile weapon.
tags: ["Missile", "Positron"]

# (int) The impact damage of the missile.
impact_damage: 100

# (int) The damage delt to objects in its blast radius.
blast_damage: 10

# (int) The blast radius of the missile, in meters.
blast_radius: 100

# (int) The maximum projectile lifetime of the missile before automatically
# detonating, in seconds.
lifetime: 10

# (int) The maximum speed of the missile weapon, in m/s.
max_speed: 1200
```

## Design Guidelines

1. See `doc/game.md` for a list of weapon tags and their effects.
2. `Decoy` weapons should deal very little impact damage and no blast damage.