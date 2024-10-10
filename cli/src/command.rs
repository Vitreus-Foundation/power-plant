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

use crate::cli::{Cli, Subcommand};
use fc_db::kv::frontier_database_dir;
use futures::future::TryFutureExt;
use polkadot_cli::NODE_VERSION;
use sc_cli::SubstrateCli;
use sc_service::DatabaseSource;
use service::{self, eth::db_config_dir, ChainSpec};
use std::net::ToSocketAddrs;

pub use crate::{error::Error, service::BlockId};
#[cfg(feature = "runtime-benchmarks")]
use chain_spec::devnet_keys::get_account_id_from_seed;
#[cfg(feature = "hostperfcheck")]
pub use polkadot_performance_test::PerfCheckError;
#[cfg(feature = "pyroscope")]
use pyroscope_pprofrs::{pprof_backend, PprofConfig};

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

type Result<T> = std::result::Result<T, Error>;

impl SubstrateCli for Cli {
    fn impl_name() -> String {
        "Vitreus Power Plant Node".into()
    }

    fn impl_version() -> String {
        env!("SUBSTRATE_CLI_IMPL_VERSION").into()
    }

    fn description() -> String {
        env!("CARGO_PKG_DESCRIPTION").into()
    }

    fn author() -> String {
        env!("CARGO_PKG_AUTHORS").into()
    }

    fn support_url() -> String {
        "support.anonymous.an".into()
    }

    fn copyright_start_year() -> i32 {
        2023
    }

    fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn ChainSpec>, String> {
        Ok(match id {
            "dev" => Box::new(chain_spec::development_config(None)),
            "devnet" => Box::new(chain_spec::devnet_config()),
            "stagenet" => Box::new(chain_spec::stagenet_config()),
            "" | "localnet" => Box::new(chain_spec::localnet_config()),
            "testnet" => Box::new(chain_spec::testnet_config()),
            "mainnet" => Box::new(chain_spec::mainnet_config()),
            path => {
                Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?)
            },
        })
    }
}

#[cfg(feature = "try-runtime")]
const DEV_ONLY_ERROR_PATTERN: &'static str =
	"can only use subcommand with --chain [polkadot-dev, kusama-dev, westend-dev, rococo-dev, wococo-dev], got ";

#[cfg(feature = "try-runtime")]
fn ensure_dev(spec: &Box<dyn service::ChainSpec>) -> std::result::Result<(), String> {
    if spec.is_dev() {
        Ok(())
    } else {
        Err(format!("{}{}", DEV_ONLY_ERROR_PATTERN, spec.id()))
    }
}

/// Runs performance checks.
/// Should only be used in release build since the check would take too much time otherwise.
fn host_perf_check() -> Result<()> {
    #[cfg(not(feature = "hostperfcheck"))]
    {
        Err(Error::FeatureNotEnabled { feature: "hostperfcheck" })
    }

    #[cfg(all(not(build_type = "release"), feature = "hostperfcheck"))]
    {
        return Err(PerfCheckError::WrongBuildType.into());
    }

    #[cfg(all(feature = "hostperfcheck", build_type = "release"))]
    {
        crate::host_perf_check::host_perf_check()?;
        return Ok(());
    }
}

/// Launch a node, accepting arguments just like a regular node,
/// accepts an alternative overseer generator, to adjust behavior
/// for integration tests as needed.
/// `malus_finality_delay` restrict finality votes of this node
/// to be at most `best_block - malus_finality_delay` height.
#[cfg(feature = "malus")]
pub fn run_node(
    run: Cli,
    overseer_gen: impl service::OverseerGen,
    malus_finality_delay: Option<u32>,
) -> Result<()> {
    run_node_inner(run, overseer_gen, malus_finality_delay, |_logger_builder, _config| {})
}

