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
    if wpm_data.is_empty() {
        return Ok(());
    }

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // --- Build data points ---
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

    // Errors shown as red markers at y=0 (bottom of graph)
    let error_points: Vec<(f64, f64)> = errors
        .iter()
        .enumerate()
        .filter(|(_, &e)| e > 0)
        .map(|(i, &e)| ((i + 1) as f64, e as f64))
        .collect();

    // Average WPM horizontal line
    let max_time = wpm_data.len() as f64;
    let avg_line: Vec<(f64, f64)> = vec![(1.0, avg_wpm), (max_time, avg_wpm)];

    // --- Axis bounds ---
    let all_wpm: f64 = raw_wpm_data
        .iter()
        .chain(wpm_data.iter())
        .copied()
        .fold(0.0f64, f64::max)
        .max(avg_wpm);

    // Nice round Y max (nearest 20 above)
    let y_max = ((all_wpm / 20.0).ceil() * 20.0).max(40.0);

    // Error Y max (right axis concept - errors are small integers)
    let max_errors = errors.iter().copied().max().unwrap_or(0).max(1) as f64;
    // Scale errors to fit in bottom 15% of graph
    let error_scale = y_max * 0.15 / max_errors;
    let scaled_error_points: Vec<(f64, f64)> = error_points
        .iter()
        .map(|(x, e)| (*x, e * error_scale))
        .collect();

    // --- Axis labels ---
    // Y axis: 0, mid, max
    let y_mid = (y_max / 2.0) as i32;
    let y_max_label = format!("{}", y_max as i32);
    let y_mid_label = format!("{}", y_mid);

    // X axis: show labels at regular intervals
    let duration = max_time as i32;
    let x_step = if duration <= 15 {
        5
    } else if duration <= 30 {
        5
    } else if duration <= 60 {
        10
    } else {
        30
    };
    let mut x_labels: Vec<Span> = Vec::new();
    let x_label_count = (duration / x_step) + 1;
    for i in 0..x_label_count {
        x_labels.push(Span::from(format!("{}", i * x_step)));
    }
    // Make sure last label is the actual duration
    if duration % x_step != 0 {
        x_labels.push(Span::from(format!("{}", duration)));
    }

    terminal.draw(|f| {
        let mut datasets = vec![
            // Raw WPM (grey, background)
            Dataset::default()
                .name("raw wpm")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(tui::style::Color::Rgb(100, 100, 100)))
                .data(&raw_points),
            // Net WPM (yellow, foreground)
            Dataset::default()
                .name("wpm")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(tui::style::Color::Yellow))
                .data(&wpm_points),
            // Average line (dim dotted)
            Dataset::default()
                .name("avg")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(tui::style::Color::Rgb(70, 70, 70)))
                .data(&avg_line),
        ];

        // Error markers (red dots at bottom)
        if !scaled_error_points.is_empty() {
            datasets.push(
                Dataset::default()
                    .name("errors")
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Scatter)
                    .style(Style::default().fg(tui::style::Color::Red))
                    .data(&scaled_error_points),
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
                    .labels(x_labels),
            )
            .y_axis(
                Axis::default()
                    .title(Span::styled(
                        "wpm",
                        Style::default().fg(tui::style::Color::DarkGray),
                    ))
                    .style(Style::default().fg(tui::style::Color::DarkGray))
                    .bounds([0.0, y_max])
                    .labels(vec![
                        Span::styled("0", Style::default().fg(tui::style::Color::DarkGray)),
                        Span::styled(
                            y_mid_label,
                            Style::default().fg(tui::style::Color::DarkGray),
                        ),
                        Span::styled(
                            y_max_label,
                            Style::default().fg(tui::style::Color::DarkGray),
                        ),
                    ]),
            );

        f.render_widget(chart, area);
    })?;

    Ok(())
}
