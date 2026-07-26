//! Response shaping utilities.
//!
//! Ensures plugin output stays under the 200-token budget required by the bounty.

/// Maximum token budget for plugin output.
pub const MAX_OUTPUT_TOKENS: usize = 200;

/// Shape a structured summary with title and sections.
/// Truncates to fit within `max_chars` while preserving readability.
pub fn shape_summary(title: &str, sections: Vec<(&str, String)>, max_chars: usize) -> String {
    let mut output = String::new();
    output.push_str(&format!("## {title}\n\n"));

    for (label, content) in &sections {
        output.push_str(&format!("**{label}:** {content}\n"));
        if output.len() >= max_chars {
            output.push_str("...");
            break;
        }
    }

    truncate_to_budget(&output, max_chars)
}

/// Truncate text to stay within a token budget.
/// Rough approximation: 1 token ≈ 4 characters.
pub fn truncate_to_token_budget(text: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens * 4;
    truncate_to_budget(text, max_chars)
}

/// Truncate text to fit within a character budget, preserving word boundaries.
/// Accounts for the ellipsis suffix ("...") in the budget.
pub fn truncate_to_budget(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }
    // Reserve 3 chars for "..." ellipsis
    let truncate_at = max_chars.saturating_sub(3);
    let mut trimmed = text[..truncate_at.min(text.len())].to_string();
    if let Some(last_space) = trimmed.rfind(' ') {
        trimmed.truncate(last_space);
    }
    trimmed.push_str("...");
    trimmed
}

/// Count approximate tokens in a string (word count).
pub fn count_tokens(text: &str) -> usize {
    text.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_long_text() {
        let long = "a".repeat(1000);
        let result = truncate_to_budget(&long, 50);
        assert!(result.len() <= 50, "result len {} > 50", result.len());
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_no_truncation_when_under_budget() {
        let short = "hello world";
        let result = truncate_to_budget(short, 50);
        assert_eq!(result, short);
    }

    #[test]
    fn test_count_tokens() {
        assert_eq!(count_tokens("hello world"), 2);
        assert_eq!(count_tokens(""), 0);
        assert_eq!(count_tokens("one two three four five"), 5);
    }

    #[test]
    fn test_shape_summary_fits_budget() {
        let sections = vec![
            ("Status", "OK".to_string()),
            ("Value", "123.45 USD".to_string()),
        ];
        let result = shape_summary("Test", sections, 500);
        assert!(!result.is_empty());
        assert!(result.contains("## Test"));
        assert!(result.contains("Status"));
        assert!(result.contains("Value"));
    }
}