fn run_node_inner<F>(
    cli: Cli,
    overseer_gen: impl service::OverseerGen,
    maybe_malus_finality_delay: Option<u32>,
    logger_hook: F,
) -> Result<()>
where
    F: FnOnce(&mut sc_cli::LoggerBuilder, &sc_service::Configuration),
{
    let runner: sc_cli::Runner<Cli> = cli
        .create_runner_with_logger_hook::<sc_cli::RunCmd, _, F>(&cli.run.base, logger_hook)
        .map_err(Error::from)?;

    // By default, enable BEEFY on all networks, unless explicitly disabled through CLI.
    let enable_beefy = !cli.run.no_beefy;

    let _grandpa_pause = if cli.run.grandpa_pause.is_empty() {
        None
    } else {
        Some((cli.run.grandpa_pause[0], cli.run.grandpa_pause[1]))
    };

    let jaeger_agent = if let Some(ref jaeger_agent) = cli.run.jaeger_agent {
        Some(
            jaeger_agent
                .to_socket_addrs()
                .map_err(Error::AddressResolutionFailure)?
                .next()
                .ok_or_else(|| Error::AddressResolutionMissing)?,
        )
    } else {
        None
    };

    let node_version =
        if cli.run.disable_worker_version_check { None } else { Some(NODE_VERSION.to_string()) };

    let secure_validator_mode = cli.run.base.validator && !cli.run.insecure_validator;

    runner.run_node_until_exit(move |config| async move {
        let hwbench = (!cli.run.no_hardware_benchmarks)
            .then_some(config.database.path().map(|database_path| {
                let _ = std::fs::create_dir_all(database_path);
                sc_sysinfo::gather_hwbench(Some(database_path))
            }))
            .flatten();

        let database_source = config.database.clone();
        let task_manager = service::build_full(
            config,
            cli.eth,
            service::NewFullParams {
                is_parachain_node: service::IsParachainNode::No,
                enable_beefy,
                force_authoring_backoff: cli.run.force_authoring_backoff,
                jaeger_agent,
                telemetry_worker_handle: None,
                node_version,
                secure_validator_mode,
                workers_path: cli.run.workers_path,
                workers_names: None,
                overseer_gen,
                overseer_message_channel_capacity_override: cli
                    .run
                    .overseer_channel_capacity_override,
                malus_finality_delay: maybe_malus_finality_delay,
                hwbench,
                execute_workers_max_num: cli.run.execute_workers_max_num,
                prepare_workers_hard_max_num: cli.run.prepare_workers_hard_max_num,
                prepare_workers_soft_max_num: cli.run.prepare_workers_soft_max_num,
            },
        )?;

        if let Some(path) = database_source.path() {
            sc_storage_monitor::StorageMonitorService::try_spawn(
                cli.storage_monitor,
                path.to_path_buf(),
                &task_manager.spawn_essential_handle(),
            )?;
        }

        Ok(task_manager)
    })
}

