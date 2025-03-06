//! The cli module defines the clap CLI interface for dotpatina.
//! For detailed usage info, run
//!
//!    ```
//!     dotpatina --help
//!    ```

use std::io::Write;
use std::path::PathBuf;

use crate::engine::{interface::PatinaInterface, PatinaEngine};
use clap::{Args, Parser, Subcommand};
use log::info;

/// [PatinaCli] renders files from templates and sets of variables as defined in patina toml files.
#[derive(Parser, Debug)]
#[clap(name = "dotpatina", version)]
pub struct PatinaCli {
    /// Global options apply to all subcommands
    #[clap(flatten)]
    global_options: GlobalOptions,

    /// The specified command to run
    #[clap(subcommand)]
    command: Command,
}

/// Options that apply globally to the CLI
#[derive(Debug, Args)]
struct GlobalOptions {
    /// The verbosity level of the CLI
    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity,
}

/// The available commands for the CLI
#[derive(Debug, Subcommand)]
enum Command {
    /// Render a patina to stdout
    #[clap(about = "Render a patina to stdout")]
    Render {
        /// Command line options
        #[clap(flatten)]
        options: PatinaCommandOptions,
    },

    /// Render and apply a patina
    #[clap(about = "Render and apply a patina")]
    Apply {
        /// Command line options
        #[clap(flatten)]
        options: PatinaCommandOptions,

        /// Don't ask for user input
        #[clap(long = "no-input")]
        no_input: bool,

        /// Don't keep a copy of previous files in the trash folder
        #[clap(long = "no-trash")]
        no_trash: bool,
    },
}

/// Options that apply to patina subcommands
#[derive(Debug, Args)]
struct PatinaCommandOptions {
    /// Included global options
    #[clap(flatten)]
    global_options: GlobalOptions,

    /// The file path to the patina toml file
    patina_path: PathBuf,

    /// Disable colors
    #[clap(long = "no-color")]
    no_color: bool,

    /// The list of tags to filter on
    #[clap(short = 't', long = "tags", help = "A set of tags to filter on")]
    tags: Vec<String>,

    /// A list of variables files
    #[clap(short = 'f', long = "vars", help = "A set of variables files")]
    variables_files: Vec<PathBuf>,
}

impl PatinaCli {
    /// Parse and return command line arguments
    pub fn parse_args() -> PatinaCli {
        PatinaCli::parse()
    }

    /// Run the CLI
    pub fn run(&mut self) {
        env_logger::Builder::new()
            .filter_level(self.global_options.verbosity.into())
            .init();

        let mut pi = CliPatinaInterface::new();
        let result = match &self.command {
            Command::Render { options } => options.engine(&pi).render_patina(),
            Command::Apply {
                options,
                no_input,
                no_trash,
            } => {
                pi.set_is_input_enabled(!*no_input);
                options.engine(&pi).apply_patina(!*no_trash)
            }
        };

        if let Err(e) = result {
            panic!("{:?}", e)
        }
    }
}

/// A struct for defining PatinaInterface behavior for the CLI.
struct CliPatinaInterface {
    /// Whether input is enabled. When `false`, the CLI will not prompt for user confirmation
    /// and the CLI will continue until the action has completed or failed.
    is_input_enabled: bool,
}

impl CliPatinaInterface {
    fn new() -> CliPatinaInterface {
        CliPatinaInterface {
            is_input_enabled: true,
        }
    }
}

impl PatinaInterface for CliPatinaInterface {
    fn output<S>(&self, s: S)
    where
        S: Into<String>,
    {
        print!("{}", s.into());
        let _ = std::io::stdout().flush();
    }

    fn set_is_input_enabled(&mut self, value: bool) {
        self.is_input_enabled = value
    }

    fn is_input_enabled(&self) -> bool {
        self.is_input_enabled
    }
}

impl PatinaCommandOptions {
    fn engine<'a, PI>(&self, pi: &'a PI) -> PatinaEngine<'a, PI>
    where
        PI: PatinaInterface,
    {
        let engine = PatinaEngine::new(
            pi,
            &self.patina_path,
            self.tags.clone(),
            self.variables_files.clone(),
        );
        info!(
            r#"New PatinaEngine
            path = {}
            tags = {:?}
        "#,
            self.patina_path.display(),
            self.tags
        );

        if self.no_color {
            colored::control::set_override(false);
        }

        engine
    }
}
