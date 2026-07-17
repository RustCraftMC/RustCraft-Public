//! Internationalization system — loads MC .lang files.
//!
//! Format: `key=value` (one per line, UTF-8).
//! Supports `%s`, `%1$s` format args like MC 1.8.9.

use std::collections::HashMap;
use std::fs;

pub struct I18n {
    translations: HashMap<String, String>,
    fallback: HashMap<String, String>,
}

impl I18n {
    /// Load a language file and an optional fallback (usually en_US).
    pub fn load(lang_path: &str, fallback_path: Option<&str>) -> Self {
        let translations = Self::parse_file(lang_path);
        let fallback = fallback_path
            .and_then(|p| fs::read_to_string(p).ok())
            .map(|s| Self::parse_lines(&s))
            .unwrap_or_default();

        I18n {
            translations,
            fallback,
        }
    }

    fn parse_file(path: &str) -> HashMap<String, String> {
        fs::read_to_string(path)
            .map(|s| Self::parse_lines(&s))
            .unwrap_or_default()
    }

    fn parse_lines(content: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(eq_pos) = line.find('=') {
                let key = &line[..eq_pos];
                let value = &line[eq_pos + 1..];
                map.insert(key.to_string(), value.to_string());
            }
        }
        map
    }

    pub fn merged_translations(&self) -> HashMap<String, String> {
        let mut merged = self.fallback.clone();
        merged.extend(self.translations.clone());
        merged
    }

    /// Look up a translation key. Falls back to the key itself if not found.
    pub fn t(&self, key: &str) -> String {
        self.translations
            .get(key)
            .or_else(|| self.fallback.get(key))
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    /// Look up a translation with format args (e.g., "%s", "%1$s").
    pub fn tf(&self, key: &str, args: &[&str]) -> String {
        let template = self.t(key);
        let mut result = String::new();
        let chars: Vec<char> = template.chars().collect();
        let mut i = 0;
        let mut arg_idx = 0usize;

        while i < chars.len() {
            if chars[i] == '%' && i + 1 < chars.len() {
                if chars[i + 1] == '%' {
                    result.push('%');
                    i += 2;
                } else if chars[i + 1] == 's' {
                    if arg_idx < args.len() {
                        result.push_str(args[arg_idx]);
                        arg_idx += 1;
                    }
                    i += 2;
                } else if chars[i + 1].is_ascii_digit() {
                    // %1$s style — extract the index
                    let start = i + 1;
                    let mut end = start;
                    while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '$') {
                        end += 1;
                    }
                    if end < chars.len() && chars[end] == 's' {
                        let num_str: String = chars[start..end - 1].iter().collect(); // without $
                        if let Ok(idx) = num_str.parse::<usize>() {
                            if idx > 0 && idx - 1 < args.len() {
                                result.push_str(args[idx - 1]);
                            }
                        }
                        i = end + 1;
                    } else {
                        result.push('%');
                        i += 1;
                    }
                } else {
                    result.push('%');
                    i += 1;
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }

        result
    }
}
