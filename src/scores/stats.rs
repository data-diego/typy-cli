const AVERAGE_WORD_LENGTH: i32 = 5;

pub struct Stats {
    pub lps: Vec<i32>,
    pub errors_ps: Vec<i32>,
    pub letter_count: i32,
    pub incorrect_letters: i32,
    pub extra_chars: i32,
    current_errors: i32,
}

impl Stats {
    pub fn new() -> Self {
        Stats {
            lps: Vec::new(),
            errors_ps: Vec::new(),
            letter_count: 0,
            incorrect_letters: 0,
            extra_chars: 0,
            current_errors: 0,
        }
    }

    pub fn add_letters(&mut self) {
        self.lps.push(self.letter_count);
        self.errors_ps.push(self.current_errors);
        self.letter_count = 0;
        self.current_errors = 0;
    }

    pub fn add_error(&mut self) {
        self.current_errors += 1;
    }

    fn total_letters(&self) -> i32 {
        self.lps.iter().sum()
    }

    fn total_seconds(&self) -> i32 {
        self.lps.len() as i32
    }

    fn minutes(&self) -> f64 {
        self.total_seconds() as f64 / 60.0
    }

    pub fn raw_wpm(&self) -> f64 {
        if self.minutes() == 0.0 {
            return 0.0;
        }
        (self.total_letters() / AVERAGE_WORD_LENGTH) as f64 / self.minutes()
    }

    pub fn wpm(&self) -> f64 {
        if self.minutes() == 0.0 {
            return 0.0;
        }
        ((self.total_letters() - self.incorrect_letters).max(0) / AVERAGE_WORD_LENGTH) as f64
            / self.minutes()
    }

    pub fn accuracy(&self) -> f64 {
        if self.total_letters() == 0 {
            return 100.0;
        }
        100.0 - (self.incorrect_letters as f64 / self.total_letters() as f64) * 100.0
    }

    pub fn correct_chars(&self) -> i32 {
        (self.total_letters() - self.incorrect_letters).max(0)
    }

    pub fn wpm_per_second(&self) -> Vec<f64> {
        let mut result = Vec::new();
        let mut total = 0i32;
        let mut total_errors = 0i32;
        for (i, &letters) in self.lps.iter().enumerate() {
            total += letters;
            if i < self.errors_ps.len() {
                total_errors += self.errors_ps[i];
            }
            let minutes = (i + 1) as f64 / 60.0;
            let net = (total - total_errors).max(0);
            result.push((net as f64 / 5.0) / minutes);
        }
        result
    }

    pub fn raw_wpm_per_second(&self) -> Vec<f64> {
        let mut result = Vec::new();
        let mut total = 0i32;
        for (i, &letters) in self.lps.iter().enumerate() {
            total += letters;
            let minutes = (i + 1) as f64 / 60.0;
            result.push((total as f64 / 5.0) / minutes);
        }
        result
    }

    pub fn consistency(&self) -> f64 {
        let wpm_values = self.wpm_per_second();
        if wpm_values.len() < 2 {
            return 100.0;
        }
        let mean: f64 = wpm_values.iter().sum::<f64>() / wpm_values.len() as f64;
        if mean == 0.0 {
            return 0.0;
        }
        let variance: f64 = wpm_values
            .iter()
            .map(|&v| (v - mean).powi(2))
            .sum::<f64>()
            / wpm_values.len() as f64;
        let cv = variance.sqrt() / mean;
        (100.0 - cv * 100.0).clamp(0.0, 100.0)
    }
}
