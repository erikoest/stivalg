use clap::arg;
use config::{*, ext::*};
use lazy_static::lazy_static;
use std::env;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub maps: String,
    pub headless: bool,
    pub params_fname: String,
    pub command: String,
}

lazy_static! {
    pub static ref CONFIG: Config = Config::new();
}

pub const CLAP_STYLING: clap::builder::styling::Styles =
    clap::builder::styling::Styles::styled()
    .header(clap_cargo::style::HEADER)
    .usage(clap_cargo::style::USAGE)
    .literal(clap_cargo::style::LITERAL)
    .placeholder(clap_cargo::style::PLACEHOLDER)
    .error(clap_cargo::style::ERROR)
    .valid(clap_cargo::style::VALID)
    .invalid(clap_cargo::style::INVALID);

impl Config {
    pub fn new() -> Self {
        // Parse command line
        let clap = clap::Command::new("stivalg")
            .bin_name("stivalg")
            .styles(CLAP_STYLING)
            .args([
                arg!(-p --params <FILE> "Read params from file"),
                arg!(-H --headless "Don't show map window"),
            ])
            .subcommand_required(false)
            .subcommand(clap::command!("compute"));

        let matches = clap.get_matches();
        let opt_params = matches.get_one::<String>("params");
        let mut headless = false;
        let mut params_fname = "";

        if let Some(params) = opt_params {
            params_fname = params;
        }

        match matches.get_one::<bool>("headless") {
            Some(h) => {
                headless = *h;
            },
            None => { },
        }

        let mut command = "";

        match matches.subcommand() {
            Some((cmd, _)) => {
                command = cmd;
                headless = true;
            },
            None => { },
        }

        // Create config with default settings
	let config = DefaultConfigurationBuilder::new()
            .add_in_memory(&[
	        ("maps", "/media/ekstern/hoydedata"),
                ("headless", &headless.to_string()),
                ("params_fname", params_fname),
                ("command", command),
            ])
            .build()
            .unwrap();

	config.reify()
    }

    pub fn map_dir(&self) -> String {
	let mut md = self.maps.clone();
	if !md.ends_with("/") {
	    md.push('/');
	}

	md
    }
}
