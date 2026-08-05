//! [`LogDrainBridge`] — real `LogDrain` backed by
//! `swe-observability-logging`'s `LoggerProvider`.

use std::sync::Arc;

use edge_application_observer::{LogDrain, LogEmitRequest, LogEmitResponse, ObserveError};
use swe_observ_logging::{LogEntry, LogLevel, LoggerProvider};

pub(crate) struct LogDrainBridge {
    backend: Arc<dyn LoggerProvider>,
}

impl LogDrainBridge {
    pub(crate) fn new(backend: Arc<dyn LoggerProvider>) -> Self {
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
        // Buffered backends (e.g. `FileLogger`) only reach disk on flush —
        // without this, entries sit in an in-memory BufWriter and are lost
        // if the process dies before the buffer fills on its own.
        self.backend.flush();
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
        let drain = LogDrainBridge::new(Arc::new(backend));
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

    /// Regression test for a real data-loss bug: `LogDrainBridge::emit` used
    /// to call `backend.emit()` without `flush()`, so entries sat in
    /// `FileLogger`'s in-memory `BufWriter` and were lost if the process
    /// died before the buffer filled on its own (confirmed live: 19 of 303
    /// entries lost to a hard kill before this fix). This test never calls
    /// `flush()` itself — only `LogDrainBridge::emit` does — so it fails if
    /// that call is ever removed.
    #[test]
    fn test_emit_persists_to_disk_via_file_backend_without_external_flush_happy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("drain_bridge.jsonl");
        let backend =
            swe_observ_logging::create_file_logger(path.to_string_lossy().to_string(), 10);
        let drain = LogDrainBridge::new(Arc::new(backend));

        drain
            .emit(LogEmitRequest {
                level: "info".to_string(),
                handler_id: "probe".to_string(),
                message: "durable_without_manual_flush".to_string(),
            })
            .unwrap();

        let contents = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!("expected the entry to already be on disk (no manual flush): {e}")
        });
        assert!(
            contents.contains("durable_without_manual_flush"),
            "expected the entry on disk immediately after emit(), got: {contents}"
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
