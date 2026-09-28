//! Wires scoped file handles (`with file open … as NAME`, G35) into the
//! interpreter: the `file` kind's scoped form gets the confined adapter with
//! the `files.open_file_stream` use case injected (one-shot `file VERB` stays
//! on the interpreter's policed FileAccess).
//!
//! ```text
//!  with file open P mode M ─▶ FileStreams ── open_file_stream (authorize) ──▶ cap-std Dir
//! ```

use crate::domain::files::FileStreamRequest;
use crate::domain::ports::PolicyEvaluator;
use crate::features::files::open_file_stream::open_file_stream;
use crate::infra::execution_driver::Interpreter;
use crate::infra::file_stream::{FileStreams, OpenFileStreamFn};
use std::sync::Arc;

pub fn register(interp: &mut Interpreter, root: &str) {
    let open: Arc<OpenFileStreamFn> =
        Arc::new(|r: FileStreamRequest, ev: &dyn PolicyEvaluator| open_file_stream(r, ev));
    interp.register_adapter("file", Arc::new(FileStreams::new(root, open)));
}
