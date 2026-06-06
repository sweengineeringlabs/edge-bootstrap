//! ApplicationConfigLoader — loads RuntimeConfig from the layered chain.

use std::env;
use std::path::{Path, PathBuf};

use crate::api::config::traits::loader::ConfigLoader;
use crate::api::config::{ConfigError, ConfigOverride};
use crate::api::runtime::RuntimeConfig;

/// Refuse to read a config file larger than this — prevents accidental or
/// deliberate memory exhaustion via an oversized TOML blob.
const MAX_CONFIG_FILE_BYTES: u64 = 1_048_576; // 1 MiB

/// Loads [`RuntimeConfig`] from the full layered chain.
///
/// `config_dirs` is an ordered list of directories applied from
/// lowest to highest priority — each directory's `application.toml`
/// (and `tenants/<id>.toml`) overlays the previous result.
///
/// Construct via [`ApplicationConfigLoader::new`] (env/cwd default),
/// [`ApplicationConfigLoader::with_dir`] (single explicit path), or
/// [`ApplicationConfigLoader::xdg`] (full XDG Base Directory chain).
pub(crate) struct ApplicationConfigLoader {
    config_dirs: Vec<PathBuf>,
}

impl ApplicationConfigLoader {
    /// Resolve config directory from `SWE_EDGE_CONFIG_DIR` env var,
    /// falling back to `config/` relative to the working directory.
    pub(crate) fn new() -> Self {
        let dir = env::var("SWE_EDGE_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("config"));
        Self {
            config_dirs: vec![dir],
        }
    }

    /// Use a single explicit directory — for consumer apps that own
    /// their config path rather than relying on env or cwd.
    pub(crate) fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dirs: vec![dir.into()],
        }
    }

    /// Build the full XDG Base Directory chain for `app_name`.
    ///
    /// Applied in order (last wins):
    /// 1. Each entry in `$XDG_CONFIG_DIRS/<app_name>/`
    ///    (default: `/etc/xdg/<app_name>/`) — lowest priority first
    /// 2. `$XDG_CONFIG_HOME/<app_name>/` (default: `~/.config/<app_name>/`)
    /// 3. `$SWE_EDGE_CONFIG_DIR/` — explicit override, if set
    ///
    /// Env vars (`SWE_EDGE_*`) are always applied on top regardless.
    pub(crate) fn xdg(app_name: &str) -> Self {
        let mut dirs: Vec<PathBuf> = Vec::new();

        // XDG_CONFIG_DIRS — system-wide, colon-separated, lowest priority.
        // The spec lists them highest-to-lowest, so reverse before applying.
        let xdg_config_dirs = env::var("XDG_CONFIG_DIRS").unwrap_or_else(|_| "/etc/xdg".to_owned());
        for segment in xdg_config_dirs.split(':').rev() {
            if !segment.is_empty() {
                dirs.push(PathBuf::from(segment).join(app_name));
            }
        }

        // XDG_CONFIG_HOME — user-level, higher priority than CONFIG_DIRS.
        if let Some(home) = dirs::config_dir() {
            dirs.push(home.join(app_name));
        }

        // Explicit override — highest file-level priority.
        if let Ok(v) = env::var("SWE_EDGE_CONFIG_DIR") {
            dirs.push(PathBuf::from(v));
        }

        Self { config_dirs: dirs }
    }

    fn base(&self) -> Result<RuntimeConfig, ConfigError> {
        let mut cfg = RuntimeConfig::default();
        for dir in &self.config_dirs {
            cfg = self.apply_file_if_exists(cfg, &dir.join("application.toml"))?;
        }
        Ok(cfg)
    }

    fn apply_file_if_exists(
        &self,
        cfg: RuntimeConfig,
        path: &Path,
    ) -> Result<RuntimeConfig, ConfigError> {
        if !path.exists() {
            return Ok(cfg);
        }
        let meta = std::fs::metadata(path)
            .map_err(|e| ConfigError::Io(format!("{}: {e}", path.display())))?;
        if meta.len() > MAX_CONFIG_FILE_BYTES {
            return Err(ConfigError::Io(format!(
                "{}: config file exceeds the 1 MiB limit ({} bytes)",
                path.display(),
                meta.len(),
            )));
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(format!("{}: {e}", path.display())))?;
        Ok(ConfigOverride::from_str(&text)?.apply_to(cfg))
    }

    fn apply_env(mut cfg: RuntimeConfig) -> Result<RuntimeConfig, ConfigError> {
        if let Ok(v) = env::var("SWE_EDGE_SERVICE_NAME") {
            cfg.service_name = v;
        }
        if let Ok(v) = env::var("SWE_EDGE_HTTP_BIND") {
            cfg.http_bind = v;
        }
        if let Ok(v) = env::var("SWE_EDGE_GRPC_BIND") {
            cfg.grpc_bind = v;
        }
        if let Ok(v) = env::var("SWE_EDGE_SHUTDOWN_TIMEOUT") {
            cfg.shutdown_timeout_secs = Self::parse_shutdown_timeout(&v)?;
        }
        if let Ok(v) = env::var("SWE_EDGE_SYSTEMD_NOTIFY") {
            cfg.systemd_notify = matches!(v.to_lowercase().as_str(), "1" | "true" | "yes");
        }
        if let Ok(v) = env::var("SWE_EDGE_TENANT_ID") {
            cfg.tenant_id = Some(v);
        }
        Ok(cfg)
    }

    fn tenant_path(&self, tenant_id: &str) -> Option<PathBuf> {
        self.config_dirs.iter().rev().find_map(|dir| {
            let p = dir.join("tenants").join(format!("{tenant_id}.toml"));
            p.exists().then_some(p)
        })
    }

    /// Merge two `toml::Value`s — for tables, overlay keys win; for scalars, overlay wins.
    fn merge_values(base: toml::Value, overlay: toml::Value) -> toml::Value {
        match (base, overlay) {
            (toml::Value::Table(mut b), toml::Value::Table(o)) => {
                for (k, v) in o {
                    b.insert(k, v);
                }
                toml::Value::Table(b)
            }
            (_, o) => o,
        }
    }

    /// Walk a dotted key path (e.g. `"observability.tracing"`) into a `toml::Value`.
    fn extract_dotted_value(val: &toml::Value, key: &str) -> Option<toml::Value> {
        let mut current = val;
        for part in key.split('.') {
            current = current.get(part)?;
        }
        Some(current.clone())
    }

    /// Parse a shutdown timeout string as a non-negative integer.
    fn parse_shutdown_timeout(v: &str) -> Result<u64, ConfigError> {
        v.parse::<u64>().map_err(|_| {
            ConfigError::BadEnvVar(format!(
                "SWE_EDGE_SHUTDOWN_TIMEOUT={v:?}: expected a non-negative integer"
            ))
        })
    }

    /// Reject any tenant ID that could escape the `tenants/` directory.
    ///
    /// Only `[a-zA-Z0-9_-]` are allowed — every other character (`.`, `/`, `\`,
    /// NUL, whitespace) can be abused in path construction.
    fn validate_tenant_id(id: &str) -> Result<(), ConfigError> {
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(ConfigError::InvalidTenantId(id.to_owned()));
        }
        Ok(())
    }
}

