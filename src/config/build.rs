use crate::config::{
    CConfiguration, CppConfiguration, GccConfiguration, GppConfiguration, ProjectConfiguration,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BuildConfiguration {
    pub(crate) project: ProjectConfiguration,
    pub(crate) c: Option<CConfiguration>,
    pub(crate) cpp: Option<CppConfiguration>,
    pub(crate) gcc: Option<GccConfiguration>,
    pub(crate) gpp: Option<GppConfiguration>,
}
