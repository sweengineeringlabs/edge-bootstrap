//! Composition-root bridge from `edge-proxy::Job` to `edge-dispatch`'s
//! `HandlerRegistry`/`Pipeline`, per ADR-021 (see
//! `docs/3-design/adr/003-adopt-adr-021-system-request-flow.md`).
mod default_http_job;

pub(crate) use default_http_job::DefaultHttpJob;