impl crate::api::config::traits::loader::ApplicationConfigLoader for ApplicationConfigLoader {}

impl ConfigLoader for ApplicationConfigLoader {
    fn load(&self) -> Result<RuntimeConfig, ConfigError> {
        Self::apply_env(self.base()?)
    }

    fn load_for_tenant(&self, tenant_id: &str) -> Result<RuntimeConfig, ConfigError> {
        Self::validate_tenant_id(tenant_id)?;
        let cfg = self.base()?;
        let tenant_path = self
            .tenant_path(tenant_id)
            .ok_or_else(|| ConfigError::UnknownTenant(tenant_id.to_owned()))?;
        let cfg = self.apply_file_if_exists(cfg, &tenant_path)?;
        let mut cfg = Self::apply_env(cfg)?;
        if cfg.tenant_id.is_none() {
            cfg.tenant_id = Some(tenant_id.to_owned());
        }
        Ok(cfg)
    }

    /// Load an arbitrary TOML section from the layered config chain.
    ///
    /// `key` is a dotted path into the config tree, e.g.
    /// `"observability.tracing"` or `"application.completion"`.
    ///
    /// Layer order (later wins):
    /// 1. Each config directory's `application.toml`
    ///
    /// The resulting table is deserialized into `T`.  Missing keys use
    /// `T`'s `#[serde(default)]` values.  Returns `ConfigError::Parse`
    /// if the section is present but cannot be deserialized.  Returns
    /// `Ok(T::default())` if the key is absent from all sources.
    fn load_section<T>(&self, key: &str) -> Result<T, ConfigError>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        let mut merged = toml::Value::Table(toml::map::Map::new());

