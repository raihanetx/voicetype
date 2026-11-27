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

                // Resolve role first — this is the key signal for diagnosing.
                let acc = match obj.as_accessible_proxy(conn.connection()).await {
                    Ok(a) => a,
                    Err(e) => {
                        log.warn(format!("focus event: cannot open accessible proxy: {e}"));
                        continue;
                    }
                };
                let role = acc.get_role().await.unwrap_or(Role::Unknown);

                // Log EVERY focus event with role + enabled, so a "no mic" case
                // is explainable from the report alone.
                log.info(format!(
                    "focus event: role={role:?} enabled={} name={}",
                    sc.enabled, obj.name
                ));

                if sc.enabled == 0 {
                    // Unfocused (or toolkit reporting 0 oddly) — hide and drop.
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
