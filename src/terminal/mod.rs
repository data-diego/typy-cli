mod game;
mod keyboard;
mod terminal_utils;

pub use game::{run, Game};
pub use terminal_utils::{calc_middle_for_text, close_typy};

#[derive(Clone)]
pub struct GhostFrame {
    pub elapsed_ms: u64,
    pub position_x: i32,
    pub position_y: i32,
}

pub enum PostGameAction {
    Quit,
    Replay { duration: u64, lang: String },
    Repeat {
        duration: u64,
        lang: String,
        words: Vec<Vec<String>>,
        ghost: Vec<GhostFrame>,
    },
}
