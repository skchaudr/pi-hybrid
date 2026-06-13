# Graceful Degradation Notes

This project should fail softly when optional dependencies or environment pieces
are missing.

## Cases we care about

### Python unavailable

- Python support is optional via the `python` Cargo feature.
- If Python support is not compiled in, the rest of the app should still build
  and run normally.
- The stable Rust path used for CI and local development should not depend on a
  Python runtime.

### No git repository

- Git operations are best-effort.
- If the current directory is not a git repository, the app should continue to
  run and surface git-specific features as unavailable instead of crashing.

### No network

- The app should still start and operate in local mode.
- Network-dependent actions should fail with a useful error rather than taking
  down the process.

## Evidence in the current codebase

- `rust-core/src/main.rs` has a headless entry path.
- `rust-core/src/agent/git.rs` already treats the missing-repo case as a normal
  "not available" condition.
- The stable Rust validation path used for B8 excludes the optional Python
  extension tests.

## Practical rule

Optional integrations should degrade to a clear unavailable state, not a panic
or hard crash.
