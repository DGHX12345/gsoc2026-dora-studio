# Week 3 Update

## Status

Week 3 implementation is complete locally. The project has moved from a mock-only dataflow API toward a real local-file control API foundation while keeping the frontend prototype runnable.

## Completed Work

- Added backend discovery for local dataflow YAML files under `examples/`.
- Wired `/api/dataflows` to return real local dataflow summaries with stable ids, names, status, node count, and edge count.
- Added `/api/dataflows/:id/definition` to load and inspect a selected dataflow file.
- Added lightweight backend parsing for the local Dora dataflow subset currently needed by Studio:
  - `nodes`
  - node `id`
  - node `path`
  - `inputs`
  - `outputs`
- Added graph generation from parsed local dataflow files for `/api/dataflows/:id/graph`.
- Added node metric placeholders from parsed local dataflow files for `/api/dataflows/:id/nodes`.
- Added JSON API error responses for missing dataflow ids and parsing/loading failures.
- Kept runtime logs and system status on the existing mock/runtime paths while the control API boundary stabilizes.
- Updated frontend API types for dataflow definitions.
- Updated the Dataflow Explorer sidebar to render backend-discovered dataflows instead of hard-coded file buttons.
- Updated the Dataflow Explorer to load the selected dataflow definition and graph by id.
- Kept frontend mock fallback behavior so the UI remains usable without the backend.
- Added initial backend tests for dataflow parsing, id generation, discovery, and not-found behavior.

## Important Clarification

The Week 3 "real dataflow" implementation reads real local dataflow files from the repository. It does not yet call Dora coordinator APIs, Dora runtime APIs, or Dora internal Rust crates.

This is an intentional intermediate step: the Studio backend now has a real local-file API boundary that can later be replaced or extended with coordinator/runtime integration.

## Current API/Data Model Direction

The control API now has these local-file-backed endpoints:

```text
GET /api/dataflows
GET /api/dataflows/:id/definition
GET /api/dataflows/:id/graph
GET /api/dataflows/:id/nodes
```

`/api/dataflows/:id/definition` returns the selected dataflow metadata, parsed nodes, and original source text. `/api/dataflows/:id/graph` currently derives a simple graph from node inputs and outputs.

## Validation

Completed local checks:

```bash
cargo fmt --manifest-path backend/Cargo.toml --check
cargo test --manifest-path backend/Cargo.toml
npm --prefix frontend run build
```

Additional smoke checks completed locally:

- Started the backend with `cargo run --manifest-path backend/Cargo.toml`.
- Verified `/api/dataflows` returns `robot-perception-test` from `examples/robot-perception-test/dataflow.yml`.
- Verified `/api/dataflows/robot-perception-test/definition` returns the selected dataflow metadata.
- Verified `/api/dataflows/robot-perception-test/graph` returns 5 nodes and 6 edges.
- Verified missing dataflow ids return a JSON 404 response.
- Started the frontend dev server and confirmed `http://127.0.0.1:5173/` returned HTTP 200.

## Remaining Notes

- The local YAML parser is intentionally lightweight and only supports the subset needed for the current example dataflow.
- Dora coordinator/runtime integration is still future work.
- Runtime metrics are still placeholders derived from the parsed node list.
- Runtime logs remain on the existing polling path.
- Full visual editor work remains out of scope for Week 3.

## Suggested Next Step

Use this implementation as the Week 3 completion update. Week 4 can continue the Control API phase by tightening runtime control behavior, replacing more placeholders with real Dora metadata, or integrating a more official Dora descriptor/control boundary if one is stable enough to use.
