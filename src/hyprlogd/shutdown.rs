use futures_util::StreamExt;
use tokio::signal;
use tokio::task::JoinHandle;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::{proxy, Connection};

use crate::log_writer::LogMsg;

#[proxy(
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1",
    interface = "org.freedesktop.login1.Manager"
)]
trait Manager {
    #[zbus(signal)]
    fn PrepareForShutdown(starting: bool);
}

pub async fn try_spawn_logind_shutdown_watcher(
    sender: tokio::sync::mpsc::Sender<LogMsg>,
) -> Option<JoinHandle<()>> {
    let conn = Connection::system().await.ok()?;

    // Check that logind exists
    let dbus = DBusProxy::new(&conn).await.ok()?;
    let name: BusName<'_> = "org.freedesktop.login1".try_into().ok()?;
    if !dbus.name_has_owner(name).await.ok()? {
        return None;
    }

    // Subscribe to the typed signal
    let mgr = ManagerProxy::new(&conn).await.ok()?;
    let mut stream = mgr.receive_PrepareForShutdown().await.ok()?;

    // listen for and handle prepare for shutdown command
    Some(tokio::spawn(async move {
        while let Some(sig) = stream.next().await {
            let Ok(args) = sig.args() else { continue };
            if args.starting {
                let _ = sender
                    .send(LogMsg::Line {
                        ts: chrono::Utc::now().timestamp_millis(),
                        class: "SYSTEM".into(),
                        title: "shutdown".into(),
                    })
                    .await;
                let _ = sender.send(LogMsg::Shutdown).await;
                break;
            }
        }
    }))
}

// fallback, not a reliable method of detecting shutdown
pub async fn wait_for_shutdown_signal() {
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate()).unwrap();

    tokio::select! {
        _ = signal::ctrl_c() => {},
        _ = sigterm.recv() => {},
    }
}
