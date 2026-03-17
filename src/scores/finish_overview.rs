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

pub fn show_stats(
    mut stdout: &std::io::Stdout,
    stats: Stats,
    theme: &ThemeColors,
    duration: u64,
    language: &str,
    is_personal_best: bool,
) -> Result<PostGameAction> {
    stdout
        .execute(Clear(ClearType::All))
        .context("Failed to clear terminal")?;
    stdout.execute(cursor::Hide)?;

    let (cols, rows) = size()?;

    if is_personal_best {
        draw_confetti(stdout, cols, rows)?;
    }

    // --- Layout: center graph horizontally and everything vertically ---
    let left_width = 15u16;
    let gap = 3u16;
    let graph_width = cols.saturating_sub(30).min(80).max(30);
    let graph_height = rows.saturating_sub(16).min(12).max(6);
    let graph_x = (cols.saturating_sub(graph_width)) / 2;
    let left_x = graph_x.saturating_sub(gap + left_width);

    // Total content height: pb_banner(1) + graph + stats_row(3) + gap(1) + leaderboard(~6) + gap(1) + menu(1) + hint(1)
    let content_height = graph_height + 3 + 1 + 7 + 1 + 1 + 1;
    let graph_y = if is_personal_best {
        rows.saturating_sub(content_height + 1) / 2 + 1 // +1 for banner above
    } else {
        rows.saturating_sub(content_height) / 2
    }
    .max(1);
    let stats_y = graph_y + graph_height + 1;

    // -- Left side stats --
    stdout.execute(MoveTo(left_x, graph_y))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    print!("wpm");
    stdout.execute(MoveTo(left_x, graph_y + 1))?;
    stdout.execute(SetForegroundColor(theme.accent))?;
    print!("{}", stats.wpm() as i32);

    stdout.execute(MoveTo(left_x, graph_y + 3))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    print!("acc");
    stdout.execute(MoveTo(left_x, graph_y + 4))?;
    stdout.execute(SetForegroundColor(theme.accent))?;
    print!("{:.1}%", stats.accuracy());

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

    // -- Graph (centered) --
    let wpm_data = stats.wpm_per_second();
    let raw_wpm_data = stats.raw_wpm_per_second();
    let avg_wpm = stats.wpm();
    let graph_area = Rect::new(graph_x, graph_y, graph_width, graph_height);
    graph::draw_graph(&wpm_data, &raw_wpm_data, &stats.errors_ps, avg_wpm, graph_area)
        .context("Failed to draw graph")?;

    // Re-hide cursor after tui graph (tui Terminal may show it on drop)
    stdout.execute(cursor::Hide)?;

    // -- Graph legend --
    {
        let legend_y = graph_y + graph_height;
        let legend_x = graph_x + 2;
        stdout.execute(MoveTo(legend_x, legend_y))?;
        stdout.execute(SetForegroundColor(Color::Yellow))?;
        print!("--");
        stdout.execute(SetForegroundColor(theme.missing))?;
        print!(" wpm  ");
        stdout.execute(SetForegroundColor(Color::Rgb { r: 100, g: 100, b: 100 }))?;
        print!("--");
        stdout.execute(SetForegroundColor(theme.missing))?;
        print!(" raw  ");
        stdout.execute(SetForegroundColor(Color::Rgb { r: 70, g: 70, b: 70 }))?;
        print!("..");
        stdout.execute(SetForegroundColor(theme.missing))?;
        print!(" avg  ");
        stdout.execute(SetForegroundColor(Color::Red))?;
        print!("*");
        stdout.execute(SetForegroundColor(theme.missing))?;
        print!(" errors");
    }

    // -- Bottom stats row (centered under graph) --
    let correct = stats.correct_chars();
    let incorrect = (stats.incorrect_letters - stats.extra_chars).max(0);
    let extra = stats.extra_chars;
    let consistency = stats.consistency();

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
        format!("time {}", duration),
        format!("{}/{}/{}", correct, incorrect, extra),
        format!("{:.0}%", consistency),
        format!("{}s", duration),
    ];
    for (i, val) in values.iter().enumerate() {
        stdout.execute(MoveTo(left_x + (i as u16) * col_width, stats_y + 1))?;
        print!("{}", val);
    }

    stdout.execute(SetForegroundColor(theme.accent))?;
    stdout.execute(MoveTo(left_x, stats_y + 2))?;
    print!("{}", language);

    // -- Leaderboard --
    let lb_y = stats_y + 4;
    if lb_y + 2 < rows.saturating_sub(4) {
        draw_leaderboard(stdout, theme, left_x, lb_y)?;
    }

    stdout.flush()?;

    // -- Interactive menu (arrows only, no letter shortcuts) --
    let menu_items = build_menu(duration, language);
    let mut selected: usize = 0; // "replay" preselected
    let menu_y = rows.saturating_sub(3);

    draw_menu(stdout, theme, &menu_items, selected, menu_y, cols)?;

    // hint line
    let hint_y = rows.saturating_sub(1);
    let hint = "< > select   enter/tab confirm   esc quit";
    let hx = cols / 2 - hint.len() as u16 / 2;
    stdout.execute(MoveTo(hx, hint_y))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    print!("{}", hint);
    stdout.flush()?;

    // -- Input loop (arrows + enter/tab/esc only) --
    loop {
        if let Ok(Event::Key(KeyEvent { code, .. })) = read() {
            match code {
                KeyCode::Esc => return Ok(PostGameAction::Quit),
                KeyCode::Left => {
                    if selected > 0 {
                        selected -= 1;
                    }
                    draw_menu(stdout, theme, &menu_items, selected, menu_y, cols)?;
                    stdout.flush()?;
                }
                KeyCode::Right => {
                    if selected < menu_items.len() - 1 {
                        selected += 1;
                    }
                    draw_menu(stdout, theme, &menu_items, selected, menu_y, cols)?;
                    stdout.flush()?;
                }
                KeyCode::Tab | KeyCode::Enter => {
                    return Ok(menu_items.into_iter().nth(selected).unwrap().action);
                }
                _ => {}
            }
        }
    }
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
    // Calculate total width: each item is " label " (unselected) or "[label]" (selected)
    let total_len: usize = items
        .iter()
        .map(|item| item.label.len() + 4) // padding/brackets + spacing
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
            score.wpm, score.accuracy, score.get_human_time()
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
