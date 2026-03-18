use super::keyboard::{handle_input, InputAction};
use anyhow::{Context, Result};
use crossterm::cursor::{self, SetCursorStyle};
use crossterm::event::poll;
use crossterm::{
    cursor::MoveTo,
    event::{read, Event, KeyEvent},
    style::{ResetColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::io::stdout;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::cursor_style::CursorKind;
use crate::config::language;
use crate::config::theme::ThemeColors;
use crate::mode::Mode;
use crate::scores::finish_overview;
use crate::scores::progress::{Data, Score};
use crate::scores::Stats;
use crate::word_provider;

pub struct Player {
    pub position_x: i32,
    pub position_y: i32,
}

impl Player {
    fn new() -> Self {
        Player {
            position_x: 0,
            position_y: 0,
        }
    }
}

pub struct Game {
    pub list: Vec<Vec<String>>,
    pub original_list: Vec<Vec<String>>,
    pub player: Player,
    pub jump_position: i32,
    pub selected_word_index: i32,
    quit: bool,
}

impl Game {
    fn new(list: Vec<Vec<String>>) -> Self {
        Game {
            original_list: list.clone(),
            list,
            player: Player::new(),
            jump_position: 0,
            selected_word_index: 0,
            quit: false,
        }
    }

    pub fn get_word_string(&self, index: i32) -> String {
        self.list.get(index as usize).unwrap().join(" ")
    }
}

/// Calculate per-line x offsets so each line is centered individually.
fn calc_line_xs(game: &Game, cols: u16, min_pad: u16) -> Vec<u16> {
    game.list
        .iter()
        .map(|words| {
            let line_len: usize = words.iter().map(|w| w.chars().count()).sum::<usize>()
                + words.len().saturating_sub(1); // spaces between words
            let center = cols / 2;
            center.saturating_sub(line_len as u16 / 2).max(min_pad)
        })
        .collect()
}

/// Redraw the entire game view at the given positions.
fn redraw_game(
    stdout: &mut std::io::Stdout,
    game: &Game,
    theme: &ThemeColors,
    line_xs: &[u16],
    y: u16,
    timer_x: u16,
    remaining: u64,
) -> Result<()> {
    stdout.execute(Clear(ClearType::All))?;

    for (i, words) in game.list.iter().enumerate() {
        print_words(line_xs[i], y + i as u16, words, stdout, theme)?;
    }

    // Timer
    stdout.execute(MoveTo(timer_x, y.saturating_sub(2)))?;
    stdout.execute(SetForegroundColor(theme.accent))?;
    print!("{:02}", remaining);

    // Cursor
    let cx = line_xs[game.player.position_y as usize];
    stdout.execute(MoveTo(
        cx + game.player.position_x as u16,
        y + game.player.position_y as u16,
    ))?;
    stdout.flush()?;

    Ok(())
}

pub fn run(mode: Mode, theme: ThemeColors, lang_override: Option<String>) -> Result<super::PostGameAction> {
    let mut stdout = stdout();

    let language = match lang_override {
        Some(lang) => language::Language { lang },
        None => language::Language::new(),
    };

    setup_terminal(&stdout).context("Failed to setup terminal")?;

    let (_, init_y, line_length) =
        super::calc_middle_for_text().context("Failed to calculate terminal size")?;

    let mut y = init_y;

    let mut game = Game::new(
        word_provider::get_words(&language.lang, line_length)
            .context("Failed to get words from file")?,
    );

    mode.transform(&mut game.list);

    let duration = mode.duration;
    let lang_name = language.lang.clone();
    let mut stats = Stats::new();

    let (cols, _) = crossterm::terminal::size()?;
    let mut line_xs = calc_line_xs(&game, cols, 4);

    for (i, words) in game.list.iter().enumerate() {
        print_words(line_xs[i], y + i as u16, words, &stdout, &theme)?;
    }
    stdout
        .execute(MoveTo(line_xs[0], y))
        .context("Failed to move cursor")?;

    let timer_expired = Arc::new(AtomicBool::new(false));
    let timer_expired_clone = Arc::clone(&timer_expired);
    let remaining_time = Arc::new(Mutex::new(mode.duration));
    let remaining_time_clone = Arc::clone(&remaining_time);
    let mut remaining_prev: u64 = 0;
    let timer_started = Arc::new(AtomicBool::new(false));
    let timer_started_clone = Arc::clone(&timer_started);

    // Display initial timer value
    let mut timer_x = (cols / 2).saturating_sub(1);
    {
        stdout
            .execute(MoveTo(timer_x, y.saturating_sub(2)))
            .context("Failed to move cursor")?;
        stdout
            .execute(SetForegroundColor(theme.accent))
            .context("Failed to set foreground color")?;
        print!("{:02}", mode.duration);
        stdout.flush().context("Failed to flush stdout")?;
        stdout
            .execute(MoveTo(line_xs[0], y))
            .context("Failed to move cursor")?;
    }

    let (tx, _) = mpsc::channel();

    let timer_thread = thread::spawn(move || {
        while !timer_started_clone.load(Ordering::Relaxed) {
            if timer_expired_clone.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        if let Err(e) = start_timer(mode.duration, timer_expired_clone, remaining_time_clone) {
            tx.send(e).expect("Failed to send error from timer thread");
        }
    });

    loop {
        if game.player.position_y == game.list.len() as i32 {
            break;
        }

        let cx = line_xs[game.player.position_y as usize];
        stdout
            .execute(MoveTo(
                cx + game.player.position_x as u16,
                y + game.player.position_y as u16,
            ))
            .context("Failed to move cursor")?;

        if timer_expired.load(Ordering::Relaxed) {
            break;
        }

        {
            let remaining = *remaining_time
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock remaining time: {}", e))?;
            stdout
                .execute(MoveTo(timer_x, y.saturating_sub(2)))
                .context("Failed to move cursor")?;
            stdout
                .execute(SetForegroundColor(theme.accent))
                .context("Failed to set foreground color")?;
            print!("{:02}", remaining);
            stdout.flush().context("Failed to flush stdout")?;
            stdout
                .execute(MoveTo(
                    cx + game.player.position_x as u16,
                    y + game.player.position_y as u16,
                ))
                .context("Failed to move cursor")?;
            if remaining != remaining_prev {
                stats.add_letters();
            }
            remaining_prev = remaining;
        }

        if poll(Duration::from_millis(5)).context("Failed to poll for events")? {
            match read().context("Failed to read event")? {
                Event::Key(KeyEvent {
                    code, modifiers, ..
                }) => {
                    if let Some(()) = super::close_typy(&code, &modifiers) {
                        timer_expired.store(true, Ordering::Relaxed);
                        game.quit = true;
                        break;
                    }
                    if !timer_started.load(Ordering::Relaxed) {
                        timer_started.store(true, Ordering::Relaxed);
                    }
                    match handle_input(&mut game, &stdout, code, &mut stats, &theme, &line_xs, y)? {
                        InputAction::Continue => continue,
                        InputAction::Break => break,
                        InputAction::None => {}
                    }
                }
                Event::Resize(new_cols, _) => {
                    let (_, new_y) = super::terminal_utils::recalc_position(line_length)?;
                    y = new_y;
                    line_xs = calc_line_xs(&game, new_cols, 4);
                    timer_x = (new_cols / 2).saturating_sub(1);
                    let remaining = *remaining_time
                        .lock()
                        .map_err(|e| anyhow::anyhow!("Failed to lock remaining time: {}", e))?;
                    redraw_game(&mut stdout, &game, &theme, &line_xs, y, timer_x, remaining)?;
                }
                _ => {}
            }
        }
    }

    let action = if !game.quit {
        stdout.execute(cursor::Hide)?;

        let current_wpm = stats.wpm() as u32;
        let is_pb = if current_wpm > 0 {
            match Data::get_scores() {
                Ok(scores) if scores.is_empty() => true,
                Ok(scores) => scores.iter().all(|s| current_wpm > s.wpm),
                Err(_) => true,
            }
        } else {
            false
        };

        let score = Score::new(
            current_wpm,
            stats.raw_wpm() as u32,
            stats.accuracy() as f32,
        );
        Data::save_data(score).context("Failed to save data")?;
        finish_overview::show_stats(&stdout, stats, &theme, duration, &lang_name, is_pb)
            .context("Failed to show stats")?
    } else {
        super::PostGameAction::Quit
    };

    reset_terminal(&stdout).context("Failed to reset terminal")?;
    timer_expired.store(true, Ordering::Relaxed);
    timer_thread
        .join()
        .map_err(|e| anyhow::anyhow!("Failed to join timer thread: {:?}", e))?;
    Ok(action)
}

fn setup_terminal(mut stdout: &std::io::Stdout) -> Result<()> {
    let cursor_kind = CursorKind::new();

    enable_raw_mode()?;
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(Clear(ClearType::All))?;
    stdout.execute(cursor_kind.style)?;

    Ok(())
}

fn reset_terminal(mut stdout: &std::io::Stdout) -> Result<()> {
    disable_raw_mode()?;
    stdout.execute(cursor::Show)?;
    stdout.execute(ResetColor)?;
    stdout.execute(LeaveAlternateScreen)?;
    stdout.execute(SetCursorStyle::DefaultUserShape)?;
    stdout.flush()?;

    Ok(())
}

fn print_words(
    x: u16,
    y: u16,
    words: &[String],
    mut stdout: &std::io::Stdout,
    theme: &ThemeColors,
) -> Result<()> {
    stdout
        .execute(MoveTo(x, y))
        .context("Failed to move cursor")?;
    stdout
        .execute(SetForegroundColor(theme.missing))
        .context("Failed to set foreground color")?;
    words.iter().for_each(|word| {
        print!("{} ", word);
    });

    Ok(())
}

fn start_timer(
    duration: u64,
    timer_expired: Arc<AtomicBool>,
    remaining_time: Arc<Mutex<u64>>,
) -> Result<()> {
    let start = Instant::now();
    while start.elapsed().as_secs() < duration {
        if timer_expired.load(Ordering::Relaxed) {
            break;
        }
        let remaining = duration - start.elapsed().as_secs();
        {
            let mut remaining_time = remaining_time
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock remaining time: {}", e))?;
            *remaining_time = remaining;
        }
        thread::sleep(Duration::from_secs(1));
    }
    timer_expired.store(true, Ordering::Relaxed);

    Ok(())
}
