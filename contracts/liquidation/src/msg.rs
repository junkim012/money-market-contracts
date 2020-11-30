use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr, Uint128};
use moneymarket::TokensHuman;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub owner: HumanAddr,
    /// borrow_amount / borrow_limit must always be bigger than  
    /// safe_ratio.
    pub safe_ratio: Decimal,
    /// Minimum liquidation amount.
    /// If a liquidation amount is below than this value,
    /// exclude the asset from the liquidation process
    pub min_liquidation: Uint128,
    /// Liquidation threshold amount in stable denom.
    /// When the current collaterals value is smaller than
    /// the threshold, all callterals will be liquidated
    pub liquidation_threshold: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    UpdateConfig {
        owner: Option<HumanAddr>,
        safe_ratio: Option<Decimal>,
        min_liquidation: Option<Uint128>,
        liquidation_threshold: Option<Uint128>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    LiquidationAmount {
        borrow_amount: Uint128,
        borrow_limit: Uint128,
        stable_denom: String,
        collaterals: TokensHuman,
        collateral_prices: Vec<Decimal>,
    },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: HumanAddr,
    pub safe_ratio: Decimal,
    pub min_liquidation: Uint128,
    pub liquidation_threshold: Uint128,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LiquidationAmountResponse {
    pub collaterals: TokensHuman,
}