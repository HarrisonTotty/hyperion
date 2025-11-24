# Communications Systems

The following directory contains defintions for the various communications systems available during ship construction. These are YAML files with the following format:


```yaml
# (str) The name, model, kind, and manufacturer of the system. The
# `name` is usually the same as the model, and is the only value that needs to
# be unique. The rest are for lore/rp purposes and will be displayed on the
# science screen when scanning a vessel.
name: Standard Radio Communications Array
model: Z96 Radio Transmission Array
kind: Radio Transmitter
manufacturer: Wolf Comms Inc.

# (str) A breif description of the system.
desc: >-
  A standard radio communications system with a moderate range and energy requirements.

# (int) The build cost of the system.
cost: 100

# (int) The weight of the system, in kg.
weight: 100

# (int) The range of the communication system at 100% power (m).
range: 100000

# [int, int] The inclusive frequency range at which the communications system
# can communicate (MHz).
frequency_range: [30, 300]

# (int) The encryption strength of the communications system (1-10).
encryption_strength: 3

# (int) The amount of energy consumed per second when provided 100% power.
energy_consumption: 10
```