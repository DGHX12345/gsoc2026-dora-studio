# M12: dviz Path Visualization Tool

**Layer**: 5 — Tool Slot System
**Depends on**: M11 (tool slot protocol), M06 (3D viewport)
**Effort**: 1.5 weeks

## Source Audit Summary

dviz is a **visualization consumer**, not a planner. It subscribes to dora
ports published by external planner nodes (e.g., `simple_planner.py`).

**Actual port names** (from `dviz-rerun-bridge/src/main.rs`):
| Port pattern | Data format | Zenoh topic |
|---|---|---|
| `waypoints` / `path` (len >= 4) | Flat `Float32Array` xy pairs | `dviz/world/planner/waypoints` |
| `trajectory` (len >= 6) | Flat `Float32Array` stride 3 (xyz) or stride 7 (xyz+qxyzw) | `dviz/world/trajectory` |
| `target_point` / `target` / `goal` | Flat `Float32Array` xy | `dviz/world/planner/target` |
| `pose` / `robot_pose` (== 7) | `[x,y,z,qx,qy,qz,qw]` | `dviz/world/robot/body` |

**Critical: no costmap support.** Costmap is on dviz's wishlist but not
implemented. We will build a basic ESDF costmap renderer from raw float data.

**Critical: all data is flat Float32Array, not structured Arrow.** No
structs, no tables, no nested types. Dimensions are implicit.

## Revision 2026-08-13 (with student, before implementation)

- **R1 interface signatures**: the "Exposed Interfaces" draft below predates
  the M11 protocol revisions. Actual interface (frontend/src/tools/types.ts):
  `onAttach(context: ToolContext)` (R9), `onBatch(batch: ToolBatch, tf?: TfTree)`
  (R1/R3), `panelComponent?: Component` bound in `registerBuiltinTools` (R11),
  plus required `category: ToolCategory`.
- **R2 line width**: core three `linewidth` is ignored on most platforms
  (always 1px). Use `Line2`/`LineMaterial` from `three/examples/jsm`
  (precedent: OrbitControls/STLLoader). Primary path: solid cyan 2px;
  alternative paths: dashed 2px (LineMaterial dashed, 30% opacity).
- **R3 costmap payload**: one .drec entry = one payload, so costmap is a
  single JSON object `{ width, height, resolution, values: number[] }`
  (objects pass through feed.ts as `json`; no separate metadata entry).
- **R4 D5 planner status scoped down**: the viewport is replay-only (no live
  data source); M03 node-status polling lives on DataflowCanvas. M12 shows
  data freshness only (batch timestamp vs replay timeline → "stale" badge).
  Runtime node status deferred to M13 live integration.
- **R5 snap camera**: NanoRobotViewer gains `defineExpose focusOn(center,
  radius)` that syncs the OrbitControls target (avoids snap-back on next
  drag). Wiring: ToolContext gains an OPTIONAL `focusOn?: (center, radius)
  => void` field (additive protocol change); VisualizationView passes the
  viewer's method when attaching tools. Tools that don't need it ignore it.
- **R6 WaypointEchoTool removal**: after DvizPathTool passes manual
  acceptance, delete `tools/demo/WaypointEchoTool.ts` +
  `WaypointEchoPanel.vue` and its registration.
