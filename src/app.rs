use crate::barrier::Barrier;
use crate::channel::{AppMsg, CanvasMsg, AppReceiver, CanvasSender};
use crate::config::CONFIG;
use crate::params::Params;
use crate::path::Path;
use crate::path::Segment;

use cmdui::{CmdApp, CmdUI, CommandPart, KeywordExpander};
use crossbeam_channel::{RecvTimeoutError, unbounded};
use hoydedata::{Atlas, Coord, MsgReceiver, MsgSender};
use std::ops::Range;
use std::str::FromStr;
use std::time::Duration;

const COMMAND_LIST: &'static [&'static str] = &[
    "add point <coord> <pos>",
    "rm point <coord> <pos>",
    "update point [<coord>|map] <pos>",
    "add barrier <coord1> <coord2> ...",
    "rm barrier <pos>",
    "read params <filename>",
    "store params <filename>",
    "show params",
    "show cost",
    "show track info",
    "set <param> <value>",
    "open track <filename>",
    "store track <filename>",
    "compute",
    "flush maps",
    "help",
];

pub fn run_cmdui(app: &mut App) {
    let kw_exp = StiKeywordExpander::new();
    CmdUI::new(app, Some(&kw_exp)).read_commands();
}

// Thread for outputting hoydedata messages
fn hoydedata_output(mrx: MsgReceiver) {
    loop {
        match mrx.recv_timeout(Duration::from_secs(1)) {
            Ok(msg) => {
                println!("{}", msg);
            },
            Err(RecvTimeoutError::Disconnected) => {
                break;
            },
            Err(RecvTimeoutError::Timeout) => {
            },
        }
    }
}

pub struct StiKeywordExpander {
}

impl StiKeywordExpander {
    pub fn new() -> Self {
        Self {}
    }

    fn expand_param(&self) -> Vec<String> {
        return vec![
            "covering_length".to_string(),
            "covering_width".to_string(),
            "grid_size_pass1".to_string(),
            "grid_size_pass2".to_string(),
            "path_width_pass2".to_string(),
        ];
    }

    fn expand_coord(&self) -> Vec<String> {
        return vec!["from-map".to_string()];
    }
}

impl KeywordExpander for StiKeywordExpander {
    fn command_list<'a>(&self) -> &'a [&'a str] {
        return COMMAND_LIST;
    }

    fn expand_keyword(&self, cp: &CommandPart, parts: &Vec<String>)
                      -> Vec<String> {
        let lpart = &parts[parts.len() - 1];

        match cp.as_str() {
            "<filename>"  => { self.expand_filename(lpart) },
            "<coord>"     => { self.expand_coord() },
            "<param>"     => { self.expand_param() },
            s             => { vec![s.to_string()] },
        }
    }
}

// 'neighbourhood' distance to objects when selecting them on map
const NEARBY: f32 = 20.0;

pub struct App {
    atlas: Atlas,
    opt_path: Option<Path>,
    path_stored: bool,
    params: Params,
    params_stored: bool,
    opt_tx: Option<CanvasSender>,
    opt_rx: Option<AppReceiver>,
}

impl App {
    pub fn new(opt_tx: Option<CanvasSender>, opt_rx: Option<AppReceiver>)
               -> Result<Self, String> {
        let (mtx, mrx): (MsgSender, MsgReceiver) = unbounded();

        std::thread::spawn(move || hoydedata_output(mrx));

        let params = if CONFIG.params_fname == "" {
            Params::from_config()
        }
        else {
            Params::from_file(&CONFIG.params_fname)?
        };

        // Send initial viewpoint data to the map window (this should be done
        // before creating the Atlas because the latter takes some time).
        if let Some(tx) = &opt_tx {
            let _ = tx.send(CanvasMsg::SetCoveringArea(
                params.covering_length, params.covering_width));
            let _ = tx.send(CanvasMsg::SetWaypoints(
                params.points.clone()));
            let _ = tx.send(CanvasMsg::SetBarriers(
                params.barriers.clone()));
            let _ = tx.send(CanvasMsg::ResetView);
        }

        Ok(Self {
            atlas: Atlas::new(1.0, Some(mtx)).unwrap(),
            opt_path: None,
            path_stored: false,
            params: params,
            params_stored: true,
            opt_tx: opt_tx,
            opt_rx: opt_rx,
        })
    }

