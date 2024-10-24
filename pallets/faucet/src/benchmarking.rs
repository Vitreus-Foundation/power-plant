//!
//! # Module Overview
//!
//! This Rust module provides benchmarking for the faucet pallet of a Substrate-based blockchain.
//! Benchmarking is used to measure the computational complexity of the faucet's core functions,
//! such as requesting funds, to determine the appropriate weights that should be applied to the
//! extrinsics. These weights help ensure that transactions are fairly priced based on the resources
//! they consume, thereby maintaining network stability.
//!
//! # Key Features and Functions
//!
//! - **Benchmarking Framework Integration**:
//!   - The module integrates with the `frame_benchmarking` framework to provide accurate performance
//!     metrics for the `request_funds` extrinsic. This involves simulating the `request_funds` function
//!     under controlled conditions and measuring the computational resources used.
//!
//! - **Extrinsic Call Benchmark**:
//!   - `request_funds()`: Benchmarks the process of requesting funds from the faucet. It involves
//!     creating a `whitelisted_caller` (a trusted account used in benchmarks) and calling the
//!     `request_funds` function to simulate how the extrinsic behaves in a real environment. After
//!     the call, an assertion checks whether the request was recorded in the `Requests` storage map.
//!
//! - **Test Suite Implementation**:
//!   - `impl_benchmark_test_suite!()`: Implements the benchmarking test suite for the faucet pallet.
//!     This macro ensures that the benchmarks are tested under the conditions defined in the mock
//!     environment (`new_test_ext`), which simulates the blockchain runtime for testing purposes.
//!
//! # Access Control and Security
//!
//! - **Controlled Test Environment**: The benchmarking module runs within a controlled environment
//!   created using `whitelisted_caller()`. This ensures that only authorized accounts are used during
//!   benchmarks, preventing unintended or unauthorized usage during the benchmarking process.
//! - **Resource Measurement for Fair Pricing**: Benchmarking measures the resource consumption of
//!   `request_funds` to determine its weight. This ensures that the transaction fee is proportional
//!   to the computational and storage resources it requires, helping to maintain fair pricing and
//!   prevent abuse of the faucet.
//!
//! # Developer Notes
//!
//! - **Feature Flag for Benchmarks**: The module is gated behind the `runtime-benchmarks` feature flag.
//!   This ensures that benchmarking code is only included when explicitly required, preventing it
//!   from being part of the production runtime and reducing the attack surface of the blockchain.
//! - **Benchmark Assertions**: The benchmark includes an assertion (`assert!(Requests::<T>::contains_key(&caller))`)
//!   to verify that the faucet request is properly recorded. This check ensures that the function
//!   behaves as expected and that the benchmark accurately reflects the normal execution path of the
//!   extrinsic.
//! - **Mock Environment**: The use of a mock environment (`crate::mock::new_test_ext()`) ensures that
//!   benchmarks are executed under controlled conditions that closely mimic the real runtime. This
//!   allows developers to gather reliable performance data that can be used to set accurate weights
//!   for the pallet's extrinsics.
//!
//! # Usage Scenarios
//!
//! - **Weight Calculation for Requesting Funds**: The primary use of this benchmarking module is to
//!   determine the weight for the `request_funds` extrinsic. By running the benchmark, developers can
//!   calculate the resources required to process a fund request, ensuring that users are charged
//!   appropriately for their transactions.
//! - **Ensuring Network Stability**: Accurate weight determination is crucial for preventing spam or
//!   abuse of the faucet. By benchmarking `request_funds` and setting an appropriate weight, the network
//!   can ensure that requesting funds is neither too cheap (leading to excessive usage) nor too expensive
//!   (discouraging legitimate use).
//!
//! # Integration Considerations
//!
//! - **Runtime Configuration**: Developers integrating this benchmarking module should ensure that
//!   the weights derived from the benchmarks are properly configured in the runtime. This involves
//!   updating the weight configuration based on the results of the benchmarking to reflect the current
//!   performance characteristics of the node hardware.
//! - **Benchmark Test Suite**: The `impl_benchmark_test_suite!()` macro facilitates the integration
//!   of benchmarking with the pallet's unit tests. This ensures that any changes to the `request_funds`
//!   extrinsic are accompanied by updated benchmarks, helping maintain consistency between the extrinsic
//!   logic and its associated weight.
//! - **Testing and Validation**: Since weights impact transaction fees and network performance, developers
//!   should validate the benchmarking results across different environments, including dev and staging
//!   networks, to ensure that the weights are accurate and effective under varied conditions.
//!
//! # Example Scenario
//!
//! Suppose developers need to determine the weight for the `request_funds` extrinsic to accurately
//! represent its resource consumption. They use this benchmarking module to run the `request_funds()`
//! benchmark, which simulates the transaction and measures the computational cost. The results are then
//! used to set the appropriate weight, ensuring that users requesting funds from the faucet are charged
//! fairly based on the resources their requests consume. This helps maintain network stability and
//! prevents abuse of the faucet's functionality in a test network environment.
//!

#![cfg(feature = "runtime-benchmarks")]
use super::*;

#[allow(unused)]
use crate::Pallet as Faucet;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn request_funds() {
        let amount = 100u32.into();
        let caller: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        request_funds(RawOrigin::Signed(caller.clone()), amount);

        assert!(Requests::<T>::contains_key(&caller));
    }

    impl_benchmark_test_suite!(Faucet, crate::mock::new_test_ext(), crate::mock::Test);
}
