use crate::app::{App, run_cmdui};
use crate::barrier::Barrier;
use crate::channel::{AppMsg, CanvasMsg, CanvasReceiver, CanvasSender,
                     AppReceiver, AppSender,
                     create_canvas_channel, create_app_channel};
use crate::path::Path;
use crate::egui_map::{init_with_app, EguiMapState};

use eframe::CreationContext;
use egui::ViewportCommand;
use galileo::{Color, MapBuilder, MapView, Map};
use galileo::control::{EventPropagation, MouseButton, UserEvent,
                       UserEventHandler};
use galileo::layer::{FeatureId, FeatureLayer};
use galileo::layer::feature_layer::Feature;
use galileo::layer::raster_tile_layer::{RasterTileLayerBuilder,
                                        RestTileProvider};
use galileo::render::point_paint::PointPaint;
use galileo::render::render_bundle::RenderBundle;
use galileo::render::text::{TextStyle, RustybuzzRasterizer};
use galileo::render::text::text_service::TextService;
use galileo::symbol::SimpleContourSymbol;
use galileo::symbol::Symbol;
use galileo_types::Geometry;
use galileo_types::cartesian::{Point2, Point3, Vector2};
use galileo_types::geo::{Crs, GeoPoint, NewGeoPoint, Projection};
use galileo_types::geo::impls::GeoPoint2d;
use galileo_types::geometry::Geom;
use galileo_types::geometry_type::{CartesianSpace2d, GeoSpace2d};
use galileo_types::impls::Contour;
use hoydedata::Coord;
use parking_lot::RwLock;
use std::f32::consts::PI;
use std::sync::Arc;
use galileo::control::MapController;

fn terminal_controller(tx: CanvasSender, rx: AppReceiver) {
    let app_result = App::new(Some(tx), Some(rx));
    match app_result {
        Ok(mut app) => {
            run_cmdui(&mut app);
        },
        Err(s) => {
            // FIXME: send exit msg to tx before quitting
            println!("Error {}", s);
        }
    }
}

fn initialize_font_service() {
    let rasterizer = RustybuzzRasterizer::default();
    TextService::initialize(rasterizer).load_fonts(
        "data/fonts");
}

pub fn init_with_canvas() {
    initialize_font_service();

    // Create canvas <-> app channels and spawn off terminal controller
    // thread
    let (canvas_tx, canvas_rx) = create_canvas_channel();
    let (app_tx, app_rx) = create_app_channel();

    let canvas_tx_cloned = canvas_tx.clone();
    let handler = std::thread::spawn(move || terminal_controller(
        canvas_tx_cloned, app_rx));

    init_with_app(Box::new(|cc| Ok(Box::new(Canvas::new(
        cc,
        canvas_tx,
        canvas_rx,
        app_tx,
        []
    ))))).expect("failed to initialize");

    // Wait for app to finish
    handler.join().unwrap();
}

struct FeaturesState {
    points: Vec<Coord>,
    barriers: Vec<Barrier>,
    tmp_barrier: Option<Barrier>,
    req_point: bool,
}

impl FeaturesState {
    fn new() -> Self {
        Self {
            points: vec![],
            barriers: vec![],
            tmp_barrier: None,
            req_point: false,
        }
    }
}

struct MouseHandler {
    state: Arc<RwLock<FeaturesState>>,
    canvas_tx: CanvasSender,
    app_tx: AppSender,
}

impl MouseHandler {
    fn new(state: Arc<RwLock<FeaturesState>>, canvas_tx: CanvasSender,
           app_tx: AppSender) -> Self {
        Self {
            state: state,
            canvas_tx: canvas_tx,
            app_tx: app_tx,
        }
    }
}