        for dir in &self.config_dirs {
            let path = dir.join("application.toml");
            if !path.exists() {
                continue;
            }
            let meta = std::fs::metadata(&path)
                .map_err(|e| ConfigError::Io(format!("{}: {e}", path.display())))?;
            if meta.len() > MAX_CONFIG_FILE_BYTES {
                return Err(ConfigError::Io(format!(
                    "{}: config file exceeds the 1 MiB limit ({} bytes)",
                    path.display(),
                    meta.len(),
                )));
            }
            let text = std::fs::read_to_string(&path)
                .map_err(|e| ConfigError::Io(format!("{}: {e}", path.display())))?;
            let val: toml::Value =
                toml::from_str(&text).map_err(|e| ConfigError::Parse(e.to_string()))?;
            if let Some(section) = Self::extract_dotted_value(&val, key) {
                merged = Self::merge_values(merged, section);
            }
        }

        if matches!(merged, toml::Value::Table(ref t) if t.is_empty()) {
            return Ok(T::default());
        }

        merged
            .try_into()
            .map_err(|e: toml::de::Error| ConfigError::Parse(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn loader_in(dir: &Path) -> ApplicationConfigLoader {
        ApplicationConfigLoader::with_dir(dir)
    }

    fn write(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::File::create(&path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();
    }

    #[test]
    fn test_new_uses_default_config_dir() {
        let l = ApplicationConfigLoader::new();
        assert_eq!(l.config_dirs, vec![PathBuf::from("config")]);
    }

    #[test]
    fn test_with_dir_uses_supplied_path() {
        let l = ApplicationConfigLoader::with_dir("/etc/myapp/edge");
        assert_eq!(l.config_dirs, vec![PathBuf::from("/etc/myapp/edge")]);
    }

    #[test]
    fn test_with_dir_load_reads_application_toml_from_supplied_dir() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "application.toml",
            r#"service_name = "consumer-app""#,
        );
        let cfg = loader_in(dir.path()).load().unwrap();
        assert_eq!(cfg.service_name, "consumer-app");
    }

    #[test]
    fn test_with_dir_load_for_tenant_reads_tenant_from_supplied_dir() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "tenants/t1.toml", r#"service_name = "t1""#);
        let cfg = loader_in(dir.path()).load_for_tenant("t1").unwrap();
        assert_eq!(cfg.service_name, "t1");
        assert_eq!(cfg.tenant_id.as_deref(), Some("t1"));
    }

    #[test]
    fn test_load_returns_defaults_when_no_application_toml() {
        let dir = TempDir::new().unwrap();
        let cfg = loader_in(dir.path()).load().unwrap();
        assert_eq!(cfg.service_name, "swe-edge");
        assert_eq!(cfg.http_bind, "0.0.0.0:8080");
        assert!(cfg.tenant_id.is_none());
    }

