mod finder;

use anyhow::Result;
use finder::find;

pub fn get_words(language: &str, line_length: i32) -> Result<Vec<Vec<String>>> {
    let mut words = Vec::new();
    for _ in 0..3 {
        words.push(find(language, line_length)?);
    }
    Ok(words)
}

#[cfg(test)]
mod word_provider_tests {
    use super::*;

    #[test]
    fn test_get_words() {
        let line_length = 70;
        let words = get_words("english", line_length);

        for word in &words.unwrap() {
            let mut length = 0;
            for w in word {
                length += w.chars().count() as i32;
            }
            assert!(length <= line_length);
        }
    }
}
