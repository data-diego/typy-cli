<div align="center">
  <pre>
,--------.,--.   ,--.,------.,--.   ,--.
'--.  .--' \  `.'  / |  .--. '\  `.'  /
   |  |     '.    /  |  '--' | '.    /
   |  |       |  |   |  | --'    |  |
   `--'       `--'   `--'        `--'
  </pre>
  <p>A minimalistic <a href="https://monkeytype.com/">Monkeytype</a> clone for the terminal.</p>
</div>

> This is a fork of [Pazl27/typy-cli](https://github.com/Pazl27/typy-cli) with the following improvements:

### What's new in this fork
- **Backspace support** — delete and retype across words and lines (original had no backspace)
- **Space mid-word protection** — pressing space mid-word counts as an error, not a skip
- **Spanish language** — built-in Spanish word list with `-l s` shorthand
- **Language flag** — `-l` / `--lang` CLI argument with shorthands (`s`, `es`, `e`, `en`)
- **Timer starts on first keypress** — no wasted seconds staring at the screen
- **Responsive layout** — text adapts to terminal width, centered paragraph style, dynamic resize
- **Alternate screen buffer** — proper full-screen rendering, no leftover text after exit
- **Instantaneous WPM graph** — per-second WPM with 3s smoothing (not cumulative), error markers on the raw line, average WPM line, color legend, proper axis scales
- **Enhanced results screen** — character breakdown (correct/incorrect/extra), consistency score, vertically and horizontally centered layout with padding
- **Personal best detection** — confetti animation + banner when you beat your record
- **Leaderboard** — top 5 scores with human-friendly timestamps ("5m ago", "Mar 16 at 6:14 pm Mexico City")
- **Quick replay menu** — arrow-key navigation to replay, change time (15/30/60/120s), switch language, or quit — no need to restart the CLI
- **Dynamic terminal resize** — both typing test and results screen redraw on resize
- **Better theme defaults** — dimmer untyped text for clearer contrast

---

## Table of contents
- [Overview](#overview)
- [Installation](#installation)
- [Flags](#flags)
- [Configuration](#configuration)
- [Results screen](#results-screen)
- [Stats](#stats)
- [Language](#language)
- [Uninstall](#uninstall)

## Overview
Typy displays random words for you to type as fast as possible, tracking your WPM, accuracy, and consistency over time.

### Features
- **Backspace support** — delete and retype across words and lines
- **Space mid-word protection** — pressing space mid-word counts as an error, not a skip
- **Multiple languages** — English and Spanish built-in, easy to add more
- **Timer starts on first keypress** — no wasted seconds
- **Responsive layout** — text and UI adapt to terminal width with proper padding and centered paragraph alignment
- **Dynamic resize** — typing test and results screen reflow when you resize the terminal
- **Enhanced results screen** — instantaneous WPM graph with error markers, average line, character breakdown, consistency score, and color legend
- **Personal best detection** — confetti animation when you set a new record
- **Leaderboard** — top 5 scores with human-readable timestamps
- **Quick replay** — arrow-key menu on results screen to replay, change time, or switch language without restarting
- **Game modes** — normal, uppercase, and punctuation modes
- **Fully configurable** — colors, cursor style, default mode, and language via TOML config

## Installation
To install Typy, you can use the [Cargo] package manager:

[Cargo]: https://doc.rust-lang.org/cargo/

```bash
cargo install --git "https://github.com/data-diego/typy-cli.git"
```

If you prefer to compile it yourself:

1. Clone the repository:
    ```bash
    git clone https://github.com/data-diego/typy-cli.git
    cd typy-cli
    ```

2. Compile and install:
    ```bash
    cargo install --path .
    ```

3. (Optional) Add an alias for quick access:
    ```bash
    echo 'alias ty="typy"' >> ~/.zshrc
    source ~/.zshrc
    ```

Word lists are downloaded automatically on first run.

## Flags
```
Usage: typy [OPTIONS]

Options:
  -t, --time <TIME>     Duration of the game [default: 30]
  -l, --lang <LANG>     Language for the word list (e.g. english, spanish)
  -s, --stats           Display game stats
  -c, --config          Create and open config file
  -m, --mode <MODE>...  Sets the mode of the game
  -h, --help            Print help
  -V, --version         Print version
```

### Examples
```bash
typy                    # English, 30 seconds
typy -t 60              # English, 60 seconds
typy -l spanish         # Spanish, 30 seconds
typy -l s -t 60         # Spanish, 60 seconds (shorthand)
typy -m uppercase       # Uppercase mode
typy -m uppercase,punctuation  # Combined modes
```

### Language shorthands
| Shorthand | Language |
|-----------|----------|
| `e`, `en`, `eng` | English |
| `s`, `es`, `esp` | Spanish |

## Configuration
Typy is configured via a TOML file at `~/.config/typy/config.toml`. Use `typy -c` to create and open it.

```toml
# ~/.config/typy/config.toml

[theme]
fg = "#516D49"
missing = "#918273"
error = "#FB4934"
accent = "#D3869B"

[cursor]
style = "SteadyBar" # DefaultUserShape, BlinkingBlock, SteadyBlock, BlinkingUnderScore, SteadyUnderScore, BlinkingBar, SteadyBar

[modes]
default_mode = "normal" # "normal", "uppercase", "punctuation", or combinations like "uppercase, punctuation"
uppercase_chance = "0.3"
punctuation_chance = "0.5"

[language]
lang = "english"
```

## Results screen

After each test you'll see a full results screen:

- **WPM graph** — instantaneous net WPM (yellow) and raw WPM (grey) per second with 3-second smoothing, error markers (red dots on the raw line), and average WPM line. Y axis uses clean multiples of 20, X axis shows time labels every 5-10s.
- **Color legend** — below the graph showing what each line/marker means
- **Character breakdown** — correct/incorrect/extra counts
- **Consistency** — how steady your typing speed was across the test
- **Leaderboard** — your top 5 scores with human-friendly timestamps
- **Personal best** — confetti animation + banner when you beat your record

### Results screen controls
| Key | Action |
|-----|--------|
| `< >` arrows | Navigate menu options |
| `Enter` / `Tab` | Confirm selection |
| `Esc` | Quit |

Menu options: replay (same settings), time presets (15s/30s/60s/120s), language switch, quit.

## Stats
Game stats are saved at `~/.local/share/typy/scores.json` (last 10 games + running averages).

View your historical stats with:
```bash
typy -s
```

## Language
Word lists are stored at `~/.local/share/typy/`. Built-in languages: `english`, `spanish`.

To add a new language, create a text file with one word per line:
```
~/.local/share/typy/german.txt
```

Then use it with:
```bash
typy -l german
```

Or set it as default in `config.toml`:
```toml
[language]
lang = "german"
```

## Uninstall
```bash
cargo uninstall typy
```
