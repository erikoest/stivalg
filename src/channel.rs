use crate::barrier::Barrier;
use crate::path::Path;

use crossbeam_channel::{Sender, Receiver, unbounded};
use hoydedata::Coord;

#[derive(Debug)]
pub enum CanvasMsg {
    SetWaypoints(Vec<Coord>),
    SetBarriers(Vec<Barrier>),
    SetPath(Path),
    SetCoveringArea(f32, f32),
    RequestPoint,
    RequestBarrier,
    RedrawTmpBarrier,
    ResetView,
    Quit,
}

pub enum AppMsg {
    SelectPoint(Coord),
    CreateBarrier(Barrier),
    Quit,
}

pub type CanvasSender = Sender<CanvasMsg>;
pub type CanvasReceiver = Receiver<CanvasMsg>;

pub type AppSender = Sender<AppMsg>;
pub type AppReceiver = Receiver<AppMsg>;

pub fn create_canvas_channel() -> (CanvasSender, CanvasReceiver) {
    unbounded()
}

pub fn create_app_channel() -> (AppSender, AppReceiver) {
    unbounded()
}
