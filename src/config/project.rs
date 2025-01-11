use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectConfiguration {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) author: Option<String>,
    pub(crate) language: Language,
    #[serde(default)]
    pub(crate) distribution: Distribution,
    #[serde(default)]
    pub(crate) sources: Vec<String>,
    #[serde(default)]
    pub(crate) includes: Vec<String>,
    #[serde(default)]
    pub(crate) dependencies: Vec<Dependency>,
    #[serde(default)]
    pub(crate) optimization: OptimizationLevel,
    #[serde(default)]
    pub(crate) enable_all_warnings: bool,
    #[serde(default)]
    pub(crate) treat_all_warnings_as_errors: bool,
}

#[derive(Deserialize, Serialize)]
pub(crate) enum Language {
    #[serde(alias = "c")]
    C,
    #[serde(rename = "C++", alias = "c++")]
    Cpp,
}

#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Distribution {
    Executable,
    StaticLibrary,
    DynamicLibrary,
}

impl Default for Distribution {
    fn default() -> Self {
        Self::Executable
    }
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub(crate) enum Dependency {
    Local { path: String },
    System { name: String },
}

#[derive(Clone, Deserialize, Serialize)]
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

impl Default for OptimizationLevel {
    fn default() -> Self {
        Self::Zero
    }
}
