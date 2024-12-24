use crate::state::Config;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    pub token_name: String,
    pub token_symbol: String,
    pub token_decimals: u8,
    pub token_code_id: u64, //I'm not sure exactly how this works and how best to query this
                            //because it is the code Id of the deployed cw20 smart contract, I believe
}

#[cw_serde]
pub enum ExecuteMsg {
    AddToWhiteList {
        address: String,
    },
    RemoveFromWhiteList {
        address: String,
    },
    Mint {
        amount: Uint128,
        recipient: Option<String>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},
    #[returns(bool)]
    IsWhitelisted { address: String },
}

#[cw_serde]
pub struct ConfigResponse {
    //TODO : Do I wanna keep all these ADDRs typed as Strings?
    pub admin: String,
    pub token_contract: Option<String>,
}
