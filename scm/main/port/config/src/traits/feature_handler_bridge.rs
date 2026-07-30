//! [`FeatureHandlerBridge`] — bridge from `FeatureRegistry` to `OptionalHandler`.

use edge_dispatch::OptionalHandler;
use swe_edge_configbuilder::{FeatureRegistry, FeatureState, OptionalSection, SectionLoaderImpl};

use crate::error::ConfigError;

/// Constructs a handler from a loaded feature config.
///
/// Owned locally: upstream `edge-domain`/`edge-application` dropped this factory-from-config
/// pattern (it is a bootstrap-level, config-driven construction concern, not a domain port).
pub trait HandlerFactory: Sized {
    /// The config type this factory is constructed from.
    type Config;
    /// Construct `Self` from a loaded `Config`.
    fn build(cfg: Self::Config) -> Result<Self, edge_application::HandlerError>;
}

/// Extends [`FeatureRegistry`] with a one-call bridge to [`OptionalHandler`].
///
/// Collapses the three-step config-to-handler pattern into one call:
///
/// ```rust,no_run
/// use swe_edge_configbuilder::{FeatureRegistry, SectionLoaderImpl, OptionalSection};
/// use swe_edge_bootstrap_config::{FeatureHandlerBridge, HandlerFactory};
/// use edge_dispatch::{Handler, HandlerError, OptionalHandler};
/// use edge_application_handler::ExecutionRequest;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct GuardConfig { token: String }
///
/// impl OptionalSection for GuardConfig {
///     fn section_name() -> &'static str { "guard" }
/// }
///
/// #[derive(Clone)]
/// struct GuardPayload(String);
/// impl edge_application::Request for GuardPayload {}
/// impl edge_application::Response for GuardPayload {}
///
/// struct GuardHandler { token: String }
///
/// #[async_trait::async_trait]
/// impl Handler for GuardHandler {
///     type Request = GuardPayload;
///     type Response = GuardPayload;
///     async fn execute(&self, req: ExecutionRequest<'_, GuardPayload>) -> Result<GuardPayload, HandlerError> {
///         Ok(req.req)
///     }
/// }
///
/// impl HandlerFactory for GuardHandler {
///     type Config = GuardConfig;
///     fn build(cfg: GuardConfig) -> Result<Self, HandlerError> {
///         Ok(GuardHandler { token: cfg.token })
///     }
/// }
///
/// # async fn example(loader: SectionLoaderImpl) {
/// let mut registry = FeatureRegistry::new();
/// let guard: OptionalHandler<GuardHandler> =
///     registry.build_handler::<GuardConfig, GuardHandler>(&loader).unwrap();
/// # }
/// ```
pub trait FeatureHandlerBridge {
    /// Load feature config `C` from `loader` and construct `H` via [`HandlerFactory::build`].
    ///
    /// - If the feature is enabled: calls `H::build(cfg)` and returns an enabled `OptionalHandler<H>`.
    /// - If the feature is disabled: returns a disabled `OptionalHandler<H>` without calling `H::build`.
    /// - If `load` or `H::build` fails: propagates the error.
    fn build_handler<C, H>(
        &mut self,
        loader: &SectionLoaderImpl,
    ) -> Result<OptionalHandler<H>, ConfigError>
    where
        C: OptionalSection,
        H: HandlerFactory<Config = C>;
}

impl FeatureHandlerBridge for FeatureRegistry {
    fn build_handler<C, H>(
        &mut self,
        loader: &SectionLoaderImpl,
    ) -> Result<OptionalHandler<H>, ConfigError>
    where
        C: OptionalSection,
        H: HandlerFactory<Config = C>,
    {
        match self.load::<C>(loader)? {
            FeatureState::Enabled(cfg) => {
                let handler =
                    H::build(cfg).map_err(|e| ConfigError::HandlerBuild(e.to_string()))?;
                Ok(OptionalHandler::enabled(handler))
            }
            FeatureState::Disabled => Ok(OptionalHandler::disabled()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use edge_application_handler::ExecutionRequest;
    use edge_dispatch::{Handler, HandlerError};
    use serde::Deserialize;
    use swe_edge_configbuilder::ConfigLoaderFactory;

    #[derive(Deserialize)]
    struct GuardConfig {
        #[allow(dead_code)]
        token: String,
    }
    impl OptionalSection for GuardConfig {
        fn section_name() -> &'static str {
            "guard"
        }
    }

    #[derive(Clone)]
    struct GuardPayload(#[allow(dead_code)] String);
    impl edge_application::Request for GuardPayload {}
    impl edge_application::Response for GuardPayload {}

    struct GuardHandler;

    #[async_trait::async_trait]
    impl Handler for GuardHandler {
        type Request = GuardPayload;
        type Response = GuardPayload;
        async fn execute(
            &self,
            req: ExecutionRequest<'_, GuardPayload>,
        ) -> Result<GuardPayload, HandlerError> {
            Ok(req.req)
        }
    }

    impl HandlerFactory for GuardHandler {
        type Config = GuardConfig;
        fn build(_cfg: GuardConfig) -> Result<Self, HandlerError> {
            Ok(GuardHandler)
        }
    }

    #[test]
    fn test_build_handler_returns_enabled_when_section_present() {
        let dir = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(e) => panic!("failed to create tempdir for test fixture: {e}"),
        };
        if let Err(e) = std::fs::write(
            dir.path().join("application.toml"),
            "[guard]\ntoken = \"abc\"\n",
        ) {
            panic!("failed to write test fixture application.toml: {e}");
        }
        let loader = ConfigLoaderFactory::create_loader_for_dir(dir.path());

        let mut registry = FeatureRegistry::new();
        let guard: OptionalHandler<GuardHandler> =
            match registry.build_handler::<GuardConfig, GuardHandler>(&loader) {
                Ok(guard) => guard,
                Err(e) => panic!("build_handler must succeed, got {e:?}"),
            };

        assert!(guard.is_enabled());
    }

    #[test]
    fn test_build_handler_returns_disabled_when_section_absent() {
        let dir = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(e) => panic!("failed to create tempdir for test fixture: {e}"),
        };
        if let Err(e) = std::fs::write(dir.path().join("application.toml"), "[other]\nx = 1\n") {
            panic!("failed to write test fixture application.toml: {e}");
        }
        let loader = ConfigLoaderFactory::create_loader_for_dir(dir.path());

        let mut registry = FeatureRegistry::new();
        let guard: OptionalHandler<GuardHandler> =
            match registry.build_handler::<GuardConfig, GuardHandler>(&loader) {
                Ok(guard) => guard,
                Err(e) => panic!("build_handler must succeed, got {e:?}"),
            };

        assert!(!guard.is_enabled());
    }
}
