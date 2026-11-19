pub fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn clean_required(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_text_is_trimmed_and_empty_values_are_dropped() {
        assert_eq!(
            clean_optional(Some("  book  ".to_owned())),
            Some("book".to_owned())
        );
        assert_eq!(clean_optional(Some("   ".to_owned())), None);
        assert_eq!(clean_optional(None), None);
    }

    #[test]
    fn required_text_is_trimmed_and_empty_values_are_dropped() {
        assert_eq!(clean_required(Some("  chapter  ")), Some("chapter"));
        assert_eq!(clean_required(Some("   ")), None);
        assert_eq!(clean_required(None), None);
    }
}
