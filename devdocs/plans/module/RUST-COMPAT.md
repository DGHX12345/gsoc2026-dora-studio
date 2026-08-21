# Rust Version Compatibility Strategy

## Problem

dora-rs requires Rust edition 2024 + rustc 1.88.0. Our environment has
Rust 1.75.0 + edition 2021. We cannot add dora crates as Cargo path
dependencies — the compiler rejects edition 2024 crates.

## Decision

**Copy type definitions, don't link crates.** Pin to the current dora
revision. Manually update copied types when dora upstream changes.

## Pinned Revision

```
Repo: /home/dora/dora
Commit: <run: cd /home/dora/dora && git rev-parse HEAD>
Date:   <run: cd /home/dora/dora && git log -1 --format=%ci>
```

## Copied Files

| Our file | dora source | Types copied |
|----------|-------------|--------------|
| `backend/src/drec/types.rs` | `libraries/recording/src/lib.rs` | RecordingHeader, RecordEntry, RecordingFooter, MAGIC, FOOTER_MAGIC, FORMAT_VERSION |
| `backend/src/protocol/types.rs` | `libraries/message/src/cli_to_coordinator.rs` | ControlRequest enum variants we use |
| `backend/src/protocol/types.rs` | `libraries/message/src/coordinator_to_cli.rs` | ControlRequestReply, NodeInfo, NodeMetricsInfo, DataflowList, DataflowStatus |
| `backend/src/protocol/types.rs` | `libraries/message/src/ws_protocol.rs` | WsMessage, WsRequest, WsResponse, WsEvent |
| `backend/src/protocol/types.rs` | `libraries/message/src/common.rs` | LogMessage, LogLevelOrStdout |

## Dependency Pins (Rust 1.75 compatibility)

| Crate | Pinned | Reason |
|-------|--------|--------|
| tonic | 0.12.3 | OTLP gRPC receiver (M15.6); 0.12 MSRV 1.71, 0.13+ needs newer rustc |
| indexmap | =2.7.0 | transitive via tonic's hyper stack; ≥2.14 uses edition 2024 (Cargo 1.75 can't parse), 2.13 needs rustc 1.82; 2.7.0 MSRV 1.71 |
| chrono | 0.4.45 (existing) | pin to rustc 1.75-compatible version |
| uuid | 1.6.0 (existing) | pin to rustc 1.75-compatible version |

New pins must be verified with `cargo build` before merge; record here.

## Update Procedure

1. `cd /home/dora/dora && git pull`
2. Diff the copied files against their dora sources (listed above)
3. If types changed: update our copies, fix any compilation errors
4. Run `cargo test` to verify
5. Update the pinned commit hash above

## Future: When We Can Link

When our Rust toolchain reaches 1.88+ and we adopt edition 2024:
- Replace copied types with `dora-recording = { path = "..." }`
- Replace copied protocol types with `dora-message = { path = "..." }`
- This is a drop-in replacement — our API surface stays the same
- Estimated: mid-2027 based on Ubuntu LTS Rust packaging cadence