    pub fn compute(&mut self) -> Result<(), String> {
        if self.params.points.len() < 2 {
            return Err("Not enough waypoints".to_string());
        }

        if let Some(p) =  Path::from_points(&self.params, &self.atlas) {
            p.print_summary(&self.atlas);
            self.opt_path.replace(p.clone());
            self.path_stored = false;

            if let Some(tx) = &self.opt_tx {
                let _ = tx.send(CanvasMsg::SetPath(p));
            }
        }
        else {
            println!("Path {} cannot be walked", self.params.points.iter()
                     .map(|c| c.to_string())
                     .collect::<Vec<String>>()
                     .join(", "));
        }

        Ok(())
    }

    pub fn parse_int_range(intstr: &str, range: Range<usize>)
                           -> Result<usize, String> {
        if let Ok(length) = intstr.parse() {
            if range.contains(&length) {
                return Ok(length);
            }
            else {
                if range.len() == 1 {
                    return Err(format!("Expected number {}", range.start));
                }
                else {
                    return Err(format!("Expected number in in range {}..{}",
                                       range.start, range.end - 1));
                }
            }
        }
        else {
            return Err(format!("Expected number, got '{}'", intstr));
        }
    }

    fn select_point_on_map(&self) -> Result<usize, String> {
        let pm = self.get_coord_from_map("Select a waypoint on map")?;

        for (i, p) in self.params.points.iter().enumerate() {
            if (pm - *p).abs_sq() < NEARBY*NEARBY {
                return Ok(i);
            }
        }

        return Err("No points selected".to_string());
    }

    fn select_barrier_on_map(&self) -> Result<usize, String> {
        let pm = self.get_coord_from_map("Select a barrier on map")?;

        for (i, b) in self.params.barriers.iter().enumerate() {
            if b.distance_sq(&pm) < NEARBY*NEARBY {
                return Ok(i);
            }
        }

        return Err("No barriers selected".to_string());
    }

    // Add waypoint
    fn add_point(&mut self, args: &Vec<String>) -> Result<(), String> {
        let mut n = self.params.points.len();
        let c;

        if args.len() == 2 {
            // Two arguments (coord, int): add point to position
            c = self.parse_coord(&args[0])?;
            n = App::parse_int_range(&args[1], 1..n + 2)?;
        }
        else  if args.len() == 1 {
            if let Ok(i) = App::parse_int_range(&args[0], 1..n + 2) {
                // One argument (int): get point from map, add to position
                c = self.get_coord_from_map("Select a point on map")?;
                n = i - 1;
            }
            else {
                // One argument (coord): add point to last position
                c = self.parse_coord(&args[0])?;
            }
        }
        else if args.len() == 0 {
            // No arguments: get point from map, add to last position
            c = self.get_coord_from_map("Select a point on map")?;
        }
        else {
            return Err("Too many arguments".to_string());
        }

        self.params.points.insert(n, c);
        self.update_waypoints();
        Ok(())
    }

    // Update existing waypoint
    fn update_point(&mut self, args: &Vec<String>) -> Result<(), String> {
        let mut n = self.params.points.len() - 1;
        let c;

        if n == 0 {
            return Err(format!("No points defined"));
        }

        if args.len() == 2 {
            // Two arguments (coord, int): update point at position
            c = self.parse_coord(&args[0])?;
            n = App::parse_int_range(&args[1], 1..n + 1)?;
        }
        else if args.len() == 1 {
            if let Ok(i) = App::parse_int_range(&args[0], 1..n + 1) {
                // One argument (int): get point from map, update position
                c = self.get_coord_from_map("Select a new position on map")?;
                n = i - 1;
            }
            else {
                // One argument (coord): update point at last position
                c = self.parse_coord(&args[0])?;
            }
        }
        else if args.len() == 0 {
            // No arguments: select point to update, then get new from map
            n = self.select_point_on_map()?;
            c = self.get_coord_from_map("Select a new position on map")?;
        }
        else {
            return Err("Expected one or two arguments".to_string());
        }

        self.params.points[n] = c;
        self.update_waypoints();
        Ok(())
    }

