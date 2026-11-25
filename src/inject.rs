use atspi::proxy::accessible::ObjectRefExt;
use atspi::proxy::{editable_text::EditableTextProxy, text::TextProxy};
use atspi::ObjectRef;
use atspi::zbus::Connection;

pub async fn inject_text(conn: &Connection, target: &ObjectRef, text: &str, window_id: Option<&str>) -> Result<(), String> {
    // Try AT-SPI EditableText first.
    let acc = target.as_accessible_proxy(conn).await.map_err(|e| e.to_string())?;
    let _ = acc; // ensure accessible
    let et_builder = EditableTextProxy::builder(conn)
        .destination(target.name.as_str())
        .map_err(|e| e.to_string())?;
    let et_builder = et_builder.path(target.path.as_str()).map_err(|e| e.to_string())?;
    let et = et_builder.cache_properties(atspi::zbus::proxy::CacheProperties::No).build().await.map_err(|e| e.to_string())?;

    // get caret offset via Text proxy
    let t_builder = TextProxy::builder(conn)
        .destination(target.name.as_str())
        .map_err(|e| e.to_string())?;
    let t_builder = t_builder.path(target.path.as_str()).map_err(|e| e.to_string())?;
    let tp = t_builder.cache_properties(atspi::zbus::proxy::CacheProperties::No).build().await.map_err(|e| e.to_string())?;

    let offset = tp.caret_offset().await.unwrap_or(0);
    if et.insert_text(offset, text, text.len() as i32).await.is_ok() {
        // move caret
        let _ = tp.set_caret_offset(offset + text.chars().count() as i32).await;
        return Ok(());
    }

    // Fallback: xdotool typing (X11). --delay avoids dropped chars in terminals.
    let mut cmd = std::process::Command::new("xdotool");
    if let Some(wid) = window_id {
        cmd.arg("type").arg("--window").arg(wid);
    } else {
        cmd.arg("type");
    }
    let status = cmd
        .arg("--delay").arg("12")
        .arg("--clearmodifiers")
        .arg("--")
        .arg(text)
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err("injection failed (atspi + xdotool)".into()),
    }
}
