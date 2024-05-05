//! A binary written as an in-place replacement for ofelia with a few different
//! configuration options and a lower memory footprint.
use std::process::exit;

use cfc::{context::ApplicationContext, utils::is_docker_env, loader::{load_labels, load_file}};
use clap::{ArgAction, Parser, Subcommand, Args};
use tokio::{task::JoinSet, time::{sleep, Duration}};
use tracing::{debug, error, info, instrument, trace, warn, Level};
use tracing_subscriber;

/// Arguments supported when running as a daemon
#[derive(Args, Debug)]
struct DaemonArgs {
    /// Whether the configuration should be obtained from docker labels or from a configuration file
    #[arg(short, long, help = "Extract configuration from docker labels", default_value = "false")]
    docker: bool,
    /// If the configuration is obtained from docker labels, the filter to use to find managed containers
    #[arg(short, long = "docker-filter", help = "Filter used to select valid docker containers")]
    filter: Option<String>,
    /// The path to the container manager's socket handle
    #[arg(long = "socket-path", help = "Configure the path to the docker socket")]
    socket_path: Option<String>,
    /// The target prefixes to use when looking for container jobs
    #[arg(long = "prefix", help = "The label prefix to use when looking for container jobs. May be provided more than once.")]
    label_prefixes: Vec<String>,
    /// When getting configuration from docker labels, how unsafe label configurations should be handled
    #[arg(long = "allow-unsafe-jobs", help = "Register potentially-unsafe jobs when parsing container labels", default_value = "false")]
    allow_unsafe: bool,
}

/// Arguments supported when running a configuration file validation check
#[derive(Args, Debug)]
struct ValidateArgs {}

/// The commands supported by the executable
#[derive(Subcommand, Debug)]
enum SubCommands {
    #[command(about="Run as a simple process")]
    Daemon(DaemonArgs),
    #[command(about="Validate the configuration files")]
    Validate(ValidateArgs)
}

/// The argument parser's output representation
#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct CliArgs {
    /// Command-specific parameters
    #[command(subcommand)]
    command: SubCommands,
    /// The path to the configuration file
    #[arg(short, long, help = "Path to the configuration file to use", global = true)]
    config: Option<String>,
    /// Whether to run in ofelia-compatibility mode.
    /// 
    /// This is equivalent to providing "--config /etc/ofelia.conf" in general,
    /// with "--allow-unsafe-jobs --prefix ofelia" with the daemon subcommand.
    /// 
    /// *Note that if --prefix or --config is used, the provided value will take precedence.*
    #[arg(long, help = "Run in ofelia compatibility mode.", global = true)]
    ofelia: bool,
    /// The verbosity level
    #[arg(short, help = "Increase verbosity", action = ArgAction::Count, global = true)]
    verbosity: u8,
}

impl CliArgs {
    pub fn get_context(&self) -> ApplicationContext {
        let mut global_context = ApplicationContext::default();

        global_context.config_path = self.config.as_ref()
            .and_then(|c| Some(c.clone()))
            .unwrap_or_else(|| {
                if self.ofelia {"/etc/ofelia.conf".to_string()}
                else {global_context.config_path}
            });
        match &self.command {
            SubCommands::Daemon(daemon_args) => {
                global_context.unsafe_labels = daemon_args.allow_unsafe;
                global_context.socket = daemon_args.socket_path.clone();
                if self.ofelia {
                    let ofelia_label = "ofelia".to_string();
                    if !global_context.label_prefixes.contains(&ofelia_label) {
                        global_context.label_prefixes.push(ofelia_label);
                    }
                    global_context.unsafe_labels = true;
                }
                for p in &daemon_args.label_prefixes {
                    if !global_context.label_prefixes.contains(p) {
                        global_context.label_prefixes.push(p.clone());
                    }
                }
                if global_context.label_prefixes.is_empty() {
                    global_context.label_prefixes.push("cfc".to_string());
                }
            },
            SubCommands::Validate(_) => {},
        }
        global_context
    }
}

#[tokio::main(flavor = "current_thread")]
#[instrument()]
async fn main() {
    let args = CliArgs::parse();
    tracing_subscriber::fmt()
        .with_max_level(
            match args.verbosity + 1 {
                //0 => Level::ERROR,
                1 => Level::WARN,
                2 => Level::INFO,
                3 => Level::DEBUG,
                _ => Level::TRACE,
            }
        ).init();
    debug!("{:?}", args);

    let global_context = args.get_context();

    match args.command {
        SubCommands::Daemon(daemon_args) => {
            // Add delay so docker has time to finish initializing container state
            if is_docker_env() {
                sleep(Duration::from_secs(1)).await;
            }
            let targets = if daemon_args.docker {
                load_labels(&global_context).await.unwrap()
            } else {
                load_file(&global_context.config_path, &global_context).await.unwrap()
            };
            trace!("Generated jobs list: {:?}", targets);
            if targets.is_empty() {
                error!("No valid job could be found, stopping with an error");
                exit(1);
            }

            let mut set = JoinSet::new();

            trace!("Registering all jobs for run");
            let base_handle = global_context.get_handle().unwrap();
            for target in targets {
                let handle = base_handle.clone();
                set.spawn(async move {target.start(handle).await});
            }

            trace!("Registering interrupt handler");

            info!("Start running all jobs");
            tokio::select! {
                interrupt = tokio::signal::ctrl_c() => {
                    interrupt.expect("Failed to listen for event");
                    warn!("Received shutdown signal, stopping all tasks before exiting");
                    set.shutdown().await;
                    exit(0);
                },
                r = set.join_next() => debug!("A job ended unexpectedly {:?}", r),
            }
            error!("Stopping. This should never happen");
        }
        SubCommands::Validate(_) => {
            match load_file(&global_context.config_path, &global_context).await {
                Ok(_) => {
                    info!["Successfully loaded configuration file"];
                },
                Err(e) => {
                    error!["Failed to load the configuration file: {}", e];
                    exit(1);
                },
            }
        },
    }
}
