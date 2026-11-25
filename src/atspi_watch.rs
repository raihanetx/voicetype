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

pub async fn run_watcher(
    conn: AccessibilityConnection,
    overlay: crate::overlay::Overlay,
    focused: Arc<Mutex<Option<ObjectRef>>>,
    window_id: Arc<Mutex<Option<String>>>,
    auto_record: bool,
    auto_tx: mpsc::Sender<()>,
) {
    if let Err(e) = conn.register_event::<StateChangedEvent>().await {
        tracing::error!("failed to register AT-SPI events: {e}");
    }
    let mut stream = conn.event_stream();
    std::pin::pin!(&mut stream);

    while let Some(result) = stream.next().await {
        match result {
            Ok(ev) => {
                let sc = match StateChangedEvent::try_from(ev) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                if sc.state != State::Focused {
                    continue;
                }
                if sc.enabled == 0 {
                    overlay.hide();
                    *focused.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    continue;
                }
                let obj = sc.item.clone();
                let acc = match obj.as_accessible_proxy(conn.connection()).await {
                    Ok(a) => a,
                    Err(_) => continue,
                };
                let role = acc.get_role().await.unwrap_or(atspi::Role::Unknown);
                let is_text = matches!(role, Role::Entry | Role::PasswordText | Role::Text | Role::Terminal);
                if !is_text {
                    overlay.hide();
                    *focused.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    continue;
                }
                let comp = match ComponentProxy::builder(conn.connection())
                    .destination(obj.name.as_str()) {
                    Ok(b) => b, Err(_) => continue,
                };
                let comp = match comp.path(obj.path.as_str()) { Ok(b) => b, Err(_) => continue };
                let comp = match comp.cache_properties(atspi::zbus::proxy::CacheProperties::No).build().await { Ok(c) => c, Err(_) => continue };
                if let Ok((x, y, w, _h)) = comp.get_extents(CoordType::Screen).await {
                    overlay.show(x + w + 4, y);
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
            }
            Err(e) => {
                tracing::error!("AT-SPI stream error: {e}");
            }
        }
    }
    tracing::warn!("AT-SPI event stream ended");
}
