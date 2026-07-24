# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project structure — extracted from the `sweengineeringlabs/edge` monorepo
  (`scm/bootstrap`), full history preserved.

### Changed
- Migrated to `edge-application@v0.18.0` across every dependency in the graph (`edge-dispatch`,
  `swe-edge-ingress-http`/`-grpc`, `edge-security-runtime*`, `swe-edge-runtime-grpc`/`-http`,
  `edge-proxy`). See `scm/CHANGELOG.md` for the full detail. `cargo build`/`test`/`clippy`/`fmt`
  are all green as of this entry.
