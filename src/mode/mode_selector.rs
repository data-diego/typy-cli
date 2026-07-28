use anyhow::Result;
use rand::Rng;
use std::str::FromStr;

use crate::config::mode_settings::ModeSettings;

#[derive(Debug, PartialEq, Clone)]
pub enum ModeType {
    Normal,
    Uppercase,
    Punctuation,
    Numbers,
}

impl FromStr for ModeType {
    type Err = ();

    fn from_str(input: &str) -> Result<ModeType, Self::Err> {
        match input {
            "uppercase" => Ok(ModeType::Uppercase),
            "punctuation" => Ok(ModeType::Punctuation),
            "numbers" => Ok(ModeType::Numbers),
            "normal" => Ok(ModeType::Normal),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct Mode {
    modes: Vec<ModeType>,
    pub duration: u64,
    settings: ModeSettings,
}

impl Mode {
    pub fn from_str(mode_strs: Vec<&str>) -> Result<Self> {
        let mut modes = Vec::new();
        let settings = ModeSettings::new();

        for mode_str in mode_strs {
            match mode_str {
                "normal" => modes.push(ModeType::Normal),
                "uppercase" => modes.push(ModeType::Uppercase),
                "punctuation" => modes.push(ModeType::Punctuation),
                "numbers" => modes.push(ModeType::Numbers),
                _ => return Err(anyhow::anyhow!("Invalid mode: {}", mode_str)),
            }
        }

        // If no specific mode is provided, default to normal
        modes.is_empty().then(|| {
            settings.default_modes.iter().for_each(|m| {
                modes.push(m.clone());
            });
        });

        modes.contains(&ModeType::Normal).then(|| {
            modes.clear();
            modes.push(ModeType::Normal);
        });

        Ok(Mode {
            modes,
            duration: 0,
            settings,
        })
    }

    pub fn add_duration(mut self, duration: u64) -> Self {
        self.duration = duration;
        self
    }

    pub fn transform(&self, list: &mut [Vec<String>]) {
        let mut rng = rand::rng();
        let punctuations = [".", ",", "!", "?", ";", ":", "-"];

        for mode in &self.modes {
            match mode {
                ModeType::Uppercase => {
                    for sublist in list.iter_mut() {
                        for item in sublist.iter_mut() {
                            let mut new_item = String::new();
                            for c in item.chars() {
                                if rng.random_bool(self.settings.uppercase_chance.into()) {
                                    new_item.push(c.to_uppercase().next().unwrap());
                                } else {
                                    new_item.push(c);
                                }
                            }
                            *item = new_item;
                        }
                    }
                }
                ModeType::Punctuation => {
                    for sublist in list.iter_mut() {
                        let len = sublist.len();
                        if len > 1 {
                            for item in sublist.iter_mut().take(len - 1) {
                                if rng.random_bool(self.settings.punctuation_chance.into()) {
                                    let punctuation =
                                        punctuations[rng.random_range(0..punctuations.len())];
                                    item.push_str(punctuation);
                                }
                            }
                        }
                    }
                }
                // ponytail: numbers replace whole words instead of being inserted as
                // extra ones, so the pre-fitted line length can't overflow. The digit
                // count is capped at the word's length for the same reason.
                ModeType::Numbers => {
                    for sublist in list.iter_mut() {
                        for item in sublist.iter_mut() {
                            if rng.random_bool(self.settings.numbers_chance.into()) {
                                let digits = item.chars().count().clamp(1, 3) as u32;
                                *item = rng.random_range(0..10u32.pow(digits)).to_string();
                            }
                        }
                    }
                }
                ModeType::Normal => {}
            }
        }
    }
}

#[cfg(test)]
mod mode_tests {
    use super::*;

    #[test]
    fn test_from_str_valid_mode() {
        let mode = Mode::from_str(vec!["normal", "uppercase", "punctuation"]).unwrap();
        assert_eq!(mode.modes, vec![ModeType::Normal]);
    }

    #[test]
    fn test_from_str_valid_modes() {
        let mode = Mode::from_str(vec!["uppercase", "punctuation"]).unwrap();
        assert_eq!(mode.modes, vec![ModeType::Uppercase, ModeType::Punctuation]);
    }

    #[test]
    fn test_from_str_invalid_mode() {
        let mode = Mode::from_str(vec!["invalid"]);
        assert!(mode.is_err());
    }

    #[test]
    fn test_add_duration() {
        let mode = Mode::from_str(vec!["normal"]).unwrap().add_duration(10);
        assert_eq!(mode.modes, vec![ModeType::Normal]);
        assert_eq!(mode.duration, 10);
    }

    #[test]
    fn test_transform_uppercase() {
        let mode = Mode::from_str(vec!["uppercase"]).unwrap();
        let mut list = vec![vec!["hello".to_string(), "world".to_string()]];
        mode.transform(&mut list);
        // Since the transformation is random, we can't assert exact values, but we can check the structure
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].len(), 2);
    }

    #[test]
    fn test_transform_numbers() {
        let mode = Mode::from_str(vec!["numbers"]).unwrap();
        let mut list = vec![vec!["hello".to_string(), "world".to_string()]];
        mode.transform(&mut list);
        for word in &list[0] {
            // A word is either untouched or replaced by digits — never longer than before.
            assert!(word == "hello" || word == "world" || word.chars().all(|c| c.is_ascii_digit()));
            assert!(word.chars().count() <= 5);
        }
    }

    #[test]
    fn test_transform_punctuation() {
        let mode = Mode::from_str(vec!["punctuation"]).unwrap();
        let mut list = vec![vec!["hello".to_string(), "world".to_string()]];
        mode.transform(&mut list);
        // Since the transformation is random, we can't assert exact values, but we can check the structure
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].len(), 2);
    }
}
