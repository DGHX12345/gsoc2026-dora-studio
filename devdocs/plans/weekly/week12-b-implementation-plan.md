# Week 12-B: UI Restructure & Visual Overhaul

## Goal

Reorganize the Studio UI so the two-part architecture (Dora core + Robot tools) is immediately visible, make components understandable to first-time users, hide advanced features behind collapsible sections, and upgrade the visual language to a modern developer-tool aesthetic.

## References

- **Linear** — engineering-first minimal, deep navy backgrounds, hairline borders
- **Vercel** — dark-first, high-contrast
- **Grafana** — layered dashboard architecture
- **Redpanda Console** — developer-friendly sidebar navigation

---

## Phase 1: Sidebar Restructure

**File: `App.vue`**

Change from flat 6-item `v-for` to grouped rendering:

```
  Dashboard (独立，突出)
  ─────────────────────
  Dora                    ← section label
    Dataflow Explorer
    Run & Monitor
    Logs & Events
  ─────────────────────
  Robot                   ← section label
    Motion Planner
    Visualization
  ─────────────────────
  [Theme toggle]          ← bottom tools
  [Coordinator status]
```

- Add `group: 'overview' | 'dora' | 'robot'` to nav item data
- Template renders three `<nav-section>` blocks with labels and separators
- Dashboard item rendered separately at top with visual prominence
- Theme toggle moved from middle of nav to bottom area

**File: `i18n.ts`**

- Add `sections` translations: `{ overview: 'Overview', dora: 'Dora', robot: 'Robot' }`
- Update `nav` labels to match

**File: `styles.css`**

- `.nav-section-label` — uppercase, small, muted, with hairline separator line
- `.nav-item` — refined hover/active states with left accent bar
- `.nav-item.primary` — Dashboard gets subtle extra visual weight
- `.sidebar-footer` — existing, keep as-is

---

## Phase 2: Dark/Light Theme Overhaul

**File: `styles.css`**

### Light mode (refine, don't replace)

- Background: `#F8FAFC` (cleaner cool gray-white)
- Cards: `rgba(255,255,255,0.80)` + `backdrop-filter: blur(12px)` (glass)
- Borders: `rgba(0,0,0,0.06)` (hairline)
- Accent: `#3B82F6` (keep current blue)

### Dark mode (rebuild, Linear-inspired)

- Background: `#080C14` (deep navy, never pure black)
- Cards: `rgba(15,23,42,0.80)` + `backdrop-filter: blur(12px)` (glass)
- Borders: `rgba(255,255,255,0.06)` (hairline)
- Accent: `#5E6AD2` (blue-violet, sparingly)
- Text: `#F1F5F9` primary, `#94A3B8` secondary
- Sidebar: `#060910` (darker than main, solid)
- Terminal panel: `#050810` (darkest, solid)

### Shared visual upgrades

- `.metric-card` — glass effect: translucent bg + backdrop-blur + hairline border
- `.status-light.online` — glow pulse: `box-shadow: 0 0 8px currentColor`
- `.graph-node` — glass bg + hairline border
- `table` — dark headers, hairline row separators
- Numbers/metrics — add `.mono` utility class for JetBrains Mono
- `.pill/.status-chip` — semi-transparent bg, borders instead of solid fills
- Transitions — smooth `transition` on hover states (160-200ms ease)

### Remove/deprecate

- Heavy `box-shadow` on cards (replace with hairline border + subtle backdrop)
- Solid white `#FFFFFF` card backgrounds (replace with glass)
- Bright color saturation on status chips

---

## Phase 3: Collapsible Sections

Use native `<details>` + `<summary>` elements for zero-JS progressive disclosure.

| Page | Section | Default | File |
|------|---------|---------|------|
| Dataflow Explorer (left) | Parser Diagnostics | Collapsed (advanced) | DataflowExplorer.vue |
| Dataflow Explorer (right) | Runtime Metrics note | Collapsed (unavailable) | DataflowExplorer.vue |
| Logs & Events | Raw Stream terminal | Collapsed (advanced) | LogsEventsView.vue |

`<details>` styled with custom disclosure triangle, smooth open animation via `details[open]` selector.

### Component clarity labels

| Location | Current | New |
|----------|---------|-----|
| Dashboard dviz card title | `dviz` | `3D Visualization (dviz)` |
| Dashboard moveit card title | `dora-moveit2` | `Motion Planning (moveit)` |
| Explorer diagnostics header | `Parser Diagnostics` | `File Info` |
| Logs terminal header | `Raw Stream` | `All Logs (Raw Output)` |
| Monitor metrics pill | `Metrics unavailable` | `Requires dora daemon` |

---

## Phase 4: Sidebar Status Bar

**File: `styles.css`** (micro-refinement)

- Keep existing coordinator status indicator
- Update text labels to be clearer: "Dora connected" / "No connection" / "Dataflow running"
- Subtle glass treatment matching new card style

---

## Validation

```bash
npm --prefix frontend run build              # PASS required
cargo fmt --manifest-path backend/Cargo.toml --check   # PASS required
cargo test --manifest-path backend/Cargo.toml           # 15/15 required
```

Manual browser check:
- [ ] Sidebar shows grouped structure with section labels
- [ ] Dashboard is first and visually distinct
- [ ] Dark mode has deep navy background, glass cards, hairline borders
- [ ] Light mode still works with refined colors
- [ ] Collapsible sections open/close smoothly
- [ ] Motion Planner and Visualization pages unchanged
- [ ] Theme toggle persists across page reloads

## Files touched (7)

| File | Scope |
|------|-------|
| `frontend/src/App.vue` | Sidebar template rewrite |
| `frontend/src/i18n.ts` | Section labels, nav item text |
| `frontend/src/styles.css` | Dark/Light theme, glass, sidebar, details |
| `frontend/src/components/DashboardView.vue` | Card titles, glass |
| `frontend/src/components/DataflowExplorer.vue` | Collapsible sections, labels |
| `frontend/src/components/LogsEventsView.vue` | Collapsible terminal |
| `frontend/src/components/RunMonitorView.vue` | Pill text |

## Files NOT touched

- All backend files
- `MotionPlannerView.vue`, `VisualizationView.vue`, `NanoRobotViewer.vue`
- `api.ts`, `mockStudio.ts`
