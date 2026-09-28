//! Ports needed by the transports feature.

// vhco:port HttpClient { resolve(HostPort) -> List<IpAddr>; send(HttpWire) -> HttpReply; wait(Delay) -> Unit }
pub use crate::domain::transports::HttpClient;
// vhco:port Codec { decode(CodecInput) -> Value; encode(CodecInput) -> Bytes }
pub use crate::domain::transports::Codec;
// vhco:port SocketStream { resolve(HostPort) -> List<IpAddr>; connect(SocketPlan) -> SocketConnection }
pub use crate::domain::transports::SocketStream;
// vhco:port ProcessRunner { run(ProcessPlan) -> ProcessResult; spawn(ProcessPlan) -> ByteStream }
pub use crate::domain::ports::PolicyEvaluator;
pub use crate::domain::transports::ProcessRunner;
