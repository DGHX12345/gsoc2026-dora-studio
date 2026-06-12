# Week 3 Scope Update

Hi @bobdingAI,

Week 2 completed the initial API/data-model boundary and improved the runtime log monitoring prototype. For Week 3, I will move from requirements/prototyping into the control API implementation phase described in the proposal.

## Difference from Week 2

- Week 2 focused on confirming the initial API shape and improving the runtime log model for mentor review.
- Week 3 will focus on making the backend control API less mock-only and more useful for real local workflows.
- Week 2 used `dora run` mainly as a temporary runtime observation bridge.
- Week 3 will keep that bridge, but the main work will shift toward dataflow discovery, loading, status/error behavior, and tests.
- Full visual editor work and advanced streaming will remain out of scope for now so the control API can stabilize first.

## Planned Week 3 Scope

I plan to focus this week on the first part of "Weeks 3-5: Control API implementation":

- Implement dataflow listing from local project files instead of relying only on mock data.
- Implement loading/inspecting a selected dataflow definition through the backend API.
- Keep the existing mock data path as a frontend fallback and development aid.
- Add clearer API error responses for missing files, invalid paths, and runtime-control failures.
- Add initial backend tests for the control API behavior.
- Keep the frontend connected to the same API shape so it can work with both real local dataflows and mock fallback data.

## Working Assumptions

- The Week 3 priority is backend control API reliability rather than visual-editor depth.
- Polling runtime logs remains acceptable for now; SSE/WebSocket streaming can be revisited after the control API has stronger real-data support.
- `dora run examples/robot-perception-test/dataflow.yml` remains an acceptable temporary local execution path while the API is being hardened.
- If mentor feedback arrives later, I will adjust the implementation, but I will proceed with this scope to avoid blocking Week 3 progress.

Please let me know if any part of this scope should be changed. Otherwise, I will use this as the Week 3 implementation direction.