/// Parses polkadot specific CLI arguments and run the service.
pub fn run() -> Result<()> {
    let cli: Cli = Cli::from_args();

    #[cfg(feature = "pyroscope")]
    let mut pyroscope_agent_maybe = if let Some(ref agent_addr) = cli.run.pyroscope_server {
        let address = agent_addr
            .to_socket_addrs()
            .map_err(Error::AddressResolutionFailure)?
            .next()
            .ok_or_else(|| Error::AddressResolutionMissing)?;
        // The pyroscope agent requires a `http://` prefix, so we just do that.
        let agent = pyro::PyroscopeAgent::builder(
            "http://".to_owned() + address.to_string().as_str(),
            "polkadot".to_owned(),
        )
        .backend(pprof_backend(PprofConfig::new().sample_rate(113)))
        .build()?;
        Some(agent.start()?)
    } else {
        None
    };

    #[cfg(not(feature = "pyroscope"))]
    if cli.run.pyroscope_server.is_some() {
        return Err(Error::PyroscopeNotCompiledIn);
    }

    match &cli.subcommand {
        None => run_node_inner(
            cli,
            polkadot_service::ValidatorOverseerGen,
            None,
            polkadot_node_metrics::logger_hook(),
        ),
        Some(Subcommand::BuildSpec(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            Ok(runner.sync_run(|config| cmd.run(config.chain_spec, config.network))?)
        },
        Some(Subcommand::CheckBlock(cmd)) => {
            let runner = cli.create_runner(cmd).map_err(Error::SubstrateCli)?;

            runner.async_run(|mut config| {
                let (client, _, import_queue, task_manager, _) =
                    service::new_chain_ops(&mut config, &cli.eth, None)?;
                Ok((cmd.run(client, import_queue).map_err(Error::SubstrateCli), task_manager))
            })
        },
        Some(Subcommand::ExportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;

            Ok(runner.async_run(|mut config| {
                let (client, _, _, task_manager, _) =
                    service::new_chain_ops(&mut config, &cli.eth, None)
                        .map_err(Error::PolkadotService)?;
                Ok((cmd.run(client, config.database).map_err(Error::SubstrateCli), task_manager))
            })?)
        },
        Some(Subcommand::ExportState(cmd)) => {
            let runner = cli.create_runner(cmd)?;

            Ok(runner.async_run(|mut config| {
                let (client, _, _, task_manager, _) =
                    service::new_chain_ops(&mut config, &cli.eth, None)?;
                Ok((cmd.run(client, config.chain_spec).map_err(Error::SubstrateCli), task_manager))
            })?)
        },
        Some(Subcommand::ImportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;

            Ok(runner.async_run(|mut config| {
                let (client, _, import_queue, task_manager, _) =
                    service::new_chain_ops(&mut config, &cli.eth, None)?;
                Ok((cmd.run(client, import_queue).map_err(Error::SubstrateCli), task_manager))
            })?)
        },
        Some(Subcommand::PurgeChain(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            Ok(runner.sync_run(|config| {
                // Remove Frontier offchain db
                let db_config_dir = db_config_dir(&config);
                match cli.eth.frontier_backend_type {
                    service::eth::BackendType::KeyValue => {
                        let frontier_database_config = match config.database {
                            DatabaseSource::RocksDb { .. } => DatabaseSource::RocksDb {
                                path: frontier_database_dir(&db_config_dir, "db"),
                                cache_size: 0,
                            },
                            DatabaseSource::ParityDb { .. } => DatabaseSource::ParityDb {
                                path: frontier_database_dir(&db_config_dir, "paritydb"),
                            },
                            _ => {
                                return Err(format!(
                                    "Cannot purge `{:?}` database",
                                    config.database
                                )
                                .into())
                            },
                        };
                        cmd.run(frontier_database_config)?;
                    },
                    service::eth::BackendType::Sql => {
                        let db_path = db_config_dir.join("sql");
                        match std::fs::remove_dir_all(&db_path) {
                            Ok(_) => {
                                println!("{:?} removed.", &db_path);
                            },
                            Err(ref err) if err.kind() == std::io::ErrorKind::NotFound => {
                                eprintln!("{:?} did not exist.", &db_path);
                            },
                            Err(err) => {
                                return Err(format!(
                                    "Cannot purge `{:?}` database: {:?}",
                                    db_path, err,
                                )
                                .into())
                            },
                        };
                    },
                };
                cmd.run(config.database)
            })?)
        },
        Some(Subcommand::Revert(cmd)) => {
            let runner = cli.create_runner(cmd)?;

            Ok(runner.async_run(|mut config| {
                let (client, backend, _, task_manager, _) =
                    service::new_chain_ops(&mut config, &cli.eth, None)?;
                let aux_revert = Box::new(|client, backend, blocks| {
                    service::revert_backend(client, backend, blocks, config).map_err(|err| {
                        match err {
                            service::Error::Blockchain(err) => err.into(),
                            // Generic application-specific error.
                            err => sc_cli::Error::Application(err.into()),
                        }
                    })
                });
                Ok((
                    cmd.run(client, backend, Some(aux_revert)).map_err(Error::SubstrateCli),
                    task_manager,
                ))
            })?)
        },
        // TODO: Do we need this subcommand?
        Some(Subcommand::PvfPrepareWorker(_cmd)) => {
            Ok(())
            // let mut builder = sc_cli::LoggerBuilder::new("");
            // builder.with_colors(false);
            // let _ = builder.init();
            //
            // #[cfg(target_os = "android")]
            // {
            //     return Err(sc_cli::Error::Input(
            //         "PVF preparation workers are not supported under this platform".into(),
            //     )
            //     .into());
            // }
            //
            // #[cfg(not(target_os = "android"))]
            // {
            //     polkadot_node_core_pvf_prepare_worker::worker_entrypoint(
            //         &cmd.socket_path,
            //         Some(&cmd.node_impl_version),
            //     );
            //     Ok(())
            // }
        },
        // TODO: Do we need this subcommand?
        Some(Subcommand::PvfExecuteWorker(_cmd)) => {
            Ok(())
            // let mut builder = sc_cli::LoggerBuilder::new("");
            // builder.with_colors(false);
            // let _ = builder.init();
            //
            // #[cfg(target_os = "android")]
            // {
            //     return Err(sc_cli::Error::Input(
            //         "PVF execution workers are not supported under this platform".into(),
            //     )
            //     .into());
            // }
            //
            // #[cfg(not(target_os = "android"))]
            // {
            //     polkadot_node_core_pvf_execute_worker::worker_entrypoint(
            //         &cmd.socket_path,
            //         Some(&cmd.node_impl_version),
            //     );
            //     Ok(())
            // }
        },
        #[cfg(feature = "runtime-benchmarks")]
        Some(Subcommand::Benchmark(cmd)) => {
            use frame_benchmarking_cli::{
                BenchmarkCmd, ExtrinsicFactory, SUBSTRATE_REFERENCE_HARDWARE,
            };
            use service::{
                benchmarking::{inherent_benchmark_data, RemarkBuilder, TransferKeepAliveBuilder},
                HeaderBackend,
            };
            use vitreus_power_plant_runtime::{Block, ExistentialDeposit};

            let runner = cli.create_runner(cmd)?;
            match cmd {
                BenchmarkCmd::Pallet(cmd) => runner
                    .sync_run(|config| cmd.run::<Block, ()>(config).map_err(Error::SubstrateCli)),
                BenchmarkCmd::Block(cmd) => runner.sync_run(|mut config| {
                    let (client, _, _, _, _) = service::new_chain_ops(&mut config, &cli.eth, None)?;
                    cmd.run(client).map_err(Error::SubstrateCli)
                }),
                BenchmarkCmd::Storage(cmd) => runner.sync_run(|mut config| {
                    let (client, backend, _, _, _) =
                        service::new_chain_ops(&mut config, &cli.eth, None)?;
                    let db = backend.expose_db();
                    let storage = backend.expose_storage();
                    cmd.run(config, client, db, storage).map_err(Error::SubstrateCli)
                }),
                BenchmarkCmd::Overhead(cmd) => runner.sync_run(|mut config| {
                    let (client, _, _, _, _) = service::new_chain_ops(&mut config, &cli.eth, None)?;
                    let ext_builder = RemarkBuilder::new(client.clone());
                    let header = client.header(client.info().genesis_hash).unwrap().unwrap();
                    let inherent_data = inherent_benchmark_data(header)
                        .map_err(|e| format!("generating inherent data: {:?}", e))?;
                    cmd.run(config, client, inherent_data, Vec::new(), &ext_builder)
                        .map_err(Error::SubstrateCli)
                }),
                BenchmarkCmd::Extrinsic(cmd) => runner.sync_run(|mut config| {
                    let (client, _, _, _, _) = service::new_chain_ops(&mut config, &cli.eth, None)?;
                    // Register the *Remark* and *TKA* builders.
                    let ext_factory = ExtrinsicFactory(vec![
                        Box::new(RemarkBuilder::new(client.clone())),
                        Box::new(TransferKeepAliveBuilder::new(
                            client.clone(),
                            get_account_id_from_seed::<sp_core::ecdsa::Public>("Alice"),
                            ExistentialDeposit::get(),
                        )),
                    ]);
                    let header = client.header(client.info().genesis_hash).unwrap().unwrap();
                    let inherent_data = inherent_benchmark_data(header)
                        .map_err(|e| format!("generating inherent data: {:?}", e))?;
                    cmd.run(client, inherent_data, Vec::new(), &ext_factory)
                        .map_err(Error::SubstrateCli)
                }),
                BenchmarkCmd::Machine(cmd) => runner.sync_run(|config| {
                    cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone())
                        .map_err(Error::SubstrateCli)
                }),
            }
        },
        #[cfg(not(feature = "runtime-benchmarks"))]
        Some(Subcommand::Benchmark) => Err(sc_cli::Error::Input(
            "Benchmarking wasn't enabled when building the node. \
			You can enable it with `--features runtime-benchmarks`."
                .into(),
        )
        .into()),
        // TODO: Do we need this subcommand?
        Some(Subcommand::HostPerfCheck) => {
            let mut builder = sc_cli::LoggerBuilder::new("");
            builder.with_colors(true);
            builder.init()?;

            host_perf_check()
        },
        Some(Subcommand::Key(cmd)) => Ok(cmd.run(&cli)?),
        #[cfg(feature = "try-runtime")]
        Some(Subcommand::TryRuntime(cmd)) => {
            use sc_service::TaskManager;
            use try_runtime_cli::block_building_info::timestamp_with_babe_info;

            let runner = cli.create_runner(cmd)?;
            let chain_spec = &runner.config().chain_spec;

            let registry = &runner.config().prometheus_config.as_ref().map(|cfg| &cfg.registry);
            let task_manager = TaskManager::new(runner.config().tokio_handle.clone(), *registry)
                .map_err(|e| Error::SubstrateService(sc_service::Error::Prometheus(e)))?;

            ensure_dev(chain_spec).map_err(Error::Other)?;

            #[cfg(feature = "kusama-native")]
            if chain_spec.is_kusama() {
                return runner.async_run(|_| {
                    Ok((
						cmd.run::<service::kusama_runtime::Block, sp_io::SubstrateHostFunctions, _>(
							Some(timestamp_with_babe_info(service::kusama_runtime_constants::time::MILLISECS_PER_BLOCK))
						)
						.map_err(Error::SubstrateCli),
						task_manager,
					))
                });
            }

            #[cfg(feature = "westend-native")]
            if chain_spec.is_westend() {
                return runner.async_run(|_| {
                    Ok((
						cmd.run::<service::westend_runtime::Block, sp_io::SubstrateHostFunctions, _>(
							Some(timestamp_with_babe_info(service::westend_runtime_constants::time::MILLISECS_PER_BLOCK))
						)
						.map_err(Error::SubstrateCli),
						task_manager,
					))
                });
            }
            // else we assume it is polkadot.
            #[cfg(feature = "polkadot-native")]
            {
                return runner.async_run(|_| {
                    Ok((
						cmd.run::<service::polkadot_runtime::Block, sp_io::SubstrateHostFunctions, _>(
							Some(timestamp_with_babe_info(service::polkadot_runtime_constants::time::MILLISECS_PER_BLOCK))
						)
						.map_err(Error::SubstrateCli),
						task_manager,
					))
                });
            }
            #[cfg(not(feature = "polkadot-native"))]
            panic!("No runtime feature (polkadot, kusama, westend, rococo) is enabled")
        },
        #[cfg(not(feature = "try-runtime"))]
        Some(Subcommand::TryRuntime) => Err(Error::Other(
            "TryRuntime wasn't enabled when building the node. \
				You can enable it with `--features try-runtime`."
                .into(),
        )),
        Some(Subcommand::ChainInfo(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            Ok(runner.sync_run(|config| cmd.run::<service::Block>(&config))?)
        },
        Some(Subcommand::FrontierDb(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            Ok(runner.sync_run(|mut config| {
                let (client, _, _, _, frontier_backend) =
                    service::new_chain_ops(&mut config, &cli.eth, None)
                        .map_err(Error::PolkadotService)?;
                let frontier_backend = match frontier_backend {
                    fc_db::Backend::KeyValue(kv) => kv,
                    _ => panic!("Only fc_db::Backend::KeyValue supported"),
                };
                cmd.run(client, frontier_backend).map_err(Error::SubstrateCli)
            })?)
        },
    }?;

    #[cfg(feature = "pyroscope")]
    if let Some(pyroscope_agent) = pyroscope_agent_maybe.take() {
        let agent = pyroscope_agent.stop()?;
        agent.shutdown();
    }
    Ok(())
}
