use crate::state::{
    read_bid, read_bid_pool, read_bid_pools, read_bids_by_user, read_collateral_info, read_config,
    Bid, BidPool, Config,
};
use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Api, CanonicalAddr, Extern, HumanAddr, Querier, StdResult, Storage, Uint128};
use moneymarket::liquidation_queue::{
    BidPoolResponse, BidPoolsResponse, BidResponse, BidsResponse, ConfigResponse,
    LiquidationAmountResponse,
};
use moneymarket::querier::query_tax_rate;
use moneymarket::tokens::TokensHuman;

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let config = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&config.owner)?,
        oracle_contract: deps.api.human_address(&config.oracle_contract)?,
        stable_denom: config.stable_denom,
        safe_ratio: config.safe_ratio,
        bid_fee: config.bid_fee,
        liquidation_threshold: config.liquidation_threshold,
        price_timeframe: config.price_timeframe,
        waiting_period: config.waiting_period,
    };

    Ok(resp)
}

pub fn query_liquidation_amount<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    borrow_amount: Uint256,
    borrow_limit: Uint256,
    collaterals: TokensHuman,
    collateral_prices: Vec<Decimal256>,
) -> StdResult<LiquidationAmountResponse> {
    let config: Config = read_config(&deps.storage)?;

    // Safely collateralized check
    if borrow_amount <= borrow_limit {
        return Ok(LiquidationAmountResponse {
            collaterals: vec![],
        });
    }

    let tax_rate = query_tax_rate(&deps)?;
    let base_fee_deductor = (Decimal256::one() - config.bid_fee) * (Decimal256::one() - tax_rate);

    let mut collaterals_value = Uint256::zero();
    let mut expected_repay_amount = Uint256::zero();
    for c in collaterals.iter().zip(collateral_prices.iter()) {
        let (collateral, price) = c;
        let collateral_value = collateral.1 * *price;
        collaterals_value += collateral_value;

        let collateral_token_raw = deps.api.canonical_address(&collateral.0)?;
        let collateral_info = read_collateral_info(&deps.storage, &collateral_token_raw)?;

        let mut collateral_to_liquidate = collateral.1;
        for slot in 0..collateral_info.max_slot {
            let (slot_available_bids, premium_rate) =
                match read_bid_pool(&deps.storage, &collateral_token_raw, slot) {
                    Ok(bid_pool) => (bid_pool.total_bid_amount, bid_pool.premium_rate),
                    Err(_) => continue,
                };
            if slot_available_bids.is_zero() {
                continue;
            };

            let mut pool_repay_amount =
                collateral_to_liquidate * *price * (Decimal256::one() - premium_rate);

            if pool_repay_amount > slot_available_bids {
                pool_repay_amount = slot_available_bids;
                let pool_collateral_to_liquidate =
                    pool_repay_amount / ((Decimal256::one() - premium_rate) * *price);

                expected_repay_amount += pool_repay_amount;
                collateral_to_liquidate = collateral_to_liquidate - pool_collateral_to_liquidate;
            } else {
                expected_repay_amount += pool_repay_amount;
                break;
            }
        }
    }

    // expected_repay_amount must be bigger than borrow_amount
    // else force liquidate all collaterals
    let expected_repay_amount = expected_repay_amount * base_fee_deductor;
    if expected_repay_amount <= borrow_amount {
        return Ok(LiquidationAmountResponse { collaterals });
    }

    // When collaterals_value is smaller than liquidation_threshold,
    // liquidate all collaterals
    let safe_borrow_amount = borrow_limit * config.safe_ratio;
    let liquidation_ratio = if collaterals_value < config.liquidation_threshold {
        Decimal256::from_uint256(borrow_amount) / Decimal256::from_uint256(expected_repay_amount)
    } else {
        Decimal256::from_uint256(borrow_amount - safe_borrow_amount)
            / Decimal256::from_uint256(expected_repay_amount - safe_borrow_amount)
    };

    // Cap the liquidation_ratio to 1
    let liquidation_ratio = std::cmp::min(Decimal256::one(), liquidation_ratio);
    Ok(LiquidationAmountResponse {
        collaterals: collaterals
            .iter()
            .zip(collateral_prices.iter())
            .map(|c| {
                let (collateral, _) = c;
                let mut collateral = collateral.clone();

                collateral.1 = collateral.1 * liquidation_ratio;
                collateral
            })
            .filter(|c| c.1 > Uint256::zero())
            .collect::<TokensHuman>(),
    })
}

