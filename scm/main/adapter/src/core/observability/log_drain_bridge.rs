//! [`LogDrainBridge`] — real `LogDrain` backed by
//! `swe-observability-logging`'s `LoggerProvider`.

use edge_application_observer::{LogDrain, LogEmitRequest, LogEmitResponse, ObserveError};
use swe_observ_logging::{LogEntry, LogLevel, LoggerProvider};

pub(crate) struct LogDrainBridge {
    backend: Box<dyn LoggerProvider>,
}

impl LogDrainBridge {
    pub(crate) fn new(backend: Box<dyn LoggerProvider>) -> Self {
        Self { backend }
    }
}

impl LogDrain for LogDrainBridge {
    fn emit(&self, req: LogEmitRequest) -> Result<LogEmitResponse, ObserveError> {
        tracing::debug!(
            node = "log_drain_bridge",
            handler_id = %req.handler_id,
            level = %req.level,
            message = %req.message,
            "log entry emitted to real backend"
        );
        self.backend.emit(&LogEntry::new(
            parse_level(&req.level),
            req.handler_id,
            req.message,
        ));
        Ok(LogEmitResponse)
    }
}

fn parse_level(level: &str) -> LogLevel {
    match level.to_ascii_lowercase().as_str() {
        "trace" => LogLevel::Trace,
        "debug" => LogLevel::Debug,
        "warn" | "warning" => LogLevel::Warn,
        "error" => LogLevel::Error,
        "fatal" => LogLevel::Fatal,
        _ => LogLevel::Info,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_reaches_the_real_backend_happy() {
        let backend = swe_observ_logging::create_local_logging_backend();
        let drain = LogDrainBridge::new(Box::new(backend));
        drain
            .emit(LogEmitRequest {
                level: "error".to_string(),
                handler_id: "echo".to_string(),
                message: "something failed".to_string(),
            })
            .unwrap();
        let recent = drain.backend.recent_entries(10);
        assert!(
            recent
                .iter()
                .any(|e| e.message == "something failed" && e.source == "echo"),
            "expected the real backend to have recorded the entry, got: {recent:?}"
        );
    }

    #[test]
    fn test_parse_level_maps_known_strings_happy() {
        assert_eq!(parse_level("ERROR"), LogLevel::Error);
        assert_eq!(parse_level("warn"), LogLevel::Warn);
        assert_eq!(parse_level("Debug"), LogLevel::Debug);
    }

    #[test]
    fn test_parse_level_defaults_to_info_for_unknown_edge() {
        assert_eq!(parse_level("not-a-real-level"), LogLevel::Info);
    }
}
