//! Ports needed by the grpc feature.

// vhco:port GrpcDriver { catalog() -> GrpcCatalog; validate_input(GrpcMethodInfo, Value) -> Unit; resolve(host: string, port: int) -> List<IpAddr>; invoke(GrpcDial) -> GrpcCall }
pub use crate::domain::ports::GrpcDriver;
pub use crate::domain::ports::{GrpcCall, GrpcReceiver, GrpcSender};
// vhco:port PolicyEvaluator { evaluate(EffectIntent) -> Permit }
pub use crate::domain::ports::PolicyEvaluator;
// vhco:port CredentialProvider { acquire(CredentialInput) -> CredentialLease }
pub use crate::domain::ports::CredentialProvider;
