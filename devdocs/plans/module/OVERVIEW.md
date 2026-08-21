# dora-studio v2.0 — Module Overview (revised)

> **Revision 2** — 2026-08-11, based on dora/dviz/moveit source audit
> **Key change**: dora crates can't be linked (Rust 1.75 vs 1.88). Strategy: copy types.

## Source Audit Findings

| Finding | Impact | Resolution |
|---------|--------|------------|
| dora edition 2024, rustc 1.88 — we have 1.75 | M00 | Copy struct definitions; pin to current dora revision |
| `RecordingReader` is sequential-only, no seek/mmap | M04 | Build our own offset index (sequential scan → binary search) |
| dviz ports: `waypoints`/`path`/`trajectory`, not `planned_path` | M12 | Correct port patterns; flat Float32Array parser |
| dviz has NO costmap support (wishlist only) | M12 | Build costmap renderer ourselves from float arrays |
| dora-moveit2: flat float32, no TrajectoryArray type, no URDF on wire | M13 | Parse flat arrays + metadata; URDF from disk; joint names from config |
| dora-moveit2: planning scene is JSON, not binary | M13 | JSON scene parser; simplified collision wireframes |
| Coordinator WebSocket is full JSON-RPC with Bearer auth | M03 | Correct auth flow; use `dora_message` copied types |
| Daemon metrics polled via `GetNodeInfo`, not pushed | M07 | Polling approach confirmed correct; 2s interval matches daemon |

## Stack Diagram (unchanged structure)

```
Layer 6: Desktop Shell
  M14: Tauri integration (unchanged)

Layer 5: Tool Slot System
  M13: MoveIt bridge (REWRITTEN — flat arrays + JSON scene + disk URDF)
  M12: dviz path tool (REWRITTEN — flat Float32Array + correct ports)
  M11: Tool slot protocol (unchanged)

Layer 4: VLM Attribution Bar
  M10: LeRobot profiles (unchanged)
  M9:  Attribution bar UI (unchanged)

Layer 3: Performance Visualization
  M8:  OTel flame graph (unchanged)
  M7:  Metrics panels — polling GetNodeInfo confirmed (unchanged)

Layer 2: .drec Replay
  M6:  3D viewport sync (unchanged)
  M5:  Timeline UI (unchanged)
  M4:  .drec parser (REWRITTEN — own offset index, no dora seek)

Layer 1: Dataflow Live Graph
  M3:  Runtime binding (minor corrections — Bearer auth, correct WS flow)
  M2:  Arrow Schema type checking (unchanged)
  M1:  Dataflow editor canvas (unchanged)

Layer 0: Foundation
  M0:  Type definitions — copy dora structs, .drec format, coordinator protocol
```

## Module Summary

