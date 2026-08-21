# D1: Dora Binary Resolution and Version Gate

## Goal

Provide one backend-owned resolution path for the dora executable and allow lifecycle operations only when the resolved executable reports dora 1.x. Version lookup failures are strict failures: the reported version is `unknown` and lifecycle operations are unavailable.

## Scope

This design covers M16.5 D1 only:

- Add `backend/src/dora_env.rs`.
- Resolve the executable from `DORA_STUDIO_DORA_BIN`, then fall back to the PATH command `dora`.
- Parse `dora --version` output into a normalized display string.
- Accept only major version 1 for lifecycle support.
- Migrate existing backend dora subprocess construction to use the shared resolver without changing endpoint shapes or command arguments.
- Add pure unit tests for resolution, version parsing, and the version gate.

Session endpoints, coordinator lifecycle management, recording, frontend changes, and settings-file version management remain outside this design.

## Behavior

### Binary resolution

`resolve_dora_bin()` returns the explicit value of `DORA_STUDIO_DORA_BIN` when the variable is present and non-empty. When it is absent or empty, it returns `dora`, allowing normal PATH lookup by `Command`.

The resolver does not scan the filesystem and does not install or modify dora. The explicit environment value is preserved as supplied so absolute paths, relative paths, and wrapper commands remain usable by the existing subprocess code.

### Version parsing

A shared version query executes `<resolved-bin> --version` with the existing asynchronous subprocess conventions. The output is trimmed and normalized to a display string such as `dora 1.0.0` or `dora 1.0.0-rc.4`. Empty output, command failure, timeout, or an unrecognized format produces `unknown`.

Version parsing is a pure function so tests do not depend on an installed dora executable. The parser accepts the dora prefix and a semantic version beginning with a numeric major component; it does not infer a supported version from arbitrary text.

### Version gate

`lifecycle_supported(version)` returns true only when the parsed version has major version `1`:

- `dora 1.0.0` and `dora 1.0.0-rc.4`: supported
- `dora 0.5.0`: unsupported
- `dora 2.0.0`: unsupported
- `unknown`, empty, or malformed output: unsupported

This is intentionally strict because M16.5 lifecycle commands target dora 1.0.0 semantics.

## Integration

Existing backend subprocess sites in `daemon.rs`, `runtime.rs`, `coordinator.rs`, and `metrics.rs` use the shared resolver for executable selection. Their command arguments, output parsing, API response models, and fallback behavior remain unchanged.

The shared version query is available to the future `session.rs` implementation. D1 does not add session routes or change the old daemon routes.

## Error handling

Dora executable resolution itself does not fail when PATH lookup is deferred to process spawn. A failed spawn or failed version query is reported through the caller's existing error/fallback path, with version state normalized to `unknown` where a version is required. No lifecycle command may be considered supported without a confirmed major version 1.

## Testing

Unit tests cover:

- non-empty `DORA_STUDIO_DORA_BIN` selection;
- empty or absent environment value falling back to `dora`;
- normalization of valid 1.0.0 and release-candidate output;
- rejection of 0.x, 2.x, empty, malformed, and arbitrary output;
- strict lifecycle support results for supported and unsupported versions.

Existing backend tests must continue to pass after command construction is migrated. Tests that need a command path use the pure resolver helpers or an injected path rather than relying on the host environment.

## Non-goals

- Downloading or installing dora.
- Persisting a selected binary in a settings file.
- Supporting dora 0.5 lifecycle commands.
- Adding session, recording, or frontend functionality.
- Changing public API response shapes.
