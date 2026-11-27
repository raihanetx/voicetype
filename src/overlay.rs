use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xproto::Arc as XArc;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

// -------- shared drawing helpers --------
fn fill_rect(conn: &RustConnection, win: Window, gc: Gcontext, c: u32, x: i16, y: i16, w: u16, h: u16) {
    let _ = conn.change_gc(gc, &ChangeGCAux::new().foreground(c));
    let _ = conn.poly_fill_rectangle(win, gc, &[Rectangle { x, y, width: w, height: h }]);
}

fn fill_arc(conn: &RustConnection, win: Window, gc: Gcontext, c: u32, x: i16, y: i16, w: u16, h: u16) {
    let _ = conn.change_gc(gc, &ChangeGCAux::new().foreground(c));
    let _ = conn.poly_fill_arc(win, gc, &[XArc { x, y, width: w, height: h, angle1: 0, angle2: 360 * 64 }]);
}

/// Draw the classic microphone glyph centred in a `size`x`size` window.
/// body: rounded capsule, then a U-shaped cradle, a stem and a base.
fn draw_mic(conn: &RustConnection, win: Window, gc: Gcontext, size: u16, fg: u32, bg: u32) {
    let s = size as i16;
    // background circle (button)
    fill_arc(conn, win, gc, bg, 0, 0, size, size);

    // capsule body (rounded vertical bar)
    let body_w = (s * 2 / 5).max(6);
    let body_h = (s * 9 / 20).max(8);
    let bx = s / 2 - body_w / 2;
    let by = s / 6;
    fill_arc(conn, win, gc, fg, bx, by, body_w as u16, body_w as u16); // top cap
    fill_rect(conn, win, gc, fg, bx, by + body_w / 2, body_w as u16, (body_h - body_w / 2) as u16);
    fill_arc(conn, win, gc, fg, bx, by + body_h - body_w, body_w as u16, body_w as u16); // bottom cap

    // cradle: two vertical bars + bottom bar under the capsule
    let cw = (s / 20).max(2);
    let cradle_x0 = bx - body_w / 2;
    let cradle_x1 = bx + body_w + body_w / 2 - cw;
    let cradle_top = by + body_h / 2;
    let cradle_bot = by + body_h + (s / 12);
    fill_rect(conn, win, gc, fg, cradle_x0, cradle_top, cw as u16, (cradle_bot - cradle_top) as u16);
    fill_rect(conn, win, gc, fg, cradle_x1, cradle_top, cw as u16, (cradle_bot - cradle_top) as u16);
    fill_rect(conn, win, gc, fg, cradle_x0, cradle_bot, (cradle_x1 - cradle_x0 + cw) as u16, cw as u16);

    // stem
    let stem_x = s / 2 - cw / 2;
    fill_rect(conn, win, gc, fg, stem_x, cradle_bot + cw, cw as u16, (s / 8).max(2) as u16);

    // base
    let base_w = (s / 3).max(8);
    let base_y = cradle_bot + cw + (s / 8).max(2);
    fill_rect(conn, win, gc, fg, s / 2 - base_w / 2, base_y, base_w as u16, cw as u16);
}

/// Draw a small gear/bug "debug" glyph.
fn draw_debug(conn: &RustConnection, win: Window, gc: Gcontext, size: u16, fg: u32, bg: u32) {
    let s = size as i16;
    fill_arc(conn, win, gc, bg, 0, 0, size, size);
    // simple gear: center circle + 8 teeth
    let cx = s / 2;
    let cy = s / 2;
    let r = (s / 6).max(3);
    fill_arc(conn, win, gc, fg, cx - r, cy - r, (r * 2) as u16, (r * 2) as u16);
    let tw = (s / 10).max(2);
    let tl = (s / 3).max(6);
    // vertical + horizontal teeth
    fill_rect(conn, win, gc, fg, cx - tw / 2, cy - tl / 2, tw as u16, tl as u16);
    fill_rect(conn, win, gc, fg, cx - tl / 2, cy - tw / 2, tl as u16, tw as u16);
}

#[derive(Clone, Copy, PartialEq)]
pub enum OverlayState {
    Idle,
    Recording,
    Processing,
    Error,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Btn {
    Mic,
    Debug,
}

#[derive(Clone)]
pub struct Overlay {
    pub click_rx: Arc<Mutex<Receiver<Btn>>>,
    cmd_tx: Arc<Mutex<Sender<Cmd>>>,
}

enum Cmd {
    ShowMic(i32, i32, i16),
    HideMic,
    ShowDebug,
    SetState(OverlayState),
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

    pub fn show_mic(&self, x: i32, y: i32, height: i16) {
        let _ = self.tx().send(Cmd::ShowMic(x, y, height));
    }
    pub fn hide_mic(&self) {
        let _ = self.tx().send(Cmd::HideMic);
    }
    pub fn show_debug(&self) {
        let _ = self.tx().send(Cmd::ShowDebug);
    }
    pub fn set_state(&self, s: OverlayState) {
        let _ = self.tx().send(Cmd::SetState(s));
    }

