use std::io::Write;

use anyhow::{Context, Result};
use crossterm::event::KeyCode;
use crossterm::style::{Attribute, SetForegroundColor};
use crossterm::ExecutableCommand;
use crossterm::{cursor::MoveTo, style::SetAttribute};

use crate::{config::theme::ThemeColors, scores::Stats};

use super::Game;

const MAX_WORD_LENGTH: usize = 100;

pub enum InputAction {
    Continue,
    Break,
    None,
}

pub fn handle_input(
    game: &mut Game,
    mut stdout: &std::io::Stdout,
    code: KeyCode,
    stats: &mut Stats,
    theme: &ThemeColors,
    x: u16,
    y: u16,
) -> Result<InputAction> {
    match code {
        KeyCode::Char(c) => {
            if c == ' ' {
                match handle_space(game, stdout, x, y)? {
                    InputAction::Continue => return Ok(InputAction::Continue),
                    InputAction::Break => return Ok(InputAction::Break),
                    InputAction::None => {}
                };
            }
            // check the typed letter
            if game.player.position_x
                < game.get_word_string(game.player.position_y).chars().count() as i32
            {
                match handle_chars(game, stats, theme, stdout, c, x, y)? {
                    InputAction::Continue => return Ok(InputAction::Continue),
                    InputAction::Break => return Ok(InputAction::Break),
                    InputAction::None => {}
                }
            } else if game.get_word_string(game.player.position_y).len() < MAX_WORD_LENGTH {
                let _ = add_incorrect_char(game, theme, stdout, c, x, y)?;
                game.player.position_x += 1;
                stats.letter_count += 1;
                stats.incorrect_letters += 1;
                stats.extra_chars += 1;
                stats.add_error();
            }

            stdout.flush().context("Failed to flush stdout")?;
        }
        KeyCode::Backspace => {
            handle_backspace(game, theme, stdout, x, y)?;
            stdout.flush().context("Failed to flush stdout")?;
        }
        _ => {}
    }
    Ok(InputAction::None)
}

fn handle_space(game: &mut Game, stdout: &std::io::Stdout, x: u16, y: u16) -> Result<InputAction> {
    if let InputAction::Continue = handle_start_of_line(game)? {
        return Ok(InputAction::Continue);
    }

    // If previous char is already a space, don't skip — let it fall through
    // to handle_chars where it will be counted as an error
    if let InputAction::Continue = handle_space_at_start(game)? {
        return Ok(InputAction::None); // fall through to handle_chars
    }

    if let InputAction::Continue = handle_end_of_line(game, stdout, x, y)? {
        return Ok(InputAction::Continue);
    }

    if game.jump_position + 1 == game.player.position_x && game.jump_position != 0 {
        return Ok(InputAction::Continue);
    }

    handle_jump_position(game, stdout, x, y)?;

    Ok(InputAction::None)
}

fn handle_start_of_line(game: &Game) -> Result<InputAction> {
    if game.player.position_x == 0 {
        return Ok(InputAction::Continue);
    }
    Ok(InputAction::None)
}

fn handle_end_of_line(
    game: &mut Game,
    mut stdout: &std::io::Stdout,
    x: u16,
    y: u16,
) -> Result<InputAction> {
    if game.selected_word_index
        == game
            .list
            .get(game.player.position_y as usize)
            .context("Failed to get word from list")?
            .len() as i32
            - 1
    {
        if game.player.position_y == game.list.len() as i32 {
            return Ok(InputAction::Break);
        }

        game.player.position_x = 0;
        game.player.position_y += 1;
        game.jump_position = 1;
        game.selected_word_index = 0;

        stdout
            .execute(MoveTo(
                x + game.player.position_x as u16,
                y + game.player.position_y as u16,
            ))
            .context("Failed to move cursor")?;
        return Ok(InputAction::Continue);
    }
    Ok(InputAction::None)
}

fn handle_space_at_start(game: &Game) -> Result<InputAction> {
    if game
        .get_word_string(game.player.position_y)
        .chars()
        .nth((game.player.position_x - 1) as usize)
        .context("Failed to get character from word")?
        == ' '
    {
        return Ok(InputAction::Continue);
    }
    Ok(InputAction::None)
}

