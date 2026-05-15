# Curler Agent Guide

This file defines the project tree contract for future automated and human-assisted changes.

## Source Tree Ownership

```text
src/
  main.rs              Binary entrypoint: terminal setup, event loop, top-level dispatch
  cli.rs               Curler CLI flags handled before the TUI starts
  app/                 App state machine, actions, focus, overlays, editing, history interaction
  domain/              Durable request/history/state models and pure transformations
  net/                 Network execution adapters
  storage/             Project discovery and filesystem path decisions
  ui/                  Ratatui rendering, layout hit-testing, reusable widgets
```

## Placement Rules

- Put HTTP request shapes, headers, cookies, body modes, curl-compatible import parsing, fingerprints, and labels in `src/domain/request.rs`.
- Put project history data structures and history persistence models in `src/domain/history.rs`.
- Put shared headers, cookies, variables, and response binding logic in `src/domain/state.rs`.
- Put filesystem/project-root discovery and `~/.curler/...` path ownership in `src/storage/`.
- Put HTTP execution details in `src/net/`. Do not call a network library directly from `app/`, `ui/`, or `domain/`.
- Put Ratatui drawing, layout rectangles, mouse hit-testing, overlays, and reusable TUI widgets in `src/ui/`.
- Put reusable TUI controls in their own file under `src/ui/`, such as `src/ui/key_value.rs`.
- Put CLI-only flags in `src/cli.rs`. Do not parse request/curl-compatible options there unless they are Curler-level flags that should exit before TUI startup.
- Keep `src/main.rs` as wiring: CLI dispatch, terminal setup/cleanup, event loop, and top-level event routing.

## Boundaries

- `domain/` must not depend on `app/`, `ui/`, `net/`, or `storage/`.
- `ui/` may read app state and call public app accessors, but it should not mutate app state.
- `app/` owns mutations. UI hit-tests return intent; app methods perform state changes.
- `net/` receives request/state data and returns response data. It should not know about panes, focus, overlays, or storage paths.
- `storage/` owns paths and file placement. It should not render UI or execute requests.

## Change Conduct

- Preserve curl-compatible import behavior unless a change is explicitly documented as Curler-specific recovery.
- Add or update tests when changing parsing, history grouping, shared state, request execution, hit-testing, or layout math.
- Keep modules focused. If a file grows because a new concept was added, prefer extracting a sibling module over adding another unrelated section.
- Do not add a new top-level folder without documenting its ownership here and in `README.md`.
- Do not make UI controls mutate state directly. Route user intent through `App`.
- Do not store runtime project data outside the `storage/`-owned path rules.
- Do not make blocking network calls from the TUI event loop.
- Run `cargo fmt` and `cargo test` before handing off changes.

## Known Next Splits

- `src/app/mod.rs` is still large. Prefer extracting future work into `app/action.rs`, `app/editor.rs`, `app/history_tree.rs`, and `app/runner.rs` when touching those areas.
- `src/ui/mod.rs` is still large. Prefer extracting new view-specific work into `ui/views/` or sibling modules instead of extending `mod.rs`.
