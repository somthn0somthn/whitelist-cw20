#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    entry_point, to_json_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response,
    StdResult, SubMsg, Uint128, WasmMsg,
};

use cw2::set_contract_version;
use cw20; // Add this for MinterResponse
use cw20_base; // Add this for InstantiateMsg

use crate::error::ContractError;
use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, CONFIG, WHITELIST}; // TODO: do i need to instantiate this in the instantiated fun

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:whitelist-cw20";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const INSTANTIATE_TOKEN_REPLY_ID: u64 = 1; //I think this effectively functions as an enum
                                               //pub const CW20_ID: u64 = 42; //TODO move this to a .env file

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin = deps.api.addr_validate(&msg.admin)?;

    //this calls a separate contract hence why you have to make
    //a separate InstantiateMsg call
    let cw20_msg = cw20_base::msg::InstantiateMsg {
        //TODO : pull these out into variables
        name: msg.token_name,
        symbol: msg.token_symbol,
        decimals: msg.token_decimals,
        initial_balances: vec![],
        mint: Some(cw20::MinterResponse {
            minter: env.contract.address.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    let instantiate_msg = WasmMsg::Instantiate {
        admin: Some(msg.admin.clone()), //TODO :: do I want admin priveliges here
        code_id: msg.token_code_id,
        msg: to_json_binary(&cw20_msg)?,
        funds: vec![],
        label: "factory token creation".to_owned(),
    };

    let instantiate_token_submsg =
        SubMsg::reply_on_success(instantiate_msg, INSTANTIATE_TOKEN_REPLY_ID);

    CONFIG.save(
        deps.storage,
        &Config {
            admin,
            token_contract: None,
        },
    )?;

    Ok(Response::new()
        .add_submessage(instantiate_token_submsg)
        .add_attribute("action", "instantiate")
        .add_attribute("admin", msg.admin))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        INSTANTIATE_TOKEN_REPLY_ID => handle_instantiate_token_reply(deps, msg),
        _ => Ok(Response::default()),
    }
}

fn handle_instantiate_token_reply(deps: DepsMut, msg: Reply) -> Result<Response, ContractError> {
    if let Some(res) = msg.result.into_result().ok() {
        let contract_address = res
            .events
            .iter()
            .find(|e| e.ty == "instantiate")
            .and_then(|e| {
                e.attributes
                    .iter()
                    .find(|attr| attr.key == "_contract_address")
            })
            .map(|attr| attr.value.clone())
            .ok_or_else(|| ContractError::NoContractAddress {})?;

        let validated_addr = deps.api.addr_validate(&contract_address)?;
        let mut config = CONFIG.load(deps.storage)?;

        config.token_contract = Some(validated_addr.clone());

        CONFIG.save(deps.storage, &config)?;

        return Ok(Response::new()
            .add_attribute("method", "handle_cw20_instantiate_reply")
            .add_attribute("cw20_contract_addr", contract_address));
    }

    Ok(Response::new().add_attribute("action", "handle_instantiate_token_reply"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddToWhiteList { address } => add_to_whitelist(deps, env, info, address),
        ExecuteMsg::RemoveFromWhiteList { address } => {
            remove_from_whitelist(deps, env, info, address)
        }
        ExecuteMsg::Mint { amount, recipient } => mint_tokens(deps, env, info, amount, recipient),
    }
}

fn add_to_whitelist(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let addr = deps.api.addr_validate(&address)?;
    WHITELIST.save(deps.storage, &addr, &())?;

    Ok(Response::new()
        .add_attribute("action", "add_to_whitelist")
        .add_attribute("whitelist_addr", addr))
}

fn remove_from_whitelist(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let addr = deps.api.addr_validate(&address)?;

    WHITELIST.remove(deps.storage, &addr);

    Ok(Response::new()
        .add_attribute("action", "remove_from_whitelist")
        .add_attribute("whitelist_addr", addr))
}

fn mint_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let token_addr = config
        .token_contract
        .ok_or(ContractError::NoContractAddress {})?;

    if info.sender != config.admin {
        let is_whitelisted = WHITELIST.may_load(deps.storage, &info.sender)?.is_some();
        if !is_whitelisted {
            return Err(ContractError::Unauthorized {});
        }
    }

    let final_recipient = match recipient {
        //TODO : validate address
        Some(addr) => deps.api.addr_validate(&addr)?,
        None => deps.api.addr_validate(&info.sender.to_string())?,
    };

    let cw20_mint_msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: final_recipient.to_string(),
        amount,
    };

    let wasm_msg = cosmwasm_std::WasmMsg::Execute {
        contract_addr: token_addr.to_string(),
        msg: to_json_binary(&cw20_mint_msg)?,
        funds: vec![],
    };

    Ok(Response::new()
        .add_message(cosmwasm_std::CosmosMsg::Wasm(wasm_msg))
        .add_attribute("action", "mint_tokens")
        .add_attribute("sender", info.sender.to_string())
        .add_attribute("final_recipient", final_recipient)
        .add_attribute("amount", amount))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => {
            let config = CONFIG.load(deps.storage)?;
            to_json_binary(&ConfigResponse {
                admin: config.admin.into_string(),
                token_contract: config.token_contract.map(|a| a.into_string()),
            })
        }
        QueryMsg::IsWhitelisted { address } => {
            let addr = deps.api.addr_validate(&address)?;
            let is_whitelisted = WHITELIST.may_load(deps.storage, &addr)?.is_some();
            to_json_binary(&is_whitelisted)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{Addr, Empty};
    use cw20_base::contract;
    use cw_multi_test::{App, AppBuilder, Contract, ContractWrapper, Executor, IntoAddr};
    use serde::de::value::MapAccessDeserializer;

    fn contract_whitelist_cw20() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(execute, instantiate, query).with_reply(reply);

        Box::new(contract)
    }

    fn contract_cw20_base() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            cw20_base::contract::execute,
            cw20_base::contract::instantiate,
            cw20_base::contract::query,
        );
        Box::new(contract)
    }

    fn setup_app() -> (App, Addr, Addr, u64) {
        let mut app = App::default();

        let cw20_code_id = app.store_code(contract_cw20_base());
        let factory_code_id = app.store_code(contract_whitelist_cw20());

        let admin = "the_admin".into_addr();

        let factory_init_msg = InstantiateMsg {
            admin: admin.clone().to_string(),
            token_name: "the token name".to_string(),
            token_symbol: "TKNSYBL".to_string(),
            token_decimals: 6,
            token_code_id: cw20_code_id, //TODO :: is this rigth
        };

        let factory_addr = app
            .instantiate_contract(
                factory_code_id,
                admin.clone(),
                &factory_init_msg,
                &[],
                "My Factory",
                None,
            )
            .unwrap();

        (app, admin, factory_addr, cw20_code_id)
    }

    #[test]
    fn test_factory_instantiates_cw20() {
        let (mut app, admin, factory_addr, _) = setup_app();
     
        let config_resp: ConfigResponse = app
            .wrap()
            .query_wasm_smart(&factory_addr, &QueryMsg::GetConfig {})
            .unwrap();

        assert_eq!(config_resp.admin, admin.clone().to_string());

        let cw20_addr = config_resp.token_contract.expect("No Contract address set");

        let token_info: cw20::TokenInfoResponse = app
            .wrap()
            .query_wasm_smart(&cw20_addr, &cw20::Cw20QueryMsg::TokenInfo {})
            .unwrap();
        //TODO :: pull these strings out into variables
        assert_eq!(token_info.name, "the token name");
        assert_eq!(token_info.symbol, "TKNSYBL");
        assert_eq!(token_info.decimals, 6);
    }

    #[test]
    fn test_add_and_remove_whitelist() {
        let (mut app, admin, factory_addr, _) = setup_app();

        let user = "user1".into_addr();

        let add_msg = ExecuteMsg::AddToWhiteList {
            address: user.to_string(),
        };
        app.execute_contract(admin.clone(), factory_addr.clone(), &add_msg, &[])
            .unwrap();

        let is_whitelisted: bool = app
            .wrap()
            .query_wasm_smart(
                &factory_addr,
                &QueryMsg::IsWhitelisted {
                    address: user.to_string(),
                },
            )
            .unwrap();
        assert!(is_whitelisted);

        let remove_msg = ExecuteMsg::RemoveFromWhiteList {
            address: user.to_string(),
        };
        app.execute_contract(admin.clone(), factory_addr.clone(), &remove_msg, &[])
            .unwrap();

        let is_whitelisted: bool = app
            .wrap()
            .query_wasm_smart(
                &factory_addr,
                &QueryMsg::IsWhitelisted {
                    address: user.to_string(),
                },
            )
            .unwrap();
        assert!(!is_whitelisted);
    }

    #[test]
    fn test_mint_tokens() {
        let (mut app, admin, factory_addr, _) = setup_app();

        let recipient = "recipient1".into_addr();

        let add_msg = ExecuteMsg::AddToWhiteList {
            address: recipient.to_string(),
        };
        app.execute_contract(admin.clone(), factory_addr.clone(), &add_msg, &[])
            .unwrap();

        let mint_msg = ExecuteMsg::Mint {
            amount: Uint128::new(1000),
            recipient: Some(recipient.to_string()),
        };
        app.execute_contract(admin.clone(), factory_addr.clone(), &mint_msg, &[])
            .unwrap();

        let config_resp: ConfigResponse = app
            .wrap()
            .query_wasm_smart(&factory_addr, &QueryMsg::GetConfig {})
            .unwrap();

        let cw20_addr = config_resp.token_contract.expect("No Contract address set");

        let balance: cw20::BalanceResponse = app
            .wrap()
            .query_wasm_smart(
                &cw20_addr,
                &cw20::Cw20QueryMsg::Balance {
                    address: recipient.to_string(),
                },
            )
            .unwrap();
        assert_eq!(balance.balance, Uint128::new(1000));

        let mint_msg2 = ExecuteMsg::Mint {
            amount: Uint128::new(234),
            recipient: None,
        };
        app.execute_contract(recipient.clone(), factory_addr.clone(), &mint_msg2, &[])
            .unwrap();

        let balance: cw20::BalanceResponse = app
            .wrap()
            .query_wasm_smart(
                &cw20_addr,
                &cw20::Cw20QueryMsg::Balance {
                    address: recipient.to_string(),
                },
            )
            .unwrap();
        assert_eq!(balance.balance, Uint128::new(1234));
    }
}

//CONT :: turn into to git
//CONT :: upload cw20 and this onto local xion instance
//CONT :: autogenerate medium article & tweet
//CONT :: user test, add XION features