fn handle_jump_position(
    game: &mut Game,
    mut stdout: &std::io::Stdout,
    x: u16,
    y: u16,
) -> Result<()> {
    game.jump_position = game
        .list
        .get(game.player.position_y as usize)
        .context("Failed to get word from list")?
        .iter()
        .take(game.selected_word_index as usize + 1)
        .map(|word| word.chars().count() + 1)
        .sum::<usize>() as i32
        - 1;
    game.player.position_x = game.jump_position;
    stdout
        .execute(MoveTo(
            x + game.player.position_x as u16,
            y + game.player.position_y as u16,
        ))
        .context("Failed to move cursor")?;
    game.selected_word_index += 1;
    Ok(())
}

fn handle_chars(
    game: &mut Game,
    stats: &mut Stats,
    theme: &ThemeColors,
    stdout: &std::io::Stdout,
    c: char,
    x: u16,
    y: u16,
) -> Result<InputAction> {
    let expected_char = game
        .get_word_string(game.player.position_y)
        .chars()
        .nth(game.player.position_x as usize)
        .context("Failed to get character from word")?;

    if c == expected_char {
        handle_correct_char(game, theme, stdout, c, x, y)?;
    } else if game
        .get_word_string(game.player.position_y)
        .chars()
        .nth(game.player.position_x as usize)
        .context("Failed to get character from word")?
        == ' '
    {
        if let InputAction::Continue = add_incorrect_char(game, theme, stdout, c, x, y)? {
            return Ok(InputAction::Continue);
        }
    } else {
        handle_incorrect_char(game, theme, stdout, expected_char, x, y)?;
    }

    update_game_state(game, stats, c)?;

    Ok(InputAction::None)
}

fn handle_correct_char(
    game: &Game,
    theme: &ThemeColors,
    mut stdout: &std::io::Stdout,
    c: char,
    x: u16,
    y: u16,
) -> Result<()> {
    stdout
        .execute(SetForegroundColor(theme.fg))
        .context("Failed to set foreground color")?;
    stdout
        .execute(MoveTo(
            x + game.player.position_x as u16,
            y + game.player.position_y as u16,
        ))
        .context("Failed to move cursor")?;
    print!("{}", c);
    Ok(())
}

fn handle_incorrect_char(
    game: &Game,
    theme: &ThemeColors,
    mut stdout: &std::io::Stdout,
    c: char,
    x: u16,
    y: u16,
) -> Result<()> {
    stdout
        .execute(SetForegroundColor(theme.error))
        .context("Failed to set foreground color")?;
    stdout
        .execute(MoveTo(
            x + game.player.position_x as u16,
            y + game.player.position_y as u16,
        ))
        .context("Failed to move cursor")?;
    print!("{}", c);
    Ok(())
}

fn add_incorrect_char(
    game: &mut Game,
    theme: &ThemeColors,
    mut stdout: &std::io::Stdout,
    c: char,
    x: u16,
    y: u16,
) -> Result<InputAction> {
    let position_x = game.player.position_x;
    let words = game.get_word_string(game.player.position_y);

    if words.len() >= MAX_WORD_LENGTH {
        return Ok(InputAction::Continue);
    }

    let before = words.chars().take(position_x as usize).collect::<String>();
    let after = words.chars().skip(position_x as usize).collect::<String>();

    stdout.execute(MoveTo(
        game.player.position_x as u16 + x,
        game.player.position_y as u16 + y,
    ))?;

    stdout.execute(SetForegroundColor(theme.error))?;
    stdout.execute(SetAttribute(Attribute::Underlined))?;
    print!("{}", c);
    stdout.execute(SetAttribute(Attribute::Reset))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    print!("{}", after);

    let new_line = format!("{}{}{}", before, c, after);
    game.list[game.player.position_y as usize] =
        new_line.split_whitespace().map(String::from).collect();
    Ok(InputAction::None)
}

