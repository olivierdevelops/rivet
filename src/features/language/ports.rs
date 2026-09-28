//! Ports needed by the language feature.

// vhco:port Parser { parse(SourceBundle) -> SyntaxTree }
pub use crate::domain::ports::Parser;

// vhco:port SourceLoader { load(string) -> SourceBundle; read_module(root: string, path: string) -> SourceFile; policy_beside(string) -> bool }
pub use crate::domain::ports::SourceLoader;
