use sp_runtime::traits::Convert;
use sp_core::{RuntimeDebug, H160, U256};
use alloy_primitives::{Address, FixedBytes, U256 as SolU256};


pub struct U256ToAlloyU256Converter;

impl Convert<U256, SolU256> for U256ToAlloyU256Converter {
    fn convert(a: U256) -> SolU256 {
        let mut raw = [0u8; 32];
        a.to_big_endian(&mut raw);
        SolU256::from_be_bytes(raw)
    }
}

pub struct AlloyU256ToU256Converter;

impl Convert<SolU256, U256> for AlloyU256ToU256Converter {
    fn convert(a: SolU256) -> U256 {
        U256::from_little_endian(a.as_le_slice())
    }
}

pub struct H160ToAlloyAddressConverter;

impl Convert<H160, Address> for H160ToAlloyAddressConverter {
    fn convert(a: H160) -> Address {
        Address::new(a.0)
    }
}

pub struct AlloyAddressToH160Converter;

impl Convert<Address, H160> for AlloyAddressToH160Converter {
    fn convert(a: Address) -> H160 {
        H160::from_slice(&a.into_array())
    }
}