    #[test]
    fn test_load_applies_application_toml_override() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "application.toml",
            r#"service_name = "ops-edge""#,
        );
        let cfg = loader_in(dir.path()).load().unwrap();
        assert_eq!(cfg.service_name, "ops-edge");
        assert_eq!(cfg.http_bind, "0.0.0.0:8080");
    }

    #[test]
    fn test_load_for_tenant_applies_tenant_toml() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "tenants/acme.toml",
            "service_name = \"acme-edge\"\nhttp_bind = \"0.0.0.0:8081\"",
        );
        let cfg = loader_in(dir.path()).load_for_tenant("acme").unwrap();
        assert_eq!(cfg.service_name, "acme-edge");
        assert_eq!(cfg.http_bind, "0.0.0.0:8081");
        assert_eq!(cfg.tenant_id.as_deref(), Some("acme"));
    }

    #[test]
    fn test_load_for_tenant_missing_file_returns_unknown_tenant_error() {
        let dir = TempDir::new().unwrap();
        let err = loader_in(dir.path()).load_for_tenant("ghost").unwrap_err();
        assert!(matches!(err, ConfigError::UnknownTenant(id) if id == "ghost"));
    }

    #[test]
    fn test_load_for_tenant_layers_over_application_toml() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "application.toml", "shutdown_timeout_secs = 60");
        write(dir.path(), "tenants/beta.toml", "service_name = \"beta\"");
        let cfg = loader_in(dir.path()).load_for_tenant("beta").unwrap();
        assert_eq!(cfg.service_name, "beta");
        assert_eq!(cfg.shutdown_timeout_secs, 60);
    }

    #[test]
    fn test_xdg_higher_priority_dir_wins_over_lower() {
        let sys_dir = TempDir::new().unwrap();
        let user_dir = TempDir::new().unwrap();
        write(sys_dir.path(), "application.toml", "service_name = \"sys\"");
        write(
            user_dir.path(),
            "application.toml",
            "service_name = \"user\"",
        );
        let loader = ApplicationConfigLoader {
            config_dirs: vec![sys_dir.path().to_path_buf(), user_dir.path().to_path_buf()],
        };
        let cfg = loader.load().unwrap();
        assert_eq!(cfg.service_name, "user"); // last dir wins
    }

    #[test]
    fn test_xdg_lower_priority_dir_fills_unset_fields() {
        let sys_dir = TempDir::new().unwrap();
        let user_dir = TempDir::new().unwrap();
        write(
            sys_dir.path(),
            "application.toml",
            "shutdown_timeout_secs = 90",
        );
        write(
            user_dir.path(),
            "application.toml",
            "service_name = \"user\"",
        );
        let loader = ApplicationConfigLoader {
            config_dirs: vec![sys_dir.path().to_path_buf(), user_dir.path().to_path_buf()],
        };
        let cfg = loader.load().unwrap();
        assert_eq!(cfg.service_name, "user");
        assert_eq!(cfg.shutdown_timeout_secs, 90); // from sys dir, not overridden
    }

    #[test]
    fn test_xdg_tenant_found_in_any_dir() {
        let sys_dir = TempDir::new().unwrap();
        let user_dir = TempDir::new().unwrap();
        // Tenant only in sys dir — user dir has no tenants/
        write(
            sys_dir.path(),
            "tenants/corp.toml",
            "service_name = \"corp\"",
        );
        let loader = ApplicationConfigLoader {
            config_dirs: vec![sys_dir.path().to_path_buf(), user_dir.path().to_path_buf()],
        };
        let cfg = loader.load_for_tenant("corp").unwrap();
        assert_eq!(cfg.service_name, "corp");
    }

    #[test]
    fn test_load_for_tenant_rejects_path_traversal_dotdot() {
        let dir = TempDir::new().unwrap();
        let err = loader_in(dir.path())
            .load_for_tenant("../../etc/passwd")
            .unwrap_err();
        assert!(matches!(err, ConfigError::InvalidTenantId(_)));
    }

    #[test]
    fn test_load_for_tenant_rejects_absolute_path() {
        let dir = TempDir::new().unwrap();
        let err = loader_in(dir.path())
            .load_for_tenant("/etc/passwd")
            .unwrap_err();
        assert!(matches!(err, ConfigError::InvalidTenantId(_)));
    }

    #[test]
    fn test_load_for_tenant_rejects_slash_in_id() {
        let dir = TempDir::new().unwrap();
        let err = loader_in(dir.path())
            .load_for_tenant("foo/bar")
            .unwrap_err();
        assert!(matches!(err, ConfigError::InvalidTenantId(_)));
    }

    #[test]
    fn test_load_for_tenant_rejects_empty_id() {
        let dir = TempDir::new().unwrap();
        let err = loader_in(dir.path()).load_for_tenant("").unwrap_err();
        assert!(matches!(err, ConfigError::InvalidTenantId(_)));
    }

    #[test]
    fn test_load_for_tenant_accepts_valid_alphanum_dash_underscore() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "tenants/tenant-01_prod.toml",
            "service_name = \"ok\"",
        );
        let cfg = loader_in(dir.path())
            .load_for_tenant("tenant-01_prod")
            .unwrap();
        assert_eq!(cfg.service_name, "ok");
    }

    #[test]
    fn test_load_rejects_application_toml_exceeding_size_limit() {
        let dir = TempDir::new().unwrap();
        // Write a file one byte over the 1 MiB limit
        let oversized = vec![b'#'; (MAX_CONFIG_FILE_BYTES + 1) as usize];
        std::fs::write(dir.path().join("application.toml"), &oversized).unwrap();
        let err = loader_in(dir.path()).load().unwrap_err();
        assert!(matches!(err, ConfigError::Io(_)));
        assert!(err.to_string().contains("1 MiB"));
    }

    #[test]
    fn test_parse_shutdown_timeout_rejects_non_numeric_value() {
        let err = ApplicationConfigLoader::parse_shutdown_timeout("not-a-number").unwrap_err();
        assert!(matches!(err, ConfigError::BadEnvVar(_)));
        assert!(err.to_string().contains("SWE_EDGE_SHUTDOWN_TIMEOUT"));
        assert!(err.to_string().contains("not-a-number"));
    }

    #[test]
    fn test_parse_shutdown_timeout_rejects_negative_representation() {
        let err = ApplicationConfigLoader::parse_shutdown_timeout("-1").unwrap_err();
        assert!(matches!(err, ConfigError::BadEnvVar(_)));
    }

    #[test]
    fn test_parse_shutdown_timeout_accepts_valid_integer() {
        assert_eq!(
            ApplicationConfigLoader::parse_shutdown_timeout("120").unwrap(),
            120
        );
        assert_eq!(
            ApplicationConfigLoader::parse_shutdown_timeout("0").unwrap(),
            0
        );
    }

    // ── load_section ────────────────────────────────────────────────────────

    #[derive(Debug, Default, serde::Deserialize, PartialEq)]
    #[serde(default)]
    struct ApplicationConfigLoaderSection {
        value: String,
        count: u32,
    }

    #[test]
    fn test_load_section_reads_from_application_toml() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "application.toml",
            "[my_section]\nvalue = \"hello\"\ncount = 7",
        );
        let section: ApplicationConfigLoaderSection =
            loader_in(dir.path()).load_section("my_section").unwrap();
        assert_eq!(section.value, "hello");
        assert_eq!(section.count, 7);
    }

    #[test]
    fn test_load_section_falls_back_to_default_when_key_absent() {
        let dir = TempDir::new().unwrap();
        let section: ApplicationConfigLoaderSection = loader_in(dir.path())
            .load_section("nonexistent_section")
            .unwrap();
        assert_eq!(section, ApplicationConfigLoaderSection::default());
    }

    #[test]
    fn test_load_section_later_application_toml_overrides_earlier() {
        let low = TempDir::new().unwrap();
        let high = TempDir::new().unwrap();
        write(low.path(), "application.toml", "[s]\nvalue = \"low\"");
        write(high.path(), "application.toml", "[s]\nvalue = \"high\"");
        let loader = ApplicationConfigLoader {
            config_dirs: vec![low.path().to_path_buf(), high.path().to_path_buf()],
        };
        let section: ApplicationConfigLoaderSection = loader.load_section("s").unwrap();
        assert_eq!(section.value, "high");
    }

    #[test]
    fn test_load_section_supports_dotted_key_path() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "application.toml",
            "[outer.inner]\nvalue = \"deep\"\ncount = 3",
        );
        let section: ApplicationConfigLoaderSection =
            loader_in(dir.path()).load_section("outer.inner").unwrap();
        assert_eq!(section.value, "deep");
        assert_eq!(section.count, 3);
    }

    #[test]
    fn test_load_section_observability_tracing_reads_from_default_toml() {
        use crate::api::config::TracingConfig;
        use swe_edge_observ_config::TracingLevel;
        let dir = TempDir::new().unwrap();
        let cfg: TracingConfig = loader_in(dir.path())
            .load_section("observability.tracing")
            .unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.level, TracingLevel::Info);
    }

    #[test]
    fn test_load_section_application_toml_overrides_default_toml_tracing() {
        use crate::api::config::TracingConfig;
        use swe_edge_observ_config::TracingLevel;
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "application.toml",
            "[observability.tracing]\nlevel = \"debug\"",
        );
        let cfg: TracingConfig = loader_in(dir.path())
            .load_section("observability.tracing")
            .unwrap();
        assert_eq!(cfg.level, TracingLevel::Debug);
        assert!(cfg.enabled); // inherited from default.toml
    }
}
