use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyModifiers};

const MAX_LINE_LENGTH: i32 = 70;
const MIN_PADDING: u16 = 4;

pub fn close_typy(code: &KeyCode, modifiers: &KeyModifiers) -> Option<()> {
    match code {
        KeyCode::Esc => Some(()),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Some(()),
        _ => None,
    }
}

/// Returns (x, y, effective_line_length) accounting for terminal width and padding.
pub fn calc_middle_for_text() -> Result<(u16, u16, i32)> {
    let (cols, rows) = crossterm::terminal::size().context("Failed to get terminal size")?;

    // Line length is capped at MAX_LINE_LENGTH but shrinks for narrow terminals
    let usable = (cols as i32) - (MIN_PADDING as i32 * 2);
    let line_length = usable.min(MAX_LINE_LENGTH).max(20);

    let x = (cols / 2).saturating_sub((line_length / 2) as u16);
    let x = x.max(MIN_PADDING);
    let y = rows / 2 - 1;

    Ok((x, y, line_length))
}

/// Recalculate x, y for existing words (line_length stays the same).
pub fn recalc_position(line_length: i32) -> Result<(u16, u16)> {
    let (cols, rows) = crossterm::terminal::size().context("Failed to get terminal size")?;

    let x = (cols / 2).saturating_sub((line_length / 2) as u16);
    let x = x.max(MIN_PADDING);
    let y = rows.saturating_sub(2) / 2;

    Ok((x, y))
}
