use crate::scores::graph;
use crate::scores::progress::Data;
use anyhow::{Context, Result};
use crossterm::cursor::{self, MoveTo};
use crossterm::event::{read, Event, KeyCode, KeyEvent};
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
use crate::terminal::PostGameAction;

struct MenuItem {
    label: String,
    action: PostGameAction,
}

/// All the data needed to draw the results screen (so we can redraw on resize)
struct DrawData {
    wpm: i32,
    accuracy: f64,
    raw_wpm: i32,
    correct: i32,
    incorrect: i32,
    extra: i32,
    consistency: f64,
    wpm_data: Vec<f64>,
    raw_wpm_data: Vec<f64>,
    active_errors: Vec<i32>,
    avg_wpm: f64,
    duration: u64,
    language: String,
    is_personal_best: bool,
}

const CONTENT_PAD_LEFT: u16 = 4;
const GRAPH_PAD_Y: u16 = 1;

pub fn show_stats(
    mut stdout: &std::io::Stdout,
    stats: Stats,
    theme: &ThemeColors,
    duration: u64,
    language: &str,
    is_personal_best: bool,
) -> Result<PostGameAction> {
    // Pre-compute all data so we can redraw on resize
    let data = DrawData {
        wpm: stats.wpm() as i32,
        accuracy: stats.accuracy(),
        raw_wpm: stats.raw_wpm() as i32,
        correct: stats.correct_chars(),
        incorrect: (stats.incorrect_letters - stats.extra_chars).max(0),
        extra: stats.extra_chars,
        consistency: stats.consistency(),
        wpm_data: stats.wpm_per_second(),
        raw_wpm_data: stats.raw_wpm_per_second(),
        active_errors: stats.active_errors_ps(),
        avg_wpm: stats.wpm(),
        duration,
        language: language.to_string(),
        is_personal_best,
    };

    if is_personal_best {
        stdout.execute(Clear(ClearType::All))?;
        stdout.execute(cursor::Hide)?;
        let (cols, rows) = size()?;
        draw_confetti(stdout, cols, rows)?;
    }

    let menu_items = build_menu(duration, language);
    let mut selected: usize = 0;

    // Initial draw
    draw_all(stdout, theme, &data, &menu_items, selected)?;

    // Input loop with resize handling
    loop {
        if let Ok(event) = read() {
            match event {
                Event::Key(KeyEvent { code, .. }) => match code {
                    KeyCode::Esc => return Ok(PostGameAction::Quit),
                    KeyCode::Left => {
                        if selected > 0 {
                            selected -= 1;
                        }
                        let (cols, _) = size()?;
                        let menu_y = size()?.1.saturating_sub(3);
                        draw_menu(stdout, theme, &menu_items, selected, menu_y, cols)?;
                        stdout.flush()?;
                    }
                    KeyCode::Right => {
                        if selected < menu_items.len() - 1 {
                            selected += 1;
                        }
                        let (cols, _) = size()?;
                        let menu_y = size()?.1.saturating_sub(3);
                        draw_menu(stdout, theme, &menu_items, selected, menu_y, cols)?;
                        stdout.flush()?;
                    }
                    KeyCode::Tab | KeyCode::Enter => {
                        return Ok(menu_items.into_iter().nth(selected).unwrap().action);
                    }
                    _ => {}
                },
                Event::Resize(_, _) => {
                    // Redraw everything on terminal resize
                    draw_all(stdout, theme, &data, &menu_items, selected)?;
                }
                _ => {}
            }
        }
    }
}

