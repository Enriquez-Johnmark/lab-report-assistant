//! Recorded model answers (G4). A live run with the owner's key saves each
//! answer under a fingerprint of the model and the exact request; tests and
//! CI replay them with no key and no network. A request nobody recorded
//! fails rather than reaching a model.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::engine::llm::{LanguageModel, LlmError, LlmReply, LlmRequest};

/// Where the project's recorded answers are committed.
pub const DIR: &str = "evalset/recorded";

#[derive(Serialize, Deserialize)]
struct Saved {
    model: String,
    text: String,
    request_id: Option<String>,
    input_tokens: u32,
    output_tokens: u32,
}

/// The fingerprint of `request` sent to `model`.
pub fn fingerprint(model: &str, request: &LlmRequest) -> String {
    let mut hash = Sha256::new();
    for part in [
        model,
        &request.system,
        &request.user,
        &request.schema.to_string(),
        &request.max_tokens.to_string(),
    ] {
        hash.update(part.as_bytes());
        hash.update([0]);
    }
    hex::encode(hash.finalize())
}

fn path(dir: &Path, model: &str, request: &LlmRequest) -> PathBuf {
    dir.join(format!("{}.json", fingerprint(model, request)))
}

/// Answers from the recordings in a folder.
pub struct Recorded {
    model: String,
    dir: PathBuf,
}

impl Recorded {
    /// The recordings of `model` in `dir`.
    pub fn new(model: &str, dir: &Path) -> Self {
        Self {
            model: model.to_owned(),
            dir: dir.to_owned(),
        }
    }
}

impl LanguageModel for Recorded {
    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(&self, request: &LlmRequest) -> Result<LlmReply, LlmError> {
        let file = path(&self.dir, &self.model, request);
        let saved = std::fs::read_to_string(&file).map_err(|_| {
            LlmError::Unavailable(format!(
                "no recorded answer for this request ({}); record it with the owner's key",
                file.display()
            ))
        })?;
        let saved: Saved = serde_json::from_str(&saved).map_err(|err| LlmError::Malformed(err.to_string()))?;
        Ok(LlmReply {
            text: saved.text,
            model: saved.model,
            request_id: saved.request_id,
            input_tokens: saved.input_tokens,
            output_tokens: saved.output_tokens,
        })
    }
}

/// A live model whose every answer is also saved to a folder.
pub struct Recording<M> {
    inner: M,
    dir: PathBuf,
}

impl<M: LanguageModel> Recording<M> {
    /// Records `inner`'s answers in `dir`.
    pub fn new(inner: M, dir: &Path) -> Self {
        Self {
            inner,
            dir: dir.to_owned(),
        }
    }

    /// The live model.
    pub fn inner(&self) -> &M {
        &self.inner
    }
}

impl<M: LanguageModel> LanguageModel for Recording<M> {
    fn model(&self) -> &str {
        self.inner.model()
    }

    async fn complete(&self, request: &LlmRequest) -> Result<LlmReply, LlmError> {
        let reply = self.inner.complete(request).await?;
        let saved = Saved {
            model: reply.model.clone(),
            text: reply.text.clone(),
            request_id: reply.request_id.clone(),
            input_tokens: reply.input_tokens,
            output_tokens: reply.output_tokens,
        };
        let json = serde_json::to_string_pretty(&saved).map_err(|err| LlmError::Malformed(err.to_string()))?;
        std::fs::create_dir_all(&self.dir)
            .and_then(|()| std::fs::write(path(&self.dir, self.inner.model(), request), format!("{json}\n")))
            .map_err(|err| LlmError::Unavailable(format!("couldn't save the recorded answer: {err}")))?;
        Ok(reply)
    }
}