    fn rm_point(&mut self, args: &Vec<String>) -> Result<(), String> {
        let len = self.params.points.len();
        let n;

        if len == 0 {
            return Err(format!("No points defined"));
        }

        if args.len() == 1 {
            // One argument (int): remove point at position
            n = App::parse_int_range(&args[0], 1..len)? - 1;
        }
        else if args.len() == 0 {
            // No arguments: select point on map
            n = self.select_point_on_map()?;
        }
        else {
            return Err("Too many arguments".to_string());
        }

        self.params.points.remove(n);
        self.update_waypoints();
        Ok(())
    }

    fn add_barrier(&mut self, args: &Vec<String>) -> Result<(), String> {
        let mut added_barrier;

        if args.len() == 0 {
            // No arguments. Select points on map.
            if let Some(rx) = &self.opt_rx {
                println!("Left click on first and intermediate points. Right click to finish.");

                if let Some(tx) = &self.opt_tx {
                    let _ = tx.send(CanvasMsg::RequestBarrier);
                }

                loop {
                    match rx.recv() {
                        Ok(AppMsg::CreateBarrier(b)) => {
                           if b.len() >= 2 {
                                added_barrier = b;
                            }
                            else {
                                added_barrier = Barrier::new();
                            }
                            break;
                        },
                        _ => { },
                    }
                }
            }
            else {
                return Err(format!("No map window."));
            }
        }
        else {
            added_barrier = Barrier::new();

            for cstr in args {
                added_barrier.add_point(Coord::from_str(cstr)?);
            }
        }

        if added_barrier.points.len() >= 2 {
            self.params.barriers.push(added_barrier);
            self.update_barriers();
        }

        Ok(())
    }

    fn rm_barrier(&mut self, args: &Vec<String>) -> Result<(), String> {
        let mut n = self.params.barriers.len();

        if n == 0 {
            return Err("No barriers defined.".to_string());
        }

        if args.len() == 1 {
            // One argument (int): remove barrier at position
            n = App::parse_int_range(&args[0], 1..n + 1)? - 1;
        }
        else if args.len() == 0 {
            n = self.select_barrier_on_map()?;
        }
        else {
            return Err("Too many arguments".to_string());
        }

        self.params.barriers.remove(n);
        self.update_barriers();
        Ok(())
    }

    fn show_params(&self) {
        self.params.print_params();
    }

    fn show_cost(&self) {
        println!("Slope (deg)      Distance/hour (km)      Elevation/hour (m)");

        for i in 0..21 {
            // slope in degrees
            let r = (i as f32)*5.0 - 50.0;
            // slope as the ratio h/d
            let s = (r*std::f32::consts::PI/180.0).tan();
            // time cost
            let c = Segment::time_by_steepness(s, s.abs());
            // horizontal distance per time, km/h
            let dpt = 3.6/c;
            // elevation per time, m/h;
            let ept = 3600.0*s/c;
            println!("{:6.2}          {:6.2}                  {:8.2}",
                     r, dpt, ept);
        }
    }

    fn show_path_info(&self) {
        if let Some(path) = &self.opt_path {
            path.print_summary(&self.atlas);
        }
        else {
            println!("No track");
        }
    }

    fn set_param(&mut self, param: &str, value: &str) -> Result<(), String> {
        let ret = self.params.set(param, value);
        if param == "covering_length" || param == "covering_width" {
            if let Some(tx) = &self.opt_tx {
                let _ = tx.send(CanvasMsg::SetCoveringArea(
                    self.params.covering_length,
                    self.params.covering_width,
                ));
            }
        }
        return ret;
    }

    fn read_params(&mut self, fname: &str) -> Result<(), String> {
        self.params = Params::from_file(fname)?;
        self.params_stored = true;
        self.reset_view();

        Ok(())
    }

    fn store_params(&mut self, opt_fname: Option<&str>) -> Result<(), String> {
        let res = self.params.write_params(opt_fname);
        if let Ok(()) = res {
            self.params_stored = true;
        }

        return res;
    }

    fn read_path(&mut self, opt_fname: Option<&str>) {
        let fname = opt_fname.unwrap_or(&self.params.output_fname);

        let p = Path::read_gpx(fname);
        self.opt_path.replace(p.clone());
        self.path_stored = true;

        if let Some(tx) = &self.opt_tx {
            let _ = tx.send(CanvasMsg::SetPath(p));
        }
    }

