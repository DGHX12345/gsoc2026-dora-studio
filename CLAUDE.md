# gsoc2026-dora-studio

GSoC 2026 project under the dora-rs org: a GUI tool for visualizing and
debugging DORA applications.

## Student

- handle: DGHX12345
- weekly_slot: Sundays 18:30-19:00 America/Los_Angeles
- student_fork: https://github.com/DGHX12345/gsoc2026-dora-studio

## Mentor

- handle: bobdingAI

## Proposal

The candidate's accepted proposal is the authoritative source for scope
and weekly goals. The PDF is shared privately with the student and
mentor (not committed to this public repo). Quote relevant excerpts in
issues and discussions when needed.

## Milestones

- `Community Bonding` — ends 2026-05-24 23:59 UTC
- `Coding Phase 1` — ends 2026-07-10 23:59 UTC (Midterm evaluation deadline)
- `Coding Phase 2` — ends 2026-08-24 23:59 UTC
- `Final Submission` — standard 2026-08-24 23:59 UTC; extension to 2026-11-02 in approved cases.

## Workflow

- Student forks this repo and opens PRs against `main`.
- Mentor (bobdingAI) reviews and merges. No auto-merge.
- Weekly meeting notes live in GitHub Discussions, category `Weekly Sync`.
- Per-meeting workflow: `/prep` → meeting → `/log` (run from this folder).

---

## AI Onboarding Guide (added 2026-08-11)

### What This Project Is

