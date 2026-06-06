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
