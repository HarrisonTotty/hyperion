# Rust Development Guidelines

Modern Rust development practices based on official guidelines and community standards.

## Code Style & Naming

Follow RFC 430 naming conventions:

| Item | Convention | Example |
|------|------------|---------|
| Crates | `snake_case` | `my_crate` |
| Modules | `snake_case` | `my_module` |
| Types (structs, enums, traits) | `UpperCamelCase` | `MyStruct` |
| Functions, methods | `snake_case` | `do_something` |
| Local variables | `snake_case` | `my_var` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_SIZE` |
| Type parameters | `UpperCamelCase`, short | `T`, `E`, `K`, `V` |

### Conversion Methods

- `as_` — Borrowed reference to borrowed reference (cheap, no allocation)
- `to_` — Borrowed reference to owned value (may allocate)
- `into_` — Consumes self, returns owned value

### Iterator Methods

- `iter()` — Returns `&T`
- `iter_mut()` — Returns `&mut T`
- `into_iter()` — Consumes collection, returns `T`

## Documentation

Use Rust's doc comment syntax (`///` for items, `//!` for modules/crates).

### Required Sections

Every public item should have:

1. **Summary** — Single line explaining what it does
2. **Extended description** — More detail if needed
3. **Examples** — At least one copy-pasteable example
4. **Panics** — Document all panic conditions
5. **Errors** — Document all error conditions for `Result` returns
6. **Safety** — Required for `unsafe` functions

```rust
/// Computes the target firing solution for a weapon system.
///
/// Calculates lead angle and time-to-impact based on target velocity
/// and distance. Uses linear interpolation for moving targets.
///
/// # Examples
///
/// ```
/// use hyperion::weapons::compute_firing_solution;
///
/// let solution = compute_firing_solution(target, weapon)?;
/// assert!(solution.time_to_impact > 0.0);
/// ```
///
/// # Errors
///
/// Returns `Err` if target is out of range or behind the ship.
///
/// # Panics
///
/// Panics if `weapon.range` is zero or negative.
pub fn compute_firing_solution(target: &Target, weapon: &Weapon) -> Result<Solution, WeaponError> {
    // ...
}
```

### Documentation Best Practices

- Use `?` in examples, not `unwrap()` or `try!`
- Add hyperlinks to related types: ``[`OtherType`]``
- Keep summary under one line (shown in module overview)
- Prefer line comments (`//`) over block comments (`/* */`)

## Error Handling

### Library Code: Use `thiserror`

Define structured, typed errors that callers can match on:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShipError {
    #[error("module '{name}' not found")]
    ModuleNotFound { name: String },

    #[error("insufficient power: need {required}W, have {available}W")]
    InsufficientPower { required: f64, available: f64 },

    #[error("invalid blueprint")]
    InvalidBlueprint(#[from] BlueprintError),

    #[error("I/O error")]
    Io(#[from] std::io::Error),
}
```

### Application Code: Use `anyhow`

For top-level error handling with rich context:

```rust
use anyhow::{Context, Result};

fn load_ship_config(path: &Path) -> Result<ShipConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read ship config from {}", path.display()))?;

    serde_yaml::from_str(&content)
        .context("failed to parse ship config YAML")
}
```

### Error Guidelines

- Libraries expose typed errors (`thiserror`)
- Applications convert to opaque errors (`anyhow`) at boundaries
- Always provide context when propagating errors
- Use `bail!` for early returns with error messages

## Testing

### Unit Tests

Place unit tests in the same file as the code being tested.

### Test Naming

Use descriptive names that explain the scenario:

- `succeeds_when_valid_input`
- `returns_error_when_module_missing`
- `panics_if_division_by_zero`

Avoid: `it_works`, `test_function`, `basic_test`

### Integration Tests

Place in `tests/` directory at crate root:

```
tests/
├── simulation_integration.rs
├── api_integration.rs
└── common/
    └── mod.rs  # Shared test utilities
```

Integration tests can only access public API — use them to verify components work together.

### Documentation Tests

Code examples in doc comments are tested automatically:

```rust
/// ```
/// let engine = Engine::new(1000.0);
/// assert_eq!(engine.max_thrust(), 1000.0);
/// ```
```

Use `no_run` for examples that shouldn't execute (I/O, network):

```rust
/// ```no_run
/// let server = Server::bind("0.0.0.0:8080").await?;
/// ```
```

## Trait Implementation

Implement common traits eagerly on public types:

| Trait | When to Implement |
|-------|-------------------|
| `Debug` | **Always** on public types |
| `Clone` | When logical copying makes sense |
| `Default` | When a sensible default exists |
| `PartialEq`, `Eq` | For comparable types |
| `Hash` | If `Eq` is implemented and type is used as key |
| `Send`, `Sync` | Ensure types are thread-safe when possible |
| `Serialize`, `Deserialize` | For data that crosses boundaries |

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleStatus {
    pub id: ModuleId,
    pub health: f64,
    pub powered: bool,
}
```

## Type Safety

### Use Newtypes for Distinct Concepts

```rust
// Bad: easy to mix up
fn set_position(x: f64, y: f64, z: f64) { }

// Good: type-safe
pub struct Meters(pub f64);
pub struct Position { x: Meters, y: Meters, z: Meters }

fn set_position(pos: Position) { }
```

### Avoid Boolean Parameters

```rust
// Bad: what does `true` mean?
ship.set_power(true);

// Good: intention is clear
pub enum PowerState { On, Off, Standby }
ship.set_power(PowerState::On);
```

### Use Builder Pattern for Complex Construction

```rust
let ship = ShipBuilder::new("USS Hyperion")
    .class(ShipClass::Cruiser)
    .add_module(Module::Reactor { output: 5000.0 })
    .add_module(Module::Shield { capacity: 1000.0 })
    .build()?;
```

## Module Organization

- Prefer many small, focused modules over large files
- Use `mod.rs` or `module_name.rs` to organize submodules
- Re-export public items at appropriate levels
- Keep implementation details private

## Linting

Enable Clippy in CI and address all warnings:

```bash
cargo clippy -- -D warnings
```

Common useful lints to enable in `Cargo.toml`:

```toml
[lints.rust]
unsafe_code = "warn"

[lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"
```