fn handle_backspace(
    game: &mut Game,
    theme: &ThemeColors,
    mut stdout: &std::io::Stdout,
    x: u16,
    y: u16,
) -> Result<()> {
    // At start of a line, go back to end of previous line
    if game.player.position_x <= 0 {
        if game.player.position_y <= 0 {
            return Ok(());
        }
        game.player.position_y -= 1;
        let py = game.player.position_y as usize;
        let prev_line = game.get_word_string(game.player.position_y);
        game.player.position_x = prev_line.chars().count() as i32;

        // Restore selected_word_index and jump_position for previous line
        let word_count = game.list[py].len() as i32;
        game.selected_word_index = word_count;
        if word_count > 0 {
            game.jump_position = game.list[py]
                .iter()
                .take(word_count as usize)
                .map(|word| word.chars().count() + 1)
                .sum::<usize>() as i32
                - 1;
        } else {
            game.jump_position = 0;
        }

        stdout.execute(MoveTo(
            x + game.player.position_x as u16,
            y + game.player.position_y as u16,
        ))?;
        return Ok(());
    }

    let current_line = game.get_word_string(game.player.position_y);
    let prev_pos = (game.player.position_x - 1) as usize;

    // If previous char is a space, backspace over it into the previous word
    if current_line.chars().nth(prev_pos) == Some(' ') {
        if game.selected_word_index <= 0 {
            return Ok(());
        }
        // Move back over the space
        game.player.position_x -= 1;
        let pos = game.player.position_x as usize;
        let py = game.player.position_y as usize;

        // Undo the word jump
        game.selected_word_index -= 1;

        // Recalculate jump_position for the previous word
        if game.selected_word_index == 0 {
            game.jump_position = 0;
        } else {
            game.jump_position = game.list[py]
                .iter()
                .take(game.selected_word_index as usize)
                .map(|word| word.chars().count() + 1)
                .sum::<usize>() as i32
                - 1;
        }

        // Redraw the space in missing color
        let original_line = game.original_list[py].join(" ");
        let orig_char = original_line.chars().nth(pos).unwrap_or(' ');
        stdout.execute(SetAttribute(Attribute::Reset))?;
        stdout.execute(SetForegroundColor(theme.missing))?;
        stdout.execute(MoveTo(x + pos as u16, y + py as u16))?;
        print!("{}", orig_char);
        return Ok(());
    }

    game.player.position_x -= 1;
    let pos = game.player.position_x as usize;
    let py = game.player.position_y as usize;

    // Find which word we're in by scanning word boundaries
    let current_words = &game.list[py];
    let original_words = &game.original_list[py];
    let mut word_start = 0usize;
    let mut word_idx = 0usize;
    for (i, word) in current_words.iter().enumerate() {
        let word_end = word_start + word.chars().count();
        if pos >= word_start && pos < word_end {
            word_idx = i;
            break;
        }
        word_start = word_end + 1; // +1 for space
    }

    // Check if this is an extra character (current word longer than original)
    if word_idx < original_words.len()
        && current_words[word_idx].chars().count() > original_words[word_idx].chars().count()
    {
        let pos_in_word = pos - word_start;
        if pos_in_word >= original_words[word_idx].chars().count() {
            // Remove the extra character
            let mut word_chars: Vec<char> = current_words[word_idx].chars().collect();
            word_chars.remove(pos_in_word);
            game.list[py][word_idx] = word_chars.into_iter().collect();

            // Redraw from current position to end of line
            let new_line = game.get_word_string(game.player.position_y);
            stdout.execute(MoveTo(x + pos as u16, y + py as u16))?;
            stdout.execute(SetAttribute(Attribute::Reset))?;
            stdout.execute(SetForegroundColor(theme.missing))?;
            let remaining: String = new_line.chars().skip(pos).collect();
            print!("{} ", remaining); // extra space to clear the removed char
            return Ok(());
        }
    }

    // Normal case: redraw the original character in "missing" color
    let original_line = game.original_list[py].join(" ");
    let orig_char = original_line.chars().nth(pos).unwrap_or(' ');
    stdout.execute(SetAttribute(Attribute::Reset))?;
    stdout.execute(SetForegroundColor(theme.missing))?;
    stdout.execute(MoveTo(x + pos as u16, y + py as u16))?;
    print!("{}", orig_char);

    Ok(())
}

fn update_game_state(game: &mut Game, stats: &mut Stats, c: char) -> Result<()> {
    let expected = game
        .get_word_string(game.player.position_y)
        .chars()
        .nth(game.player.position_x as usize)
        .context("Failed to get character from word")?;

    if c == expected {
        stats.letter_count += 1;
    } else {
        stats.incorrect_letters += 1;
        stats.letter_count += 1;
        stats.add_error();
        if expected == ' ' {
            stats.extra_chars += 1;
        }
    }

    if expected == ' ' && c != ' ' {
        game.selected_word_index += 1;
    }
    game.player.position_x += 1;

    Ok(())
}
