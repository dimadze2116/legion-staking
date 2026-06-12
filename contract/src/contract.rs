use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{LookupMap, UnorderedMap},
    json_types::U128,
    near, store::Vector,
    AccountId, Balance, Gas, Promise, PromiseOrValue, Timestamp,
};
use serde::{Deserialize, Serialize};

use crate::events::*;
use crate::rewards::*;

pub const STORAGE_COST: Balance = 1_000_000_000_000_000_000_000; // 0.001 NEAR per stake
pub const GAS_FOR_NFT_TRANSFER: Gas = Gas(10_000_000_000_000); // 10 TGas
pub const GAS_FOR_RESOLVE_TRANSFER: Gas = Gas(10_000_000_000_000); // 10 TGas
pub const GAS_FOR_NFT_ON_TRANSFER: Gas = Gas(25_000_000_000_000); // 25 TGas

/// Lock periods in seconds
pub const LOCK_10_DAYS: u64 = 864_000;   // 10 * 86400
pub const LOCK_20_DAYS: u64 = 1_728_000; // 20 * 86400
pub const LOCK_30_DAYS: u64 = 2_592_000; // 30 * 86400

/// Duration multipliers in basis points (10000 = 1x)
pub const MULT_10D: u32 = 10_000;
pub const MULT_20D: u32 = 15_000;
pub const MULT_30D: u32 = 20_000;

/// Tier bonus in basis points
pub const TIER_1_9: u32 = 0;       // 0%
pub const TIER_5_PLUS: u32 = 2_500; // 25%
pub const TIER_10_PLUS: u32 = 5_000; // 50%
pub const TIER_25_PLUS: u32 = 7_500; // 75%
pub const TIER_50_PLUS: u32 = 10_000; // 100%

