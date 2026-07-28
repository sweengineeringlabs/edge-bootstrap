//! `Validator` — domain validation trait.

/// Validates a value against domain constraints before it is used.
///
/// Implement this trait in `core/` to express invariants that cannot
/// be captured by the type system alone (e.g. non-empty strings,
/// numeric ranges, regex patterns).
pub trait Validator {
    type Target;
    type Error;

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
        let err = NonEmptyString.validate(&String::new()).unwrap_err();
        assert_eq!(err, "must not be empty");
    }
}