| # | Module | Weeks | Key Change from v1 Plan |
|---|--------|-------|------------------------|
| M0 | Type Definitions | 0.5 | **Copy types, don't link crates** |
| M1 | Dataflow Canvas | 2.0 | Unchanged |
| M2 | Arrow Schema Check | 1.0 | Unchanged |
| M3 | Runtime Binding | 1.5 | Add Bearer auth, correct WS handshake |
| M4 | .drec Parser + Index | 1.5 | **Own offset index builder** (+0.5 wk) |
| M5 | Timeline UI | 1.5 | Unchanged |
| M6 | 3D Viewport Sync | 1.0 | Unchanged |
| M7 | Metrics Panels | 1.5 | Confirmed: poll `GetNodeInfo` |
| M8 | OTel Flame Graph | 1.0 | Unchanged |
| M9 | Attribution Bar | 1.5 | **Rev 2026-08-13**: bar 集成进 VisualizationView Replay 覆盖层；D4 仅 seek（幽灵渲染移 M12/M13）；载荷用 Studio 结构化格式，Arrow IPC 标 unavailable 不伪造 |
| M10 | LeRobot Profiles | 1.0 | **Rev 2026-08-13**: Python+pyarrow 子进程桥解析 Parquet（Rust 无法 link dora）；B601 真实数据优先验证；angle_unit 配置字段（B601 是度） |
| M11 | Tool Slot Protocol | 1.0 | Unchanged |
| M11.5 | Monitoring Control & Efficiency | 1.0 | **NEW**: on-demand toggle (default off) + WS data source (M07) + optional OTLP receiver (M08) |
| M12 | dviz Path Tool | 1.5 | **Flat f32, correct ports, own costmap** |
| M13 | MoveIt Bridge | 1.5 | **Flat f32, JSON scene, disk URDF** |
| M14 | Tauri Shell | 1.5 | Unchanged |
| M15 | Live Planning & Simulation | 2.5 | **CONFIRMED ESSENTIAL 2026-08-14**: rviz-style trio recreated in dora ecosystem — Studio frontend + dora-moveit2 planning + dora-mujoco physics; live data bridge (`studio_bridge` node, no new Rust deps); Phase A rides on M13. Module roles: Visualization = renderer/sim viewport, Motion Planner = planning console (algorithm implantation, Plan/Execute/Stop, scene management) — each irreplaceable |
| M15.5 | dora 1.0 Upgrade | 1.0 | **NEW 2026-08-17**: align Studio runtime/protocol with dora 1.0 (rc.4→release). Audit: CLI/.drec/YAML survive; WS client already pre-adapted in M09 (Hello handshake, 1.0-shape types) but unactivated — venv still 0.5. Work: venv upgrade (dora-studio-1.0, old kept as fallback) + per-field type diff + dynamic version string + CLI fixture checks + demo regression. Placement: before M15.6/M14 so everything downstream ships on 1.0 |
| M15.6 | OTLP gRPC Receiver | 1.0 | **NEW 2026-08-17 (student chose option A)**: dora 0.5/1.0 telemetry is gRPC-only (`with_tonic()`, `DORA_OTLP_ENDPOINT`) so M11.5 D3's HTTP receiver can't consume dora spans. Add OTLP gRPC receiver (tonic/h2, Rust 1.75 precheck required) so flame graphs reach push mode without Jaeger. e2e verified against dora 1.0 after M15.5 | 
| M16.5 | Terminal-Free Operations | 1.5 | **NEW 2026-08-17**: zero-terminal daily ops — one-click `dora up`/`dora down` session control (state = coordinator reachability, external sessions visible/stoppable), Run Monitor switches to coordinator-registered flows (`dora start/stop --name`), frame-stream recording (Record/Stop UI, frame count, open in Replay). dora 1.0 only with version gate; `DORA_STUDIO_DORA_BIN` resolution; one-time env installs (Jaeger/dviz/moveit) move to README. Two PRs (lifecycle / recording) |
| M17 | dora Version Manager | 0.5 | **NEW 2026-08-17 (design-only, before M16)**: environment panel detects installed dora versions (scanned candidate paths), shows active version + compatibility (1.x full / 0.5 degraded), switch persists to settings file (= UI for `DORA_STUDIO_DORA_BIN`). No download/install (uv's job) |
| M18 | Dataflow Explorer 2.0 | 2.0 | **NEW 2026-08-17 (design-only, mentor priority)**: real nodes from user projects (project-dir scan + manual submit), vendored dora 1.0 std type catalog, compat engine replicating schema_compatible/widening/type_rules with `dora validate` final check, two-way canvas editing (write-back confirm / save-as), port URN pickers, type_rules as edge declarations |
| M16 | LeRobot Interop (dorobot) | 0.5 | **NEW 2026-08-14, NON-ESSENTIAL**: data-level interop with dorobot-studio (no code merge); tasks.jsonl/episodes.jsonl support in reader. Optional — project completeness does not depend on it |

**Total**: 19.5 weeks (was 18.0). Parallel execution on independent layers recovers to ~12 weeks.

## Critical Path

```
M0 → M1 → M2 ─┬─ M4 → M5 → M6 → M11 → M12 → M13 → M14
               │                                     └→ M15 ─→ M15.5 (dora 1.0) ─→ M15.6 (OTLP gRPC) ─→ M16.5 (terminal-free) ─→ M17 (version mgr) ─→ M18 (explorer 2.0)
               ├─ M3 → M7 → M8
               └─ M9 → M10 ─→ M16 (lerobot interop)
```

M3, M4, M9 can start in parallel after M1. M15 Phase B/C is post-M13.
M16 is post-M10, independent of M13-M15, and **optional** — drop it freely
if scheduling is tight (see M16 plan).

## Risk Register (updated)

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Copied dora types drift from upstream | Low | Medium | Pin revision; document divergence in RUST-COMPAT.md |
| .drec index build slow for 1GB+ files | Medium | Medium | Parallel index build; show progress; test with real .drec |
| dviz planner not running → no path data | High | Medium | Ship synthetic waypoint generator for dev/testing |
| MoveIt joint names unavailable on wire | High | Medium | Config file reader; manual mapping for known robots |
| URDF loading in Three.js (custom parser needed) | Medium | Medium | Reuse dviz URDF parser (Rust) or build lightweight JS parser |
| 19.5 weeks > 12 weeks | Medium | High | M14 (Tauri) post-v2.0; M8/M10 can be trimmed |
| M15: dora pip/CLI version mismatch blocks local dataflow run | Medium | Medium | python3.11 + uv verified installed (2026-08-14); B1 spike first; fallback remote/container host |
| M15: dora-moveit2 collision checking stubbed | Medium | Medium | Complete FCL integration in local dora-moveit2 repo, or demo avoidance via user algorithm + M12 costmap |
| M15.5: rc.4 → 1.0.0 release protocol drift (WS/CLI) | Medium | Medium | Per-field diff script rerunnable; single `DORA_VERSION` constant to sync |
| M15.5: dora-moveit2/dora-mujoco incompatible with dora 1.0 Python API | Medium | High | Old venv (0.5) kept as fallback; adapt in dora-moveit2 repo if needed |
| M15.6: tonic/h2 won't compile on Rust 1.75 | Low | High | Precheck first (`cargo add tonic`); fall back to Jaeger query mode and record |
| M16: dorobot LeRobot metadata drift | Low | Low | Fixture test pins v2 layout |
