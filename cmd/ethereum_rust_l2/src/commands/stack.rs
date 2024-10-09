use crate::{config::EthereumRustL2Config, utils::config::confirm};
use clap::Subcommand;
use eyre::ContextCompat;
use libsecp256k1::SecretKey;
use std::path::{Path, PathBuf};

pub const CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

#[derive(Subcommand)]
pub(crate) enum Command {
    #[clap(
        about = "Initializes the L2 network in the provided L1.",
        long_about = "Initializing an L2 involves deploying and setting up the contracts in the L1 and running an L2 node.",
        visible_alias = "i"
    )]
    Init {
        #[arg(
            long = "skip-l1-deployment",
            help = "Skips L1 deployment. Beware that this will only work if the L1 is already set up. L1 contracts must be present in the config."
        )]
        skip_l1_deployment: bool,
    },
    #[clap(about = "Shutdown the stack.")]
    Shutdown {
        #[clap(long, help = "Shuts down the local L1 node.", default_value_t = true)]
        l1: bool,
        #[clap(long, help = "Shuts down the L2 node.", default_value_t = true)]
        l2: bool,
    },
    #[clap(about = "Starts the stack.")]
    Start {
        #[clap(long, help = "Starts a local L1 node.", required = false)]
        l1: bool,
        #[clap(long, help = "Starts the L2 node.", required = false)]
        l2: bool,
    },
    #[clap(about = "Cleans up the stack. Prompts for confirmation.")]
    Purge,
    #[clap(
        about = "Re-initializes the stack. Prompts for confirmation.",
        long_about = "Re-initializing a stack means to shutdown, cleanup, and initialize the stack again. It uses the `shutdown` and `cleanup` commands under the hood."
    )]
    Restart,
}