impl UserEventHandler for MouseHandler {
    fn handle(&self, ev: &UserEvent, map: &mut Map) -> EventPropagation {
        let proj = Crs::EPSG3857
            .get_projection::<GeoPoint2d, Point2>()
            .unwrap();

        let mut state = self.state.write();

        match ev {
            UserEvent::Click(MouseButton::Left, mouse_event) => {
                if let Some(position) = map.view()
                    .screen_to_map(mouse_event.screen_pointer_position) {
                    if let Some(b) = state.tmp_barrier.as_mut() {
                        let gp = proj.unproject(&position).unwrap();
                        let c = Coord::from_latlon(gp.lat(), gp.lon());
                        if b.len() == 0 {
                            b.add_point(c);
                            b.add_point(c);
                        }
                        else {
                            b.update_point(b.len() - 1, c);
                            b.add_point(c);
                        }
                        let _ = self.canvas_tx.send(
                            CanvasMsg::RedrawTmpBarrier);
                    }
                    else if state.req_point {
                        let gp = proj.unproject(&position).unwrap();
                        let c = Coord::from_latlon(gp.lat(), gp.lon());
                        let _ = self.app_tx.send(AppMsg::SelectPoint(c));
                        state.req_point = false;
                    }
                }

                EventPropagation::Stop
            },
            UserEvent::PointerMoved(mouse_event) => {
                if let Some(b) = state.tmp_barrier.as_mut() {
                    if b.len() >= 2 {
                        if let Some(position) = map.view()
                            .screen_to_map(mouse_event.screen_pointer_position)
                        {
                            let gp = proj.unproject(&position).unwrap();
                            let c = Coord::from_latlon(gp.lat(), gp.lon());
                            b.update_point(b.len() - 1, c);
                            let _ = self.canvas_tx.send(
                                CanvasMsg::RedrawTmpBarrier);
                        }
                    }
                }

                EventPropagation::Stop
            },
            UserEvent::Click(MouseButton::Right, mouse_event) => {
                if let Some(mut b) = state.tmp_barrier.take() {
                    if let Some(position) = map.view()
                        .screen_to_map(mouse_event.screen_pointer_position) {
                        let gp = proj.unproject(&position).unwrap();
                        let c = Coord::from_latlon(gp.lat(), gp.lon());
                        if b.len() >= 2 {
                            b.update_point(b.len() - 1, c);
                            let _ = self.canvas_tx.send(
                                CanvasMsg::RedrawTmpBarrier);
                        }
                        let _ = self.app_tx.send(AppMsg::CreateBarrier(b));
                    }
                }

                EventPropagation::Stop
            },
            _ => EventPropagation::Propagate,
        }
    }
}

pub struct Canvas {
    state: Arc<RwLock<EguiMapState>>,
    features_state: Arc<RwLock<FeaturesState>>,
    rx: CanvasReceiver,
    waypoints: Arc<RwLock<FeatureLayer<GeoPoint2d, Waypoint, WaypointSymbol,
                                       GeoSpace2d>>>,
    areas: Arc<RwLock<FeatureLayer<Point2, Contour<Point2>,
                                   SimpleContourSymbol, CartesianSpace2d>>>,
    tracks: Arc<RwLock<FeatureLayer<Point2, Contour<Point2>,
                                    SimpleContourSymbol, CartesianSpace2d>>>,
    tmp_barrier_id: Option<FeatureId>,
    covering_length: Option<f32>,
    covering_width: Option<f32>,
}

