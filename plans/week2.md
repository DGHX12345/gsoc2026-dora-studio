# Week 2 Update

## Status

Week 2 implementation is complete locally. The frontend and backend skeletons still compile and run, and the runtime log monitoring path now has a clearer API/data-model boundary for mentor review.

## Completed Work

- Kept the initial Studio API surface focused on health, system status, dataflows, graph data, node metrics, runtime lifecycle, and runtime logs.
- Extended the runtime log API from a simple display row into a structured Studio log model:
  - display time
  - full timestamp
  - node name
  - log level
  - cleaned message
  - raw message
  - captured source stream
  - optional source file and source line
- Updated backend runtime capture so stdout and stderr from `dora run` are preserved separately before being converted into Studio log entries.
- Added lightweight backend parsing for runtime logs:
  - timestamp extraction
  - known-node detection for the sample dataflow
  - stdout/stderr source detection
  - source file and line extraction from tokens like `camera.py:44`
  - normalized log-level classification for info/warning/error display
  - cleaned messages for UI display while preserving the raw log line
- Updated the Logs & Events UI to show grouped info/warning/error logs, the raw combined stream, source metadata, source location, and full-log modal views.
- Reused a shared log-line renderer in the frontend so grouped cards, terminal preview, and modal views display the same metadata consistently.
- Updated mock backend data and frontend fallback data to match the runtime log API shape, so the frontend can use the same model for fallback and live runtime data.
- Added layout adjustments so long log messages and source metadata wrap cleanly instead of breaking the log view.
- Ignored generated example output under `examples/**/out/` to keep local runtime-test artifacts out of git.
- Kept `examples/robot-perception-test/dataflow.yml` as the lightweight local testing path for runtime observation.

## Current API/Data Model Direction

The current runtime log response shape is intended to remain stable when the backend later moves from captured CLI output to Dora coordinator or runtime APIs:

```ts
type StudioLog = {
  time: string
  timestamp: string
  node: string
  level: 'info' | 'warn' | 'error'
  message: string
  rawMessage: string
  source: string
  sourceFile: string | null
  sourceLine: string | null
}
```

The backend currently exposes this through `/api/runtime/logs`, with `/api/runtime/start`, `/api/runtime/stop`, and `/api/runtime/status` still serving as the temporary local runtime lifecycle bridge.

## Implementation Notes

- The backend still uses `dora run examples/robot-perception-test/dataflow.yml` as the temporary local runtime bridge.
- The log parser is intentionally lightweight for Week 2. It is enough to validate the API shape and UI behavior, but it should be replaced or tightened once Studio connects to coordinator/runtime APIs.
- The frontend remains polling-based for runtime logs. SSE or WebSocket streaming is still an open Week 3 decision.
- The current UI keeps the Chinese-first test interface while the GitHub-facing update text stays English-only.

## Validation

Completed local checks:

```bash
cargo fmt --check
cargo test
npm --prefix frontend run build
```

The frontend dev server was also started locally, and `http://127.0.0.1:5173/` returned HTTP 200.

## Mentor Confirmation Needed

1. Does this satisfy the Week 02 goal of agreeing on the initial API/data model while keeping the project skeletons runnable?
2. Is the runtime log response model acceptable as the first stable boundary for Studio log monitoring?
3. Should Week 3 keep polling for runtime logs, or introduce SSE/WebSocket streaming?
4. Is `dora run` acceptable as a temporary local testing bridge before coordinator/control integration?
5. Are there any API fields that should be renamed or removed before deeper frontend/backend integration?
6. Should the parser keep trying to infer node/source metadata from CLI output, or should Week 3 prioritize replacing this with coordinator/runtime-provided metadata?

## Suggested Next Step

Use the current implementation and this document as the Week 2 mentor update. After mentor feedback, Week 3 can focus on either streaming logs or the next coordinator-facing runtime control layer.
