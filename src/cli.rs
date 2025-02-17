use std::io::Write;
use std::path::PathBuf;

use crate::engine::{interface::PatinaInterface, PatinaEngine};
use clap::{Args, Parser, Subcommand};

/// The patina CLI renders files from templates and sets of variables as defined in patina toml files.
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
    #[clap(short = 't', long = "A list of tags to filter on")]
    tags: Vec<String>,
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
        // let engine = PatinaEngine::new(&pi, patina_path, tags)
        let result = match &self.command {
            Command::Render { options } => options.engine(&pi).render_patina(),
            Command::Apply { options, no_input } => {
                pi.set_is_input_enabled(!*no_input);
                options.engine(&pi).apply_patina()
            }
        };

        if let Err(e) = result {
            panic!("{:?}", e)
        }
    }
}

struct CliPatinaInterface {
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
        PatinaEngine::new(pi, &self.patina_path, self.tags.clone())
    }
}
