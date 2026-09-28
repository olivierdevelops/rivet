//! Ports needed by the registry feature.

// vhco:port Registry { describe(CatalogQuery) -> Catalog; outputs(OutputQuery) -> OutputReport; effect_sites(EffectQuery) -> List<EffectSite> }
pub use crate::domain::ports::Registry;
