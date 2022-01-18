use crate::config::{
    CConfiguration, CPPConfiguration, GCCConfiguration, GPPConfiguration, ProjectConfiguration,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BuildConfiguration {
    pub(crate) project: ProjectConfiguration,
    pub(crate) c: Option<CConfiguration>,
    pub(crate) cpp: Option<CPPConfiguration>,
    pub(crate) gcc: Option<GCCConfiguration>,
    pub(crate) gpp: Option<GPPConfiguration>,
}
