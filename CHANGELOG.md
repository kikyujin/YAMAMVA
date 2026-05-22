# Changelog

All notable changes to YamAMVA will be documented in this file.

## [0.9.0] - 2026-05-22

### Added
- Core scenario engine: YAML parse → state machine → command dispatch
- 5 built-in nodes: `do`, `jump`, `when`, `incase`, `end`
- Expression evaluator: comparison, logic, arithmetic operators
- Registration model: game-defined node types with PASS/BLOCKING flags
- `when` filtering on both node-level and element-level
- `$result` special variable for BLOCKING command responses
- Save/restore: JSON snapshot at any execution point
- Metadata API: `characters`, `backgrounds`, `bgm`, `format` sections
- C API (FFI): 10 functions, extern "C", cdylib output
- Python FFI test harness (`test_yamamva_ffi.py`)
- Example scenarios: `ellmar_tour.yaml`, `oyatsu_adv.yaml`

### Validated
- Unity 6 integration via C# P/Invoke (ELLMAR project)
  - NavMesh movement, VRM emotion control, scenario-driven character behavior
- 21 Rust tests + 5 FFI tests passing

### Known Limitations
- No `#include` or scenario composition (single-file only)
- No parallel scene execution
- Expression engine does not support parentheses (flat precedence)
- String interpolation in `do` expressions not supported

## [Unreleased]

### Planned
- Parenthesized expressions
- Scenario include/import
- WASM build target
- crates.io publication