fn draw_all(
    mut stdout: &std::io::Stdout,
    theme: &ThemeColors,
    data: &DrawData,
    menu_items: &[MenuItem],
    selected: usize,
) -> Result<()> {
    stdout.execute(Clear(ClearType::All))?;
    stdout.execute(cursor::Hide)?;

    let (cols, rows) = size()?;

    // --- Layout with padding ---
    let left_width = 15u16;
    let gap = 3u16;
    let graph_width = cols.saturating_sub(30).min(80).max(30);
    let graph_height = rows.saturating_sub(18).min(12).max(6);
    let graph_x = (cols.saturating_sub(graph_width)) / 2;
    let left_x = graph_x.saturating_sub(gap + left_width).max(CONTENT_PAD_LEFT);

    // Vertical centering
    let content_height = graph_height + GRAPH_PAD_Y + 3 + 1 + 7 + 1 + 1 + 1;
    let graph_y = if data.is_personal_best {
        rows.saturating_sub(content_height + 1) / 2 + 1
    } else {
        rows.saturating_sub(content_height) / 2
    }
    .max(1 + GRAPH_PAD_Y);
    let stats_y = graph_y + graph_height + GRAPH_PAD_Y + 1;

    // -- Left side stats --
    stdout.execute(MoveTo(left_x, graph_y + GRAPH_PAD_Y))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    print!("wpm");
    stdout.execute(MoveTo(left_x, graph_y + GRAPH_PAD_Y + 1))?;
    stdout.execute(SetForegroundColor(theme.accent))?;
    print!("{}", data.wpm);

    stdout.execute(MoveTo(left_x, graph_y + GRAPH_PAD_Y + 3))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    print!("acc");
    stdout.execute(MoveTo(left_x, graph_y + GRAPH_PAD_Y + 4))?;
    stdout.execute(SetForegroundColor(theme.accent))?;
    print!("{:.1}%", data.accuracy);

    stdout.execute(MoveTo(left_x, graph_y + GRAPH_PAD_Y + 6))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    print!("raw");
    stdout.execute(MoveTo(left_x, graph_y + GRAPH_PAD_Y + 7))?;
    stdout.execute(SetForegroundColor(theme.accent))?;
    print!("{}", data.raw_wpm);

    // Personal best banner
    if data.is_personal_best {
        let banner = "*** NEW PERSONAL BEST! ***";
        let bx = cols / 2 - banner.len() as u16 / 2;
        stdout.execute(MoveTo(bx, graph_y.saturating_sub(1)))?;
        stdout.execute(SetForegroundColor(Color::Yellow))?;
        print!("{}", banner);
    }

    stdout.flush()?;

    // -- Graph (centered with padding) --
    let graph_area = Rect::new(graph_x, graph_y, graph_width, graph_height);
    graph::draw_graph(
        &data.wpm_data,
        &data.raw_wpm_data,
        &data.active_errors,
        data.avg_wpm,
        graph_area,
    )
    .context("Failed to draw graph")?;

    // Re-hide cursor after tui graph
    stdout.execute(cursor::Hide)?;

    // -- Graph legend --
    {
        let legend_y = graph_y + graph_height;
        let legend_x = graph_x + 2;
        stdout.execute(MoveTo(legend_x, legend_y))?;
        stdout.execute(SetForegroundColor(Color::Yellow))?;
        print!("--");
        stdout.execute(SetForegroundColor(theme.missing))?;
        print!(" wpm   ");
        stdout.execute(SetForegroundColor(Color::Rgb {
            r: 100,
            g: 100,
            b: 100,
        }))?;
        print!("--");
        stdout.execute(SetForegroundColor(theme.missing))?;
        print!(" raw   ");
        stdout.execute(SetForegroundColor(Color::Red))?;
        print!("*");
        stdout.execute(SetForegroundColor(theme.missing))?;
        print!(" errors");
    }

    // -- Bottom stats row --
    let stats_total_width = graph_width + gap + left_width;
    let col_width = stats_total_width / 4;

    stdout.execute(SetForegroundColor(theme.missing))?;
    let labels = ["test type", "characters", "consistency", "time"];
    for (i, label) in labels.iter().enumerate() {
        stdout.execute(MoveTo(left_x + (i as u16) * col_width, stats_y))?;
        print!("{}", label);
    }

    stdout.execute(SetForegroundColor(theme.accent))?;
    let values = [
        format!("time {}", data.duration),
        format!("{}/{}/{}", data.correct, data.incorrect, data.extra),
        format!("{:.0}%", data.consistency),
        format!("{}s", data.duration),
    ];
    for (i, val) in values.iter().enumerate() {
        stdout.execute(MoveTo(left_x + (i as u16) * col_width, stats_y + 1))?;
        print!("{}", val);
    }

    stdout.execute(SetForegroundColor(theme.accent))?;
    stdout.execute(MoveTo(left_x, stats_y + 2))?;
    print!("{}", data.language);

    // -- Leaderboard --
    let lb_y = stats_y + 4;
    if lb_y + 2 < rows.saturating_sub(4) {
        draw_leaderboard(stdout, theme, left_x, lb_y)?;
    }

    // -- Menu --
    let menu_y = rows.saturating_sub(3);
    draw_menu(stdout, theme, menu_items, selected, menu_y, cols)?;

    // -- Hint line --
    let hint_y = rows.saturating_sub(1);
    let hint = "< > select   enter/tab confirm   esc quit";
    let hx = cols / 2 - hint.len() as u16 / 2;
    stdout.execute(MoveTo(hx, hint_y))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    print!("{}", hint);

    stdout.flush()?;
    Ok(())
}

