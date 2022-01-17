use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
pub(crate) struct EzConfiguration {
    pub(crate) gcc_location: Option<String>,
}