- **R7 panel reactivity**: batch broadcast does not re-render panels (the
  reference panel's `stats()` computed never refreshes). DvizPathTool
  exposes `subscribe(listener)`; DvizPathPanel subscribes onMounted and
  unsubscribes onBeforeUnmount.
- **R8 dataflow-YAML recommendation (D5)**: reuse `getDataflows` +
  `getDataflowGraph` (both exist in api.ts) to feed `findRecommendations`
  with node outputs when no replay is loaded; .drec streams remain the
  replay-mode source (M11 R10).
- **R9 subscribePorts**: four patterns (D1's three + `costmap|esdf`).
  Distinct per-node colors: cyan (first planner), magenta, orange.
- **R10 demo data**: extend `generate_tool_demo` with a `target_point`
  stream and a `costmap` stream (deterministic JSON object per R3), keeping
  existing stream assertions intact.
- **R11 world-frame rendering**: dviz world topics are already world-frame
  and path payloads carry no frame metadata on the wire. M12 renders path
  data as-is; the `tf` argument stays reserved for M13 (moveit).

## Deliverables

### D1: Flat Float32Array path parser

Arrow port subscription (corrected):
```typescript
subscribePorts: [
  { nodeIdPattern: /.*/, outputIdPattern: /^(waypoints|path)$/i },
  { nodeIdPattern: /.*/, outputIdPattern: /^trajectory$/i },
  { nodeIdPattern: /.*/, outputIdPattern: /^(target_point|target|goal)$/i },
]
```

Parser logic:
- `waypoints`: flat `[x1,y1, x2,y2, ...]` → pairs → 3D linestrip (z=0.05)
- `trajectory`: stride 3 → `[x,y,z, ...]` points; stride 7 → skip quaternion
- `target_point`: `[x, y]` → sphere marker at (x, y, 0.05)
- All: cast Float32Array directly (no Arrow struct decode needed)

### D2: Path renderer

New file: `frontend/src/tools/dviz/DvizPathTool.ts`

Implements `ViewportTool`:
- `onAttach(scene)`: create path line objects, start/end markers, costmap plane
- `onBatch(batch, tf)`: parse flat f32 data, apply TF transforms, update lines
- `onTimelineSeek(ts)`: show paths for this timestamp
- `onDetach()`: remove all path objects from scene

Rendering:
- Primary path (first `waypoints` or `trajectory`): solid cyan line with arrow
  direction markers every N waypoints. Line width 2px via Line2/LineMaterial.
- Alternative/candidate paths: semi-transparent dashed white lines (30% opacity)
  - Multiple `waypoints` outputs from same node = multiple paths
  - First = primary (cyan), rest = alternatives (white dashed)
- Start marker: green sphere; End marker: red sphere
- Target marker: pink sphere (from `target_point`)

Performance: 5+ paths at ≥30fps. Use InstancedMesh for arrow markers.

### D3: Costmap renderer (built from scratch)

Since dviz has no costmap, build a simple ESDF costmap from float array data:

- Subscribe to `costmap` / `esdf` ports if available (fallback: synthetic grid
  for testing)
- Data format: flat Float32Array `[v1, v2, ..., vN]` + metadata `{width, height, resolution}`
- Render as semi-transparent textured plane in Three.js
- Color ramp: blue (low cost / free space) → yellow (medium) → red (high cost / obstacle)
- Opacity slider (0-100%) in control panel
- Toggle show/hide
- Resolution target: ≥100x100 cells

### D4: Path control panel

Sidebar panel when tool is attached:
- Path list: name (from node_id + output_id) + waypoint count + cost (if available)
- Toggle per-path visibility (eye icon)
- Costmap opacity slider
- Costmap toggle
- "Snap camera to path" button
- Path info: total length, waypoint count

### D5: Planner node integration

- Watch for nodes with output ports matching `waypoints|path|trajectory`
- Auto-recommend dviz tool when detected
- Multiple planners → distinct colors (cyan, magenta, orange)
- Planner node status from M03 reflected in panel:
  - Running: paths update live
  - Crashed: last-known paths shown with "stale" badge
- Synthetic data generator for dev/testing (when no planner running):
  Generate figure-8 waypoints + random costmap grid

## Acceptance Criteria

- [ ] Correctly parse flat Float32Array `waypoints` (xy pairs) → 3D linestrip
- [ ] Correctly parse `trajectory` stride-3 and stride-7 formats
- [ ] 5+ candidate paths render at ≥30fps
- [ ] Costmap renders at ≥100x100 resolution, opacity slider works
- [ ] Timeline seek: paths update within 200ms
- [ ] Tool auto-recommends when matching port detected in dataflow
- [ ] Multiple planner nodes render with distinct colors
- [ ] Works with synthetic data (no real planner required for dev)
- [ ] `npm run build` passes

## Exposed Interfaces

```typescript
// frontend/src/tools/dviz/DvizPathTool.ts
// (signatures corrected per Revision R1 — supersedes the M11-era draft)
import type { Component } from 'vue';

export class DvizPathTool implements ViewportTool {
  readonly id = 'dviz-path';
  readonly displayName = 'dviz Path Visualization';
  readonly category = 'planning';
  readonly subscribePorts = [
    { nodeIdPattern: /.*/, outputIdPattern: /^(waypoints|path)$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^trajectory$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^(target_point|target|goal)$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^(costmap|esdf)$/i },
  ];

  onAttach(context: ToolContext): void;
  onBatch(batch: ToolBatch, tf?: TfTree): void;
  onTimelineSeek?(timestampNs: number): void;
  onDetach(): void;
  panelComponent?: Component; // bound in registerBuiltinTools (R11)
}
```
