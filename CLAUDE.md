# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build, test, install

```bash
cargo build              # debug build
cargo build --release    # optimized
cargo test               # run all tests
cargo test test_get_words           # run a single test by name
cargo test --package typy <name>    # scope to crate
cargo install --path .   # install to ~/.cargo/bin/typy (release profile)
```

**After making code changes, rebuild AND reinstall** so the user picks up the new binary:

```bash
cargo build && cargo install --path .
```

`cargo build` alone only updates `target/debug/typy` — the user's `typy` command on PATH points to `~/.cargo/bin/typy` from `cargo install`, so skipping the install step means changes aren't reflected when they run `typy`.

## Run

```bash
cargo run -- -t 60 -l spanish    # pass CLI args after `--`
cargo run -- -s                  # show historical stats
cargo run -- -c                  # open config
```

Word lists download to `~/.local/share/typy/` on first run. Scores live at `~/.local/share/typy/scores.json`. Config is `~/.config/typy/config.toml`.

## Architecture

This is a Monkeytype-style typing test in the terminal, built on `crossterm` (raw mode + alternate screen) for input/rendering and `tui` only for the results-screen graph.

### Top-level flow (`src/main.rs`)

`main` runs an outer game loop driven by `PostGameAction`:
- `terminal::run(...)` plays one game and returns an action.
- `Quit` exits the loop.
- `Replay { duration, lang }` starts a fresh game (new random words) with possibly different settings.
- `Repeat { duration, lang, words, ghost }` replays the **same words** with the previous run's cursor as a ghost; this is "unranked" (no PB, no save).

This loop pattern is why post-game choices (replay/repeat/different time/different language) don't require restarting the CLI — the loop just feeds new params back into `terminal::run`.

### Game loop (`src/terminal/game.rs`)

`run()` is the heart of a single game. Key invariants:

- **Two parallel buffers**: `game.list` (mutable, reflects user's typed extra chars) and `game.original_list` (immutable reference for redraws / backspace recovery). Editing or backspacing must keep both in sync conceptually — `original_list` is the source of truth for the "missing" (untyped) color when redrawing.
- **Cursor model**: `(player.position_x, player.position_y)` is column/row within the word grid (`Vec<Vec<String>>`, three lines). `selected_word_index` and `jump_position` track word-level navigation for the space-jump behavior.
- **Per-line centering**: `calc_line_xs()` computes a separate x offset per line so each line is individually centered (paragraph style). Any cursor positioning must use `line_xs[py]`, never a single global x.
- **Timer**: a background thread (`start_timer`) decrements `remaining_time` and flips `timer_expired`. The timer doesn't start until `timer_started` flips on first keypress (see "Timer starts on first keypress" feature).
- **Ghost replay**: during a `Repeat` session, `ghost_data` is the previous run's `Vec<GhostFrame>` and the loop renders the ghost cursor's current position by finding the latest frame whose `elapsed_ms <= now`. New ghost frames are recorded into `ghost_frames` whenever the player's position changes.
- **Tab restart**: pressing Tab mid-game sets `restart_requested`, breaks the loop, and returns `PostGameAction::Replay` directly (skipping the stats screen / score save).
- **Resize**: `Event::Resize` recomputes `line_xs`, `y`, `timer_x` and calls `redraw_game()` to repaint everything. Don't add UI elements without handling them in `redraw_game`.

### Input handling (`src/terminal/keyboard.rs`)

`handle_input()` dispatches on `KeyCode`:
- **Char**: either consumes an expected char, marks an error, or appends as an "extra char" past word end (capped at `MAX_WORD_LENGTH = 100`). Extra chars mutate `game.list` to insert into the current word.
- **Space**: a complex state machine — at start of word/line it's a no-op; mid-word it inserts an error char; at a word boundary it jumps to the next word and updates `jump_position` / `selected_word_index`.
- **Backspace**: walks back across word/line boundaries and restores the original char in "missing" color from `original_list`. It also has special handling for removing extra chars vs. just uncovering originals.

- **Delete word** (`handle_delete_word`): opt/ctrl+backspace and ctrl+w delete back to the start of the previous word. It is implemented as repeated `handle_backspace` calls (stopping at a word start) so all word-index / extra-char / line-boundary bookkeeping lives in one place. `is_delete_word` maps the several byte sequences terminals use for these chords; `handle_input` also swallows any other `Char` carrying CONTROL so chords never register as typed text.

`InputAction { Continue, Break, None }` is how nested handlers signal "skip rest of frame", "end game", or "fall through".

### Modes (`src/mode/mode_selector.rs`)

A `Mode` is parsed from CLI strings (`normal`, `uppercase`, `punctuation`, comma-combos) and applied via `mode.transform(&mut g.list)` after words are fetched. Combinations stack (e.g. uppercase + punctuation). Probabilities (`uppercase_chance`, `punctuation_chance`) come from config.

### Stats and scoring (`src/scores/`)

- `stats.rs` (`Stats`) — accumulates per-second WPM, raw WPM, errors, character counts during the game; computes accuracy, consistency at the end.
- `progress/data.rs` — persists the last 10 games + running averages to `scores.json`. `Score::new` is what's written each (non-repeat) game.
- `finish_overview.rs` — full results screen: graph (left), legend, character breakdown, per-category leaderboard with tab navigation, menu (replay/repeat/15s/30s/60s/120s/english/spanish/quit). Returns the chosen `PostGameAction`.
- `graph.rs` — renders the WPM graph using `tui::widgets::Chart` into a `Rect` (the only place `tui` is used).

PB detection compares the new score against scores filtered by **same language AND same duration**, then takes the max WPM (tiebreak: accuracy). PBs trigger confetti and skip the normal results layout offset.

### Word source (`src/word_provider/`)

`get_words(lang, line_length)` returns three lines of words. `finder.rs` reads from `~/.local/share/typy/<lang>.txt` and downloads the file from a built-in URL on first run. To add a language at runtime, drop `~/.local/share/typy/<lang>.txt` (one word per line) and pass `-l <lang>`.

### Config (`src/config/`)

TOML-driven. `theme`, `cursor`, `modes`, `language` tables map to `config_tables/*.rs`. `config::theme::ThemeColors::new()` and friends are called during startup; missing config = sensible defaults.

## Conventions worth knowing

- All terminal positioning goes through `crossterm::cursor::MoveTo`. After any `tui` chart draw (only `graph::draw_graph`), the cursor must be re-hidden because `tui` re-shows it.
- Layout math on terminal size is all `u16`, so **always use `saturating_sub`** — a plain `cols / 2 - width / 2` panics in debug and wraps to ~65535 (drawing offscreen) in release whenever the window is smaller than the content. `draw_graph` additionally clamps its `Rect` to the frame and skips drawing when too small, because `tui` panics if a widget renders outside its buffer.
- The CLI uses `clap` derive. Adding a flag = add to the `Cli` struct and thread it through `main`.
- `anyhow::Result` and `.context("...")` are the standard error pattern throughout.
- The repo is a fork of `Pazl27/typy-cli` with substantial divergence (backspace, ghost replay, leaderboard, repeat mode, language switching). Don't assume parity with upstream.