dora-studio is a **local web-based GUI** for visualizing, running, and debugging [dora-rs](https://github.com/dora-rs/dora) applications. It is NOT a replacement for the dora CLI — it's a companion Studio that lets you see dataflow structure, runtime status, logs, and robot visualization in one browser window.

### The Two-Part Architecture

- **Part 1 — Dora Core (pages 01-03):** Dashboard, Dataflow Explorer, Run & Monitor, Logs & Events. Observability centered on dora dataflows.
- **Part 2 — Robot Tools (pages 05-06):** Motion Planner, Visualization/dviz. Companion tools for robot visualization.

### Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust, Axum 0.6, Tokio |
| Frontend | Vue 3 (composition API), Vite, TypeScript |
| Styling | Plain CSS with CSS variables, no UI framework |
| 3D Viewer | Three.js (via custom `NanoRobotViewer.vue`) |
| Dora CLI | `dora` binary on PATH (v0.5+) |
| Models | MuJoCo XML + STL assets in `models/` |

### Repository Layout

```
backend/src/
  main.rs           Route definitions + handler logic
  models.rs         All serde response types
  dataflows.rs      YAML discovery, parsing, graph generation
  coordinator.rs    dora CLI wrapper: dora list --format json
  runtime.rs        dora run subprocess manager + log capture
  daemon.rs         dora daemon subprocess manager (Week 12-B)
  external.rs       dviz/moveit status checks, robot profile, snapshots
frontend/src/
  App.vue           Shell: sidebar nav + topbar + page switching
  api.ts            All backend API calls (with fallback pattern)
  i18n.ts           zh/en locale strings
  types.ts          ViewId type
  styles.css        All global styles (~3700 lines)
  components/
    DashboardView.vue        System overview + quick start
    DataflowExplorer.vue     YAML graph + source viewer
    RunMonitorView.vue       Start/stop dataflows + node table
    LogsEventsView.vue       Three-level log viewer + raw stream
    VisualizationView.vue    dviz-style 3D viewport + base controls
    MotionPlannerView.vue    Arm joint editor + planning UI
    NanoRobotViewer.vue      Shared Three.js MuJoCo robot viewer
  data/mockStudio.ts         Type definitions (mock data values deleted)
examples/         Example dora dataflows
models/           MuJoCo XML + STL for Nano robot
plans/            Weekly planning (local only, not upstream)
```

### What's Real vs What's Not

**Real Dora Integration:**
- Dataflow discovery: filesystem scan of `examples/` for YAML
- Graph generation: custom YAML parser → nodes/edges/inputs/outputs
- Runtime control: `dora run <path>` subprocess (runtime.rs)
- Runtime logs: subprocess stdout/stderr captured into ring buffer
- Coordinator status: `dora list --format json` CLI call
- Daemon control: `dora daemon` subprocess (daemon.rs, Week 12-B)
- Status cross-reference: dataflow list + graph nodes show running/stopped in real time
- dviz/moveit status: binary/package existence checks

**Unavailable (explicitly marked, never faked):**
- Per-node CPU/memory/pending: requires dora daemon WebSocket (not yet integrated)
- Live topic data streaming: requires Zenoh or WebSocket
- Lifecycle events: require coordinator WebSocket protocol

**Placeholders (clearly labeled):**
- Motion Planner: Plan/Execute/Stop disabled; read-only MuJoCo mirror
- Visualization: base controls local preview only; no data publishing

### Current Worktree: week12-b

Branch `worktree-week12-b`, based on `week11-shared-nano-dviz-base-control`.

**Week 12 changes:**
- Deleted `backend/src/mock.rs`
- system_status returns "unavailable" instead of fake data
- dataflow_nodes no longer injects fake CPU/memory values
- dataflow_logs cross-references with runtime
- dataflows and graph endpoints cross-reference status with runtime/coordinator
- All four core pages: mock fallbacks removed, YAML source viewer added
- Sidebar restructured with Dora/Robot section labels and separators
- Dark/light theme overhauled: CSS variables, glassmorphism, hairline borders
- Collapsible `<details>` for advanced sections (File Info, Raw Stream, Runtime Metrics)
- Dashboard Quick Start panel with "Start dora daemon" button
- Backend daemon manager: spawns/stops dora daemon on demand
- AppState struct holding both RuntimeManager and DaemonManager

### Key Conventions

1. Chinese for direct chat; English for GitHub (commits, PRs, discussions)
2. No Claude/AI co-author signatures on commits or PRs
3. plans/ files local only — never in upstream PRs
4. Never fake data — show unavailable explicitly
5. Observe first, edit later
6. Motion Planner and Visualization pages stay stable
7. `dora` CLI must be on PATH for runtime features

### How to Run

```bash
# Backend (terminal 1)
cd <worktree-root>
cargo run --manifest-path backend/Cargo.toml

# Frontend (terminal 2)
cd <worktree-root>
npm --prefix frontend run dev

# Browser → http://localhost:5173
```

Backend on `127.0.0.1:3001`. Override: `DORA_STUDIO_BACKEND_ADDR=127.0.0.1:4001`.

### Validation

```bash
cargo fmt --manifest-path backend/Cargo.toml --check
cargo test --manifest-path backend/Cargo.toml          # 15/15
npm --prefix frontend run build                         # vue-tsc + vite
```

### API Surface

```
GET    /api/health
GET    /api/system/status
GET    /api/coordinator/status
GET    /api/daemon/status
POST   /api/daemon/start
POST   /api/daemon/stop
GET    /api/dviz/status, /topics, /displays, /snapshot
GET    /api/robot/profile
GET    /api/moveit/status, /snapshot
GET    /api/dataflows
GET    /api/dataflows/:id/definition, /graph, /nodes, /logs
POST   /api/dataflows/:id/start, /stop, /restart
GET    /api/runtime/status, /logs
POST   /api/runtime/start, /stop
```

### Key Architecture Decisions

1. **CLI-backed, not WebSocket-backed** — backend calls `dora` CLI as subprocesses. dora coordinator has a `/api/control` WebSocket with JSON-RPC but Studio hasn't integrated it yet. This is the biggest architectural gap.
2. **Single Axum State** — `Arc<AppState>` holds `RuntimeManager` + `DaemonManager`.
3. **Fallback API pattern** — `withFallback<T>(path, fallback)` returns `{ data, source, error }`.
4. **No vue-router** — pages switched via `v-if` on reactive `activeView` ref in App.vue.
5. **Polling** — Dashboard 5s, Logs 1.2s. No WebSocket/SSE on frontend.

### Dora Source Reference

Core source at `/home/dora/dora`. Key files for WebSocket integration:
- `binaries/coordinator/src/ws_server.rs` — WebSocket `/api/control`
- `libraries/message/src/ws_protocol.rs` — JSON-RPC types
- `libraries/message/src/cli_to_coordinator.rs` — List, Start, Stop, LogSubscribe, GetNodeInfo
- `binaries/daemon/src/lib.rs:330-500` — per-node metrics via sysinfo
- `binaries/daemon/src/log.rs` — per-node JSONL logs
- `binaries/coordinator/src/events.rs` — lifecycle event bus

### Design Principles

- **Simple for newcomers** — Dashboard shows status at a glance; progressive disclosure via collapsible sections
- **Powerful for developers** — drill down to graph → node metrics → topic data → per-node logs
- **科技感 (tech feel)** — Linear/Vercel-inspired dark theme, glassmorphism, restrained color, hairline borders
- **No fake data** — all placeholder/unavailable content explicitly labeled
