use stivalg::{Path, Params};
use hoydedata::Atlas;

fn main() {
    let atlas = Atlas::new_mockup();
    let params = Params::from_config();

    if let Some(p) = Path::from_points(&params, &atlas) {
        p.print_summary(&atlas);
//        println!("Storing track to {}", &params.output);
//        p.write_gpx(&params.output);
    }
    else {
        println!("Path {} cannot be walked", params.points.iter()
                 .map(|c| c.to_string())
                 .collect::<Vec<String>>()
                 .join(", "))
    }
}
