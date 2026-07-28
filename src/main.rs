mod config;
mod mode;
mod scores;
mod terminal;
mod word_provider;

use anyhow::{Context, Result};
use clap::Parser;
use mode::Mode;
use scores::progress::display;
use terminal::PostGameAction;

#[derive(Parser)]
#[command(name = "typy")]
#[command(version = "0.1.0")]
#[command(author = "Pazl27")]
#[command(
    about = "Monkeytype clone in the terminal for more information check: https://github.com/Pazl27/typy-cli"
)]
#[command(long_about = None)]
struct Cli {
    #[arg(
        short = 't',
        long = "time",
        default_value = "30",
        help = "Duration of the game"
    )]
    time: u64,

    #[arg(short = 's', long = "stats", help = "Display game stats")]
    stats: bool,

    #[arg(short = 'c', long = "config", help = "Create and open config file")]
    config: bool,

    #[arg(short = 'm', long = "mode", num_args = 1.., help = "Sets the mode of the game")]
    mode: Vec<String>,

    #[arg(short = 'l', long = "lang", help = "Language for the word list (e.g. english, spanish)")]
    lang: Option<String>,

    #[arg(short = 'p', long = "symbols", help = "Sprinkle punctuation symbols into the words")]
    symbols: bool,

    #[arg(short = 'n', long = "numbers", help = "Sprinkle numbers into the words")]
    numbers: bool,
}

fn resolve_lang(input: &str) -> String {
    match input {
        "s" | "es" | "esp" => "spanish".to_string(),
        "e" | "en" | "eng" => "english".to_string(),
        _ => input.to_string(),
    }
}

/// Runs the `[zoom] enter` hook now and the `exit` hook on drop, so the terminal
/// is restored on every exit path (quit, Esc, Ctrl-C, panic).
struct ZoomGuard(Option<String>);

impl ZoomGuard {
    fn new() -> Self {
        let (enter, exit, steps) = match config::toml_parser::get_config().lock().unwrap().get_zoom()
        {
            Some(zoom) => (zoom.enter, zoom.exit, zoom.steps.unwrap_or(1)),
            None => (None, None, 1),
        };
        if let Some(cmd) = enter {
            for _ in 0..steps {
                run_hook(&cmd);
            }
        }
        ZoomGuard(exit)
    }
}

impl Drop for ZoomGuard {
    fn drop(&mut self) {
        if let Some(cmd) = self.0.take() {
            // Fire-and-forget: the hook may deliberately wait (e.g. for held
            // modifier keys to clear) and must not delay typy's exit.
            let _ = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn();
        }
    }
}

// ponytail: best-effort and output-swallowed — a broken hook must never garble
// the screen or stop the game. Report failures if that turns out to be confusing.
fn run_hook(cmd: &str) {
    let _ = std::process::Command::new("sh").arg("-c").arg(cmd).output();
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let theme = config::theme::ThemeColors::new();

    if cli.config {
        config::create_config()?;
        config::open_config()?;
        return Ok(());
    }

    if cli.stats {
        display::draw()?;
        return Ok(());
    }

    let mut mode_strs: Vec<&str> = cli.mode.iter().map(|s| s.as_str()).collect();
    if cli.symbols {
        mode_strs.push("punctuation");
    }
    if cli.numbers {
        mode_strs.push("numbers");
    }
    // `-p`/`-n` are additive, so drop the `normal` that would otherwise wipe them.
    if mode_strs.len() > 1 {
        mode_strs.retain(|m| *m != "normal");
    }

    let mut duration = cli.time;
    let mut lang: Option<String> = cli.lang.map(|l| resolve_lang(&l));

    let _zoom = ZoomGuard::new();

    let mut repeat_words: Option<Vec<Vec<String>>> = None;
    let mut ghost_data: Option<Vec<terminal::GhostFrame>> = None;

    loop {
        let mode = Mode::from_str(mode_strs.clone())
            .context("Failed to parse mode")?
            .add_duration(duration);

        match terminal::run(mode, theme.clone(), lang.clone(), repeat_words.take(), ghost_data.take())? {
            PostGameAction::Quit => break,
            PostGameAction::Replay {
                duration: new_dur,
                lang: new_lang,
            } => {
                duration = new_dur;
                lang = Some(new_lang);
            }
            PostGameAction::Repeat {
                duration: new_dur,
                lang: new_lang,
                words,
                ghost,
            } => {
                duration = new_dur;
                lang = Some(new_lang);
                repeat_words = Some(words);
                ghost_data = Some(ghost);
            }
        }
    }

    Ok(())
}
