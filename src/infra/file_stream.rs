//! Scoped file handles: `with file open PATH mode read|write|append as NAME`
//! (G35). The `files.open_file_stream` use case (injected as a closure)
//! validates the mode/chunk size and authorizes the mode's access verbs; this
//! adapter then opens a root-relative, no-follow handle under the bundle root.
//!
//! ```text
//!  with file open P mode M as h ──▶ open_file_stream (authorize) ──▶ cap-std Dir (no-follow)
//!     for chunk in h        read   ─▶ bytes chunks of ≤ chunk_size until EOF
//!     h.write bytes|text V  write  ─▶ create/truncate │ append ─▶ existing file only
//!  end ─────────────────────────────▶ flush + sync, handle dropped
//!  with file watch …  ──▶ unsupported.stage_c (no watcher exists in this build)
//! ```

use super::execution_driver::{EffectAdapter, EffectCtx, EvalArg, EvaluatedForm, ResourceHandle};
use super::file_access::{io_err, refuse_hardlink, refuse_symlink_components, rel};
use crate::domain::files::{FileStreamMode, FileStreamPlan, FileStreamRequest};
use crate::domain::ir::EffectForm;
use crate::domain::ports::PolicyEvaluator;
use crate::domain::{RivetError, RivetResult, Value};
use async_trait::async_trait;
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The `files.open_file_stream` use case as the adapter receives it.
pub type OpenFileStreamFn =
    dyn Fn(FileStreamRequest, &dyn PolicyEvaluator) -> RivetResult<FileStreamPlan> + Send + Sync;

// vhco:infra file_stream satisfies FileAccess
// vhco:file readwrite <bundle root>/** -- `with file open` handles on paths authorized by files.open_file_stream, resolved under the bundle root without following links
pub struct FileStreams {
    root: PathBuf,
    open: std::sync::Arc<OpenFileStreamFn>,
}

impl FileStreams {
    pub fn new(root: &str, open: std::sync::Arc<OpenFileStreamFn>) -> FileStreams {
        FileStreams {
            root: PathBuf::from(root),
            open,
        }
    }
}

fn arg_text(a: Option<&EvalArg>) -> Option<String> {
    match a? {
        EvalArg::Word(w) => Some(w.clone()),
        EvalArg::Value(Value::Text(s)) => Some(s.clone()),
        EvalArg::Value(v) => Some(v.to_display()),
    }
}

/// `open PATH mode M` → (path, mode as written); mode defaults to read.
fn head(args: &EvaluatedForm) -> RivetResult<(String, String)> {
    let path = match args.head.get(1) {
        Some(EvalArg::Value(Value::Text(p))) => p.clone(),
        _ => {
            return Err(RivetError::validation(
                "validation.file_path",
                "`with file open` needs a text PATH: `with file open PATH mode read as NAME`",
            ));
        }
    };
    let mode = args
        .head
        .iter()
        .position(|a| a.word() == Some("mode"))
        .map(|i| arg_text(args.head.get(i + 1)).unwrap_or_default())
        .unwrap_or_else(|| "read".into());
    Ok((path, mode))
}

fn chunk_size(args: &EvaluatedForm) -> RivetResult<Option<i64>> {
    // `chunk_size N` as its own option line or trailing on the `with` line.
    let trailing = args
        .head
        .iter()
        .position(|a| a.word() == Some("chunk_size"))
        .map(|i| args.head.get(i + 1));
    let value = match (
        args.options.iter().find(|(k, _)| k == "chunk_size"),
        trailing,
    ) {
        (Some((_, vals)), _) => vals.first(),
        (None, Some(v)) => v,
        (None, None) => return Ok(None),
    };
    match value {
        Some(EvalArg::Value(Value::Int(n))) => Ok(Some(*n)),
        _ => Err(RivetError::validation(
            "validation.chunk_size",
            "`chunk_size` takes an integer byte count",
        )),
    }
}

/// Open the plan's path under `root`: symlinks anywhere on the path are
/// refused; reads need a regular file; writes refuse hard-linked files.
fn open_sync(root: &PathBuf, plan: &FileStreamPlan) -> RivetResult<std::fs::File> {
    let dir = Dir::open_ambient_dir(root, ambient_authority()).map_err(|e| {
        RivetError::not_found("file.root", format!("bundle root {}: {e}", root.display()))
    })?;
    let path = plan.path.as_str();
    let r = rel(path)?;
    let file = match plan.mode {
        FileStreamMode::Read => {
            refuse_symlink_components(&dir, &r, path, true)?;
            let meta = dir.symlink_metadata(&r).map_err(|e| io_err(path, e))?;
            if !meta.is_file() {
                return Err(RivetError::validation(
                    "file.not_regular",
                    format!("{path} is not a regular file"),
                ));
            }
            dir.open(&r).map_err(|e| io_err(path, e))?
        }
        FileStreamMode::Write | FileStreamMode::Append => {
            refuse_symlink_components(&dir, &r, path, false)?;
            refuse_hardlink(&dir, &r, path)?;
            let mut o = OpenOptions::new();
            if plan.mode == FileStreamMode::Write {
                o.write(true).create(true).truncate(true);
            } else {
                // Append never creates a file (S37).
                o.append(true);
            }
            dir.open_with(&r, &o).map_err(|e| io_err(path, e))?
        }
    };
    Ok(file.into_std())
}

