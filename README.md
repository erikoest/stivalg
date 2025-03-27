# stivalg

Optimal track finding tool.

This application uses the dijkstra algorithm over a map of geographic
elevation data and tries to determine the best track to walk from one
point to another. Intermediate waypoints can be specified. Non-passable
barriers can be specified as line segments. The application uses the
galileo module for visualizing the track with waypoints etc. over a
topographic openstreetmap map. The resulting track can be
stored in a gpx file.

## Usage

  * Build code:
  <pre>
    cargo build release
  </pre>
  * Download geotiff maps and install them on local storage according to
    instructions at https://github.com/erikoest/hoydedata
  * Run application:
  <pre>
    ./target/release/stivalg
    > help
  </pre>

## Limitations

The application currently has a few limitations:

* The cost curve (time per slope and length) is hardcoded and is currently
  adjusted to giving good track suggestions for skiing trips.
* There is no awareness of water. Sea, lakes and rivers are considered to
  be passable. Again, this fits skiing trips well.
* The application uses the hoydedata module for map lookups, which is
  limited to norwegian geodata.
