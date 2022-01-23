use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectConfiguration {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) author: Option<String>,
    pub(crate) language: Language,
    pub(crate) distribution: Distribution,
    pub(crate) sources: Option<Vec<String>>,
    pub(crate) includes: Option<Vec<String>>,
    pub(crate) optimization: Option<OptimizationLevel>,
    pub(crate) enable_all_warnings: Option<bool>,
    pub(crate) treat_all_warnings_as_errors: Option<bool>,
}

#[derive(Deserialize, Serialize)]
pub(crate) enum Language {
    #[serde(alias = "c")]
    C,
    #[serde(rename = "C++", alias = "c++")]
    CPP,
}

#[derive(PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Distribution {
    Executable,
    StaticLibrary,
    DynamicLibrary,
}

#[derive(Deserialize, Serialize)]
pub(crate) enum OptimizationLevel {
    #[serde(rename = "0")]
    Zero,
    #[serde(rename = "1")]
    One,
    #[serde(rename = "2")]
    Two,
    #[serde(rename = "3")]
    Three,
    #[serde(rename = "4")]
    Four,
    #[serde(alias = "size")]
    Size,
    #[serde(alias = "debug")]
    Debug,
}
