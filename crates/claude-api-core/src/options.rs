//! Per-request options.

/// Headers that vary per request: extra `anthropic-beta` values and the target workspace.
///
/// ```
/// use claude_api_core::RequestOptions;
///
/// let options = RequestOptions::default().beta("fast-mode-2026-02-01").workspace_id("wrkspc_01Example");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestOptions {
    pub(crate) betas: Vec<String>,
    pub(crate) workspace_id: Option<String>,
}

impl RequestOptions {
    /// Adds an `anthropic-beta` value for this request, on top of the client's defaults.
    pub fn beta(mut self, beta: impl Into<String>) -> Self {
        let beta = beta.into();
        if !self.betas.contains(&beta) {
            self.betas.push(beta);
        }
        self
    }

    /// Sends `anthropic-workspace-id`, selecting the workspace a multi-workspace key acts in.
    pub fn workspace_id(mut self, workspace_id: impl Into<String>) -> Self {
        self.workspace_id = Some(workspace_id.into());
        self
    }
}
