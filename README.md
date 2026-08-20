# dora-studio

A GSoC 2026 prototype for visualizing, running, and debugging DORA applications.

The current prototype focuses on a practical Studio workflow: discover local dataflows, inspect their graph structure, start and stop a selected dataflow, monitor runtime state, review logs, and mirror Nano Full robotics state through shared Visualization and Motion Planner views.

## Current Scope

Implemented areas:

- Dashboard with coordinator status, runtime summary cards, recent events, and roadmap panels.
- Dataflow Explorer for local `dataflow.yml` discovery, graph rendering, node inspection, and parser diagnostics.
- Run & Monitor page for selecting a discovered dataflow and controlling it through the backend runtime bridge.
- Logs & Events page with runtime log polling, log-level grouping, raw stream view, and connected/fallback states.
- Visualization page with a dviz-oriented 3D viewport layout, shared Nano Full robot viewer, and simulated local base controls.
- Motion Planner page with robot profile data, read-only MoveIt/MuJoCo snapshot summaries, shared Nano Full Three.js visual mirror, planning scene, trajectory preview, and IK solver panels.
- `models/` shared MuJoCo XML/STL assets used by the shared Nano robot viewer.
- Light/dark theme toggle with local preference persistence.

The Visualization and Motion Planner pages now share a Nano Full robot viewer foundation with different page responsibilities. Visualization owns the main dviz-style canvas and a local simulated base pose preview, while Motion Planner keeps the arm-focused preview and fixed base pose. Neither page streams real 3D data or executes motion planning commands.

## Repository Layout

```text
backend/   Rust Axum API server for dataflow discovery, runtime control, logs, and external tool status checks
frontend/  Vue 3 + Vite frontend for the Studio prototype
examples/  Example DORA dataflows used by the prototype
models/    Self-generated MuJoCo XML/STL assets used by the shared Nano robot viewer
```

## Prerequisites

Required:

- Rust toolchain with Cargo
- Node.js and npm
- DORA CLI available on `PATH` for runtime control

Optional:

- `dviz` for visualization status detection
- `dora-moveit2` for motion-planning status detection

The application remains usable without optional tools because the frontend shows fallback states when integrations are unavailable.

## Run Locally

Start the backend API server:

```bash
cargo run --manifest-path backend/Cargo.toml
```

The backend listens on `127.0.0.1:3001` by default. To use another address:

```bash
DORA_STUDIO_BACKEND_ADDR=127.0.0.1:4001 cargo run --manifest-path backend/Cargo.toml
```

Install frontend dependencies if needed:

```bash
npm --prefix frontend install
```

Start the frontend dev server:

```bash
npm --prefix frontend run dev
```

The frontend uses `http://127.0.0.1:3001/api` by default. To point it at a different backend API URL:

```bash
VITE_DORA_STUDIO_API_URL=http://127.0.0.1:4001/api npm --prefix frontend run dev
```

## Demo Walkthrough

Recommended walkthrough order:

1. Open Dashboard to show the overall Studio concept, coordinator status, recent events, and future workflow panels.
2. Open Dataflow Explorer to select a discovered dataflow and inspect its graph, nodes, edges, and diagnostics.
3. Open Run & Monitor to start the selected dataflow, refresh runtime status, and inspect node metrics.
4. Open Logs & Events to review grouped info, warning, and error logs from the runtime API or fallback data.
5. Open Visualization to inspect backend-provided dviz topics/displays, select topics, filter metadata, toggle displays locally, refresh the snapshot, and verify the shared Nano Full viewer plus local base preview updates.
6. Open Motion Planner to show the profile-bound dora-moveit2 control surface, read-only MoveIt snapshot, shared Nano Full MuJoCo visual mirror, scene objects, trajectories, IK, and disabled execution controls.
7. Use the sidebar theme toggle to verify light and dark presentation.
8. Use Export report to download a Markdown snapshot of the current prototype state.

## Backend API Surface

```text
GET    /models/*
GET    /api/health
GET    /api/system/status
GET    /api/coordinator/status
GET    /api/dviz/status
GET    /api/dviz/topics
GET    /api/dviz/displays
GET    /api/dviz/snapshot
GET    /api/robot/profile
GET    /api/moveit/status
GET    /api/moveit/snapshot
GET    /api/dataflows
GET    /api/dataflows/:id/definition
GET    /api/dataflows/:id/graph
GET    /api/dataflows/:id/nodes
GET    /api/dataflows/:id/logs
POST   /api/dataflows/:id/start
POST   /api/dataflows/:id/stop
POST   /api/dataflows/:id/restart
GET    /api/runtime/status
GET    /api/runtime/logs
POST   /api/runtime/start
POST   /api/runtime/stop
GET    /api/palette
GET    /api/projects/list
POST   /api/projects/add
POST   /api/projects/delete
POST   /api/projects/nodes
GET    /api/types/catalog
GET    /api/types/:urn
POST   /api/schema/check
POST   /api/dataflows/:id/save
POST   /api/dataflows/save-as
```

## Project Directories (Dataflow Explorer 2.0)

The Dataflow Explorer scans the built-in `examples/` directory plus any
project directories you configure. Add a directory from the Explorer's
Source tab ("Add project directory"); it is persisted in
`~/.config/dora-studio/settings.json` under `projectDirs`.

Nodes discovered in your dataflow YAML files populate the node palette,
including port type URNs (`input_types` / `output_types`). Edge
compatibility follows dora 1.0 semantics (widening, `type_rules`,
structural struct checks) and saving runs `dora validate` as a final
check. Write-back edits only touch node blocks and `type_rules`; all
other content (comments, `env`, etc.) is preserved, and a backup is
written to `~/.config/dora-studio/backups/` before each write-back.

Manual nodes (nodes without a dataflow YAML) can be submitted from the
palette; they are stored in `manualNodes` in the same settings file and
are marked as manually declared in the UI.

## Validation

Run the standard checks before preparing a PR:

```bash
cargo fmt --manifest-path backend/Cargo.toml --check
cargo test --manifest-path backend/Cargo.toml
npm --prefix frontend run build
```

For UI changes, also run the backend and frontend locally, then walk through the Studio pages in a browser and confirm the shared Nano Full viewer behaves correctly in both Motion Planner and Visualization.

## Current Limitations

- Runtime control still uses a local `dora run` subprocess bridge.
- Coordinator integration currently uses CLI-backed status queries rather than a direct WebSocket client.
- Node CPU, memory, and pending-message values are still prototype metrics.
- dviz status detection is process/install-level; real Zenoh data forwarding is planned for Phase 2.
- dora-moveit2 status and snapshot data are read-only metadata/demo boundaries; real planning and execution endpoints are planned for later Phase 2 work.
- The Nano Full viewer loads MuJoCo XML/STL assets for browser-side visualization only; Studio does not run MuJoCo physics or own simulation state.
- The current graph parser supports the subset of DORA YAML needed by the example flows and reports diagnostics for unsupported top-level sections.
