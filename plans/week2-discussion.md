# Week 2 Scope Confirmation

Hi @bobdingAI,

The proposal defines Weeks 1-2 as the requirements and prototyping phase: define the API contract, scaffold the frontend/backend structure, and agree on the initial API/data model.

Week 1 already produced runnable Vue frontend and Rust backend scaffolds, plus an initial Studio prototype. For Week 2, I propose focusing on locking the initial requirements/API direction and improving the runtime log monitoring path.

## Proposed Week 2 Scope

- Confirm the initial Studio API contract and data model.
- Improve the Run & Monitor and Logs & Events UI.
- Improve the backend runtime log model with timestamp, node, level, and message fields.
- Keep a lightweight sample dataflow that generates info/warning/error logs for testing.
- Keep full visual editor work out of scope for now.

## Points to Confirm

1. Does this scope satisfy the Week 02 proposal goal: initial API/data model agreed, frontend/backend skeletons compile and run?
2. Are the current API areas reasonable for the initial contract: system status, dataflows, graph, node metrics, runtime lifecycle, and runtime logs?
3. Should runtime logs stay as polling for now, or should I add SSE/WebSocket streaming this week?
4. Is using `dora run` acceptable as a temporary local testing bridge before coordinator/control integration?
5. Are there any requirement changes you want before I finalize the Week 2 plan?

If this direction looks good, I will finalize the weekly plan and start implementation.
