//! Ports needed by the audit feature.

// vhco:port Registry { describe(input: CatalogQuery) -> Catalog; effect_sites(input: EffectQuery) -> List<EffectSite> }
pub use crate::domain::ports::Registry;
// vhco:port PolicyEvaluator { evaluate(input: EffectIntent) -> Permit }
pub use crate::domain::ports::PolicyEvaluator;
// vhco:port TraceStore { read(input: TraceQuery) -> TraceResult }
pub use crate::domain::ports::TraceStore;
// vhco:port FileProbe { stat(input: FileProbeInput) -> FileProbeResult }
pub use crate::domain::ports::FileProbe;
