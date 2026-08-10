use anyhow::{anyhow, Result};
use url::Url;

/// Actions that can be performed via the `warp://linear/...` deeplink.
#[derive(Debug, PartialEq, Eq)]
pub enum LinearAction {
    /// Open a new agent view tab to work on a Linear issue.
    WorkOnIssue,
}

impl LinearAction {
    pub fn parse(url: &Url) -> Result<Self> {
        match url.path() {
            "/work" => Ok(Self::WorkOnIssue),
            other => Err(anyhow!(
                "Received \"linear\" intent with unexpected path: {other}"
            )),
        }
    }
}
