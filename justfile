# Groh: `just` builds frontend (Bun) then backend (Cargo).
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

default: build

build: frontend
    cargo build --release

frontend:
    cd frontend; bun install --frozen-lockfile; bun run build

dev:
    cd frontend; bun run dev

backend:
    cargo run

[unix]
run: build
    ./target/release/groh

[windows]
run: build
    .\target\release\groh.exe

check:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace --all-targets

[unix]
clean:
    cargo clean
    rm -rf frontend/node_modules frontend/build

[windows]
clean:
    cargo clean
    Remove-Item -Recurse -Force frontend/node_modules, frontend/build -ErrorAction SilentlyContinue