pub fn query_bid<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    bid_idx: Uint128,
) -> StdResult<BidResponse> {
    let bid: Bid = read_bid(&deps.storage, bid_idx)?;

    Ok(BidResponse {
        idx: bid.idx,
        collateral_token: deps.api.human_address(&bid.collateral_token)?,
        bidder: deps.api.human_address(&bid.bidder)?,
        amount: bid.amount,
        premium_slot: bid.premium_slot,
        pending_liquidated_collateral: bid.pending_liquidated_collateral,
        product_snapshot: bid.product_snapshot,
        sum_snapshot: bid.sum_snapshot,
        wait_end: bid.wait_end,
        epoch_snapshot: bid.epoch_snapshot,
        scale_snapshot: bid.scale_snapshot,
    })
}

pub fn query_bids_by_user<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    collateral_token: HumanAddr,
    bidder: HumanAddr,
    start_after: Option<Uint128>,
    limit: Option<u8>,
) -> StdResult<BidsResponse> {
    let collateral_token_raw = deps.api.canonical_address(&collateral_token)?;
    let bidder_raw = deps.api.canonical_address(&bidder)?;

    let bids: Vec<BidResponse> = read_bids_by_user(
        &deps.storage,
        &collateral_token_raw,
        &bidder_raw,
        start_after,
        limit,
    )?
    .iter()
    .map(|bid| {
        let res = BidResponse {
            idx: bid.idx,
            collateral_token: deps.api.human_address(&bid.collateral_token)?,
            bidder: deps.api.human_address(&bid.bidder)?,
            amount: bid.amount,
            premium_slot: bid.premium_slot,
            pending_liquidated_collateral: bid.pending_liquidated_collateral,
            product_snapshot: bid.product_snapshot,
            sum_snapshot: bid.sum_snapshot,
            wait_end: bid.wait_end,
            epoch_snapshot: bid.epoch_snapshot,
            scale_snapshot: bid.scale_snapshot,
        };
        Ok(res)
    })
    .collect::<StdResult<Vec<BidResponse>>>()?;

    Ok(BidsResponse { bids })
}

pub fn query_bid_pool<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    collateral_token: HumanAddr,
    bid_slot: u8,
) -> StdResult<BidPoolResponse> {
    let collateral_token_raw: CanonicalAddr = deps.api.canonical_address(&collateral_token)?;
    let bid_pool: BidPool = read_bid_pool(&deps.storage, &collateral_token_raw, bid_slot)?;

    Ok(BidPoolResponse {
        sum_snapshot: bid_pool.sum_snapshot,
        product_snapshot: bid_pool.product_snapshot,
        total_bid_amount: bid_pool.total_bid_amount,
        premium_rate: bid_pool.premium_rate,
        current_epoch: bid_pool.current_epoch,
        current_scale: bid_pool.current_scale,
    })
}

pub fn query_bid_pools<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    collateral_token: HumanAddr,
    start_after: Option<u8>,
    limit: Option<u8>,
) -> StdResult<BidPoolsResponse> {
    let collateral_token_raw = deps.api.canonical_address(&collateral_token)?;

    let bid_pools: Vec<BidPoolResponse> =
        read_bid_pools(&deps.storage, &collateral_token_raw, start_after, limit)?
            .iter()
            .map(|bid_pool| BidPoolResponse {
                sum_snapshot: bid_pool.sum_snapshot,
                product_snapshot: bid_pool.product_snapshot,
                total_bid_amount: bid_pool.total_bid_amount,
                premium_rate: bid_pool.premium_rate,
                current_epoch: bid_pool.current_epoch,
                current_scale: bid_pool.current_scale,
            })
            .collect();

    Ok(BidPoolsResponse { bid_pools })
}
