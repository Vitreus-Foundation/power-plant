// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Provides "fake" runtime api implementations
//!
//! These are used to provide a type that implements these runtime apis without requiring to import the native runtimes.

use crate::Block;
use sc_cli::RuntimeVersion;
use sp_core::OpaqueMetadata;
use sp_runtime::traits::Block as BlockT;
use sp_runtime::{ApplyExtrinsicResult, ExtrinsicInclusionMode};

sp_api::decl_runtime_apis! {
    /// This runtime api is only implemented for the test runtime!
    pub trait GetLastTimestamp {
        /// Returns the last timestamp of a runtime.
        fn get_last_timestamp() -> u64;
    }
}

struct Runtime;

sp_api::impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            unimplemented!()
        }

        fn execute_block(_: Block) {
            unimplemented!()
        }

        fn initialize_block(_: &<Block as BlockT>::Header) -> ExtrinsicInclusionMode {
            unimplemented!()
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            unimplemented!()
        }

        fn metadata_at_version(_: u32) -> Option<OpaqueMetadata> {
            unimplemented!()
        }

        fn metadata_versions() -> Vec<u32> {
            unimplemented!()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(_: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            unimplemented!()
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            unimplemented!()
        }

        fn inherent_extrinsics(_: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            unimplemented!()
        }

        fn check_inherents(
            _: Block,
            _: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            unimplemented!()
        }
    }
}
