use crate::barrier::Barrier;

use hoydedata::Coord;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::Read;

fn default_grid_size_pass1() -> f32 { 25.0 }
fn default_grid_size_pass2() -> f32 { 1.0 }
fn default_covering_length() -> f32 { 1.1 }
fn default_covering_width() -> f32 { 1.1 }
fn default_path_width_pass2() -> f32 { 1000.0 }
fn default_track_name() -> String { "Stivalg".to_string() }

#[derive(Deserialize, Serialize)]
pub struct Params {
    pub points: Vec<Coord>,
    #[serde(default)]
    pub barriers: Vec<Barrier>,
    #[serde(default = "default_grid_size_pass1")]
    pub grid_size_pass1: f32,
    #[serde(default = "default_grid_size_pass2")]
    pub grid_size_pass2: f32,
    #[serde(default = "default_covering_length")]
    pub covering_length: f32,
    #[serde(default = "default_covering_width")]
    pub covering_width: f32,
    #[serde(default = "default_path_width_pass2")]
    pub path_width_pass2: f32,
    #[serde(default)]
    pub params_fname: String,
    #[serde(default)]
    pub output_fname: String,
    #[serde(default = "default_track_name")]
    pub track_name: String,
}

impl Params {
    pub fn from_config() -> Self {
        Self {
            points: vec![],
            barriers: vec![],
            grid_size_pass1: default_grid_size_pass1(),
            grid_size_pass2: default_grid_size_pass2(),
            covering_length: default_covering_length(),
            covering_width: default_covering_width(),
            path_width_pass2: default_path_width_pass2(),
            params_fname: "".to_string(),
            output_fname: "".to_string(),
            track_name: default_track_name(),
        }
    }

    pub fn from_file(fname: &str) -> Result<Params, String> {
        let mut data = "".to_string();
        let mut f = File::open(fname).expect("Unable to open file");
        f.read_to_string(&mut data).expect("Unable to read file");

        match serde_json::from_str::<Params>(&data) {
            Ok(params) => {
                Ok(params)
            },
            Err(e) => {
                Err(e.to_string())
            },
        }
    }

    pub fn write_params(&self, opt_fname: Option<&str>) -> Result<(), String> {
        let fname;

        if let Some(some_fname) = opt_fname {
            // fname must end with .json
            if !some_fname.ends_with(".json") {
                return Err("Filename must end with .json".to_string());
            }

            fname = some_fname;
        }
        else {
            if self.params_fname == "" {
                return Err("Missing filename.".to_string());
            }
            else {
                fname = &self.params_fname;
            }
        }

        let data = serde_json::to_string(&self).unwrap();
        fs::write(fname, data).expect("Unable to write file");

        Ok(())
    }

    pub fn print_params(&self) {
        if self.points.is_empty() {
            println!("No waypoints");
        }
        else {
            println!("Waypoints:");
            for p in &self.points {
                println!("  {}", p);
            }
        }
        if self.barriers.is_empty() {
            println!("No barriers");
        }
        else {
            println!("Barriers:");
            for b in &self.barriers {
                println!("  {}", b);
            }
        }

        println!("grid_size_pass1:  {}", self.grid_size_pass1);
        println!("grid_size_pass2:  {}", self.grid_size_pass2);
        println!("covering_length:  {}", self.covering_length);
        println!("covering_width:   {}", self.covering_width);
        println!("path_width_pass2: {}", self.path_width_pass2);
        println!("params_name:      {}", &self.params_fname);
        println!("output_fname:     {}", &self.output_fname);
        println!("track_name:       {}", &self.track_name);
    }

    fn parse_float(value: &str) -> Result<f32, String> {
        if let Ok(f) = value.parse() {
            Ok(f)
        }
        else {
            Err(format!("Invalid value '{}'", value))
        }
    }

    pub fn set(&mut self, param: &str, value: &str) -> Result<(), String> {
        match param {
            "grid_size_pass1" => {
                self.grid_size_pass1 = Params::parse_float(value)?;
            },
            "grid_size_pass2" => {
                self.grid_size_pass2 = Params::parse_float(value)?;
            },
            "covering_length" => {
                self.covering_length = Params::parse_float(value)?;
            },
            "covering_width" => {
                self.covering_width = Params::parse_float(value)?;
            },
            "path_width_pass2" => {
                self.path_width_pass2 = Params::parse_float(value)?;
            },
            /*
            "params_fname" => {
                self.params_fname = value.to_string()
            },
            "output_fname" => {
                self.output_fname = value.to_string()
            },
            */
            "track_name" => {
                self.track_name = value.to_string()
            },
            s => {
                return Err(format!("Invalid parameter '{}'", s));
            }
        }

        Ok(())
    }
}
