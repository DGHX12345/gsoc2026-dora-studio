# M06: 3D Viewport Time Synchronization

**Layer**: 2 — .drec Replay
**Depends on**: M05 (timeline UI), existing NanoRobotViewer
**Effort**: 1 week

## Purpose

When the user scrubs the replay timeline, the Three.js 3D viewport
renders the corresponding frame: TF transforms, point clouds, joint
angles, and planned paths all at the same timestamp.

## Deliverables

### D1: Time-synchronized data model

New file: `frontend/src/replay-scene.ts`

- `ReplayScene` class: holds all visualizable data at a given timestamp
- Data layers:
  - `tfTree: TfTree` — coordinate frame transforms (base_link → camera → end_effector)
  - `pointClouds: Map<string, PointCloud>` — per-topic point cloud snapshots
  - `jointStates: Map<string, JointState>` — per-joint-group angles
  - `paths: Map<string, PlannedPath>` — planned trajectories from planner nodes
  - `images: Map<string, ImageData>` — camera images as textures
- `clear()` — wipe all layers
- `loadFromEntries(entries: RecordEntry[])` — populate from recording entries

### D2: TF tree parser

- Parse Arrow batches containing TF transform messages
- Build transform tree: parent → child with `(translation, rotation)`
- Apply to Three.js scene: update `Object3D.position` and `.quaternion` for each frame
- Interpolation: if timestamp is between two TF samples, lerp/slerp

### D3: Point cloud renderer

- Parse Arrow batches containing point cloud data (x, y, z, intensity/rgb)
- Create/update Three.js `Points` or `PointCloud` objects
- Color mapping: intensity → grayscale, or RGB if available
- Performance: >100k points at ≥30fps (use BufferGeometry + InstancedMesh fallback)

### D4: Joint state → robot pose

- Parse Arrow batches containing joint angles (float array)
- Apply to existing NanoRobotViewer: update MuJoCo model joint positions
- Interpolation between joint state samples for smooth animation

### D5: Viewport sync with timeline

- `ReplayScene` subscribes to `PlaybackEngine.onTimeChange`
- On timeline change:
  1. Fetch entries at new timestamp: `GET /api/recording/:id/entries?timestamp=T`
  2. Parse entries into ReplayScene layers
  3. Update Three.js scene objects
  4. Render frame
- Target: 200ms end-to-end from scrub to render
- During playback: skip frames if rendering can't keep up (drop frames, not lag)

### D6: Split viewport mode

Update existing VisualizationView.vue:
- New mode toggle: "Live" | "Replay"
- Live mode: existing behavior (local preview)
- Replay mode: viewport driven by PlaybackEngine
- Both modes share the same NanoRobotViewer instance

## Acceptance Criteria

- [ ] Scrub timeline → robot pose in 3D viewport updates within 200ms
- [ ] Point cloud renders at ≥30fps with 100k+ points
- [ ] TF tree correctly positions robot links (base → arm → end_effector)
- [ ] Joint interpolation produces smooth animation during playback
- [ ] Switching Live/Replay mode preserves viewport camera position
- [ ] Multiple data types (TF + point cloud + joint state) all sync to same timestamp

## Exposed Interfaces

```typescript
// frontend/src/replay-scene.ts
export class ReplayScene {
  constructor(scene: THREE.Scene);
  
  // Data loading
  loadFromEntries(entries: RecordEntry[]): void;
  clear(): void;
  
  // Layer access
  readonly tfTree: TfTree;
  readonly pointClouds: Map<string, THREE.Points>;
  readonly jointStates: Map<string, JointState>;
  
  // Render update
  updateFromTimestamp(timestampNs: number): Promise<void>;
  
  // Cleanup
  dispose(): void;
}
```

```typescript
// Integration with PlaybackEngine (M05)
// In component:
const engine = new PlaybackEngine(recordingId);
const scene = new ReplayScene(threeScene);

engine.onTimeChange = async (timestamp) => {
  await scene.updateFromTimestamp(timestamp);
};
```
