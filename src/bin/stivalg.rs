use stivalg::{CONFIG, App, init_with_canvas, run_cmdui};

use hoydedata::{set_map_dir, unmount_all_maps};
use cmdui::CmdApp;

fn run_headless() -> Result<(), String> {
    let mut app = App::new(None, None)?;

    match CONFIG.command.as_str() {
        "compute" => {
            app.startup();
            app.compute()?;
            app.exit();
        }
        "" => {
            run_cmdui(&mut app);
        },
        _ => {
            println!("Invalid command");
        },
    }

    Ok(())
}

fn main() -> Result<(), String> {
    set_map_dir(&CONFIG.map_dir());

    if CONFIG.headless {
        run_headless()?;
    }
    else {
        init_with_canvas();
    }

    unmount_all_maps();

    Ok(())
}
