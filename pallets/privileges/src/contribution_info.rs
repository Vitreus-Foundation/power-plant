use core::default::Default;
use super::*;

#[derive(Clone, Encode, Decode, RuntimeDebug, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CurrentQuarter<T: Config> {
    pub current_year: u32,
    pub current_quarter: u8,
    pub year_first_block: BlockNumberFor<T>,
    pub end_block: BlockNumberFor<T>,
}

impl<T: Config> Default for CurrentQuarter<T>
{
    fn default() -> Self {
        CurrentQuarter {
            current_year: 2023,
            current_quarter: 3,
            year_first_block: 0_u32.into(),
            end_block: 15_u32.into(),
        }
    }
}

// let vip_pool: u32 = period.pool_balance.saturated_into();
//         let revenue: u32 = period.avenue.saturated_into();
//         let total_term: u32 = (period.quarter_info.end_block - period.quarter_info.year_first_block).saturated_into();
//         let final_reward: u32 = contribution_info.iter().fold(0, |acc, &item| {
//             // Formula: Revenue * [ (UserTokens / Wallet) * Participation (start/end)  ]
//
//             let participation: u32 = item.locked().saturated_into();
//             let start_participation: u32 = item.starting_block.saturated_into();
//
//             Self::deposit_event(Event::<T>::RewardClaimed {
//                 account,
//                 reward_info: final_reward.into()
//             });
//
//
//             (revenue as f32 * ((participation as f32 / vip_pool as f32 ) * ((total_term - start_participation) as f32 / total_term as f32 ))) as u32
//         });

#[derive(Clone, Encode, Decode, RuntimeDebug, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct QuarterResult<T: Config> {
    pub pool_balance: BalanceOf<T>,
    pub avenue: BalanceOf<T>,
    pub quarter_info: CurrentQuarter<T>,
}

impl<T: Config> QuarterResult<T> {
    pub fn new(
        quarter_info: CurrentQuarter<T>,
        pool_balance: BalanceOf<T>,
    ) -> QuarterResult<T> {
        QuarterResult {
            pool_balance,
            avenue: Default::default(),
            quarter_info,
        }
    }
}

#[derive(Copy, Clone, Encode, Decode, RuntimeDebug, Eq, PartialEq, TypeInfo)]
pub enum PenaltyType {
    Declining,
    Flat,
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct ContributionInfo<Balance, BlockNumber> {
    locked_amount: Balance,
    pub starting_block: BlockNumber,
    tax_type: PenaltyType,
}

impl<Balance, BlockNumber> ContributionInfo<Balance, BlockNumber>
{
    pub fn new(
        amount: Balance,
        starting_block: BlockNumber,
        tax_type: PenaltyType,
    ) -> ContributionInfo<Balance, BlockNumber>
    {
        ContributionInfo { locked_amount: amount, starting_block, tax_type }
    }

    pub fn locked(self) -> Balance {
        self.locked_amount
    }
}