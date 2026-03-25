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
    is_repeat: bool,
) -> Result<()> {
    stdout.execute(Clear(ClearType::All))?;

    for (i, words) in game.list.iter().enumerate() {
        print_words(line_xs[i], y + i as u16, words, stdout, theme)?;
    }

    // Timer
    stdout.execute(MoveTo(timer_x, y.saturating_sub(2)))?;
    stdout.execute(SetForegroundColor(theme.accent))?;
    print!("{:02}", remaining);

    // Unranked label
    if is_repeat {
        let (cols, _) = crossterm::terminal::size()?;
        let label = "unranked";
        let label_x = (cols / 2).saturating_sub(label.len() as u16 / 2);
        stdout.execute(MoveTo(label_x, y.saturating_sub(3)))?;
        stdout.execute(SetForegroundColor(crossterm::style::Color::Rgb { r: 120, g: 120, b: 120 }))?;
        print!("{}", label);
    }

    // Cursor
    let cx = line_xs[game.player.position_y as usize];
    stdout.execute(MoveTo(
        cx + game.player.position_x as u16,
        y + game.player.position_y as u16,
    ))?;
    stdout.flush()?;

    Ok(())
}

pub fn run(
    mode: Mode,
    theme: ThemeColors,
    lang_override: Option<String>,
    repeat_words: Option<Vec<Vec<String>>>,
    ghost_data: Option<Vec<super::GhostFrame>>,
) -> Result<super::PostGameAction> {
    let mut stdout = stdout();

    let language = match lang_override {
        Some(lang) => language::Language { lang },
        None => language::Language::new(),
    };

    setup_terminal(&stdout).context("Failed to setup terminal")?;

    let (_, init_y, line_length) =
        super::calc_middle_for_text().context("Failed to calculate terminal size")?;

    let mut y = init_y;

    let is_repeat = repeat_words.is_some();

    let mut game = if let Some(words) = repeat_words {
        Game::new(words)
    } else {
        let mut g = Game::new(
            word_provider::get_words(&language.lang, line_length)
                .context("Failed to get words from file")?,
        );
        mode.transform(&mut g.list);
        g
    };

    // Save the clean word list for potential repeat
    let clean_words = game.list.clone();

    let duration = mode.duration;
    let lang_name = language.lang.clone();
    let mut stats = Stats::new();

    // Ghost recording (built during this run)
    let mut ghost_frames: Vec<super::GhostFrame> = Vec::new();
    let mut game_start_instant: Option<Instant> = None;

    // Ghost playback state
    let ghost_color = crossterm::style::Color::Rgb { r: 60, g: 160, b: 180 };
    let mut prev_ghost_pos: Option<(i32, i32)> = None;

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

        // Show "unranked" label for repeat mode
        if is_repeat {
            let label = "unranked";
            let label_x = (cols / 2).saturating_sub(label.len() as u16 / 2);
            stdout.execute(MoveTo(label_x, y.saturating_sub(3)))?;
            stdout.execute(SetForegroundColor(crossterm::style::Color::Rgb { r: 120, g: 120, b: 120 }))?;
            print!("{}", label);
        }

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

        // Ghost cursor rendering
        if let Some(ref ghost) = ghost_data {
            if let Some(start) = game_start_instant {
                let elapsed = start.elapsed().as_millis() as u64;
                let frame = ghost.iter().rev().find(|f| f.elapsed_ms <= elapsed);

                if let Some(frame) = frame {
                    let gx = frame.position_x;
                    let gy = frame.position_y;

                    // Clear previous ghost position if player hasn't typed past it
                    if let Some((pgx, pgy)) = prev_ghost_pos {
                        if (pgx, pgy) != (gx, gy) {
                            let player_past = game.player.position_y > pgy
                                || (game.player.position_y == pgy && game.player.position_x > pgx);
                            if !player_past && (pgy as usize) < game.original_list.len() {
                                let orig_line = game.original_list[pgy as usize].join(" ");
                                if let Some(ch) = orig_line.chars().nth(pgx as usize) {
                                    let lx = line_xs[pgy as usize];
                                    stdout.execute(MoveTo(lx + pgx as u16, y + pgy as u16))?;
                                    stdout.execute(SetForegroundColor(theme.missing))?;
                                    print!("{}", ch);
                                }
                            }
                        }
                    }

                    // Paint new ghost position if player hasn't typed past it
                    let player_past = game.player.position_y > gy
                        || (game.player.position_y == gy && game.player.position_x > gx);
                    if !player_past && (gy as usize) < game.original_list.len() {
                        let orig_line = game.original_list[gy as usize].join(" ");
                        if let Some(ch) = orig_line.chars().nth(gx as usize) {
                            let lx = line_xs[gy as usize];
                            stdout.execute(MoveTo(lx + gx as u16, y + gy as u16))?;
                            stdout.execute(SetForegroundColor(ghost_color))?;
                            print!("{}", ch);
                        }
                    }

                    // Show ghost diff indicator
                    let ghost_total = total_chars(gx, gy, &game.original_list);
                    let player_total = total_chars(
                        game.player.position_x,
                        game.player.position_y,
                        &game.original_list,
                    );
                    let diff = player_total - ghost_total;
                    let diff_label = if diff > 0 {
                        format!("+{}", diff)
                    } else if diff < 0 {
                        format!("{}", diff)
                    } else {
                        "=".to_string()
                    };
                    let (cur_cols, _) = crossterm::terminal::size()?;
                    let ghost_label = format!("ghost: {}  ", diff_label);
                    let diff_x = (cur_cols / 2).saturating_sub(ghost_label.len() as u16 / 2);
                    stdout.execute(MoveTo(diff_x, y + game.list.len() as u16 + 1))?;
                    let diff_color = if diff > 0 {
                        crossterm::style::Color::Green
                    } else if diff < 0 {
                        crossterm::style::Color::Red
                    } else {
                        crossterm::style::Color::Rgb { r: 120, g: 120, b: 120 }
                    };
                    stdout.execute(SetForegroundColor(diff_color))?;
                    print!("{}", ghost_label);

                    prev_ghost_pos = Some((gx, gy));

                    // Restore cursor to player position
                    let cx = line_xs[game.player.position_y as usize];
                    stdout.execute(MoveTo(
                        cx + game.player.position_x as u16,
                        y + game.player.position_y as u16,
                    ))?;
                    stdout.flush()?;
                }
            }
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
                        game_start_instant = Some(Instant::now());
                    }
                    let prev_x = game.player.position_x;
                    let prev_y = game.player.position_y;
                    match handle_input(&mut game, &stdout, code, &mut stats, &theme, &line_xs, y)? {
                        InputAction::Continue => {
                            // Record ghost frame if position changed
                            if let Some(start) = game_start_instant {
                                if game.player.position_x != prev_x || game.player.position_y != prev_y {
                                    ghost_frames.push(super::GhostFrame {
                                        elapsed_ms: start.elapsed().as_millis() as u64,
                                        position_x: game.player.position_x,
                                        position_y: game.player.position_y,
                                    });
                                }
                            }
                            continue;
                        }
                        InputAction::Break => break,
                        InputAction::None => {}
                    }
                    // Record ghost frame if position changed
                    if let Some(start) = game_start_instant {
                        if game.player.position_x != prev_x || game.player.position_y != prev_y {
                            ghost_frames.push(super::GhostFrame {
                                elapsed_ms: start.elapsed().as_millis() as u64,
                                position_x: game.player.position_x,
                                position_y: game.player.position_y,
                            });
                        }
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
                    redraw_game(&mut stdout, &game, &theme, &line_xs, y, timer_x, remaining, is_repeat)?;
                }
                _ => {}
            }
        }
    }

    let action = if !game.quit {
        stdout.execute(cursor::Hide)?;

        let is_pb = if is_repeat {
            false // Repeats never count as PB
        } else {
            let current_wpm = stats.wpm() as u32;
            let current_acc = stats.accuracy() as f32;
            if current_wpm > 0 {
                match Data::get_scores() {
                    Ok(scores) => {
                        let cat_scores: Vec<&Score> = scores
                            .iter()
                            .filter(|s| s.language == lang_name && s.duration == duration)
                            .collect();
                        if cat_scores.is_empty() {
                            true
                        } else {
                            let best_wpm = cat_scores.iter().map(|s| s.wpm).max().unwrap_or(0);
                            if current_wpm > best_wpm {
                                true
                            } else if current_wpm == best_wpm {
                                let best_acc_at_wpm = cat_scores
                                    .iter()
                                    .filter(|s| s.wpm == best_wpm)
                                    .map(|s| s.accuracy)
                                    .fold(0.0f32, f32::max);
                                current_acc > best_acc_at_wpm
                            } else {
                                false
                            }
                        }
                    }
                    Err(_) => true,
                }
            } else {
                false
            }
        };

        if !is_repeat {
            let score = Score::new(
                stats.wpm() as u32,
                stats.raw_wpm() as u32,
                stats.accuracy() as f32,
                lang_name.clone(),
                duration,
            );
            Data::save_data(score).context("Failed to save data")?;
        }

        finish_overview::show_stats(
            &stdout,
            stats,
            &theme,
            duration,
            &lang_name,
            is_pb,
            &clean_words,
            ghost_frames,
            is_repeat,
        )
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

/// Calculate total character position for ghost comparison.
fn total_chars(x: i32, y: i32, words: &[Vec<String>]) -> i32 {
    let mut total = 0;
    for i in 0..y as usize {
        if i < words.len() {
            total += words[i].join(" ").chars().count() as i32;
        }
    }
    total + x
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
