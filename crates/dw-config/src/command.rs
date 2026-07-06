use anyhow::Result;
use dw_core::{ConfigColorMode, ConfigRootPath, DevWorkflowRoot};
use serde::{Deserialize, Serialize};

use crate::{
    InitReport, InitRequest, RefreshReport, RefreshRequest, config_doctor, config_show, init_root,
    refresh_root, resolve_root, set_color_mode, set_user_root,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitCommandArgs {
    pub profile: String,
    pub root: Option<String>,
    pub dry_run: bool,
    pub no_save: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshCommandArgs {
    pub root: Option<String>,
    pub profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigRootSetReport {
    pub path: ConfigRootPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigColorSetReport {
    pub mode: ConfigColorMode,
}

pub fn show(root: Option<&DevWorkflowRoot>) -> crate::ConfigShow {
    config_show(root.map(DevWorkflowRoot::as_str))
}

pub fn doctor(root: Option<&DevWorkflowRoot>) -> crate::ConfigDoctorReport {
    config_doctor(root.map(DevWorkflowRoot::as_str))
}

pub fn set_root(path: &ConfigRootPath) -> Result<ConfigRootSetReport> {
    Ok(ConfigRootSetReport {
        path: ConfigRootPath::from(set_user_root(path.as_str())?),
    })
}

pub fn set_color(mode: &ConfigColorMode) -> Result<ConfigColorSetReport> {
    Ok(ConfigColorSetReport {
        mode: set_color_mode(*mode)?,
    })
}

pub fn init(args: InitCommandArgs) -> Result<InitReport> {
    Ok(init_root(InitRequest {
        root: args.root,
        profile: args.profile,
        no_save: args.no_save,
        dry_run: args.dry_run,
    })?)
}

pub fn refresh(args: RefreshCommandArgs) -> Result<RefreshReport> {
    let root = resolve_root(args.root.as_deref());
    Ok(refresh_root(RefreshRequest {
        root,
        profile: Some(args.profile),
    })?)
}
