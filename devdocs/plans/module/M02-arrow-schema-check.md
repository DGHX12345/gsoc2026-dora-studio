# M02: Arrow Schema Type Checking

**Layer**: 1 — Dataflow Live Graph
**Depends on**: M01 (canvas + YAML generation)
**Effort**: 1 week

## Purpose

When two nodes are connected on the canvas, check Arrow schema
compatibility in real time. Compatible ports show green, incompatible
show red. This catches type errors at edit time instead of at `dora run`
time.

## Deliverables

### D1: Schema registry

New file: `backend/src/schema_registry.rs`

- Parse dora operator manifests to extract input/output port schemas
- Schema source: dora node-hub manifests OR operator `__init__.py`
  docstrings OR user-provided `.schema.yaml` files
- `SchemaRegistry` maps `(operator_type, port_name)` → `ArrowSchema`
- `ArrowSchema` struct: list of `(field_name, data_type, nullable)` tuples

Fallback: if a node has no schema metadata, mark ports as "unknown"
(yellow) rather than blocking the connection.

### D2: Compatibility checker

- `check_compatibility(source: &ArrowSchema, sink: &ArrowSchema) -> CheckResult`
- `CheckResult::Compatible` — fields match name + type
- `CheckResult::Incompatible(reasons)` — list of mismatches (missing field, wrong type)
- `CheckResult::Unknown` — one or both schemas unavailable

Type compatibility rules:
- Exact match: `int64` ↔ `int64` ✓
- Widening: `int32` → `int64` ✓ (no data loss)
- Narrowing: `int64` → `int32` ⚠ (potential overflow, yellow)
- Incompatible: `int64` → `utf8` ✗ (red)
- Nullable → non-nullable ⚠ (potential null error)

### D3: Schema API endpoint

- `POST /api/schema/check` — body: `{ source_operator, source_port, sink_operator, sink_port }` → `CheckResult`
- `GET /api/schema/operator/:name` — returns registered input/output schemas

### D4: Canvas visual feedback

Update M01 canvas edge rendering:
- Green edge: compatible schema
- Red edge + tooltip: incompatible, show mismatch details
- Yellow edge: unknown schema, show warning
- Check fires on `connect` event, result renders within 100ms

## Acceptance Criteria

- [ ] Connect two ports with matching schema → green edge in <100ms
- [ ] Connect incompatible types (int64 → utf8) → red edge + error tooltip
- [ ] Connect unknown schema ports → yellow edge + "no schema data" warning
- [ ] Schema registry loads from at least 3 dora operator types (camera, object_detection, control)
- [ ] `cargo test` includes schema compatibility test cases

## Exposed Interfaces

```rust
// backend/src/schema_registry.rs
pub struct SchemaRegistry { ... }
impl SchemaRegistry {
    pub fn load_builtin() -> Self;
    pub fn register(&mut self, operator: &str, port: &str, schema: ArrowSchema);
    pub fn get(&self, operator: &str, port: &str) -> Option<&ArrowSchema>;
    pub fn check(&self, src: PortRef, sink: PortRef) -> CheckResult;
}

pub enum CheckResult {
    Compatible,
    Incompatible(Vec<SchemaMismatch>),
    Unknown,
}
```

```typescript
// frontend - canvas edge type annotation
interface SchemaEdgeStyle {
  color: 'green' | 'red' | 'yellow';
  tooltip?: string;
  mismatches?: SchemaMismatch[];
}
```
