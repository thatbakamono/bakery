use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GppConfiguration {
    #[serde(default)]
    pub(crate) additional_pre_arguments: Vec<String>,
    #[serde(default)]
    pub(crate) additional_post_arguments: Vec<String>,
}
