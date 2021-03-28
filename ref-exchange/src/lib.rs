// TODOS implement oracle and updating next price, implement tokens


use std::convert::TryInto;

use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet, Vector};
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{assert_one_yocto, env, log, near_bindgen, AccountId, Balance, PanicOnDefault, Promise};

near_sdk::setup_alloc!();

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Contract {
    cached_bull_divisor: u128,
    cached_bear_divisor: u128,
    fee_basis_points: u128,
    max_app_fee_basis_points: u128,
    basis_points_divisor: u128,
    initial_rebase_divisor: u128,
    // funding_interval: u128,
    min_funding_divisor: u128,
    max_funding_divisor: u128,
    // bull_token: AccountId,
    // bear_token: AccountId,
    price_feed: AccountId,
    multiplier_basis_points: u128,
    max_profit_basis_points: u128,
    fee_reserve: u128,
    app_fee_basis_points: u128,
    app_fee_reserve: u128,
    funding_divisor: u128,
    // last_funding_time: u128,
    last_price: u128,
    // is_initialized: bool,
    app_fees: LookupMap<AccountId, u128>,

    // Maps tracking balances of bulls and bears
    bulls: LookupMap<AccountId, u128>,
    bears: LookupMap<AccountId, u128>,
    bull_total_supply: u128,
    bear_total_supply: u128

}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(price_feed: ValidAccountId, multiplier_basis_points: u128, 
    max_profit_basis_points: u128, funding_divisor: u128, app_fee_basis_points: u128, last_price: u128) -> Self {
        Self {
            price_feed: price_feed.as_ref().clone(),
            multiplier_basis_points,
            max_profit_basis_points,
            funding_divisor,
            app_fee_basis_points,
            last_price,
            bulls: LookupMap::new(b"b".to_vec()),
            bears: LookupMap::new(b"b".to_vec()),
            fee_basis_points: 20,
            max_app_fee_basis_points: 20,
            basis_points_divisor: 10000,
            initial_rebase_divisor: 10_u128.pow(10),
            min_funding_divisor: 500,
            max_funding_divisor: 1000000,
            app_fees: LookupMap::new(b"a".to_vec()),
            app_fee_reserve: 0,
            bear_total_supply: 1,
            bull_total_supply: 1,
            cached_bear_divisor: 1,
            cached_bull_divisor: 1,
            fee_reserve: 0,
        }
    }

    #[payable]
    pub fn rebase(&mut self) -> bool {
        let next_price = self.last_price + 100;
        // let intervals = self.last_funding_time + self.funding_interval;
        let divisors = self.get_divisors(self.last_price, next_price);
        self.last_price = next_price;
        self.cached_bull_divisor = divisors.0;
        self.cached_bear_divisor = divisors.1;
        return true;
    }

    #[payable]
    pub fn get_divisors(&mut self, last_price: u128, next_price: u128) -> (u128, u128) {
        let mut total_bulls = self.bull_total_supply / self.cached_bull_divisor;
        let mut total_bears = self.bear_total_supply / self.cached_bear_divisor;
        let ref_supply = if total_bulls < total_bears { total_bulls } else {total_bears};
        let delta = if next_price > last_price {next_price - last_price } else {last_price - next_price};
        let mut profit = (((ref_supply * delta)/last_price) * self.multiplier_basis_points) / self.basis_points_divisor;
        let max_profit = (ref_supply * self.max_profit_basis_points) / self.basis_points_divisor;

        if profit > max_profit {
            profit = max_profit;
        }

        total_bulls = if next_price > last_price {total_bulls + profit} else {total_bulls - profit};
        total_bears = if next_price > last_price {total_bears - profit} else {total_bears + profit};

        let bull_divisor = self.get_next_divisor(self.bull_total_supply, total_bulls, self.cached_bull_divisor);
        let bear_divisor = self.get_next_divisor(self.bear_total_supply, total_bears, self.cached_bear_divisor);
        return (bull_divisor, bear_divisor);
    }

    #[payable]
    pub fn get_next_divisor(&mut self, ref_supply: u128, next_supply: u128, fallback_divisor: u128) -> u128 {
        let divisor = (((ref_supply * 10)/next_supply)+9)/10;
        if divisor == 0 {
            return fallback_divisor;
        }
        return divisor;
    }

    #[payable]
    pub fn collect_fees(&mut self, amount: u128) -> u128 {
        let fee = (amount * self.fee_basis_points) / self.basis_points_divisor;
        self.fee_reserve += fee;
        return fee;
    }

    pub fn get_token_value(&self, is_bull: bool) -> u128 {
        let sender_id = env::predecessor_account_id();
        if is_bull {
            match self.bulls.get(&sender_id) {
                Some(value) => {
                    value
                },
                None => {
                    return 0;
                }
            }
        } else {
            match self.bears.get(&sender_id) {
                Some(value) => {
                    value
                },
                None => {
                    return 0;
                }
            }
        }

    }

    #[payable]
    pub fn buy(&mut self, is_bull: bool, amount: U128) -> u128 {
        let sender_id = env::predecessor_account_id();
        let amount: Balance = amount.into();

        rebase();

        let fee = self.collect_fees(amount);
        let mut token_amount = self.get_token_value(is_bull);
        // token_amount = token_amount;
        if token_amount > 0 {
            token_amount = if is_bull {token_amount/self.cached_bull_divisor} else {token_amount/self.cached_bear_divisor};
            token_amount += amount;
        } else {
            token_amount += amount;
        }
        if is_bull {
            self.bulls.insert(&sender_id, &token_amount);
        } else {
            self.bears.insert(&sender_id, &token_amount);
        }
        return token_amount;
    }

    #[payable]
    pub fn sell(&mut self, is_bull: bool, amount: u128) -> u128 {
        let sender_id = env::predecessor_account_id();

        rebase();

        let fee = self.collect_fees(amount);
        let mut token_amount = self.get_token_value(is_bull);
        token_amount -= amount;
        if is_bull {
            self.bulls.insert(&sender_id, &token_amount);
        } else {
            self.bears.insert(&sender_id, &token_amount);
        }
        Promise::new(sender_id).transfer(amount);
        return token_amount;
    }
}
