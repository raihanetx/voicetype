use atspi::connection::AccessibilityConnection;
use atspi::proxy::accessible::ObjectRefExt;
use atspi::proxy::component::ComponentProxy;
use atspi::ObjectRef;
use atspi::Role;
use atspi::State;
use atspi::CoordType;
use atspi::events::object::StateChangedEvent;
use futures_lite::StreamExt;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Roles that behave like a text input surface we can dictate into.
fn is_text_like(role: Role) -> bool {
    matches!(
        role,
        Role::Entry
            | Role::PasswordText
            | Role::Text
            | Role::Terminal
            | Role::Paragraph
            | Role::DocumentText
            | Role::DocumentFrame
    )
}

pub async fn run_watcher(
    conn: AccessibilityConnection,
    overlay: crate::overlay::Overlay,
    focused: Arc<Mutex<Option<ObjectRef>>>,
    window_id: Arc<Mutex<Option<String>>>,
    auto_record: bool,
    auto_tx: mpsc::Sender<()>,
    log: crate::session_log::SessionLog,
) {
    // Make sure the accessibility bridge is enabled for this session: without
    // it many apps never emit focus events, which is why the mic would never
    // appear. This is best-effort.
    match atspi::connection::set_session_accessibility(true).await {
        Ok(()) => log.info("accessibility bridge enabled (session wide)"),
        Err(e) => log.warn(format!("could not enable accessibility bridge: {e}")),
    }
    match atspi::connection::read_session_accessibility().await {
        Ok(v) => log.info(format!("session accessibility enabled = {v}")),
        Err(e) => log.warn(format!("could not read accessibility status: {e}")),
    }

    match conn.register_event::<StateChangedEvent>().await {
        Ok(()) => log.info("registered AT-SPI StateChanged listener"),
        Err(e) => log.error(format!("failed to register AT-SPI events: {e}")),
    }
    let mut stream = conn.event_stream();
    std::pin::pin!(&mut stream);

    while let Some(result) = stream.next().await {
        match result {
            Ok(ev) => {
                let sc = match StateChangedEvent::try_from(ev) {
                    Ok(s) => s,
                    Err(e) => {
                        log.warn(format!("non-state event ignored: {e}"));
                        continue;
                    }
                };
                if sc.state != State::Focused {
                    continue;
                }

                let obj = sc.item.clone();

                let acc = match obj.as_accessible_proxy(conn.connection()).await {
                    Ok(a) => a,
                    Err(e) => {
                        log.warn(format!("focus event: cannot open accessible proxy: {e}"));
                        continue;
                    }
                };
                let role = acc.get_role().await.unwrap_or(Role::Unknown);

                // Log EVERY focus event with role + enabled, so a "no mic"
                // case is explainable from the report alone.
                log.info(format!(
                    "focus event: role={role:?} enabled={} name={}",
                    sc.enabled, obj.name
                ));

                if sc.enabled == 0 {
                    overlay.hide_mic();
                    *focused.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    continue;
                }

                if !is_text_like(role) {
                    log.info(format!("focus on non-text role {role:?}; hiding mic"));
                    overlay.hide_mic();
                    *focused.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    continue;
                }

                let comp = match ComponentProxy::builder(conn.connection())
                    .destination(obj.name.as_str())
                {
                    Ok(b) => b,
                    Err(e) => {
                        log.warn(format!("component builder failed: {e}"));
                        continue;
                    }
                };
                let comp = match comp.path(obj.path.as_str()) {
                    Ok(b) => b,
                    Err(e) => {
                        log.warn(format!("component path failed: {e}"));
                        continue;
                    }
                };
                let comp = match comp
                    .cache_properties(atspi::zbus::proxy::CacheProperties::No)
                    .build()
                    .await
                {
                    Ok(c) => c,
                    Err(e) => {
                        log.warn(format!("component proxy build failed: {e}"));
                        continue;
                    }
                };

                match comp.get_extents(CoordType::Screen).await {
                    Ok((x, y, w, h)) => {
                        if w <= 0 || h <= 0 {
                            log.warn(format!("text field has zero size ({w}x{h}); skipping"));
                            continue;
                        }
                        overlay.show_mic(x + w + 4, y, h as i16);
                        log.info(format!("mic shown at ({},{}) for field {w}x{h}", x + w + 4, y));
                        *focused.lock().unwrap_or_else(|e| e.into_inner()) = Some(obj);
                        if let Ok(out) = Command::new("xdotool").arg("getactivewindow").output() {
                            let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
                            if !id.is_empty() {
                                *window_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
                            }
                        }
                        if auto_record {
                            let _ = auto_tx.try_send(());
                        }
                    }
                    Err(e) => {
                        log.warn(format!("get_extents failed: {e}"));
                    }
                }
            }
            Err(e) => {
                log.error(format!("AT-SPI stream error: {e}"));
            }
        }
    }
    log.warn("AT-SPI event stream ended");
}

