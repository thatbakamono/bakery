use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GPPConfiguration {
    pub(crate) additional_pre_arguments: Option<Vec<String>>,
    pub(crate) additional_post_arguments: Option<Vec<String>>,
}
