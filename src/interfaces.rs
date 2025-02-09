use serde::{Deserialize, Serialize};

pub mod admins;
pub mod collections;
pub mod files;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SimpleOk {
    pub ok: bool,
}