fn build_menu(duration: u64, language: &str) -> Vec<MenuItem> {
    vec![
        MenuItem {
            label: "replay".to_string(),
            action: PostGameAction::Replay {
                duration,
                lang: language.to_string(),
            },
        },
        MenuItem {
            label: "15s".to_string(),
            action: PostGameAction::Replay {
                duration: 15,
                lang: language.to_string(),
            },
        },
        MenuItem {
            label: "30s".to_string(),
            action: PostGameAction::Replay {
                duration: 30,
                lang: language.to_string(),
            },
        },
        MenuItem {
            label: "60s".to_string(),
            action: PostGameAction::Replay {
                duration: 60,
                lang: language.to_string(),
            },
        },
        MenuItem {
            label: "120s".to_string(),
            action: PostGameAction::Replay {
                duration: 120,
                lang: language.to_string(),
            },
        },
        MenuItem {
            label: "english".to_string(),
            action: PostGameAction::Replay {
                duration,
                lang: "english".to_string(),
            },
        },
        MenuItem {
            label: "spanish".to_string(),
            action: PostGameAction::Replay {
                duration,
                lang: "spanish".to_string(),
            },
        },
        MenuItem {
            label: "quit".to_string(),
            action: PostGameAction::Quit,
        },
    ]
}

fn draw_menu(
    mut stdout: &std::io::Stdout,
    theme: &ThemeColors,
    items: &[MenuItem],
    selected: usize,
    y: u16,
    cols: u16,
) -> Result<()> {
    let total_len: usize = items
        .iter()
        .map(|item| item.label.len() + 4)
        .sum::<usize>();

    let start_x = (cols as usize).saturating_sub(total_len) / 2;

    // Clear the menu line
    stdout.execute(MoveTo(0, y))?;
    print!("{}", " ".repeat(cols as usize));

    let mut cx = start_x as u16;

    for (i, item) in items.iter().enumerate() {
        stdout.execute(MoveTo(cx, y))?;

        if i == selected {
            stdout.execute(SetForegroundColor(theme.accent))?;
            print!("[{}]", item.label);
            cx += item.label.len() as u16 + 2;
        } else {
            stdout.execute(SetForegroundColor(theme.missing))?;
            print!(" {} ", item.label);
            cx += item.label.len() as u16 + 2;
        }

        if i < items.len() - 1 {
            cx += 1;
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
            score.get_human_time()
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

    // Build up confetti over multiple frames
    for frame in 0..6 {
        let count = 30 + frame * 15; // more particles each frame
        for _ in 0..count {
            let cx: u16 = rng.random_range(0..cols);
            let cy: u16 = rng.random_range(0..rows);
            let ch = confetti_chars.choose(&mut rng).unwrap();
            let color = colors.choose(&mut rng).unwrap();
            stdout.execute(MoveTo(cx, cy))?;
            stdout.execute(SetForegroundColor(*color))?;
            print!("{}", ch);
        }
        stdout.flush()?;
        thread::sleep(Duration::from_millis(120));
    }

    // Show the "NEW PERSONAL BEST!" banner on top of confetti
    let banner = "*** NEW PERSONAL BEST! ***";
    let bx = cols / 2 - banner.len() as u16 / 2;
    let by = rows / 2;
    stdout.execute(MoveTo(bx, by))?;
    stdout.execute(SetForegroundColor(Color::Yellow))?;
    print!("{}", banner);
    stdout.flush()?;

    // Hold for a moment so the user can see it
    thread::sleep(Duration::from_millis(800));

    stdout.execute(Clear(ClearType::All))?;
    Ok(())
}
