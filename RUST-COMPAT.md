# Rust Version Compatibility Strategy

## Problem

dora-rs requires Rust edition 2024 + rustc 1.88.0. Our environment has
Rust 1.75.0 + edition 2021. We cannot add dora crates as Cargo path
dependencies — the compiler rejects edition 2024 crates.

## Decision

Copy type definitions, don't link crates. Pin to the current dora
revision. Manually update copied types when dora upstream changes.

## Pinned Revision

Repo: /home/dora/dora
Commit: use `git -C /home/dora/dora rev-parse HEAD` to retrieve.
Date:   use `git -C /home/dora/dora log -1 --format=%ci` to retrieve.

## Copied Files

| Our file | dora source | Types copied |
|----------|-------------|--------------|
| `backend/src/drec/types.rs` | `libraries/recording/src/lib.rs` | RecordingHeader, RecordEntry, RecordingFooter, MAGIC, FOOTER_MAGIC, FORMAT_VERSION |
| `backend/src/protocol/types.rs` | `libraries/message/src/ws_protocol.rs` | WsMessage, WsRequest, WsResponse, WsEvent |
| `backend/src/protocol/types.rs` | `libraries/message/src/coordinator_to_cli.rs` | NodeInfo, NodeMetricsInfo, NodeStatus, DataflowList, TraceSummary, TraceSpan |
| `backend/src/protocol/types.rs` | `libraries/message/src/common.rs` | LogMessage, LogLevel |

## Dependency Version Pins

| Crate | Version | Reason |
|-------|---------|--------|
| uuid | 1.6.0 | 1.24+ requires rustc 1.85 |
| chrono | 0.4.45 | Latest compatible with rustc 1.75 |

## Update Procedure

1. `cd /home/dora/dora && git pull`
2. Diff the copied files against their dora sources (listed above)
3. If types changed: update our copies, fix any compilation errors
4. Run `cargo test --manifest-path backend/Cargo.toml` to verify
5. Update the pinned commit hash above

## Future: When We Can Link

When our Rust toolchain reaches 1.88+ and we adopt edition 2024:
- Replace copied types with `dora-recording = { path = "..." }`
- Replace copied protocol types with `dora-message = { path = "..." }`
- This is a drop-in replacement — our API surface stays the same
