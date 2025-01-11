use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CppConfiguration {
    pub(crate) standard: Option<CppStandard>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) enum CppStandard {
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
    #[serde(rename = "23")]
    TwentyThree,
    #[serde(rename = "26")]
    TwentySix,
}

impl CppStandard {
    pub(crate) fn latest() -> CppStandard {
        CppStandard::TwentySix
    }
}
