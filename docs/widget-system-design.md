# Widget System Design

Groh renders dashboard widgets by executing sandboxed Rhai scripts into typed Rust element trees, streaming snapshots over Server-Sent Events (`GET /api/events`), and rendering those trees with recursive Svelte 5 components.

## Data Flow

1. **Docker watcher (`src/docker.rs`)** connects to the local Docker socket, fetches containers with `groh.*` labels, nests `groh.parent` children, computes add/remove deltas via `compute_delta`, and publishes snapshots on a Tokio `watch` channel.
2. **Configuration (`src/config.rs`)** reads `groh.toml` once into typed `serde` structs (`LoadedConfig`), validates unique widget IDs, checks `[widgets.<id>.config]` tables, and resolves `Source` variants (`Builtin`, `Absolute`, `WorkspaceRelative`, `WidgetRelative`). A missing or invalid `groh.toml` fails startup.
3. **Widget runtime (`src/widgets.rs`)** parses each script's `const META` header (requiring `title`, `interval`, and `size`, plus optional `needs`, `hosts`, and `config` schema) and runs an async supervisor backed by a Tokio `JoinSet`, `Semaphore` concurrency bound, `spawn_blocking`, and `tokio::time::timeout`.
4. **Rhai sandbox (`src/script/`)** enforces operation, collection, string, and monotonic `Instant` deadline limits, exposing only capabilities listed in `META.needs`:
   - `docker:containers` -> `docker::containers()`
   - `net:http` -> `http::get(url)` / `http::get_json(url)` checked against `META.hosts` on every request and redirect hop
   - `time:now` -> `time::now()`
5. **Typed Elements (`src/view.rs`)** expose Rhai constructors returning `Node` values validated for `MAX_NODES` (`2000`) and `MAX_DEPTH` (`64`). Container card layout, grouping, sorting, health counts, and color choices live entirely in `widgets/Containers.groh`.

## Elements

`Node` variants map 1:1 to Svelte components in `frontend/src/lib/elements/` rendered recursively by `frontend/src/lib/Node.svelte`:

- `Text`, `Box`, `Row`, `Column`, `Separator`, `Bar`, `StatusDot`, `Button`, `Link`, `Image`, `Icon`, `Window`

`Window.svelte` owns its trigger button, open state, and `Escape` key handler. `App.svelte` passes a generic text search query to `Node.svelte`, which filters `Text` subtrees without widget or container knowledge.

## Palette & Time

- **Palette**: Defined once in `frontend/src/lib/palette.css` (`--background`, `--foreground`, and 16 generic color variables across dark and light themes) and mirrored for terminal output in `src/palette.rs`. No semantic aliases.
- **Time**: `chrono` 0.4 parses RFC 3339 container start timestamps and formats UTC terminal event logs.
