use std::io;
use tui::backend::CrosstermBackend;
use tui::layout::Rect;
use tui::style::Style;
use tui::symbols;
use tui::text::Span;
use tui::widgets::{Axis, Chart, Dataset, GraphType};
use tui::Terminal;

use anyhow::Result;

pub fn draw_graph(
    wpm_data: &[f64],
    raw_wpm_data: &[f64],
    errors: &[i32],
    avg_wpm: f64,
    area: Rect,
) -> Result<(), io::Error> {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let wpm_points: Vec<(f64, f64)> = wpm_data
        .iter()
        .enumerate()
        .map(|(i, &v)| ((i + 1) as f64, v))
        .collect();

    let raw_points: Vec<(f64, f64)> = raw_wpm_data
        .iter()
        .enumerate()
        .map(|(i, &v)| ((i + 1) as f64, v))
        .collect();

    let error_points: Vec<(f64, f64)> = errors
        .iter()
        .enumerate()
        .filter(|(_, &e)| e > 0)
        .map(|(i, _)| {
            let wpm_val = if i < wpm_data.len() {
                wpm_data[i]
            } else {
                0.0
            };
            ((i + 1) as f64, wpm_val)
        })
        .collect();

    let avg_line: Vec<(f64, f64)> = if !wpm_data.is_empty() {
        vec![(1.0, avg_wpm), (wpm_data.len() as f64, avg_wpm)]
    } else {
        vec![]
    };

    let max_time = wpm_data.len().max(1) as f64;
    let max_wpm = raw_wpm_data
        .iter()
        .chain(wpm_data.iter())
        .copied()
        .fold(0.0f64, f64::max)
        .max(avg_wpm)
        * 1.2;
    let max_wpm = if max_wpm == 0.0 { 100.0 } else { max_wpm };

    let time_label = format!("{}", max_time as i32);
    let wpm_label = format!("{}", max_wpm as i32);

    terminal.draw(|f| {
        let mut datasets = vec![
            Dataset::default()
                .name("raw")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(tui::style::Color::DarkGray))
                .data(&raw_points),
            Dataset::default()
                .name("wpm")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(tui::style::Color::Yellow))
                .data(&wpm_points),
        ];

        if !avg_line.is_empty() {
            datasets.push(
                Dataset::default()
                    .name("avg")
                    .marker(symbols::Marker::Dot)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(tui::style::Color::Rgb(80, 80, 80)))
                    .data(&avg_line),
            );
        }

        if !error_points.is_empty() {
            datasets.push(
                Dataset::default()
                    .name("err")
                    .marker(symbols::Marker::Dot)
                    .graph_type(GraphType::Scatter)
                    .style(Style::default().fg(tui::style::Color::Red))
                    .data(&error_points),
            );
        }

        let chart = Chart::new(datasets)
            .x_axis(
                Axis::default()
                    .title(Span::styled(
                        "time (s)",
                        Style::default().fg(tui::style::Color::DarkGray),
                    ))
                    .style(Style::default().fg(tui::style::Color::DarkGray))
                    .bounds([0.0, max_time + 1.0])
                    .labels(
                        ["0", &time_label]
                            .iter()
                            .cloned()
                            .map(Span::from)
                            .collect(),
                    ),
            )
            .y_axis(
                Axis::default()
                    .title(Span::styled(
                        "wpm",
                        Style::default().fg(tui::style::Color::DarkGray),
                    ))
                    .style(Style::default().fg(tui::style::Color::DarkGray))
                    .bounds([0.0, max_wpm])
                    .labels(
                        ["0", &wpm_label]
                            .iter()
                            .cloned()
                            .map(Span::from)
                            .collect(),
                    ),
            );

        f.render_widget(chart, area);
    })?;

    Ok(())
}
