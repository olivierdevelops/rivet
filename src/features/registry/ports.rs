//! Ports needed by the registry feature.

// vhco:port Registry { describe(CatalogQuery) -> Catalog; outputs(OutputQuery) -> OutputReport; effect_sites(EffectQuery) -> List<EffectSite> }
pub use crate::domain::ports::Registry;

// vhco:port CatalogStore { current() -> CatalogSnapshot; compile(SourceBundle) -> CatalogSnapshot; publish(CatalogSnapshot) -> Unit }
pub use crate::domain::ports::CatalogStore;