impl Command {
    pub async fn run(self, cfg: EthereumRustL2Config) -> eyre::Result<()> {
        let root = std::path::Path::new(CARGO_MANIFEST_DIR)
            .parent()
            .map(std::path::Path::parent)
            .context("Failed to get parent")?
            .context("Failed to get grandparent")?;
        let l2_crate_path = root.join("crates/l2");
        let contracts_path = l2_crate_path.join("contracts");

        let l1_rpc_url = cfg.network.l1_rpc_url.clone();
        let l2_rpc_url = cfg.network.l2_rpc_url.clone();

        match self {
            Command::Init { skip_l1_deployment } => {
                // Delegate the command whether to init in a local environment
                // or in a testnet. If the L1 RPC URL is localhost, then it is
                // a local environment and the local node needs to be started.
                if l1_rpc_url.contains("localhost") {
                    start_l1(&l2_crate_path).await?;
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                if !skip_l1_deployment {
                    contract_deps(&contracts_path)?;
                    deploy_l1(&l1_rpc_url, &cfg.wallet.private_key, &contracts_path)?;
                }
                start_l2(root.to_path_buf(), &l2_rpc_url).await?;
            }
            Command::Shutdown { l1, l2 } => {
                if l1 && confirm("Are you sure you want to shutdown the local L1 node?")? {
                    shutdown_l1(&l2_crate_path)?;
                }
                if l2 && confirm("Are you sure you want to shutdown the L2 node?")? {
                    shutdown_l2()?;
                }
            }
            Command::Start { l1, l2 } => {
                if l1 {
                    start_l1(&l2_crate_path).await?;
                }
                if l2 {
                    start_l2(root.to_path_buf(), &l2_rpc_url).await?;
                }
            }
            Command::Purge => {
                if confirm("Are you sure you want to purge the stack?")? {
                    std::fs::remove_dir_all(l2_crate_path.join("volumes"))?;
                    std::fs::remove_dir_all(contracts_path.join("out"))?;
                    std::fs::remove_dir_all(contracts_path.join("lib"))?;
                    std::fs::remove_dir_all(contracts_path.join("cache"))?;
                } else {
                    println!("Aborted.");
                }
            }
            Command::Restart => {
                if confirm("Are you sure you want to restart the stack?")? {
                    Box::pin(async {
                        Self::Shutdown { l1: true, l2: true }.run(cfg.clone()).await
                    })
                    .await?;
                    Box::pin(async { Self::Purge.run(cfg.clone()).await }).await?;
                    Box::pin(async {
                        Self::Init {
                            skip_l1_deployment: false,
                        }
                        .run(cfg.clone())
                        .await
                    })
                    .await?;
                } else {
                    println!("Aborted.");
                }
            }
        }
        Ok(())
    }
}

fn contract_deps(contracts_path: &PathBuf) -> eyre::Result<()> {
    if !contracts_path.join("lib/forge-std").exists() {
        let cmd = std::process::Command::new("forge")
            .arg("install")
            .arg("foundry-rs/forge-std")
            .arg("--no-git")
            .arg("--root")
            .arg(contracts_path)
            .current_dir(contracts_path)
            .spawn()?
            .wait()?;
        if !cmd.success() {
            eyre::bail!("Failed to install forge-std");
        }
    }
    Ok(())
}

fn deploy_l1(
    l1_rpc_url: &str,
    deployer_private_key: &SecretKey,
    contracts_path: &PathBuf,
) -> eyre::Result<()> {
    // Run 'which solc' to get the path of the solc binary
    let solc_path_output = std::process::Command::new("which").arg("solc").output()?;

    let solc_path = String::from_utf8_lossy(&solc_path_output.stdout)
        .trim()
        .to_string();

    let cmd = std::process::Command::new("forge")
        .current_dir(contracts_path)
        .arg("script")
        .arg("script/DeployL1.s.sol:DeployL1Script")
        .arg("--rpc-url")
        .arg(l1_rpc_url)
        .arg("--private-key")
        .arg(hex::encode(deployer_private_key.serialize())) // TODO: In the future this must be the operator's private key.
        .arg("--broadcast")
        .arg("--use")
        .arg(solc_path)
        .spawn()?
        .wait()?;
    if !cmd.success() {
        eyre::bail!("Failed to run L1 deployer script");
    }
    Ok(())
}

fn shutdown_l1(l2_crate_path: &Path) -> eyre::Result<()> {
    let local_l1_docker_compose_path = l2_crate_path.join("docker-compose-l2.yml");
    let cmd = std::process::Command::new("docker")
        .arg("compose")
        .arg("-f")
        .arg(local_l1_docker_compose_path)
        .arg("down")
        .current_dir(l2_crate_path)
        .spawn()?
        .wait()?;
    if !cmd.success() {
        eyre::bail!("Failed to shutdown L1");
    }
    Ok(())
}

fn shutdown_l2() -> eyre::Result<()> {
    let cmd = std::process::Command::new("pkill")
        .arg("-f")
        .arg("ethereum_rust")
        .spawn()?
        .wait()?;
    if !cmd.success() {
        eyre::bail!("Failed to run local L1");
    }
    Ok(())
}

async fn start_l1(l2_crate_path: &Path) -> eyre::Result<()> {
    create_volumes(l2_crate_path)?;
    docker_compose_l2_up(l2_crate_path)?;
    Ok(())
}

fn create_volumes(l2_crate_path: &Path) -> eyre::Result<()> {
    let volumes_path = l2_crate_path.join("volumes/reth/data");
    std::fs::create_dir_all(volumes_path)?;
    Ok(())
}

fn docker_compose_l2_up(l2_crate_path: &Path) -> eyre::Result<()> {
    let local_l1_docker_compose_path = l2_crate_path.join("docker-compose-l2.yml");
    let cmd = std::process::Command::new("docker")
        .arg("compose")
        .arg("-f")
        .arg(local_l1_docker_compose_path)
        .arg("up")
        .arg("-d")
        .current_dir(l2_crate_path)
        .spawn()?
        .wait()?;
    if !cmd.success() {
        eyre::bail!("Failed to run local L1");
    }
    Ok(())
}

async fn start_l2(root: PathBuf, l2_rpc_url: &str) -> eyre::Result<()> {
    let l2_genesis_file_path = root.join("test_data/genesis-l2.json");
    let cmd = std::process::Command::new("cargo")
        .arg("run")
        .arg("--release")
        .arg("--bin")
        .arg("ethereum_rust")
        .arg("--features")
        .arg("l2")
        .arg("--")
        .arg("--network")
        .arg(l2_genesis_file_path)
        .arg("--http.port")
        .arg(l2_rpc_url.split(':').last().unwrap())
        .current_dir(root)
        .spawn()?
        .wait()?;
    if !cmd.success() {
        eyre::bail!("Failed to run L2 node");
    }
    Ok(())
}
