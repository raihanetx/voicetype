use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xproto::Arc as XArc;
use x11rb::protocol::Event;
// use x11rb::rust_connection::RustConnection;

#[derive(Clone)]
pub struct Overlay {
    pub click_rx: Arc<Mutex<Receiver<()>>>,
    cmd_tx: Arc<Mutex<Sender<OverlayCmd>>>,
}

enum OverlayCmd {
    Show(i32, i32),
    Hide,
    SetColor(bool),
    SetState(OverlayState),
}

/// Visual state of the floating dot.
#[derive(Clone, Copy, PartialEq)]
pub enum OverlayState {
    Idle,       // blue: waiting for input
    Recording,  // red: capturing audio
    Processing, // yellow: transcribing
    Error,      // orange/red ring: something failed
}

impl OverlayState {
    fn color(self) -> u32 {
        match self {
            OverlayState::Idle => 0x3366FF,
            OverlayState::Recording => 0xFF3333,
            OverlayState::Processing => 0xFFCC00,
            OverlayState::Error => 0xFF6600,
        }
    }
}

impl Overlay {
    pub fn new() -> Self {
        let (click_tx, click_rx) = channel();
        let (cmd_tx, cmd_rx) = channel();
        std::thread::spawn(move || run_x11(click_tx, cmd_rx));
        Self {
            click_rx: Arc::new(Mutex::new(click_rx)),
            cmd_tx: Arc::new(Mutex::new(cmd_tx)),
        }
    }

    pub fn show(&self, x: i32, y: i32) {
        let _ = self.cmd_tx.lock().unwrap_or_else(|e| e.into_inner()).send(OverlayCmd::Show(x, y));
    }
    pub fn hide(&self) {
        let _ = self.cmd_tx.lock().unwrap_or_else(|e| e.into_inner()).send(OverlayCmd::Hide);
    }
    pub fn set_recording(&self, rec: bool) {
        let _ = self.cmd_tx.lock().unwrap_or_else(|e| e.into_inner()).send(OverlayCmd::SetColor(rec));
    }
    /// Set the dot to an explicit visual state (for the debug indicator).
    pub fn set_state(&self, state: OverlayState) {
        let _ = self.cmd_tx.lock().unwrap_or_else(|e| e.into_inner()).send(OverlayCmd::SetState(state));
    }
}

fn run_x11(click_tx: Sender<()>, cmd_rx: Receiver<OverlayCmd>) {
    let (conn, screen_num) = match x11rb::connect(None) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("X11 connect failed: {e}");
            crate::notify_error(&format!("VoiceType could not connect to the display.\n\n{e}\n\nIs DISPLAY set (X11 session, not Wayland)?"));
            return;
        }
    };
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let depth = screen.root_depth;
    let visual = screen.root_visual;

    let win = match conn.generate_id() { Ok(id) => id, Err(e) => { tracing::error!("generate_id: {e}"); return; } };
    let width = 36u16;
    let height = 36u16;
    if let Err(e) = conn.create_window(
        depth,
        win,
        root,
        0, 0, width, height, 0,
        WindowClass::INPUT_OUTPUT,
        visual,
        &CreateWindowAux::new()
            .override_redirect(1)
            .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS),
    ) {
        tracing::error!("create_window: {e}"); return;
    }

    let gc = match conn.generate_id() { Ok(id) => id, Err(e) => { tracing::error!("generate_id gc: {e}"); return; } };
    if let Err(e) = conn.create_gc(gc, win, &CreateGCAux::new()) {
        tracing::error!("create_gc: {e}"); return;
    }

    let mut visible = false;
    let mut recording = false;
    let mut state = OverlayState::Idle;
    let mut dirty = false;

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                OverlayCmd::Show(x, y) => {
                    let _ = conn.configure_window(win, &ConfigureWindowAux::new().x(x).y(y));
                    let _ = conn.map_window(win);
                    visible = true;
                    state = OverlayState::Idle;
                    dirty = true;
                }
                OverlayCmd::Hide => {
                    let _ = conn.unmap_window(win);
                    visible = false;
                }
                OverlayCmd::SetColor(r) => {
                    recording = r;
                    state = if r { OverlayState::Recording } else { OverlayState::Idle };
                    dirty = true;
                }
                OverlayCmd::SetState(s) => {
                    state = s;
                    recording = s == OverlayState::Recording;
                    dirty = true;
                }
            }
            let _ = conn.flush();
        }

        if let Ok(Some(ev)) = conn.poll_for_event() {
            match ev {
                Event::ButtonPress(_) => {
                    let _ = click_tx.send(());
                }
                Event::Expose(_) => {
                    dirty = true;
                }
                _ => {}
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }

        if dirty && visible {
            let color = state.color();
            let _ = conn.change_gc(gc, &ChangeGCAux::new().foreground(color));
            let _ = conn.poly_fill_arc(win, gc, &[XArc { x: 2, y: 2, width: (width-4) as u16, height: (height-4) as u16, angle1: 0, angle2: 360*64 }]);
            let _ = conn.flush();
            dirty = false;
            let _ = recording;
        }
    }
}
