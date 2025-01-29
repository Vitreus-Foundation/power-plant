#![cfg_attr(not(feature = "std"), no_std)]

use parity_scale_codec::Decode;
use sp_runtime::Percent;

sp_api::decl_runtime_apis! {
    pub trait GovernanceApi<Balance>
    where
        Balance: Decode,
    {
        /// Returns the current electorate value.
        fn electorate() -> Balance;

        /// Calculates the minimum level of approval ratio required for a referendum to pass.
        fn threshold(referendum_index: u32) -> Option<Percent>;
    }
}
