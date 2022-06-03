use std::fs::File;

use serde_json::{from_reader, json, Map, Value};

use crate::{
    error::TerraRustScriptError,
    network::{Chain, NetworkConfig},
};

#[derive(Clone, Debug)]
pub struct GroupConfig {
    pub network_config: NetworkConfig,
    pub name: String,
    pub file_path: String,
    pub proposal: bool,
}

impl GroupConfig {
    pub async fn new(
        name: String,
        network_config: NetworkConfig,
        file_path: String,
        proposal: bool,
    ) -> anyhow::Result<GroupConfig> {
        check_group_existance(&name, &file_path)?;
        Ok(GroupConfig {
            network_config,
            name,
            file_path,
            proposal,
        })
    }

    pub fn get_contract_address(
        &self,
        contract_name: &str,
    ) -> Result<String, TerraRustScriptError> {
        let file = File::open(&self.file_path)
            .expect(&format!("file should be present at {}", self.file_path));
        let json: serde_json::Value = from_reader(file)?;
        let maybe_address = json[self.name.clone()][contract_name].get("addr");
        match maybe_address {
            Some(addr) => Ok(addr.as_str().unwrap().into()),
            None => Err(TerraRustScriptError::AddrNotInFile(
                contract_name.to_owned(),
            )),
        }
    }

    pub fn get_contract_code_id(&self, contract_name: &str) -> Result<u64, TerraRustScriptError> {
        let file = File::open(&self.file_path)
            .expect(&format!("file should be present at {}", self.file_path));
        let json: serde_json::Value = from_reader(file).unwrap();
        let maybe_code_id = json[self.name.clone()][contract_name].get("code_id");
        match maybe_code_id {
            Some(code_id) => Ok(code_id.as_u64().unwrap()),
            None => Err(TerraRustScriptError::AddrNotInFile(
                contract_name.to_owned(),
            )),
        }
    }

    pub fn get_saved_state(&self) -> Map<String, Value> {
        let file = File::open(&self.file_path)
            .expect(&format!("file should be present at {}", self.file_path));
        let json: serde_json::Value = from_reader(file).unwrap();
        json.get(&self.name).unwrap().as_object().unwrap().clone()
    }
}

#[inline]
fn check_group_existance(name: &String, file_path: &String) -> anyhow::Result<()> {
    let file = File::open(file_path).expect(&format!("file should be present at {}", file_path));
    let mut cfg: serde_json::Value = from_reader(file).unwrap();
    let maybe_group = cfg.get(name);
    match maybe_group {
        Some(_) => Ok(()),
        None => {
            cfg[name] = json!({});
            serde_json::to_writer_pretty(File::create(file_path)?, &cfg)?;
            Ok(())
        }
    }
}