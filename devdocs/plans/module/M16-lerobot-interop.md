# M16: LeRobot Interop with dorobot-studio

**Layer**: 4+ — VLM Attribution / data interop
**Depends on**: M10 (LeRobot profiles)
**Effort**: 0.5 week
**Priority**: **NON-ESSENTIAL** — optional polish; doing or skipping M16 does
not affect the completeness of dora-studio as a GSoC deliverable (M10 already
consumes standard LeRobot datasets; M16 only adds convenience metadata and
cross-tool workflow docs). Schedule only if time remains after M13/M14/M15-B.

## Requirement

dorobot-studio (`/home/dora/dorobot-studio`) is a native (Makepad) studio
for LeRobot-format robot-learning datasets. Assess merge value; the
conclusion (2026-08-14) is **no code merge** — instead achieve
**data-level interop**: datasets recorded/annotated in dorobot-studio open
directly in dora-studio's attribution workflow (and vice versa).

## Verified facts (2026-08-14)

- dorobot-studio WRITES standard LeRobot **v2.x layout**
  (`dorobot-flex/src/data/lerobot_writer.rs`): `meta/info.json`,
  `meta/episodes.jsonl` (index/length/tasks), `meta/tasks.jsonl`
  (task_index → description), `data/chunk-NNN/episode_XXXXXX.parquet`,
  `videos/chunk-NNN/<camera>/episode_XXXXXX.mp4`. Staging + commit model.
- dora-studio M10 bridge (`backend/scripts/lerobot_reader.py`) reads v1
  (`episode_*.parquet`) and v2 (`chunk-*/file-*.parquet`) layouts via
  pyarrow; task descriptions currently come from a non-`task_index`
  parquet column (B601 legacy), NOT from `meta/tasks.jsonl`.
- Both projects consume the same Hugging Face LeRobot standard — interop
  is mostly gap-closing, not new protocol design.
- Merge verdict recap: UI stacks incompatible (Makepad DSL vs Vue/Web),
  user bases differ (dataset researchers vs dora app debuggers); sharing
  is limited to data formats + (optionally) Rust data crates
  (`dorobot-types`, parquet reader) — the latter only if a concrete need
  appears.

## Deliverables

- **D1 conformance fixture**: generate/obtain a small dorobot-written
  dataset (use dorobot's own writer tests or record one episode) and add
  it as a test fixture for `lerobot_reader.py` scan + frames.
- **D2 task metadata**: extend `lerobot_reader.py` scan to prefer
  `meta/tasks.jsonl` (and `meta/episodes.jsonl`) for task descriptions
  when present; fall back to the current column heuristic.
- **D3 video awareness**: report `videos/` MP4 paths in the dataset info
  panel as "recorded in dorobot-studio / external player" (Studio does
  not decode video in M16; honest unavailable state otherwise).
- **D4 workflow doc**: `docs/lerobot-dorobot-interop.md` — record in
  dorobot-studio → open in dora-studio attribution (and the reverse
  direction: any standard LeRobot dataset opens in both).

## Acceptance Criteria

- [ ] dorobot-written dataset scans and plays frames in Studio attribution with zero conversion
- [ ] Task descriptions resolve from tasks.jsonl when present
- [ ] Fixture test in `cargo test` (lerobot::tests) + reader unit tests
- [ ] Workflow doc committed (English)

## Risks

| Risk | Prob | Mitigation |
|------|------|-----------|
| dorobot v2 metadata drift (both projects evolving) | Low | fixture test pins the layout; re-verify on drift |
| dorobot fields beyond LeRobot standard (custom features) | Medium | reader reports unknown columns as unavailable, never fakes |
