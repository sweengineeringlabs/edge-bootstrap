//! [`FeatureRegistryExt`] — bridge from `FeatureRegistry` to `OptionalHandler`.

use edge_dispatch::OptionalHandler;
use edge_domain::HandlerFactory;
use swe_edge_configbuilder::{FeatureRegistry, FeatureState, OptionalSection, SectionLoaderImpl};

use crate::api::config::error::ConfigError;

/// Extends [`FeatureRegistry`] with a one-call bridge to [`OptionalHandler`].
///
/// Collapses the three-step config-to-handler pattern into one call:
///
/// ```rust,no_run
/// use swe_edge_configbuilder::{FeatureRegistry, SectionLoaderImpl, OptionalSection};
/// use swe_edge_bootstrap::{FeatureRegistryExt, HandlerFactory};
/// use edge_dispatch::{Handler, HandlerError, OptionalHandler};
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct GuardConfig { token: String }
///
/// impl OptionalSection for GuardConfig {
///     fn section_name() -> &'static str { "guard" }
/// }
///
/// struct GuardHandler { token: String }
///
/// #[async_trait::async_trait]
/// impl Handler for GuardHandler {
///     type Request = String;
///     type Response = String;
///     fn id(&self) -> &str { "guard" }
///     fn pattern(&self) -> &str { "/guard" }
///     async fn execute(&self, req: String) -> Result<String, HandlerError> {
///         Ok(req)
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
pub trait FeatureRegistryExt {
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

impl FeatureRegistryExt for FeatureRegistry {
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