    fn tx(&self) -> std::sync::MutexGuard<'_, Sender<Cmd>> {
        self.cmd_tx.lock().unwrap_or_else(|e| e.into_inner())
    }
}

struct IconWin {
    win: Window,
    gc: Gcontext,
    size: u16,
    visible: bool,
    state: OverlayState,
    dirty: bool,
}

impl IconWin {
    fn create(conn: &RustConnection, root: Window, depth: u8, visual: Visualid, w: u16, h: u16, mask: EventMask, re: bool) -> Option<Self> {
        let win = conn.generate_id().ok()?;
        let mut aux = CreateWindowAux::new().event_mask(mask);
        if re {
            aux = aux.override_redirect(1);
        }
        conn.create_window(depth, win, root, 0, 0, w, h, 0, WindowClass::INPUT_OUTPUT, visual, &aux).ok()?;
        let gc = conn.generate_id().ok()?;
        conn.create_gc(gc, win, &CreateGCAux::new()).ok()?;
        Some(Self { win, gc, size: w, visible: false, state: OverlayState::Idle, dirty: true })
    }

    fn move_to(&self, conn: &RustConnection, x: i32, y: i32) {
        let _ = conn.configure_window(self.win, &ConfigureWindowAux::new().x(x).y(y));
    }

    fn show(&mut self, conn: &RustConnection) {
        let _ = conn.map_window(self.win);
        let _ = conn.configure_window(self.win, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE));
        self.visible = true;
    }

    fn hide(&mut self, conn: &RustConnection) {
        let _ = conn.unmap_window(self.win);
        self.visible = false;
    }

    fn mic_colors(state: OverlayState) -> (u32, u32) {
        // (foreground glyph, background)
        match state {
            OverlayState::Idle => (0xFFFFFF, 0x2F6FED),
            OverlayState::Recording => (0xFFFFFF, 0xD93025),
            OverlayState::Processing => (0x000000, 0xF2B600),
            OverlayState::Error => (0xFFFFFF, 0xC2410C),
        }
    }
}

fn run_x11(click_tx: Sender<Btn>, cmd_rx: Receiver<Cmd>) {
    let (conn, screen_num) = match x11rb::connect(None) {
        Ok(v) => v,
        Err(e) => {
            crate::notify_error(&format!("VoiceType could not connect to the display.\n\n{e}\n\nIs DISPLAY set (X11 session, not Wayland)?"));
            return;
        }
    };
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let depth = screen.root_depth;
    let visual = screen.root_visual;
    let sw = screen.width_in_pixels as i32;

    let mic_size = 32u16;
    let dbg_size = 26u16;

    let mut mic = match IconWin::create(&conn, root, depth, visual, mic_size, mic_size,
        EventMask::EXPOSURE | EventMask::BUTTON_PRESS, true) {
        Some(m) => m, None => { crate::notify_error("could not create mic window"); return; }
    };
    let mut dbg = match IconWin::create(&conn, root, depth, visual, dbg_size, dbg_size,
        EventMask::EXPOSURE | EventMask::BUTTON_PRESS, true) {
        Some(d) => d, None => { crate::notify_error("could not create debug window"); return; }
    };

    // Debug icon lives permanently in the top-right corner.
    dbg.move_to(&conn, sw - dbg_size as i32 - 8, 8);
    dbg.show(&conn);
    let _ = conn.flush();

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                Cmd::ShowMic(x, y, h) => {
                    // vertically centre the mic on the field
                    let cy = y + (h as i32 - mic_size as i32) / 2;
                    mic.move_to(&conn, x, cy.max(0));
                    mic.state = OverlayState::Idle;
                    mic.dirty = true;
                    mic.show(&conn);
                }
                Cmd::HideMic => {
                    mic.hide(&conn);
                }
                Cmd::ShowDebug => {
                    dbg.show(&conn);
                }
                Cmd::SetState(s) => {
                    mic.state = s;
                    mic.dirty = true;
                }
            }
            let _ = conn.flush();
        }

        if let Ok(Some(ev)) = conn.poll_for_event() {
            match ev {
                Event::ButtonPress(b) => {
                    if b.event == mic.win {
                        let _ = click_tx.send(Btn::Mic);
                    } else if b.event == dbg.win {
                        let _ = click_tx.send(Btn::Debug);
                    }
                }
                Event::Expose(e) => {
                    if e.window == mic.win { mic.dirty = true; }
                    if e.window == dbg.win { dbg.dirty = true; }
                }
                _ => {}
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }

        if mic.dirty && mic.visible {
            let (fg, bg) = IconWin::mic_colors(mic.state);
            draw_mic(&conn, mic.win, mic.gc, mic.size, fg, bg);
            mic.dirty = false;
        }
        if dbg.dirty && dbg.visible {
            draw_debug(&conn, dbg.win, dbg.gc, dbg.size, 0xF2F2F2, 0x303030);
            dbg.dirty = false;
        }
        if mic.dirty || dbg.dirty {
            let _ = conn.flush();
        }
    }
}
