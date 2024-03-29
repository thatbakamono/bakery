use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
pub(crate) struct ToolchainConfiguration {
    pub(crate) gcc_location: Option<String>,
    pub(crate) gpp_location: Option<String>,
    pub(crate) ar_location: Option<String>,
}