pub type TokenId = String;
pub type EpochTimestamp = u64;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct Stake {
    pub owner_id: AccountId,
    pub token_id: TokenId,
    pub nft_contract_id: AccountId,
    pub staked_at: EpochTimestamp,
    pub lock_duration: u64,
    pub unlocked_at: EpochTimestamp,
    pub last_claim_epoch: EpochTimestamp,
    pub active: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct DurationMultiplier {
    pub duration_sec: u64,
    pub multiplier_bps: u32,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct TierBonus {
    pub min_nfts: u32,
    pub bonus_bps: u32,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct ContractMetadata {
    pub owner_id: AccountId,
    pub reward_token: AccountId,
    pub total_staked: u64,
    pub reward_pool: U128,
    pub epoch_duration: u64,
    pub duration_multipliers: Vec<DurationMultiplier>,
    pub tier_bonuses: Vec<TierBonus>,
}

#[near(contract_state)]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct StakingContract {
    pub owner_id: AccountId,
    pub reward_token: AccountId,
    pub stakes: UnorderedMap<TokenId, Stake>,
    pub user_stakes: LookupMap<AccountId, Vector<TokenId>>,
    pub total_staked: u64,
    pub reward_pool: Balance,
    pub epoch_duration: u64,
    pub last_epoch_update: EpochTimestamp,
    pub duration_multipliers: Vec<DurationMultiplier>,
    pub tier_bonuses: Vec<TierBonus>,
    pub epoch_rewards: LookupMap<AccountId, Balance>,
}

#[near]
impl StakingContract {
    #[init]
    pub fn new(owner_id: AccountId, reward_token: AccountId) -> Self {
        let mut this = Self {
            owner_id,
            reward_token,
            stakes: UnorderedMap::new(b"s"),
            user_stakes: LookupMap::new(b"u"),
            total_staked: 0,
            reward_pool: 0,
            epoch_duration: 86_400, // 1 day = 86400 seconds
            last_epoch_update: near_sdk::env::block_timestamp() / 1_000_000_000,
            duration_multipliers: vec![
                DurationMultiplier { duration_sec: LOCK_10_DAYS, multiplier_bps: MULT_10D },
                DurationMultiplier { duration_sec: LOCK_20_DAYS, multiplier_bps: MULT_20D },
                DurationMultiplier { duration_sec: LOCK_30_DAYS, multiplier_bps: MULT_30D },
            ],
            tier_bonuses: vec![
                TierBonus { min_nfts: 0, bonus_bps: TIER_1_9 },
                TierBonus { min_nfts: 5, bonus_bps: TIER_5_PLUS },
                TierBonus { min_nfts: 10, bonus_bps: TIER_10_PLUS },
                TierBonus { min_nfts: 25, bonus_bps: TIER_25_PLUS },
                TierBonus { min_nfts: 50, bonus_bps: TIER_50_PLUS },
            ],
            epoch_rewards: LookupMap::new(b"r"),
        };
        this
    }

    // --- Stake entry point ---
    /// Called by user to initiate staking via nft_transfer_call on the NFT contract.
    /// The NFT contract will call back `nft_on_transfer` on this contract.
    /// User attaches lock_duration_sec in msg parameter.
    pub fn stake(
        &mut self,
        nft_contract_id: AccountId,
        token_id: TokenId,
        lock_duration_sec: u64,
    ) -> Promise {
        let owner_id = near_sdk::env::predecessor_account_id();
        let msg = format!("{}:{}", lock_duration_sec, owner_id);

        // Validate lock duration
        self.validate_duration(lock_duration_sec);

        // Cross-call NFT contract to transfer token to this contract
        near_sdk::ext::nft_contract::ext(Into::into(nft_contract_id.clone()))
            .with_attached_deposit(1) // 1 yoctoNEAR for access key
            .with_static_gas(GAS_FOR_NFT_TRANSFER)
            .nft_transfer_call(
                Into::into(near_sdk::env::current_account_id()),
                token_id.clone(),
                msg,
                Option::<String>::None,
            )
    }

    // --- NFT on transfer callback ---
    #[private]
    pub fn nft_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_id: AccountId,
        token_id: TokenId,
        msg: String,
    ) -> PromiseOrValue<bool> {
        // Parse msg = "lock_duration:owner_id"
        let parts: Vec<&str> = msg.split(':').collect();
        let lock_duration: u64 = parts[0].parse().unwrap_or(0);
        let owner_id: AccountId = parts[1].parse()
            .unwrap_or_else(|_| near_sdk::env::panic_str("Invalid owner in msg"));

        // Validate the sender is the NFT contract
        let nft_contract_id = near_sdk::env::predecessor_account_id();

        // Validate duration
        self.validate_duration(lock_duration);

        let now = near_sdk::env::block_timestamp() / 1_000_000_000;
        let stake = Stake {
            owner_id: owner_id.clone(),
            token_id: token_id.clone(),
            nft_contract_id,
            staked_at: now,
            lock_duration,
            unlocked_at: now + lock_duration,
            last_claim_epoch: self.to_epoch(now),
            active: true,
        };

        // Store the stake
        near_sdk::assert!(
            self.stakes.insert(&token_id, &stake).is_none(),
            "Token already staked"
        );

        // Add to user stakes list
        let mut user_list = self.user_stakes.get(&owner_id).unwrap_or_else(|| {
            Vector::new(format!("uv:{}", owner_id).as_bytes())
        });
        user_list.push(&token_id);
        self.user_stakes.insert(&owner_id, &user_list);

        self.total_staked += 1;

        emit_stake(&owner_id, &token_id, lock_duration);
        PromiseOrValue::Value(true)
    }

    // --- Unstake ---
    pub fn unstake(&mut self, token_id: TokenId) -> Promise {
        let owner_id = near_sdk::env::predecessor_account_id();
        let stake = self.stakes.get(&token_id)
            .unwrap_or_else(|| near_sdk::env::panic_str("Stake not found"));

        near_sdk::assert_eq!(stake.owner_id, owner_id, "Not your stake");
        near_sdk::assert!(!stake.active, "Must claim rewards first");

        // Process rewards before unstaking
        self.process_rewards(&owner_id);

        // Remove from stakes
        self.stakes.remove(&token_id);

        // Remove from user list
        let mut user_list = self.user_stakes.get(&owner_id).unwrap();
        let updated: Vec<TokenId> = user_list.iter()
            .filter(|tid| tid != &token_id)
            .collect();
        let mut new_list = Vector::new(format!("uv:{}", owner_id).as_bytes());
        for tid in updated { new_list.push(&tid); }
        self.user_stakes.insert(&owner_id, &new_list);

        self.total_staked -= 1;

        // Return NFT to owner
        near_sdk::ext::nft_contract::ext(stake.nft_contract_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_RESOLVE_TRANSFER)
            .nft_transfer(owner_id.clone(), token_id.clone(), Option::<String>::None)
    }

    // --- Claim rewards ---
    pub fn claim(&mut self) -> Promise {
        let owner_id = near_sdk::env::predecessor_account_id();
        let amount = self.process_rewards(&owner_id);

        if amount == 0 {
            near_sdk::env::panic_str("No rewards to claim");
        }

        self.reward_pool -= amount;

        emit_claim(&owner_id, amount);

        // Transfer NEAR to user
        near_sdk::Promise::new(owner_id).transfer(amount)
    }

    // --- Validate lock duration ---
    fn validate_duration(&self, duration: u64) {
        let valid = self.duration_multipliers.iter()
            .any(|d| d.duration_sec == duration);
        near_sdk::assert!(valid, "Invalid lock duration");
    }

    // --- Convert timestamp to epoch ---
    pub fn to_epoch(&self, timestamp: EpochTimestamp) -> u64 {
        timestamp / self.epoch_duration
    }
}

// --- NFT trait extension for cross-contract calls ---
#[near(protocol = "ext")]
pub mod nft_contract {
    use near_sdk::PromiseOrValue;

    pub fn nft_transfer(
        receiver_id: AccountId,
        token_id: TokenId,
        memo: Option<String>,
    ) -> Promise;

    pub fn nft_transfer_call(
        receiver_id: AccountId,
        token_id: TokenId,
        msg: String,
        memo: Option<String>,
    ) -> PromiseOrValue<bool>;

    pub fn nft_resolve_transfer(
        authorized_id: AccountId,
        owner_id: AccountId,
        token_id: TokenId,
        approved_ids: Option<Vec<AccountId>>,
        memo: Option<String>,
    ) -> bool;
}