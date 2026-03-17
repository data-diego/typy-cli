use crate::scores::graph;
use crate::scores::progress::Data;
use anyhow::{Context, Result};
use crossterm::cursor::MoveTo;
use crossterm::event::{read, Event, KeyEvent};
use crossterm::style::{Color, SetForegroundColor};
use crossterm::terminal::{size, Clear, ClearType};
use crossterm::ExecutableCommand;
use rand::seq::IndexedRandom;
use rand::Rng;
use std::io::Write;
use std::thread;
use std::time::Duration;
use tui::layout::Rect;

use crate::config::theme::ThemeColors;
use crate::scores::Stats;
use crate::terminal;

pub fn show_stats(
    mut stdout: &std::io::Stdout,
    stats: Stats,
    theme: &ThemeColors,
    duration: u64,
    language: &str,
    is_personal_best: bool,
) -> Result<()> {
    stdout
        .execute(Clear(ClearType::All))
        .context("Failed to clear terminal")?;

    let (cols, rows) = size()?;

    if is_personal_best {
        draw_confetti(stdout, cols, rows)?;
    }

    // Layout
    let left_x = 3u16;
    let graph_x = 18u16;
    let graph_width = cols.saturating_sub(graph_x + 3).min(80);
    let graph_height = rows.saturating_sub(14).min(12).max(6);
    let graph_y = 2u16;
    let stats_y = graph_y + graph_height + 1;

    // -- Left side stats --

    // WPM
    stdout.execute(MoveTo(left_x, graph_y))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    print!("wpm");
    stdout.execute(MoveTo(left_x, graph_y + 1))?;
    stdout.execute(SetForegroundColor(theme.accent))?;
    print!("{}", stats.wpm() as i32);

    // ACC
    stdout.execute(MoveTo(left_x, graph_y + 3))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    print!("acc");
    stdout.execute(MoveTo(left_x, graph_y + 4))?;
    stdout.execute(SetForegroundColor(theme.accent))?;
    print!("{:.1}%", stats.accuracy());

    // RAW
    stdout.execute(MoveTo(left_x, graph_y + 6))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    print!("raw");
    stdout.execute(MoveTo(left_x, graph_y + 7))?;
    stdout.execute(SetForegroundColor(theme.accent))?;
    print!("{}", stats.raw_wpm() as i32);

    // Personal best banner
    if is_personal_best {
        let banner = "*** NEW PERSONAL BEST! ***";
        let bx = cols / 2 - banner.len() as u16 / 2;
        stdout.execute(MoveTo(bx, 0))?;
        stdout.execute(SetForegroundColor(Color::Yellow))?;
        print!("{}", banner);
    }

    stdout.flush()?;

    // -- Graph --
    let wpm_data = stats.wpm_per_second();
    let raw_wpm_data = stats.raw_wpm_per_second();
    let avg_wpm = stats.wpm();
    let graph_area = Rect::new(graph_x, graph_y, graph_width, graph_height);
    graph::draw_graph(&wpm_data, &raw_wpm_data, &stats.errors_ps, avg_wpm, graph_area)
        .context("Failed to draw graph")?;

    // -- Bottom stats row --
    let correct = stats.correct_chars();
    let incorrect = (stats.incorrect_letters - stats.extra_chars).max(0);
    let extra = stats.extra_chars;
    let consistency = stats.consistency();

    let col_width = (cols / 5).max(14);

    // Labels
    stdout.execute(SetForegroundColor(theme.missing))?;
    let labels = ["test type", "characters", "consistency", "time"];
    for (i, label) in labels.iter().enumerate() {
        stdout.execute(MoveTo(left_x + (i as u16) * col_width, stats_y))?;
        print!("{}", label);
    }

    // Values
    stdout.execute(SetForegroundColor(theme.accent))?;
    let values = [
        format!("time {}", duration),
        format!("{}/{}/{}", correct, incorrect, extra),
        format!("{:.0}%", consistency),
        format!("{}s", duration),
    ];
    for (i, val) in values.iter().enumerate() {
        stdout.execute(MoveTo(left_x + (i as u16) * col_width, stats_y + 1))?;
        print!("{}", val);
    }

    // Language under test type
    stdout.execute(SetForegroundColor(theme.accent))?;
    stdout.execute(MoveTo(left_x, stats_y + 2))?;
    print!("{}", language);

    // -- Leaderboard --
    let lb_y = stats_y + 4;
    if lb_y + 2 < rows {
        draw_leaderboard(stdout, theme, left_x, lb_y)?;
    }

    stdout.flush()?;

    loop {
        if let Ok(Event::Key(KeyEvent {
            code, modifiers, ..
        })) = read()
        {
            if terminal::close_typy(&code, &modifiers).is_some() {
                break;
            }
        }
    }

    Ok(())
}

fn draw_leaderboard(
    mut stdout: &std::io::Stdout,
    theme: &ThemeColors,
    x: u16,
    y: u16,
) -> Result<()> {
    let scores = Data::get_scores().unwrap_or_else(|_| Vec::new());
    if scores.is_empty() {
        return Ok(());
    }

    let mut sorted = scores;
    sorted.sort_by(|a, b| b.wpm.cmp(&a.wpm));
    sorted.truncate(5);

    stdout.execute(MoveTo(x, y))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    print!("--- leaderboard ---");

    for (i, score) in sorted.iter().enumerate() {
        let row = y + 1 + i as u16;
        stdout.execute(MoveTo(x, row))?;
        stdout.execute(SetForegroundColor(theme.accent))?;
        print!("#{}", i + 1);
        stdout.execute(SetForegroundColor(theme.fg))?;
        print!(
            "  {} wpm   {:.1}% acc   {}",
            score.wpm,
            score.accuracy,
            score.get_date()
        );
    }

    Ok(())
}

fn draw_confetti(mut stdout: &std::io::Stdout, cols: u16, rows: u16) -> Result<()> {
    let confetti_chars = ['*', '+', '.', 'o', 'x', '#', '~'];
    let colors = [
        Color::Yellow,
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
    ];
    let mut rng = rand::rng();

    for _ in 0..4 {
        for _ in 0..50 {
            let cx: u16 = rng.random_range(0..cols);
            let cy: u16 = rng.random_range(0..rows);
            let ch = confetti_chars.choose(&mut rng).unwrap();
            let color = colors.choose(&mut rng).unwrap();
            stdout.execute(MoveTo(cx, cy))?;
            stdout.execute(SetForegroundColor(*color))?;
            print!("{}", ch);
        }
        stdout.flush()?;
        thread::sleep(Duration::from_millis(150));
    }

    stdout.execute(Clear(ClearType::All))?;
    Ok(())
}
