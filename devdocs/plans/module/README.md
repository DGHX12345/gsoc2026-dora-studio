# dora-studio v2.0 — Implementation Plan

> **Branch**: `week12-2.0`
> **Architecture**: Axum + dora crates (M0-M12) → Tauri shell (M14)
> **Timeline**: 12 weeks, 15 modules across 6 layers
> **Status**: Draft — pending mentor review

## Module Stack Overview

```
Layer 6: Desktop Shell
  M14: Tauri integration

Layer 5: Tool Slot System
  M13: MoveIt bridge
  M12: dviz path visualization
  M11: Tool slot protocol

Layer 4: VLM Attribution Bar
  M10: LeRobot profiles
  M9:  Attribution bar UI

Layer 3: Performance Visualization
  M8:  OTel flame graph
  M7:  Metrics panels

Layer 2: .drec Replay
  M6:  3D viewport sync
  M5:  Timeline UI
  M4:  .drec parser

Layer 1: Dataflow Live Graph
  M3:  Runtime binding (Hot Reload + status)
  M2:  Arrow Schema type checking
  M1:  Dataflow editor canvas

Layer 0: Foundation
  M0:  dora crate integration
```

Each module depends only on modules below it. No module may depend on a module at the same or higher layer.

## Quality Gates

Every module must pass all gates before the next module begins:

| Gate | What | Who |
|------|------|-----|
| G1: Build | `cargo build` + `npm run build` clean | author |
| G2: Test | All existing + new tests pass | author |
| G3: Review | Code review against acceptance criteria | mentor/self |
| G4: Integration | End-to-end smoke test with real dora | author |

## Module Format

Each module has its own file (`MXX-name.md`) containing:
- Dependencies (modules it builds on)
- Deliverables (concrete artifacts: files, APIs, components)
- Acceptance criteria (verifiable statements)
- Exposed interfaces (what upper modules can use)
- Estimated effort (weeks)
