# Ship Classes

The following directory contains the various ship classes available in the game.

Each ship class is specified as a YAML file with the following form:

```yaml
# (str) The name of the ship class
name: Defender

# (str) A breif description of the ship class.
desc: >-
  The largest class of defensive ship, outfitted with advanced defensive
  capabilities Defender class ships are capable of taking more damage than any
  other class.

# (Small|Medium|Large) The general size of the ship.
size: Large

# (Offense|Defense|Support|Versatile) The intended role of the ship.
role: Defense

# (int) The maximum weight the ship will support (kg).
max_weight: 400

# (int) The maximum number of ship modules attachable to the ship.
max_modules: 20

# (int) The base HP of the ship.
base_hp: 600

# (int) The number of build points associated with the ship class.
build_points: 1000

# (obj) A collection of special bonuses provided by the ship class.
bonuses:
  module_hp: 0.3 # +30% HP of all modules
  module_cost_defense: -0.2 # -20% cost of defense modules.
```

## Available Ship Class Bonuses

Below are the various potential ship class bonuses. Note that some bonuses may be restricted to a particular _group_ of ship modules indicated by `<GROUP>`. See `modules.yaml` for the available groups.

### `module_cost` or `module_cost_<GROUP>`

Reduces the cost of all ship modules or a subset of ship modules, specified as a negative value. A value of `-0.2` would correspond to -20% module cost.

### `module_hp` or `module_hp_<GROUP>`

Grants additional HP to all ship modules or a subset of ship modules. A value of `0.3` would correspond to +30% module HP.

### `module_weight` or `module_weight_<GROUP>`

Reduces the weight of all ship modules or a subset of ship modules, specified as a negative value. A value of `-0.2` would correspond to -20% module weight.

## Design Guidelines

1. `Small` ships should allow at max 10 to 13 modules.
2. `Medium` ships should allow at max 13 to 17 modules.
3. `Large` ships should allow at max 17 to 20 modules.