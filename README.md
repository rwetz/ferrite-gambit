# Gambit

Live Lichess TV, with a text-mode board.

A working development scaffold, not a published release.

## Run

Requires Rust 1.92 or later and the normal GPUI native toolchain. Ferrite Design
is pinned to a published GitHub commit and fetched automatically by Cargo.

```powershell
git clone https://github.com/rwetz/ferrite-gambit.git
cd ferrite-gambit
cargo run
```

No API key is required.

## Current slice

- One anonymous NDJSON TV stream with bounded event buffering.
- Validated FEN rendering, players, and reported clock values.
- Automatic reconnect with a 60-second backoff; cancellation on close.
- Open the current game in Lichess.

The title-bar menu and Ctrl+Shift+P expose actions and scheme/appearance changes.
`FERRITE_SCHEME`, `FERRITE_APPEARANCE`, and `FERRITE_FPS` supply launch defaults.
These scaffolds do not yet persist their own theme or use the planned shared settings store.

## Next

Channel selection, board orientation, move history, and desktop pinned-board mode.
This scaffold is a spectator. Account login and playing games are separate future work.

## API reference

[Official integration documentation](https://lichess.org/api)

## Validation

```powershell
cargo check
cargo clippy -- -D warnings
cargo test
cargo run -- --smoke           # verify the real service without opening a window
```

Compilation and service tests do not establish native window, scrolling, or device quality.
