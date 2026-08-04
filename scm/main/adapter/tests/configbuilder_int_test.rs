//! Integration tests for `swe-edge-configbuilder` usage in the server crate (Rule 95).
//!
//! `swe-edge-configbuilder` is used in `src/saf/server_svc.rs` via
//! `ConfigLoaderFactory::create_config_builder()` and `ConfigBuilderImpl`.
//! These tests exercise those types directly so that Rule 95 (dependency must
//! have integration/e2e test coverage) is satisfied.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use swe_edge_configbuilder::{ConfigBuilderImpl, ConfigLoaderFactory};

// ── ConfigLoaderFactory ───────────────────────────────────────────────────────

/// `create_loader_for_dir` must return a loader that succeeds for a non-existent
/// directory (the loader is lenient about absent dirs; it only errors on paths
/// that exist but are not directories).
#[test]
fn test_create_loader_for_dir_succeeds_for_absent_dir() {
    let dir = std::path::PathBuf::from("/tmp/swe-edge-server-absent-dir-does-not-exist");
    let loader = ConfigLoaderFactory::create_loader_for_dir(dir);
    // A missing directory is not an error — validate() must pass.
    assert!(
        loader.validate().is_ok(),
        "loader must accept a non-existent config dir"
    );
}

/// `create_loader_for_dir` must load a section written to that directory.
#[test]
fn test_create_loader_for_dir_loads_written_section() {
    #[derive(serde::Deserialize, Default, PartialEq, Debug)]
    struct Probe {
        value: u32,
    }

    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("application.toml"), "[probe]\nvalue = 42\n")
        .expect("write toml");

    let loader = ConfigLoaderFactory::create_loader_for_dir(dir.path());
    let result: Probe = loader
        .load_section("probe")
        .expect("load_section must succeed");

    assert_eq!(result.value, 42, "loaded value must match written TOML");
}

/// `create_loader_for_dir` must return the type default when the section is absent.
#[test]
fn test_create_loader_for_dir_returns_default_for_absent_section() {
    #[derive(serde::Deserialize, Default, PartialEq, Debug)]
    struct Probe {
        value: u32,
    }

    let dir = tempfile::tempdir().expect("tempdir");
    // Write a TOML file that does NOT contain the requested section.
    std::fs::write(dir.path().join("application.toml"), "[other]\nx = 1\n").expect("write toml");

    let loader = ConfigLoaderFactory::create_loader_for_dir(dir.path());
    let result: Probe = loader
        .load_section("probe")
        .expect("absent section must return default, not error");

    assert_eq!(
        result,
        Probe::default(),
        "absent section must yield the type default"
    );
}

// ── ConfigBuilderImpl ─────────────────────────────────────────────────────────

/// `with_name` / `with_version` must store the supplied values.
#[test]
fn test_config_builder_impl_stores_name_and_version() {
    let builder: ConfigBuilderImpl = ConfigLoaderFactory::create_config_builder()
        .with_name("test-app")
        .with_version("1.2.3");

    assert_eq!(builder.name(), "test-app");
    assert_eq!(builder.version(), "1.2.3");
}

/// `build_loader` on a builder pointing at a temp dir must produce a working
/// loader — this exercises the full `ConfigBuilderImpl → build_loader()` path
/// that `ServerConfigLoader::create_config_builder` relies on.
#[test]
fn test_config_builder_impl_build_loader_reads_section() {
    #[derive(serde::Deserialize, Default, PartialEq, Debug)]
    struct Widget {
        enabled: bool,
    }

    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("application.toml"),
        "[widget]\nenabled = true\n",
    )
    .expect("write toml");

    let loader = ConfigLoaderFactory::create_config_builder()
        .with_name("test-server")
        .with_version("0.0.0")
        .with_config_dir(dir.path())
        .build_loader()
        .expect("build_loader must succeed");

    let widget: Widget = loader
        .load_section("widget")
        .expect("load_section must succeed");

    assert!(widget.enabled, "section value must reflect written TOML");
}

/// `build_loader` must fail with `ConfigError` when a config dir exists but is
/// a file rather than a directory (path-validation path in `DefaultConfigBuilder`).
#[test]
fn test_config_builder_impl_build_loader_rejects_file_as_config_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("not-a-dir.toml");
    std::fs::write(&file_path, "# placeholder").expect("write file");

    let result = ConfigLoaderFactory::create_config_builder()
        .with_name("test-server")
        .with_version("0.0.0")
        .with_config_dir(&file_path)
        .build_loader();

    assert!(
        result.is_err(),
        "build_loader must return Err when config dir is a file, not a directory"
    );
}
