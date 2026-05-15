# TODOS

This file tracks the gap between the current `curler` scaffold and a fuller modern HTTP client.

## HTTP Execution

- Add request cancellation while a request is running.
- Replace soft cancellation with hard cancellation when the HTTP backend supports it.
- Add configurable timeout controls.
- Add retry controls.
- Add redirect policy controls and redirect history display.
- Expose decompression/content negotiation behavior.
- Add proxy support.
- Add request and response size limits in the UI.

## Async Runtime

- Evaluate `tokio` + `reqwest` as the next HTTP runner backend.
- Keep Ratatui as the UI framework while moving network work to async tasks.
- Preserve the current request-runner boundary so the backend can be swapped without rewriting app state.
- Use async execution for hard cancellation, streaming response bodies, SSE, WebSocket, and richer timeout control.
- Decide whether the Crossterm event loop should stay synchronous or move to async event streams after the HTTP runner is isolated.

## Protocol Coverage

- Evaluate HTTP/2 support.
- Evaluate HTTP/3 support.
- Add TLS/certificate controls.
- Add custom CA certificate support.
- Add insecure TLS toggle for local development.
- Add client certificate support.
- Add streaming response support.
- Add SSE support.
- Add WebSocket support.

## Request Editing

- Keep curl argument import faithful as the first priority; avoid surprising transformations when importing existing curl commands.
- Add import diagnostics that distinguish curl-faithful parsing from Curler-friendly recovery, such as malformed `-F` JSON imported as Raw with a warning.
- Preserve body content when switching body modes instead of destructively rewriting unsupported conversions.
- Add optional raw-body-to-form-data parsing support for simple `key=value` payloads, with no mutation when conversion is not possible.
- Implement real multipart form-data encoding.
- Add file upload support for multipart form-data.
- Add binary request body support beyond placeholder mode.
- Add content-type helpers.
- Add auth helpers for Basic, Bearer, API key, and common token flows.
- Add cookie editing UI beyond storage/application plumbing.
- Add query string normalization and optional key/value conversion.
- Add request duplication.
- Add real "new host", "new path", and "new request" flows from the history pane.

## CLI

- Add `--print-import` to parse curl/request arguments and print the imported draft without starting the TUI.
- Add `--project-root <path>` to override project discovery.
- Add `--history-dir <path>` for custom storage during tests or scripted workflows.
- Add `--no-history` for one-off requests that should not persist.
- Add `--run` to import, save, execute, print a compact result, and exit without opening the TUI.

## Response Handling

- Pretty-print JSON responses.
- Pretty-print XML and HTML responses where useful.
- Add response body search.
- Add copy response body/header/status actions.
- Add save response body to file.
- Add binary response download handling.
- Add response timing and size metrics.
- Add request/response raw view.
- Add rendered/header/body tabs in the Response pane.

## History And State

- Improve generated history labels.
- Support user-defined request names for every history variant.
- Add history search/filter.
- Add import/export for project histories.
- Add environment profiles.
- Add clearer distinction between global, project, and local state.
- Persist shared header edits more explicitly.
- Add safe delete confirmation for host/path subtree deletion.

## Variables

- Build a UI for response variable bindings.
- Support JSONPath-like selectors more fully.
- Support variable previews and unresolved-variable warnings.
- Support environment-specific variable values.
- Support secret variables with redacted display.

## TUI UX

- Add scroll support for long request/response panes.
- Add focus indicators for active text cells and response sections.
- Add command palette behavior.
- Add context menu keyboard navigation.
- Add visible running/loading state.
- Add error details pane for failed requests.
- Improve narrow-terminal layout.
- Add help content for right-click menus.

## Curl Compatibility

- Expand curl argument parser coverage.
- Support common flags such as `--url`, `--user`, `--compressed`, `--location`, `--connect-timeout`, and `--max-time`.
- Preserve raw imported curl arguments for export.
- Add export current request as curl.
- Add curl parity tests for common command shapes.

## Testing

- Add integration tests for real request construction without network.
- Add snapshot tests for key TUI layouts.
- Add parser fixtures for curl command imports.
- Add persistence tests for project history/state files.
- Add regression tests for variable binding and shared headers.