impl Canvas {
    pub fn new(
        cc: &CreationContext<'_>,
        canvas_tx: CanvasSender,
        canvas_rx: CanvasReceiver,
        app_tx: AppSender,
        _: impl IntoIterator<Item = Box<dyn UserEventHandler>>,
    ) -> Self {

        let ctx = cc.egui_ctx.clone();
        let render_state = cc
            .wgpu_render_state
            .clone()
            .expect("failed to get wgpu context");

        // Get tiles from the opentopomap provider
        let provider = RestTileProvider::new(
            |index| {
                format!(
                    // "https://tile.openstreetmap.org/{}/{}/{}.png",
                    "https://tile.opentopomap.org/{}/{}/{}.png",
                    index.z, index.x, index.y
                )
            },
            None,
            false,
        );

        let raster_layer = RasterTileLayerBuilder::new_with_provider(provider)
        //        .with_file_cache_checked(".tile_cache")
            .build()
            .expect("failed to create layer");

        let (lat, lon) = Coord::from("N6969971.14E182124.64").latlon();

        // Build the map
        let mut map = MapBuilder::default()
            .with_latlon(lat, lon)
            .with_resolution(30.0)
            .with_layer(raster_layer)
            .build();

        // Add a layer for the waypoints
        let wp_layer = Arc::new(RwLock::new(FeatureLayer::new(
            vec![],
            WaypointSymbol::new(),
            Crs::WGS84
        )));
        map.layers_mut().push(wp_layer.clone());

        // Add a layer for the covering areas
        let areas_layer = Arc::new(RwLock::new(FeatureLayer::new(
            vec![],
            SimpleContourSymbol::new(Color::RED, 1.5),
            Crs::EPSG3857
        )));
        map.layers_mut().push(areas_layer.clone());

        // Add a layer for the tracks. We'll add content to it later
        let tracks_layer = Arc::new(RwLock::new(FeatureLayer::new(
            vec![],
            SimpleContourSymbol::new(Color::RED, 3.0),
            Crs::EPSG3857
        )));
        map.layers_mut().push(tracks_layer.clone());

        let map_state = Arc::new(RwLock::new(
            EguiMapState::new(map, ctx, render_state)));

        let features_state = Arc::new(RwLock::new(
            FeaturesState::new()));

        let ret = Self {
            state: map_state.clone(),
            features_state: features_state.clone(),
            rx: canvas_rx,
            waypoints: wp_layer,
            areas: areas_layer,
            tracks: tracks_layer,
            covering_length: None,
            covering_width: None,
            tmp_barrier_id: None,
        };

        // Create a mouse handler for the app
        let mouse_handler = MouseHandler::new(
            features_state, canvas_tx, app_tx);
        let mut state_mut = map_state.write();

        state_mut.add_handler(mouse_handler);
        state_mut.add_handler(MapController::default());

        return ret;
    }

    fn set_waypoints(&mut self, points: Vec<Coord>) {
        let mut layer = self.waypoints.write();

        // Remove old features
        let fs = layer.features_mut();
        let ids: Vec<FeatureId> = fs.iter().map(|(id, _)| id).collect();

        for id in ids {
            fs.remove(id);
        }

        let n = points.len();

        for i in 0..n {
            let p = points[i];
            let label = if i == 0 {
                format!("{} (start)", i + 1)
            }
            else if i == n - 1 {
                format!("{} (end)", i + 1)
            }
            else {
                format!("{}", i + 1)
            };

            let (lat, lon) = p.latlon();
            let wp = Waypoint::new(label, lat, lon);
            let _ = layer.features_mut().add(wp);
        }

        self.features_state.write().points = points;

        layer.update_all_features();
    }

    fn reset_view(&mut self) {
        let state = self.features_state.read();

        if state.points.len() < 2 {
            return;
        }

        let mut layer = self.waypoints.write();
        layer.update_all_features();

        if state.points.len() < 2 {
            return;
        }

        let (mut n, mut s, mut e, mut w) = (
            f32::NEG_INFINITY, f32::INFINITY,
            f32::NEG_INFINITY, f32::INFINITY,
        );

        // Find two coordinates spanning all the waypoints
        for p in &state.points {
            n = n.max(p.n);
            s = s.min(p.n);
            e = e.max(p.e);
            w = w.min(p.e);
        }

        let Some(covering_length) = self.covering_length else { return; };

        // Determine center of map view
        let (lat, lon) = Coord::new((e + w)/2.0, (n + s)/2.0).latlon();
        // Try to find a reasonable resolution which will encompass the
        // area to be used for the calculation.
        let res = (n - s).max(e - w)*covering_length*0.0030;

        let view = MapView::new(&GeoPoint2d::latlon(lat, lon), res as f64);
        self.state.write().map_mut().set_view(view);
    }

