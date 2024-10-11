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

//! Vitreus CLI library.

use clap::Parser;
use std::path::PathBuf;
use vitreus_service::eth::EthConfiguration;

#[allow(missing_docs)]
#[derive(Debug, Parser)]
pub enum Subcommand {
    /// Build a chain specification.
    BuildSpec(sc_cli::BuildSpecCmd),

    /// Validate blocks.
    CheckBlock(sc_cli::CheckBlockCmd),

    /// Export blocks.
    ExportBlocks(sc_cli::ExportBlocksCmd),

    /// Export the state of a given block into a chain spec.
    ExportState(sc_cli::ExportStateCmd),

    /// Import blocks.
    ImportBlocks(sc_cli::ImportBlocksCmd),

    /// Remove the whole chain.
    PurgeChain(sc_cli::PurgeChainCmd),

    /// Revert the chain to a previous state.
    Revert(sc_cli::RevertCmd),

    #[allow(missing_docs)]
    #[command(name = "prepare-worker", hide = true)]
    PvfPrepareWorker(ValidationWorkerCommand),

    #[allow(missing_docs)]
    #[command(name = "execute-worker", hide = true)]
    PvfExecuteWorker(ValidationWorkerCommand),

    /// Sub-commands concerned with benchmarking.
    #[cfg(feature = "runtime-benchmarks")]
    #[command(subcommand)]
    Benchmark(frame_benchmarking_cli::BenchmarkCmd),

    /// Sub-commands concerned with benchmarking.
    #[cfg(not(feature = "runtime-benchmarks"))]
    Benchmark,

    /// Runs performance checks such as PVF compilation in order to measure machine
    /// capabilities of running a validator.
    HostPerfCheck,

    /// Key management CLI utilities
    #[command(subcommand)]
    Key(sc_cli::KeySubcommand),

    /// Db meta columns information.
    ChainInfo(sc_cli::ChainInfoCmd),

    FrontierDb(fc_cli::FrontierDbCmd),
}

#[allow(missing_docs)]
#[derive(Debug, Parser)]
pub struct ValidationWorkerCommand {
    /// The path to the validation host's socket.
    #[arg(long)]
    pub socket_path: String,
    /// Calling node implementation version
    #[arg(long)]
    pub node_impl_version: String,
}

#[allow(missing_docs)]
#[derive(Debug, Parser)]
#[group(skip)]
pub struct RunCmd {
    #[clap(flatten)]
    pub base: sc_cli::RunCmd,

    /// Setup a GRANDPA scheduled voting pause.
    ///
    /// This parameter takes two values, namely a block number and a delay (in
    /// blocks). After the given block number is finalized the GRANDPA voter
    /// will temporarily stop voting for new blocks until the given delay has
    /// elapsed (i.e. until a block at height `pause_block + delay` is imported).
    #[arg(long = "grandpa-pause", num_args = 2)]
    pub grandpa_pause: Vec<u32>,

    /// Disable the BEEFY gadget.
    ///
    /// Currently enabled by default 'Vitreus'.
    #[arg(long)]
    pub no_beefy: bool,

    /// Allows a validator to run insecurely outside of Secure Validator Mode. Security features
    /// are still enabled on a best-effort basis, but missing features are no longer required. For
    /// more information see <https://github.com/w3f/polkadot-wiki/issues/4881>.
    #[arg(long = "insecure-validator-i-know-what-i-do", requires = "validator")]
    pub insecure_validator: bool,

    /// Enable the block authoring backoff that is triggered when finality is lagging.
    #[arg(long)]
    pub force_authoring_backoff: bool,

    /// Path to the directory where auxiliary worker binaries reside.
    ///
    /// TESTING ONLY: if the path points to an executable rather then directory,
    /// that executable is used both as preparation and execution worker.
    #[arg(long, value_name = "PATH")]
    pub workers_path: Option<PathBuf>,

    /// Add the destination address to the jaeger agent.
    ///
    /// Must be valid socket address, of format `IP:Port`
    /// commonly `127.0.0.1:6831`.
    #[arg(long)]
    pub jaeger_agent: Option<String>,

    /// Add the destination address to the `pyroscope` agent.
    ///
    /// Must be valid socket address, of format `IP:Port`
    /// commonly `127.0.0.1:4040`.
    #[arg(long)]
    pub pyroscope_server: Option<String>,

    /// Override the maximum number of pvf execute workers.
    ///
    ///  **Dangerous!** Do not touch unless explicitly advised to.
    #[arg(long)]
    pub execute_workers_max_num: Option<usize>,

    /// Override the maximum number of pvf workers that can be spawned in the pvf prepare
    /// pool for tasks with the priority below critical.
    ///
    ///  **Dangerous!** Do not touch unless explicitly advised to.
    #[arg(long)]
    pub prepare_workers_soft_max_num: Option<usize>,

    /// Override the absolute number of pvf workers that can be spawned in the pvf prepare pool.
    ///
    ///  **Dangerous!** Do not touch unless explicitly advised to.
    #[arg(long)]
    pub prepare_workers_hard_max_num: Option<usize>,

    /// Disable automatic hardware benchmarks.
    ///
    /// By default these benchmarks are automatically ran at startup and measure
    /// the CPU speed, the memory bandwidth and the disk speed.
    ///
    /// The results are then printed out in the logs, and also sent as part of
    /// telemetry, if telemetry is enabled.
    #[arg(long)]
    pub no_hardware_benchmarks: bool,

    /// Overseer message capacity override.
    ///
    /// **Dangerous!** Do not touch unless explicitly adviced to.
    #[arg(long)]
    pub overseer_channel_capacity_override: Option<usize>,

    /// TESTING ONLY: disable the version check between nodes and workers.
    #[arg(long, hide = true)]
    pub disable_worker_version_check: bool,
}

#[allow(missing_docs)]
#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Option<Subcommand>,

    #[clap(flatten)]
    pub run: RunCmd,

    #[clap(flatten)]
    pub storage_monitor: sc_storage_monitor::StorageMonitorParams,

    #[command(flatten)]
    pub eth: EthConfiguration,
}
