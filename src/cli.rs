use std::path::PathBuf;

use crate::engine::{apply_patina_from_file, interface::PatinaOutput, render_patina_from_file};
use clap::{Args, Parser, Subcommand};

/// The patina CLI renders files from templates and sets of variables as defined in patina toml files.
#[derive(Parser, Debug)]
#[clap(name = "patina", version)]
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
}

impl PatinaCli {
    /// Parse and return command line arguments
    pub fn parse_args() -> PatinaCli {
        PatinaCli::parse()
    }

    /// Run the CLI
    pub fn run(&self) {
        env_logger::Builder::new()
            .filter_level(self.global_options.verbosity.into())
            .init();

        match &self.command {
            Command::Render { options } => self.render(options),
            Command::Apply { options } => self.apply(options),
        }
    }

    fn render(&self, options: &PatinaCommandOptions) {
        match render_patina_from_file(&options.patina_path, self) {
            Ok(patina_render) => patina_render,
            Err(e) => panic!("{:?}", e),
        };
    }

    fn apply(&self, options: &PatinaCommandOptions) {
        if let Err(e) = apply_patina_from_file(&options.patina_path, self) {
            panic!("{:?}", e);
        }
    }
}

impl PatinaOutput for PatinaCli {
    fn output(&self, s: &str) {
        print!("{}", s)
    }
}