    fn draw_covering_areas(&self) {
        let state = self.features_state.read();

        if state.points.len() < 2 {
            return;
        }

        let Some(covering_length) = self.covering_length else {
            println!("No length");
            return; };
        let Some(covering_width) = self.covering_width else { return; };

        let mut layer = self.areas.write();

        // Create ellipses spanning the areas to be covered
        let len = state.points.len();

        let proj = Crs::EPSG3857
            .get_projection::<GeoPoint2d, Point2>()
            .unwrap();

        for i in 0..len - 1 {
            let p1 = state.points[i];
            let p2 = state.points[i + 1];

            let o = (p1 + p2)*0.5;
            let a = (p1 - o)*covering_length;
            let da = a.abs();
            let db = da*covering_width/covering_length;

            // Transform points from unit circle to ellipse with major axis da,
            // minor axis db and orientation along the a vector.
            //
            // Orientation of vector a:
            // cos(A) = a.x/da
            // sin(A) = a.y/da
            //
            // Squeeze circle into ellipsis:
            // A1 = [da 0
            //       0 db]
            //
            // Rotate ellipsis to the orientation of vector a:
            // A2 = [cos(A) -sin(A) = 1/da*[a.x  -a.y
            //       sin(A) cos(A)]         a.y  a.x]
            //
            // Combine transforms:
            // A1*A2 = [a.x         a.y
            //          -a.y*db/da  a.x*db/da]
            //
            let ta = a.e;
            let tb = -a.n*db/da;
            let tc = a.n;
            let td = a.e*db/da;

            let mut points = vec!();

            for j in 0..50 {
                let a = 2.0*PI*(j as f32)/50.0;
                // Point on circle
                let pe1 = a.cos();
                let pn1 = a.sin();

                // Stretch-transform point so it ends up on an ellipe and
                // translate it to new center point.
                let pe2 = ta*pe1 + tb*pn1 + o.e;
                let pn2 = tc*pe1 + td*pn1 + o.n;

                let (lat, lon) = Coord::new(pe2, pn2).latlon();

                let geop = GeoPoint2d::latlon(lat, lon);
                let p = proj.project(&geop).unwrap();
                points.push(p);
            }

            let contour = Contour::closed(points);
            let _ = layer.features_mut().add(contour);
        }

        layer.update_all_features();
    }

    fn draw_barriers(&self) {
        let mut layer = self.areas.write();

        let proj = Crs::EPSG3857
            .get_projection::<GeoPoint2d, Point2>()
            .unwrap();

        for b in &self.features_state.write().barriers {
            let mut points = vec!();

            for c in &b.points {
                let (lat, lon) = c.latlon();
                let geop = GeoPoint2d::latlon(lat, lon);
                let p = proj.project(&geop).unwrap();
                points.push(p);
            }

            let contour = Contour::open(points);
            let _ = layer.features_mut().add(contour);
        }

        layer.update_all_features();
    }

    fn redraw_covering_areas_and_barriers(&mut self) {
        // Remove old features
        {
            let mut layer = self.areas.write();

            let fs = layer.features_mut();
            let ids: Vec<FeatureId> = fs.iter().map(|(id, _)| id).collect();

            for id in ids {
                fs.remove(id);
            }

            self.tmp_barrier_id.take();
        }

        self.draw_covering_areas();
        self.draw_barriers();
    }

    fn redraw_tmp_barrier(&mut self) {
        let mut layer = self.areas.write();
        let mut state = self.features_state.write();

        // Remove old feature if there is any
        if let Some(id) = self.tmp_barrier_id {
            let fs = layer.features_mut();
            fs.remove(id);
            layer.update_feature(id);
        }

        if let Some(barrier) = state.tmp_barrier.as_mut() {
            let proj = Crs::EPSG3857
                .get_projection::<GeoPoint2d, Point2>()
                .unwrap();

            let mut points = vec!();

            for c in &barrier.points {
                let (lat, lon) = c.latlon();
                let geop = GeoPoint2d::latlon(lat, lon);
                let p = proj.project(&geop).unwrap();
                points.push(p);
            }

            let contour = Contour::open(points);
            let id = layer.features_mut().add(contour);
            layer.update_feature(id);
            self.tmp_barrier_id.replace(id);
        }
    }

