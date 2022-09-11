use std::{
    env,
    fmt::Debug,
    rc::Rc,
    str::{from_utf8, FromStr},
    time::Duration,
};

use cosmrs::{
    cosmwasm::{MsgExecuteContract, MsgInstantiateContract, MsgMigrateContract},
    AccountId,
};

use cosmwasm_std::{Addr, Coin, Empty};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::from_str;
use tokio::runtime::Runtime;

use crate::{
    contract::ContractCodeReference, cosmos_modules, data_structures::parse_cw_coins,
    error::CosmScriptError, sender::Wallet, state::ChainState, tx_handler::TxHandler,
    CosmTxResponse, DaemonState, NetworkKind,
};

#[derive(Clone)]
pub struct Daemon {
    pub sender: Wallet,
    pub state: Rc<DaemonState>,
    pub runtime: Rc<Runtime>,
}

impl Daemon {
    pub fn new(
        sender: &Wallet,
        state: &Rc<DaemonState>,
        runtime: &Rc<Runtime>,
    ) -> anyhow::Result<Self> {
        let instance = Self {
            sender: sender.clone(),
            state: state.clone(),
            runtime: runtime.clone(),
        };
        Ok(instance)
    }

    async fn wait(&self) {
        match self.state.kind {
            NetworkKind::Local => tokio::time::sleep(Duration::from_secs(6)).await,
            NetworkKind::Mainnet => tokio::time::sleep(Duration::from_secs(60)).await,
            NetworkKind::Testnet => tokio::time::sleep(Duration::from_secs(30)).await,
        }
    }
}

impl ChainState for Daemon {
    type Out = Rc<DaemonState>;

    fn state(&self) -> Self::Out {
        self.state.clone()
    }
}

// Execute on the real chain, returns tx response
impl TxHandler for Daemon {
    type Response = CosmTxResponse;
    fn execute<E: Serialize>(
        &self,
        exec_msg: &E,
        coins: &[cosmwasm_std::Coin],
        contract_address: &Addr,
    ) -> Result<Self::Response, CosmScriptError> {
        let exec_msg: MsgExecuteContract = MsgExecuteContract {
            sender: self.sender.pub_addr()?,
            contract: AccountId::from_str(contract_address.as_str())?,
            msg: serde_json::to_vec(&exec_msg)?,
            funds: parse_cw_coins(coins)?,
        };
        let result = self
            .runtime
            .block_on(self.sender.commit_tx(vec![exec_msg], None))?;
        Ok(result)
    }

    fn instantiate<I: Serialize + Debug>(
        &self,
        code_id: u64,
        init_msg: &I,
        label: Option<&str>,
        admin: Option<&Addr>,
        coins: &[Coin],
    ) -> Result<Self::Response, CosmScriptError> {
        let sender = &self.sender;

        let init_msg = MsgInstantiateContract {
            code_id,
            label: Some(label.unwrap_or("instantiate_contract").to_string()),
            admin: admin.map(|a| FromStr::from_str(a.as_str()).unwrap()),
            sender: sender.pub_addr()?,
            msg: serde_json::to_vec(&init_msg)?,
            funds: parse_cw_coins(coins)?,
        };

        let result = self
            .runtime
            .block_on(sender.commit_tx(vec![init_msg], None))?;
        // let address = &result.get_attribute_from_logs("instantiate", "_contract_address")[0].1;

        Ok(result)
    }

    fn query<Q: Serialize + Debug, T: Serialize + DeserializeOwned>(
        &self,
        query_msg: &Q,
        contract_address: &Addr,
    ) -> Result<T, CosmScriptError> {
        let sender = &self.sender;
        let mut client = cosmos_modules::cosmwasm::query_client::QueryClient::new(sender.channel());
        let resp = self.runtime.block_on(client.smart_contract_state(
            cosmos_modules::cosmwasm::QuerySmartContractStateRequest {
                address: contract_address.to_string(),
                query_data: serde_json::to_vec(&query_msg)?,
            },
        ))?;

        Ok(from_str(from_utf8(&resp.into_inner().data).unwrap())?)
    }

    fn migrate<M: Serialize + Debug>(
        &self,
        migrate_msg: &M,
        new_code_id: u64,
        contract_address: &Addr,
    ) -> Result<Self::Response, CosmScriptError> {
        let exec_msg: MsgMigrateContract = MsgMigrateContract {
            sender: self.sender.pub_addr()?,
            contract: AccountId::from_str(contract_address.as_str())?,
            msg: serde_json::to_vec(&migrate_msg)?,
            code_id: new_code_id,
        };
        let result = self
            .runtime
            .block_on(self.sender.commit_tx(vec![exec_msg], None))?;
        Ok(result)
    }

    fn upload(
        &self,
        contract_source: ContractCodeReference<Empty>,
    ) -> Result<Self::Response, CosmScriptError> {
        let sender = &self.sender;
        let path = match contract_source {
            ContractCodeReference::WasmCodePath(path) => path,
            ContractCodeReference::ContractEndpoints(_) => {
                return Err(CosmScriptError::StdErr(
                    "Blockchain deamon upload requires wasm file.".into(),
                ))
            }
        };

        let wasm_path = if path.contains(".wasm") {
            path.to_string()
        } else {
            format!("{}/{}.wasm", env::var("WASM_DIR").unwrap(), path)
        };

        log::debug!("{}", wasm_path);

        let file_contents = std::fs::read(wasm_path)?;
        let store_msg = cosmrs::cosmwasm::MsgStoreCode {
            sender: sender.pub_addr()?,
            wasm_byte_code: file_contents,
            instantiate_permission: None,
        };
        let result = self
            .runtime
            .block_on(sender.commit_tx(vec![store_msg], None))?;

        log::info!("uploaded: {:?}", result.txhash);

        // let code_id = result.get_attribute_from_logs("store_code", "code_id")[0]
        //     .1
        //     .parse::<u64>()?;
        // log::info!("code_id: {:?}", code_id);
        // self.save_code_id(code_id)?;

        // Extra time-out to ensure contract code propagation
        self.runtime.block_on(self.wait());
        Ok(result)
    }
}