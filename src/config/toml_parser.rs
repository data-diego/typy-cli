use dirs::home_dir;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use toml;

#[derive(Serialize, Deserialize, Clone)]
pub struct ThemeTable {
    pub fg: Option<String>,
    pub missing: Option<String>,
    pub error: Option<String>,
    pub accent: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CursorTable {
    pub style: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ModesTable {
    pub default_mode: Option<String>,
    pub uppercase_chance: Option<String>,
    pub punctuation_chance: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LanguageTable {
    pub lang: Option<String>,
}

/// Shell commands run when typy starts and when it exits. No terminal exposes
/// font size over an escape sequence, so the command is left up to the user.
#[derive(Serialize, Deserialize, Clone)]
pub struct ZoomTable {
    pub enter: Option<String>,
    pub exit: Option<String>,
    /// How many times to run `enter` — one per zoom step. Defaults to 1.
    pub steps: Option<u32>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ConfigToml {
    theme: Option<ThemeTable>,
    cursor: Option<CursorTable>,
    modes: Option<ModesTable>,
    language: Option<LanguageTable>,
    zoom: Option<ZoomTable>,
}

impl ConfigToml {
    pub fn new() -> Self {
        let mut config_filepaths: Vec<PathBuf> = vec![PathBuf::from("./config.toml")];

        if let Some(home_path) = home_dir() {
            config_filepaths.push(home_path.join(".config/typy/config.toml"));
        }

        let mut content = "".to_owned();

        for filepath in config_filepaths {
            let result = fs::read_to_string(filepath);

            if result.is_ok() {
                content = result.unwrap();
                break;
            }
        }
        let config_toml: ConfigToml =
            toml::from_str(&content).unwrap_or_else(|_| ConfigToml::default());
        config_toml
    }

    pub fn get_theme(&self) -> Option<ThemeTable> {
        self.theme.clone()
    }

    pub fn get_cursor(&self) -> Option<CursorTable> {
        self.cursor.clone()
    }

    pub fn get_modes(&self) -> Option<ModesTable> {
        self.modes.clone()
    }

    pub fn get_language(&self) -> Option<LanguageTable> {
        self.language.clone()
    }

    pub fn get_zoom(&self) -> Option<ZoomTable> {
        self.zoom.clone()
    }
}

// Declare the static instance of ConfigToml using lazy_static
lazy_static! {
    static ref CONFIG: Mutex<ConfigToml> = Mutex::new(ConfigToml::new());
}

// Helper function to access the static CONFIG
pub fn get_config() -> &'static Mutex<ConfigToml> {
    &CONFIG
}
