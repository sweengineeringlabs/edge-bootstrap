//! `Validator` — domain validation trait.

/// Validates a value against domain constraints before it is used.
///
/// Implement this trait in `core/` to express invariants that cannot
/// be captured by the type system alone (e.g. non-empty strings,
/// numeric ranges, regex patterns).
pub trait Validator {
    /// The type this validator checks.
    type Target;
    /// The error returned when validation fails.
    type Error;

    /// Validate `value`, returning `Err` with details when a constraint is violated.
    fn validate(&self, value: &Self::Target) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NonEmptyString;
    impl Validator for NonEmptyString {
        type Target = String;
        type Error = String;
        fn validate(&self, value: &String) -> Result<(), String> {
            if value.is_empty() {
                Err("must not be empty".into())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn test_validator_double_accepts_non_empty_value() {
        assert!(NonEmptyString.validate(&"x".to_string()).is_ok());
    }

    #[test]
    fn test_validator_double_rejects_empty_value() {
        match NonEmptyString.validate(&String::new()) {
            Err(err) => assert_eq!(err, "must not be empty"),
            Ok(()) => panic!("expected validation of an empty string to be rejected"),
        }
    }
}
