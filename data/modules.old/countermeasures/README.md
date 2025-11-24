# Countermeasures

The following directory contains the YAML files for the various countermeasures available in the game. These have the following format:

```yaml
# (str) The name, model, kind, and manufacturer of the countermeasure. The
# `name` is usually the same as the model, and is the only value that needs to
# be unique. The rest are for lore/rp purposes and will be displayed on the
# science screen when scanning a vessel.
name: Chaff Pod
model: AX-9 Chaff Pod
kind: Rocket-Propelled Sensor Pod Array
manufacturer: Guardian Systems Inc.

# (str) A breif description of the countermeasure.
desc: >-
  A rocket-propelled sensor pod capable of jamming nearby missile guidence systems.

# (int) The cost of the countermeasure.
cost: 10

# (int) The weight of the countermeasure, in kg.
weight: 1

# (int) The amount of forward thrust provided by the countermeasure's engine.
forward_thrust: 400

# (float) The maximum turn rate of the countermeasure, in deg/s.
max_turn_rate: 1.0

# (float) The time it takes to load/prepare the countermeasure when provided 100% power, in seconds.
load_time: 15.0

# (list) A list of weapon tags associated with the countermeasure.
tags: ["Missile", "Positron"]

# (int) The blast radius of the countermeasure, in meters.
blast_radius: 100

# (int) The maximum projectile lifetime of the countermeasure before automatically
# detonating, in seconds.
lifetime: 10

# (int) The maximum speed of the countermeasure, in m/s.
max_speed: 1200
```

## Design Guidelines

1. See `doc/game.md` for a list of weapon tags and their effects.