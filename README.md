# curler

`curler` is a terminal Postman-like HTTP client for people who already use `curl`, but want reusable request history, editable request state, and a TUI workflow.

## Stack

- Language: Rust 2024
- Terminal UI: Ratatui rendered through RustUI shared keymap/style primitives
- Terminal backend: Crossterm
- Shared TUI framework: rustui from `https://github.com/rinoale/rustui.git`
- HTTP client: ureq with rustls TLS
- Storage: JSON files under the current project's Curler history directory

## Features

- Standalone Rust binary crate.
- Terminal setup and cleanup with raw mode, alternate screen, and mouse support.
- Keyboard and mouse event loop.
- RustUI-backed unified keymap for global and pane-local shortcuts.
- Vim-style focus movement with `Ctrl-H/J/K/L`, plus `Tab` and `Shift-Tab`.
- Project-scoped history under `~/.curler/projects/histories/`.
- Curl-like command-line import.
- History tree grouped by host, method, and path.
- Separate request history variants for query string, body, headers, and cookies.
- Click to expand hosts and paths.
- Click to select request histories.
- Right-click context menus for history, method, headers, body, and logs.
- Rename and delete request history entries.
- Method dropdown for `GET`, `POST`, `PUT`, `PATCH`, and `DELETE`.
- Plain query-string input.
- Local header key/value editor with add, clear, and delete interactions.
- Shared header key/value editor for project state.
- Central workspace pane dividers are draggable; Menu, Actions, and Logs stay fixed.
- Body modes for Raw, Form Data, URL Encoded, and Binary.
- Form Data and URL Encoded body key/value editors.
- Raw body text input.
- Shared variables using `{{placeholder}}` syntax.
- JSON response bindings for dynamic variables.
- Real HTTP/HTTPS execution.
- Background request execution so the TUI stays responsive while HTTP runs.
- Response pane with status, headers, body, and expandable header list.
- Logs pane for operational feedback.

## How To Run

Run the app from the project root:

```sh
cargo run
```

Import a request from command-line arguments:

```sh
cargo run -- https://example.com
```

You can also pass curl-like arguments:

```sh
cargo run -- -X POST https://example.com/api -H "Authorization: Bearer {{access_token}}" -d '{"hello":true}'
```

## CLI Options

Curler-level flags are recognized only when they are the sole argument, so curl flags can still be imported.

```sh
curler --help
curler -h
curler --version
curler -v
```

## Install

Install with the repository script:

```sh
scripts/install.sh
```

The script builds an optimized executable and installs it to `/usr/local/bin/curler` by default, using `sudo` only when the install directory is not writable. Override the destination with `INSTALL_DIR`:

```sh
INSTALL_DIR="$HOME/.curler/bin" scripts/install.sh
```

If you install into `"$HOME/.curler/bin"`, add Curler to your Bash `PATH` by appending this to `~/.bashrc`:

```sh
# curler
export PATH="$HOME/.curler/bin:$PATH"
```

Reload your shell:

```sh
source ~/.bashrc
```

Then run:

```sh
curler
```

## Basic Workflow

1. Start `curler`.
2. Select or edit the host/path, query, headers, body, and method.
3. Press `Ctrl-R` or click `Run`.
4. Curler saves the request to history and executes it in a background worker.
5. Inspect status, headers, and body in the Response pane.
6. Click `[+] N more headers` to expand response headers.
7. Use the History pane to reselect previous request variants.

## Shortcuts

- `Ctrl-Q`: quit
- `Ctrl-R`: run current request
- `Ctrl-S`: save current request without running
- `Ctrl-P`: command palette placeholder
- `Ctrl-H/J/K/L`: move focus left/down/up/right
- `Tab` / `Shift-Tab`: move focus
- History pane: `j/k` move, `Enter` or Space expand/select, `a` add placeholder, `d` delete, `r` rename
- Method pane: `Enter` or Space opens dropdown, `1-5` selects method
- Response pane: `h`, `Enter`, or Space toggles response headers

## Mouse

- Left-click panes to focus them.
- Left-click history rows to expand/select.
- Left-click `Run`, `File`, `Help`, and dropdown options.
- Left-click `+ Add Header`, `+ Add Shared Header`, or `+ Add Field` to create new rows.
- Drag central workspace pane borders to resize. Menu, Actions, and Logs are fixed.
- Right-click supported components to open context-specific menus.

## Project Data

Curler discovers the current project and stores request history under:

```text
~/.curler/projects/histories/
```

On startup, Curler automatically creates the required project data directory:

```text
~/.curler/projects/histories/<project-name>-<project-hash>/
```

Inside that project directory, Curler uses:

```text
history.json
state.json
```

Those files are created when there is data to save, such as importing, saving, or running a request. The install directory is separate: `~/.curler/bin` is created by the manual install step above, not by the app itself.

History is grouped by:

```text
host -> method + path -> request variant
```

For example:

```text
https://google.com
  GET search
    qs:q=hello hdr:...
```

## Development

### Source Tree

Curler keeps domain logic, app state, adapters, and TUI code in separate modules:

```text
src/
  main.rs              Binary entrypoint, terminal setup, event loop
  cli.rs               Curler-level CLI flags such as --help and --version
  app/                 TUI application state machine, RustUI keymap, and user actions
  domain/              HTTP request/history/state models and pure logic
  net/                 HTTP backend adapters
  storage/             Project discovery and filesystem path ownership
  ui/                  Ratatui rendering with RustUI palette roles, layout hit-testing, and reusable widgets
scripts/
  install.sh           Build and install the release binary
```

Folder responsibilities:

- `app/`: owns `App`, the RustUI key-to-action map, focus, overlays, request editing, history selection, run state, and dispatch behavior.
- `domain/`: owns durable concepts: `RequestDraft`, body modes, headers/cookies, project history, shared state, variables, and response bindings.
- `net/`: owns network execution details. The current backend is `ureq`; future `tokio`/`reqwest` work should stay behind this boundary.
- `storage/`: owns filesystem/project discovery such as `~/.curler/projects/histories/`.
- `ui/`: owns RustUI palette-backed Ratatui drawing and mouse hit-testing. Shared UI controls should go into focused widget modules such as `ui/key_value.rs`.
- `scripts/`: owns developer and release helper scripts such as `scripts/install.sh`.
- `cli.rs`: owns flags that Curler handles before starting the TUI. Request/curl-compatible arguments should continue into `App::load`.
- `main.rs`: wires CLI, terminal setup/cleanup, the event loop, and top-level mouse/keyboard dispatch.

Run checks:

```sh
cargo check
cargo test
```