#[async_trait]
impl EffectAdapter for FileStreams {
    async fn open(
        &self,
        ctx: &EffectCtx,
        _form: &EffectForm,
        args: EvaluatedForm,
    ) -> RivetResult<Box<dyn ResourceHandle>> {
        match args.head.first().and_then(EvalArg::word) {
            Some("open") => {}
            Some("watch") => {
                return Err(RivetError::unsupported(
                    "unsupported.stage_c",
                    "`with file watch` is Stage C and not available in this build (see rivet.capabilities)",
                ));
            }
            other => {
                return Err(RivetError::validation(
                    "validation.file_scope",
                    format!(
                        "`with file {}` is not a scoped form; use `with file open PATH mode read|write|append as NAME` or the one-shot `file VERB PATH`",
                        other.unwrap_or("")
                    ),
                ));
            }
        }
        let (path, mode) = head(&args)?;
        let plan = (self.open)(
            FileStreamRequest {
                path,
                mode,
                chunk_size: chunk_size(&args)?,
                operation_id: ctx.operation_id.clone(),
                span: Some(ctx.span.clone()),
            },
            ctx.policy.as_ref(),
        )?;
        let root = self.root.clone();
        let p = plan.clone();
        let file = tokio::task::spawn_blocking(move || open_sync(&root, &p))
            .await
            .map_err(|e| RivetError::internal(format!("file task failed: {e}")))??;
        Ok(Box::new(FileHandle {
            plan,
            file: Some(tokio::fs::File::from_std(file)),
            written: 0,
        }))
    }
}

struct FileHandle {
    plan: FileStreamPlan,
    file: Option<tokio::fs::File>,
    written: u64,
}

impl FileHandle {
    fn file(&mut self) -> RivetResult<&mut tokio::fs::File> {
        self.file.as_mut().ok_or_else(|| {
            RivetError::new(
                crate::domain::ErrorKind::Cleanup,
                "cleanup.closed",
                format!("{} is already closed", self.plan.path),
            )
        })
    }
}

#[async_trait]
impl ResourceHandle for FileHandle {
    async fn next(&mut self, _ctx: &EffectCtx) -> RivetResult<Option<Value>> {
        if self.plan.mode != FileStreamMode::Read {
            return Err(RivetError::unsupported(
                "unsupported.iterate",
                format!(
                    "a file opened with mode {} cannot be iterated; open it with mode read",
                    self.plan.mode.as_str()
                ),
            ));
        }
        let (limit, path) = (self.plan.chunk_size, self.plan.path.clone());
        let mut buf = Vec::with_capacity(limit.min(1 << 20) as usize);
        let f = self.file()?;
        f.take(limit)
            .read_to_end(&mut buf)
            .await
            .map_err(|e| io_err(&path, e))?;
        Ok(if buf.is_empty() {
            None
        } else {
            Some(Value::Bytes(buf))
        })
    }

    async fn call(
        &mut self,
        _ctx: &EffectCtx,
        method: &str,
        args: Vec<EvalArg>,
    ) -> RivetResult<Value> {
        if method != "write" {
            return Err(RivetError::unsupported(
                "unsupported.method",
                format!("a file handle has no `{method}` method; use `NAME.write bytes|text V`"),
            ));
        }
        if self.plan.mode == FileStreamMode::Read {
            return Err(RivetError::unsupported(
                "unsupported.method",
                "a file opened with mode read cannot be written; open it with mode write or append",
            ));
        }
        let (codec, value) = match args.as_slice() {
            [EvalArg::Word(c), EvalArg::Value(v)] => (Some(c.as_str()), v),
            [EvalArg::Value(v)] => (None, v),
            _ => {
                return Err(RivetError::validation(
                    "validation.file_write",
                    "expected `NAME.write bytes V` or `NAME.write text V`",
                ));
            }
        };
        let bytes = match (codec, value) {
            (Some("bytes") | None, Value::Bytes(b)) => b.clone(),
            (Some("text") | None, Value::Text(s)) => s.clone().into_bytes(),
            (Some("text"), other) => other.to_display().into_bytes(),
            (c, other) => {
                return Err(RivetError::validation(
                    "file.codec",
                    format!(
                        "`write {}` cannot encode a {} value",
                        c.unwrap_or(""),
                        other.type_name()
                    ),
                ));
            }
        };
        let path = self.plan.path.clone();
        let f = self.file()?;
        f.write_all(&bytes).await.map_err(|e| io_err(&path, e))?;
        self.written += bytes.len() as u64;
        Ok(Value::object([
            ("path", Value::text(&path)),
            ("written", Value::Int(bytes.len() as i64)),
            ("total", Value::Int(self.written as i64)),
        ]))
    }

    async fn property(&mut self, _ctx: &EffectCtx, name: &str) -> RivetResult<Value> {
        match name {
            "path" => Ok(Value::text(&self.plan.path)),
            "mode" => Ok(Value::text(self.plan.mode.as_str())),
            "written" => Ok(Value::Int(self.written as i64)),
            _ => Err(RivetError::unsupported(
                "unsupported.property",
                format!("a file handle has no `{name}` property (path, mode, written)"),
            )),
        }
    }

    async fn close(mut self: Box<Self>) -> RivetResult<()> {
        let path = self.plan.path.clone();
        if let Some(mut f) = self.file.take()
            && self.plan.mode != FileStreamMode::Read
        {
            f.flush().await.map_err(|e| io_err(&path, e))?;
            f.sync_all().await.map_err(|e| io_err(&path, e))?;
        }
        Ok(())
    }
}