    fn store_path(&mut self, opt_fname: Option<&str>) {
        if let Some(path) = &self.opt_path {
            let fname;

            if let Some(some_fname) = opt_fname {
                fname = some_fname;
                self.params.output_fname = fname.to_string();
                self.params_stored = false;
            }
            else {
                fname = &self.params.output_fname;
            }

            path.write_gpx(fname, &self.params.track_name, &self.atlas);
            self.path_stored = true;
        }
        else {
            println!("No track");
        }
    }

    fn help(&self) {
        println!("{}", COMMAND_LIST.into_iter()
                 .map(|c| c.replace("<bool>", "on/off"))
                 .collect::<Vec<String>>()
                 .join("\n")
        );
    }

    fn expects_num_arguments(parts: &Vec<String>, n: usize)
                             -> Result<(), String> {
        if parts.len() < n {
            return Err(format!("Expected {} arguments", n));
        }
        else {
            return Ok(());
        }
    }

    fn get_coord_from_map(&self, msg: &str) -> Result<Coord, String> {
        if let Some(rx) = &self.opt_rx {
            // request point from canvas
            println!("{}", msg);

            if let Some(tx) = &self.opt_tx {
                let _ = tx.send(CanvasMsg::RequestPoint);
            }

            // Wait for selected point from canvas
            loop {
                match rx.recv() {
                    Ok(AppMsg::SelectPoint(c)) => {
                        return Ok(c);
                    },
                    _ => { },
                }
            }
        }
        else {
            return Err(format!("No map window."));
        }
    }

    fn parse_coord(&self, coordstr: &str) -> Result<Coord, String> {
        if let Ok(coord) = coordstr.parse() {
            return Ok(coord);
        }
        else {
            return Err(format!("Expected coord, got '{}'", coordstr));
        }
    }

    fn update_waypoints(&self) {
        if let Some(tx) = &self.opt_tx {
            let _ = tx.send(CanvasMsg::SetWaypoints(
                self.params.points.clone()));
        }
    }

    fn update_barriers(&self) {
        if let Some(tx) = &self.opt_tx {
            let _ = tx.send(CanvasMsg::SetBarriers(
                self.params.barriers.clone()));
        }
    }

    fn reset_view(&self) {
        if let Some(tx) = &self.opt_tx {
            let _ = tx.send(CanvasMsg::ResetView);
        }
    }
}

impl CmdApp for App {
    fn command_list<'a>(&self) -> &'a [&'a str] {
        return COMMAND_LIST;
    }

    fn execute_line(&mut self, cmd: &str, args: &Vec<String>)
                    -> Result<(), String> {
        println!("Executing command {} - {}", cmd, args.join(" "));
        match cmd {
            "add point" => {
                self.add_point(args)?;
            },
            "update point" => {
                self.update_point(args)?
            },
            "rm point" => {
                self.rm_point(args)?;
            },
            "add barrier" => {
                self.add_barrier(args)?;
            },
            "rm barrier" => {
                self.rm_barrier(args)?;
            },
            "read params" => {
                App::expects_num_arguments(args, 1)?;
                self.read_params(&args[0])?;
            },
            "store params" => {
                self.store_params(<dyn CmdApp>::opt_part(args, 0))?;
            },
            "show params" => {
                self.show_params();
            },
            "show cost" => {
                self.show_cost();
            },
            "show track info" => {
                self.show_path_info();
            },
            "set" => {
                App::expects_num_arguments(args, 2)?;
                self.set_param(&args[0], &args[1])?;
            },
            "open track" => {
                self.read_path(<dyn CmdApp>::opt_part(args, 0));
            },
            "store track" => {
                self.store_path(<dyn CmdApp>::opt_part(args, 0));
            },
            "compute" => {
                self.compute()?;
            },
            "flush maps" => {
                println!("Not implemented.");
            },
            "help" => {
                self.help();
            },
            _ => {
                unreachable!("Bad command");
            },
        }

        Ok(())
    }

    fn exit(&mut self) {
        if !self.params_stored {
            println!("Save params to {}? (Y/n)", &self.params.params_fname);
            if self.confirm_yes_no() {
                let _ = self.params.write_params(None);
                // FIXME: Handle error.
            }
        }

        if !self.path_stored {
            println!("Save track to {}? (Y/n)", &self.params.output_fname);
            if self.confirm_yes_no() {
                self.store_path(None);
            }
        }

        if let Some(tx) = &self.opt_tx {
            let _ = tx.send(CanvasMsg::Quit);
        }
    }
}
