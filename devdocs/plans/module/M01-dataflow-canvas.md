# M01: Dataflow Editor Canvas

**Layer**: 1 — Dataflow Live Graph
**Depends on**: M00 (for protocol types)
**Effort**: 2 weeks

## Purpose

Build the visual drag-drop dataflow editor. Users create nodes on a
canvas, connect ports with edges, and the editor generates valid dora
YAML. This is the centerpiece of the v2.0 Studio experience.

## UI Design Language

Research from n8n, Foxglove Studio, and Linear/Vercel informs every
visual decision. Our aesthetic is **"machined dark"** — surgical,
dense, and calm, with a single chromatic accent (cyan #00d4ff).

### Surface Hierarchy (Linear-inspired)

```
Canvas base:     #08080d  (near-black with blue tint — never pure #000)
Panel surface:   #0f0f14  (first elevation step)
Card surface:    #16161d  (node cards, elevated panels)
Hover surface:   #1c1c24  (hover state)
Active surface:  #22222d  (selected / focused)
```

Depth is conveyed through **color-stepping only**. No drop shadows.
Cards use 1px hairline borders: `#1a1a2e` default, `#2a2a5e` on hover,
cyan `#00d4ff` at 30% opacity when focused.

### Typography

- System UI font stack (-apple-system, "Segoe UI", "Noto Sans SC", sans-serif)
- Monospace stack for code/YAML: "JetBrains Mono", "Fira Code", monospace
- Body text: `#b0b0c0` — never pure white at reading sizes
- Headers: `#e0e0f0` — slightly brighter, but still restrained
- Node names: `#d0d0e0` at 13px, weight 510 (if Inter available)

### Spacing & Radius

- Base grid: 4px
- Node card radius: 8px
- Panel radius: 12px
- Button radius: 6px standard, 9999px for primary CTA pills
- Port dots: 10px diameter circles (8px inner + 1px border)
- Connection line stroke: 2px

### Color Semantics

- Cyan `#00d4ff` — primary accent (focus rings, selected nodes, active ports)
- Green `#22c55e` — running / success / schema-compatible
- Red `#ef4444` — crashed / error / schema-incompatible
- Yellow `#f59e0b` — warning / unknown schema / reloading
- Gray `#6b7280` — inactive / disabled / unknown status
- Electric gradient `#00d4ff → #7c3aed` — connection lines (n8n-inspired)
  Applied via SVG linearGradient on edge path strokes

### Special Effects

- **Spotlight glow**: on node card hover, a radial gradient follows the cursor
  (CSS custom properties `--mouse-x`, `--mouse-y`, zero JS overhead)
- **Noise texture**: SVG `<feTurbulence>` overlay at 2% opacity on the canvas
  background — breaks sterile flat-color feel (Linear-inspired)
- **Focus ring**: `0px 0px 0px 1px #00d4ff` at 33% opacity, 0px blur
  (Linear ring technique — sharp, machined, not a fuzzy glow)
- **Grid**: subtle dot grid at 24px spacing, color `#ffffff` at 3% opacity

## Deliverables

### D1: Three-region layout

Replace the current DataflowExplorer page with a full-window layout:

```
┌──────────┬────────────────────────────────────┐
│  Node    │  Toolbar (Run | Stop | Save | ...) │
│  Palette │────────────────────────────────────┤
│          │                                    │
│  search  │                                    │
│  [====]  │        Dataflow Canvas             │
│          │        (SVG-based)                 │
│  camera  │                                    │
│  planner │     dot grid background            │
│  control │     nodes + bezier edges           │
│  ...     │                                    │
│          │                                    │
│  drag→   │                                    │
│          │                                    │
├──────────┴────────────────────────────────────┤
│  Status bar (node count | edge count | YAML valid/invalid) │
└────────────────────────────────────────────────────────────┘
```

Left palette: 240px wide, collapsible. Shows available node types
grouped by category (perception, planning, control, llm, hardware).

### D2: Canvas component

New file: `frontend/src/components/DataflowCanvas.vue`

SVG-based (not Canvas2D) — better for accessibility, hit testing, CSS
styling, and DOM integration with Vue.

- Pan and zoom: `transform: translate(tx, ty) scale(s)` on a `<g>` wrapper
  - Wheel zoom centered on cursor position
  - Trackpad pinch-to-zoom
  - Middle-mouse drag to pan
- Dot grid background: SVG `<pattern>` at 24px, responsive to zoom scale
- Connection lines: SVG `<path>` with cubic bezier curves
  - Gradient stroke: cyan `#00d4ff` → purple `#7c3aed`
  - Arrow markers at target end
  - Hover: path widens to 3px, shows delete button at midpoint
- Node cards (foreignObject or pure SVG):
  ```
  ┌─────────────────────┐
  │ ○ in_1    camera ●  │  ← left dots = inputs, right dots = outputs
  │ ○ in_2   [Python] ● │  ← language badge (colored tag)
  │           out_1    ● │
  └─────────────────────┘
  ```
  - Port dots: left side (inputs, `#22c55e`), right side (outputs, `#00d4ff`)
  - Port dot hover: scales 1.3x, ring appears
  - Language badge: 4px pill, colored by runtime (Python=blue, Rust=orange, C++=purple)
  - Spotlight glow on hover
- Selection states:
  - Node: cyan ring on focus
  - Edge: path highlights
  - Multi-select via shift-click or drag-select rectangle

### D3: Node palette

New file: `frontend/src/components/NodePalette.vue`

- Search bar at top (filters nodes by name/category, <100ms response)
- Category groups with collapsible headers:
  - Perception (camera, lidar, object_detection)
  - Planning (planner, path_follower)
  - Control (controller, joint_trajectory)
  - LLM (vlm_node, llm_node)
  - Hardware (camera_driver, motor_driver)
- Each entry: icon + name + brief description (one line)
- Drag from palette → drop on canvas (HTML5 drag-drop API)
- Node catalog source: scan `examples/` YAML + built-in registry

### D4: YAML generation engine

New file: `backend/src/dataflow_builder.rs`

- `DataflowBuilder` accumulates nodes, edges, env, and metadata
- `add_node(id, operator_id, config)` — add a node definition
- `connect(src_node, src_port, dst_node, dst_port)` — add a connection
- `to_yaml() -> String` — serialize to valid dora YAML format
- `from_yaml(yaml) -> Result<Self>` — deserialize, preserving positions
- `validate() -> Result<(), Vec<BuildError>>` — check orphans, missing ports, cycles

API endpoints:
- `POST /api/dataflow/build` — body: JSON graph → returns YAML string
- `POST /api/dataflow/validate` — body: JSON graph → returns BuildError list
- `POST /api/dataflow/parse` — body: YAML string → returns JSON graph

### D5: YAML side panel

- Toggle between "Visual" and "YAML" modes (or split view)
- Split view: YAML on left ~35%, Canvas on right ~65% (resizable divider)
- Syntax highlighting: keywords (nodes, inputs, outputs, env) in cyan,
  values in white, comments in gray
- Bidirectional sync: canvas edit → YAML regenerates; YAML edit → canvas
  rebuilds. Debounced at 300ms.
- Error gutter: red markers on lines with parse errors

### D6: Toolbar

```
[▶ Run] [■ Stop] [↻ Restart]  |  [New] [Open...] [Save] [Save As...]  |  [✓ Validate]
```

- Run/Stop/Restart: integrated with M03 runtime
- New/Open/Save: file operations on examples/ directory
- Validate: calls `/api/dataflow/validate`, shows error count badge
- All buttons: outlined style, 6px radius, 1px border
- Run button: pill shape (9999px), cyan accent — the only colored CTA

### D7: Status bar

- Left: node count, edge count
- Center: validation status (✓ Valid / ✗ N errors)
- Right: canvas zoom percentage, cursor position

## Acceptance Criteria

- [ ] Drag 3 nodes from palette to canvas, connect them, generate valid YAML
- [ ] Generated YAML passes `dora run` without errors
- [ ] Edit YAML text → canvas reflects changes within 300ms
- [ ] Pan and zoom smooth at 60fps with 50+ nodes on screen
- [ ] Save/Load roundtrip preserves all node positions and connections
- [ ] Node spotlight glow on hover (no layout jank)
- [ ] Connection lines use gradient stroke
- [ ] Port dots show green (connected) / gray (unconnected) states
- [ ] Dark theme surface hierarchy clearly distinguishes canvas/panels/cards
- [ ] `npm run build` passes (vue-tsc strict)

## Exposed Interfaces

```typescript
// DataflowCanvas.vue exposes:
interface DataflowCanvasExpose {
  getGraph(): DataflowGraph;
  loadGraph(graph: DataflowGraph): void;
  addNode(type: string, position: Vec2): void;
  clear(): void;
  zoomToFit(): void;
}

// Events emitted:
// @graph-changed(graph: DataflowGraph)
// @node-selected(nodeId: string)
// @edge-selected(edgeId: string)
// @node-double-clicked(nodeId: string)  // open properties panel
```

```rust
// backend/src/dataflow_builder.rs
pub struct DataflowBuilder { ... }
impl DataflowBuilder {
    pub fn new() -> Self;
    pub fn add_node(&mut self, spec: NodeSpec) -> &mut Self;
    pub fn connect(&mut self, edge: EdgeSpec) -> Result<&mut Self, BuildError>;
    pub fn remove_node(&mut self, id: &str) -> Result<(), BuildError>;
    pub fn remove_edge(&mut self, id: &str) -> Result<(), BuildError>;
    pub fn to_yaml(&self) -> String;
    pub fn from_yaml(yaml: &str) -> Result<Self, BuildError>;
    pub fn validate(&self) -> Result<(), Vec<BuildError>>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: String,
    pub operator_id: String,
    pub runtime: Runtime, // Python | Rust | C | Cpp
    pub inputs: BTreeMap<String, PortSpec>,
    pub outputs: BTreeMap<String, PortSpec>,
    pub position: Option<(f32, f32)>, // canvas coordinates
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EdgeSpec {
    pub id: String,
    pub source_node: String,
    pub source_port: String,
    pub target_node: String,
    pub target_port: String,
}
```
