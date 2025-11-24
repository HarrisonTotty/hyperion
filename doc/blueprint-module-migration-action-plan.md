# Blueprint & Compiler Module Migration Action Plan

**Last Updated:** November 10, 2025
**Status:** Planning

---

## Objective

Migrate `src/models/blueprint.rs` and `src/compiler.rs` to fully support the new two-tier module system (module slots + module variants) as defined in `doc/modules.md` and `doc/module-ui-implementation-plan.md`. This migration will ensure all blueprint logic, validation, and compilation are compatible with the latest HYPERION module architecture.

---

## Phase 1: Analysis & Preparation

### 1.1 Review New Module System
- Study `doc/modules.md` and `doc/module-ui-implementation-plan.md` for the new data model and API requirements.
- Identify all new types: `ModuleSlot`, `ModuleVariant`, `ModuleInstance` (with `module_slot_id` and `variant_id`).

### 1.2 Audit Existing Code
- Catalog all usages of legacy fields (`module_id`, `kind`) in `src/models/blueprint.rs` and `src/compiler.rs`.
- List all validation, serialization, and logic that depend on the old format.
- Identify all test cases and helper functions that require updates.

---

## Phase 2: Model Refactor

### 2.1 Update `ModuleInstance` Struct
- Change struct fields from `module_id`, `kind` to `module_slot_id`, `variant_id`.
- Update all serialization and deserialization logic.
- Refactor all usages in `ShipBlueprint` and related types.

### 2.2 Update Validation Logic
- Refactor validation functions to use `module_slot_id` and `variant_id`.
- Update error and warning enums to reference new fields.
- Ensure required module, max allowed, and variant configuration checks use slot/variant logic.

### 2.3 Update Test Cases
- Refactor all tests to use new struct fields and logic.
- Add new tests for slot/variant validation and error handling.

---

## Phase 3: Compiler Refactor

### 3.1 Update Module Compilation Logic
- Refactor all logic in `src/compiler.rs` to use `module_slot_id` and `variant_id`.
- Update variant lookup, stat aggregation, and cost calculation to use the new architecture.
- Remove all legacy references to `module_id` and `kind`.

### 3.2 Update Helper Functions
- Refactor helper functions for module lookup, variant selection, and stat calculation.
- Ensure compatibility with new API endpoints and data structures.

### 3.3 Update Compiler Tests
- Refactor all compiler tests to use new fields and logic.
- Add new tests for slot/variant compilation and error handling.

---

## Phase 4: Integration & Validation

### 4.1 End-to-End Testing
- Validate blueprint creation, mutation, and compilation with the new module system.
- Ensure all API endpoints and UI components work with updated models.
- Run all unit and integration tests; fix any failures.

### 4.2 Documentation & Migration Notes
- Update developer documentation to reflect new model and compiler logic.
- Document migration steps, breaking changes, and compatibility notes.
- Provide guidance for updating existing blueprints and data.

---

## Phase 5: Cleanup & Finalization

### 5.1 Remove Legacy Code
- Remove all unused legacy fields, functions, and tests.
- Ensure codebase is clean and only supports the new module system.

### 5.2 Final Review
- Conduct code review and QA for all changes.
- Confirm all requirements from `doc/modules.md` and `doc/module-ui-implementation-plan.md` are met.
- Mark migration as complete.

---

## Success Criteria
- All blueprint and compiler logic uses `module_slot_id` and `variant_id` exclusively.
- All validation and compilation functions support the two-tier module system.
- All tests pass and cover slot/variant logic.
- Documentation is updated and migration steps are clear.
- No regressions or compatibility issues remain.

---

**Document Version:** 1.0
**Maintainer:** GitHub Copilot
