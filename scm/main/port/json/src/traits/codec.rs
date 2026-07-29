//! `Codec` — JSON codec interface for HTTP and gRPC routes.

/// Marker trait for JSON codec implementations.
pub trait Codec: Send + Sync {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct CodecDouble;
    impl Codec for CodecDouble {}

    #[test]
    fn test_codec_double_is_object_safe_as_dyn() {
        let _: Arc<dyn Codec> = Arc::new(CodecDouble);
    }
}
