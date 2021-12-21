use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CPPConfiguration {
    pub(crate) standard: Option<CPPStandard>,
}

#[derive(Deserialize, Serialize)]
pub(crate) enum CPPStandard {
    #[serde(rename = "98")]
    NinetyEight,
    #[serde(rename = "3")]
    Three,
    #[serde(rename = "11")]
    Eleven,
    #[serde(rename = "14")]
    Fourteen,
    #[serde(rename = "17")]
    Seventeen,
    #[serde(rename = "20")]
    Twenty,
}
