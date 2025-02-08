use crate::CallFee;
use frame_support::traits::Get;

/// Custom fee calculation for specified scenarios
pub trait CustomFee<RuntimeCall, DispatchInfo, Balance, ConstantFee>
where
    ConstantFee: Get<Balance>,
{
    fn dispatch_info_to_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: Option<&DispatchInfo>,
        calculated_fee: Option<Balance>,
    ) -> CallFee<Balance>;

    fn custom_fee() -> Balance;

    fn weight_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: Option<&DispatchInfo>,
        calculated_fee: Option<Balance>,
    ) -> Balance;

    fn ethereum_fee() -> Balance {
        Self::custom_fee()
    }
}