/// Polling fallback: if an app doesn't emit focus events, ask the accessibility
/// tree directly "what is focused right now?" and mirror the same show/hide
/// behaviour. Runs alongside the event listener.
pub async fn run_poller(
    conn: AccessibilityConnection,
    overlay: crate::overlay::Overlay,
    focused: Arc<Mutex<Option<ObjectRef>>>,
    window_id: Arc<Mutex<Option<String>>>,
    log: crate::session_log::SessionLog,
) {
    use atspi::proxy::accessible::AccessibleProxy;

    // Desktop root lives on the registry service. We only use it to get the
    // list of applications (their ObjectRefs carry real unique names).
    let root = match AccessibleProxy::builder(conn.connection())
        .destination("org.a11y.atspi.Registry")
        .and_then(|b| b.path("/org/a11y/atspi/accessible/root"))
    {
        Ok(b) => match b
            .cache_properties(atspi::zbus::proxy::CacheProperties::No)
            .build()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log.warn(format!("poller: cannot build desktop root: {e}"));
                return;
            }
        },
        Err(e) => {
            log.warn(format!("poller: bad desktop root path: {e}"));
            return;
        }
    };

    let mut last_key: Option<(String, String)> = None;

    loop {
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;

        let mut visited = 0usize;
        let mut found: Option<(ObjectRef, Role, (i32, i32, i32, i32))> = None;

        if let Ok(apps) = root.get_children().await {
            'outer: for app in apps {
                if let Some(hit) =
                    find_focused_text(conn.connection(), app, 0, &mut visited).await
                {
                    found = Some(hit);
                    break 'outer;
                }
            }
        }

        match found {
            Some((obj, role, ext)) => {
                let key = (obj.name.to_string(), obj.path.to_string());
                if last_key.as_ref() != Some(&key) {
                    log.info(format!(
                        "poller found focused {role:?} at ({},{}) {}x{}",
                        ext.0, ext.1, ext.2, ext.3
                    ));
                    last_key = Some(key);
                }
                if ext.2 > 0 && ext.3 > 0 {
                    overlay.show_mic(ext.0 + ext.2 + 4, ext.1, ext.3 as i16);
                    *focused.lock().unwrap_or_else(|e| e.into_inner()) = Some(obj);
                    if let Ok(out) = Command::new("xdotool").arg("getactivewindow").output() {
                        let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        if !id.is_empty() {
                            *window_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
                        }
                    }
                }
            }
            None => {
                if last_key.is_some() {
                    log.info("poller: no focused text field; hiding mic");
                    last_key = None;
                }
                overlay.hide_mic();
            }
        }
    }
}

/// Depth- and node-bounded search for the focused text-like node.
/// Operates on `ObjectRef`s (which carry authentic unique names) and opens
/// proxies internally, so the result is directly usable for injection.
fn find_focused_text<'a>(
    conn: &'a atspi::zbus::Connection,
    obj: ObjectRef,
    depth: u32,
    visited: &'a mut usize,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Option<(ObjectRef, Role, (i32, i32, i32, i32))>> + Send + 'a>,
> {
    Box::pin(async move {
        if depth > 12 || *visited > 400 {
            return None;
        }
        *visited += 1;

        let proxy = match obj.as_accessible_proxy(conn).await {
            Ok(p) => p,
            Err(_) => return None,
        };

        if let Ok(state) = proxy.get_state().await {
            if state.contains(State::Focused) {
                if let Ok(role) = proxy.get_role().await {
                    if is_text_like(role) {
                        let ext = extents_of(conn, &obj).await.unwrap_or((0, 0, 0, 0));
                        return Some((obj, role, ext));
                    }
                }
            }
        }

        if let Ok(children) = proxy.get_children().await {
            for child in children {
                if *visited > 400 {
                    break;
                }
                if let Some(hit) = find_focused_text(conn, child, depth + 1, visited).await {
                    return Some(hit);
                }
            }
        }
        None
    })
}

async fn extents_of(
    conn: &atspi::zbus::Connection,
    obj: &ObjectRef,
) -> Option<(i32, i32, i32, i32)> {
    let comp = ComponentProxy::builder(conn)
        .destination(obj.name.as_str())
        .ok()?
        .path(obj.path.as_str())
        .ok()?
        .cache_properties(atspi::zbus::proxy::CacheProperties::No)
        .build()
        .await
        .ok()?;
    comp.get_extents(CoordType::Screen).await.ok()
}