    fn set_track(&self, path: &Path) {
	let mut points = vec!();

        // Remove old track
        let mut layer = self.tracks.write();
        let fs = layer.features_mut();
        let ids: Vec<FeatureId> = fs.iter().map(|(id, _)| id).collect();

        for id in ids {
            fs.remove(id);
        }

        let proj = Crs::EPSG3857
            .get_projection::<GeoPoint2d, Point2>()
            .unwrap();

        for c in path {
            let (lat, lon) = c.latlon();
            let geop = GeoPoint2d::latlon(lat, lon);
            let p = proj.project(&geop).unwrap();
	    points.push(p);
	}

	let contour = Contour::open(points);

        let _ = fs.add(contour);
        layer.update_all_features();
    }

    fn check_channel(&mut self) -> bool {
        while let Ok(o) = self.rx.try_recv() {
            match o {
                CanvasMsg::SetPath(path) => {
                    self.set_track(&path);
                },
                CanvasMsg::SetWaypoints(points) => {
                    self.set_waypoints(points);
                    self.redraw_covering_areas_and_barriers();
                },
                CanvasMsg::SetBarriers(barriers) => {
                    self.features_state.write().barriers = barriers;
                    self.redraw_covering_areas_and_barriers();
                },
                CanvasMsg::SetCoveringArea(length, width) => {
                    self.covering_length.replace(length);
                    self.covering_width.replace(width);
                    self.redraw_covering_areas_and_barriers();
                },
                CanvasMsg::RequestPoint => {
                    // FIXME: Ensure that point has not already been requested
                    self.features_state.write().req_point = true;
                },
                CanvasMsg::RequestBarrier => {
                    // FIXME: Ensure that barrier has not already been requested
                    self.features_state.write().tmp_barrier
                        .replace(Barrier::new());
                    self.tmp_barrier_id.take();
                },
                CanvasMsg::RedrawTmpBarrier => {
                    self.redraw_tmp_barrier();
                },
                CanvasMsg::ResetView => {
                    self.reset_view();
                },
                CanvasMsg::Quit => {
                    return true;
                },
            }
        }

        return false;
    }
}

impl eframe::App for Canvas {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let quit = self.check_channel();

        if quit {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            self.state.write().render(ui);
        });
    }
}

struct Waypoint {
    label: String,
    lat: f64,
    lon: f64,
}

impl Waypoint {
    fn new(label: String, lat: f64, lon: f64) -> Self {
        Self {
            label: label,
            lat: lat,
            lon: lon,
        }
    }
}

impl Feature for Waypoint {
    type Geom = Self;

    fn geometry(&self) -> &Self::Geom {
        self
    }
}

impl GeoPoint for Waypoint {
    type Num = f64;

    fn lat(&self) -> Self::Num {
        self.lat
    }

    fn lon(&self) -> Self::Num {
        self.lon
    }
}

impl Geometry for Waypoint {
    type Point = GeoPoint2d;

    fn project<P: Projection<InPoint = Self::Point> + ?Sized>(
        &self,
        projection: &P,
    ) -> Option<Geom<P::OutPoint>> {
        GeoPoint2d::latlon(self.lat, self.lon).project(projection)
    }
}

struct WaypointSymbol {
    style: TextStyle,
}

impl WaypointSymbol {
    fn new() -> Self {
        Self {
            style: TextStyle {
                font_family: vec!["Noto Sans".to_string()],
                font_size: 15.0,
                font_color: Color::RED,
                horizontal_alignment: Default::default(),
                vertical_alignment: Default::default(),
                weight: Default::default(),
                style: Default::default(),
                outline_width: Default::default(),
                outline_color: Default::default(),
            }
        }
    }
}

impl Symbol<Waypoint> for WaypointSymbol {
    fn render<'a> (
        &self,
        feature: &Waypoint,
        geometry: &'a galileo_types::geometry::Geom<Point3>,
        min_resolution: f64,
        bundle: &mut RenderBundle,
    ) {
        let Geom::Point(point) = geometry else {
            return;
        };

        // Draw point
        bundle.add_point(
            point,
            &PointPaint::circle(Color::RED, 8.0),
            min_resolution,
        );
        // Print caption
        bundle.add_label(
            point,
            &feature.label,
            &self.style,
            Vector2::new(0.0, 10.0),
            true,
        );
    }
}
