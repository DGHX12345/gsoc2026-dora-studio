# M05: Timeline UI — Playback Controls & Multi-Stream Alignment

**Layer**: 2 — .drec Replay
**Depends on**: M04 (recording reader API)
**Effort**: 1.5 weeks

## Purpose

Build the time-axis playback control that is the heart of the .drec
replay experience. Users can play/pause/scrub/speed-change through
recorded data, with all streams synchronized by Arrow timestamp.

## Deliverables

### D1: Timeline component

New file: `frontend/src/components/ReplayTimeline.vue`

- Horizontal time axis with tick marks (auto-scale: seconds/minutes/hours)
- Current position indicator (red vertical line)
- Play/Pause button (spacebar shortcut)
- Speed selector: 0.5x / 1x / 2x / 5x
- Scrub handle: drag to arbitrary position
- Frame step buttons: forward/back 1 frame
- Timestamp display: current time / total duration (HH:MM:SS.mmm)
- Zoom: scroll wheel on timeline to expand/contract time scale
- Mini-map overview: compressed view of entire recording with activity indicators per stream

### D2: Multi-stream alignment

- All streams share one timeline, driven by Arrow timestamp
- `GET /api/recording/:id/entries?timestamp=T` returns entries from all streams at timestamp T (within 1 frame tolerance)
- Frontend fetches entries at current playback position
- Data panel updates: show per-stream data at current timestamp
- Alignment guarantee: all displayed data is from the same logical frame

### D3: Playback engine

New file: `frontend/src/playback.ts`

- `PlaybackEngine` class managing playback state machine
- States: Stopped | Playing | Paused | Seeking
- `play(speed)`: start RAF loop, advance `currentTime` by `speed * deltaTime`
- `pause()`: freeze currentTime
- `seek(timestamp)`: set currentTime, fire seek event
- `stop()`: reset to beginning
- Seek debounce: while dragging scrub handle, don't fetch; fetch on release
- Prefetch: when playing at 2x/5x, preload upcoming entries ahead of playback position

### D4: Stream panel grid

New component in DataflowExplorer or standalone page:
- Grid of per-stream mini panels, each showing:
  - Stream label (node_id / output_id)
  - Data type badge (Image, PointCloud, JointState, etc.)
  - Current value preview (thumbnail for images, text for scalars)
  - Sparkline/timeline indicator showing activity pattern
- Click a stream panel → expand to detail view
- Streams can be toggled on/off to reduce visual noise

### D5: Frame-accurate jump

- Text input: type timestamp (e.g., "1:23.456") → seek to exact frame
- Keyboard shortcuts: arrow keys = ±1 frame, shift+arrow = ±10 frames, ctrl+arrow = ±100 frames
- Bookmark system: mark interesting timestamps, jump between them

## Acceptance Criteria

- [ ] 1GB+ .drec: timeline scrubbing responsive within 200ms (fetch + render)
- [ ] Playback at 5x speed: no dropped frames, prefetch keeps up
- [ ] All stream panels show data from the same timestamp (alignment error < 1 frame)
- [ ] Seek to arbitrary timestamp: correct data displayed within 200ms
- [ ] Keyboard shortcuts work: space (play/pause), arrows (frame step), digits (speed)
- [ ] Timeline zoom: from full-recording overview down to single-frame view

## Exposed Interfaces

```typescript
// frontend/src/playback.ts
export class PlaybackEngine {
  constructor(recordingId: string);
  
  // State
  readonly state: 'stopped' | 'playing' | 'paused' | 'seeking';
  readonly currentTime: number; // nanoseconds
  readonly speed: number;       // 0.5 | 1 | 2 | 5
  
  // Controls
  play(speed?: number): void;
  pause(): void;
  seek(timestampNs: number): void;
  stop(): void;
  stepForward(frames?: number): void;
  stepBackward(frames?: number): void;
  
  // Events (Vue refs or callbacks)
  onTimeChange: (timestamp: number) => void;
  onStateChange: (state: string) => void;
  onEntriesLoaded: (entries: Map<string, RecordEntry[]>) => void;
  
  // Bookmarks
  addBookmark(timestamp: number, label?: string): void;
  removeBookmark(id: string): void;
  readonly bookmarks: Bookmark[];
}
```
