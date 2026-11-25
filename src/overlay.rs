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
        let _ = self.cmd_tx.lock().unwrap().send(OverlayCmd::Show(x, y));
    }
    pub fn hide(&self) {
        let _ = self.cmd_tx.lock().unwrap().send(OverlayCmd::Hide);
    }
    pub fn set_recording(&self, rec: bool) {
        let _ = self.cmd_tx.lock().unwrap().send(OverlayCmd::SetColor(rec));
    }
}

fn run_x11(click_tx: Sender<()>, cmd_rx: Receiver<OverlayCmd>) {
    let (conn, screen_num) = match x11rb::connect(None) {
        Ok(v) => v,
        Err(_) => return,
    };
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let depth = screen.root_depth;
    let visual = screen.root_visual;

    let win = conn.generate_id().unwrap();
    let width = 36u16;
    let height = 36u16;
    conn.create_window(
        depth,
        win,
        root,
        0, 0, width, height, 0,
        WindowClass::INPUT_OUTPUT,
        visual,
        &CreateWindowAux::new()
            .override_redirect(1)
            .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS),
    ).unwrap();

    let gc = conn.generate_id().unwrap();
    conn.create_gc(gc, win, &CreateGCAux::new()).unwrap();

    let mut visible = false;
    let mut recording = false;

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                OverlayCmd::Show(x, y) => {
                    let _ = conn.configure_window(win, &ConfigureWindowAux::new().x(x).y(y));
                    let _ = conn.map_window(win);
                    visible = true;
                }
                OverlayCmd::Hide => {
                    let _ = conn.unmap_window(win);
                    visible = false;
                }
                OverlayCmd::SetColor(r) => {
                    recording = r;
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
                    let color = if recording { 0xFF3333u32 } else { 0x3366FFu32 };
                    let _ = conn.change_gc(gc, &ChangeGCAux::new().foreground(color));
                    let _ = conn.poly_fill_arc(win, gc, &[XArc { x: 2, y: 2, width: (width-4) as u16, height: (height-4) as u16, angle1: 0, angle2: 360*64 }]);
                    let _ = conn.flush();
                }
                _ => {}
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }
}
