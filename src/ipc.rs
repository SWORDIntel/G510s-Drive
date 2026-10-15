use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use serde::{Deserialize, Serialize};

pub fn socket_path() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    let run_dir = PathBuf::from(format!("/run/user/{uid}"));
    if run_dir.is_dir() {
        run_dir.join("g510d.sock")
    } else {
        PathBuf::from("/tmp/g510d.sock")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum IpcRequest {
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "toast")]
    Toast {
        message: String,
        #[serde(default = "default_toast_duration")]
        duration_ms: u64,
    },
    #[serde(rename = "set_page")]
    SetPage { number: usize },
    #[serde(rename = "register_page")]
    RegisterPage {
        id: String,
        title: String,
        #[serde(default)]
        line1: String,
        #[serde(default)]
        line2: String,
        #[serde(default)]
        line3: String,
        #[serde(default)]
        bar: Option<f32>,
    },
    #[serde(rename = "update_page")]
    UpdatePage {
        id: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        line1: Option<String>,
        #[serde(default)]
        line2: Option<String>,
        #[serde(default)]
        line3: Option<String>,
        #[serde(default)]
        bar: Option<f32>,
    },
    #[serde(rename = "unregister_page")]
    UnregisterPage { id: String },
    #[serde(rename = "list_pages")]
    ListPages,
    #[serde(rename = "alert")]
    Alert {
        #[serde(default = "default_alert_mode")]
        mode: String,
        #[serde(default = "default_alert_color")]
        color: String,
        #[serde(default = "default_alert_times")]
        times: u32,
        #[serde(default = "default_alert_duration")]
        duration: f32,
    },
    #[serde(rename = "get_status")]
    GetStatus,
}

fn default_toast_duration() -> u64 {
    2500
}
fn default_alert_mode() -> String {
    "flash".into()
}
fn default_alert_color() -> String {
    "#ff0000".into()
}
fn default_alert_times() -> u32 {
    3
}
fn default_alert_duration() -> f32 {
    2.5
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl IpcResponse {
    pub fn success(data: Option<serde_json::Value>) -> Self {
        Self {
            ok: true,
            error: None,
            data,
        }
    }

    pub fn err(msg: &str) -> Self {
        Self {
            ok: false,
            error: Some(msg.to_string()),
            data: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DynamicPage {
    pub id: String,
    pub title: String,
    pub line1: String,
    pub line2: String,
    pub line3: String,
    pub bar: Option<f32>,
}

#[derive(Clone, Default)]
pub struct IpcState {
    pub dynamic_pages: Arc<Mutex<Vec<DynamicPage>>>,
    pub pending_toast: Arc<Mutex<Option<(String, Duration)>>>,
    pub pending_page: Arc<Mutex<Option<usize>>>,
    pub pending_alert: Arc<Mutex<Option<crate::alerts::RgbAlert>>>,
    pub active_page: Arc<AtomicUsize>,
}

pub struct IpcServer {
    state: IpcState,
    socket: PathBuf,
}

impl IpcServer {
    pub fn new(state: IpcState) -> Self {
        let socket = socket_path();
        Self { state, socket }
    }

    pub fn start(&self, running: Arc<AtomicBool>) -> io::Result<thread::JoinHandle<()>> {
        if self.socket.exists() {
            let _ = fs::remove_file(&self.socket);
        }

        let listener = UnixListener::bind(&self.socket)?;
        listener.set_nonblocking(true)?;

        // Ensure user rw permissions (0600)
        let _ = fs::set_permissions(&self.socket, fs::Permissions::from_mode(0o600));

        let state = self.state.clone();
        let sock_path = self.socket.clone();

        let handle = thread::spawn(move || {
            eprintln!("[ipc] Listening on {}", sock_path.display());

            while running.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let client_state = state.clone();
                        thread::spawn(move || {
                            handle_client(stream, client_state);
                        });
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(e) => {
                        eprintln!("[ipc] Accept error: {e}");
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }

            let _ = fs::remove_file(&sock_path);
            eprintln!("[ipc] Stopped socket listener");
        });

        Ok(handle)
    }
}

fn handle_client(stream: UnixStream, state: IpcState) {
    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(_) => return,
    };
    let reader = BufReader::new(stream);

    for line in reader.lines().flatten() {
        if line.trim().is_empty() {
            continue;
        }

        let resp = match serde_json::from_str::<IpcRequest>(&line) {
            Ok(req) => process_request(req, &state),
            Err(e) => IpcResponse::err(&format!("Invalid JSON request: {e}")),
        };

        let mut payload = match serde_json::to_vec(&resp) {
            Ok(bytes) => bytes,
            Err(_) => b"{\"ok\":false,\"error\":\"Serialization error\"}".to_vec(),
        };
        payload.push(b'\n');
        if writer.write_all(&payload).is_err() {
            break;
        }
    }
}

pub fn process_request(req: IpcRequest, state: &IpcState) -> IpcResponse {
    match req {
        IpcRequest::Ping => IpcResponse::success(Some(serde_json::json!({"status": "pong"}))),
        IpcRequest::Toast { message, duration_ms } => {
            if let Ok(mut t) = state.pending_toast.lock() {
                *t = Some((message, Duration::from_millis(duration_ms.max(500))));
            }
            IpcResponse::success(None)
        }
        IpcRequest::SetPage { number } => {
            if let Ok(mut p) = state.pending_page.lock() {
                *p = Some(number);
            }
            IpcResponse::success(None)
        }
        IpcRequest::RegisterPage {
            id,
            title,
            line1,
            line2,
            line3,
            bar,
        } => {
            if id.trim().is_empty() {
                return IpcResponse::err("Page ID cannot be empty");
            }
            let title_clean = if title.trim().is_empty() {
                id.to_uppercase()
            } else {
                title.trim().to_string()
            };
            if let Ok(mut pages) = state.dynamic_pages.lock() {
                if let Some(existing) = pages.iter_mut().find(|p| p.id == id) {
                    existing.title = title_clean;
                    existing.line1 = line1;
                    existing.line2 = line2;
                    existing.line3 = line3;
                    existing.bar = bar;
                } else {
                    pages.push(DynamicPage {
                        id,
                        title: title_clean,
                        line1,
                        line2,
                        line3,
                        bar,
                    });
                }
                IpcResponse::success(None)
            } else {
                IpcResponse::err("State lock error")
            }
        }
        IpcRequest::UpdatePage {
            id,
            title,
            line1,
            line2,
            line3,
            bar,
        } => {
            if let Ok(mut pages) = state.dynamic_pages.lock() {
                if let Some(existing) = pages.iter_mut().find(|p| p.id == id) {
                    if let Some(t) = title {
                        if !t.trim().is_empty() {
                            existing.title = t.trim().to_string();
                        }
                    }
                    if let Some(l) = line1 {
                        existing.line1 = l;
                    }
                    if let Some(l) = line2 {
                        existing.line2 = l;
                    }
                    if let Some(l) = line3 {
                        existing.line3 = l;
                    }
                    if bar.is_some() {
                        existing.bar = bar;
                    }
                    IpcResponse::success(None)
                } else {
                    IpcResponse::err(&format!("No registered page with ID '{id}'"))
                }
            } else {
                IpcResponse::err("State lock error")
            }
        }
        IpcRequest::UnregisterPage { id } => {
            if let Ok(mut pages) = state.dynamic_pages.lock() {
                let initial = pages.len();
                pages.retain(|p| p.id != id);
                if pages.len() < initial {
                    IpcResponse::success(None)
                } else {
                    IpcResponse::err(&format!("No page with ID '{id}' found to remove"))
                }
            } else {
                IpcResponse::err("State lock error")
            }
        }
        IpcRequest::ListPages => {
            if let Ok(pages) = state.dynamic_pages.lock() {
                IpcResponse::success(Some(serde_json::to_value(&*pages).unwrap_or_default()))
            } else {
                IpcResponse::err("State lock error")
            }
        }
        IpcRequest::Alert {
            mode,
            color,
            times,
            duration,
        } => {
            let alert = if mode == "pulse" {
                crate::alerts::RgbAlert::Pulse { color, duration }
            } else {
                crate::alerts::RgbAlert::Flash { color, times }
            };
            if let Ok(mut a) = state.pending_alert.lock() {
                *a = Some(alert);
            }
            IpcResponse::success(None)
        }
        IpcRequest::GetStatus => {
            let pages_count = state.dynamic_pages.lock().map_or(0, |p| p.len());
            let current_page = state.active_page.load(Ordering::SeqCst) + 1;
            IpcResponse::success(Some(serde_json::json!({
                "daemon": "g510d",
                "version": env!("CARGO_PKG_VERSION"),
                "active_page": current_page,
                "dynamic_pages_count": pages_count,
                "socket": socket_path().to_string_lossy(),
            })))
        }
    }
}

/// Send a synchronous request to the g510d daemon socket
pub fn send_ipc_request(req: &IpcRequest) -> io::Result<IpcResponse> {
    let sock = socket_path();
    let mut stream = UnixStream::connect(&sock)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    let mut payload = serde_json::to_vec(req).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    payload.push(b'\n');

    stream.write_all(&payload)?;

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;

    serde_json::from_str(&response_line)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_request_serialization() {
        let toast = IpcRequest::Toast {
            message: "Hello".into(),
            duration_ms: 1000,
        };
        let serialized = serde_json::to_string(&toast).unwrap();
        assert!(serialized.contains("\"cmd\":\"toast\""));

        let reg = IpcRequest::RegisterPage {
            id: "disk".into(),
            title: "DISK".into(),
            line1: "Root: 45GB free".into(),
            line2: "Home: 120GB free".into(),
            line3: "Var: 10GB free".into(),
            bar: Some(75.5),
        };
        let s2 = serde_json::to_string(&reg).unwrap();
        assert!(s2.contains("\"cmd\":\"register_page\""));
        assert!(s2.contains("\"id\":\"disk\""));

        let list = IpcRequest::ListPages;
        assert_eq!(serde_json::to_string(&list).unwrap(), "{\"cmd\":\"list_pages\"}");
    }

    #[test]
    fn test_process_request_lifecycle() {
        let state = IpcState::default();

        // 1. Ping
        let res_ping = process_request(IpcRequest::Ping, &state);
        assert!(res_ping.ok);

        // 2. Register page
        let res_reg = process_request(
            IpcRequest::RegisterPage {
                id: "test1".into(),
                title: "TEST".into(),
                line1: "Line 1".into(),
                line2: "Line 2".into(),
                line3: "Line 3".into(),
                bar: Some(50.0),
            },
            &state,
        );
        assert!(res_reg.ok);
        assert_eq!(state.dynamic_pages.lock().unwrap().len(), 1);

        // 3. Update page
        let res_up = process_request(
            IpcRequest::UpdatePage {
                id: "test1".into(),
                title: None,
                line1: Some("Updated L1".into()),
                line2: None,
                line3: None,
                bar: Some(80.0),
            },
            &state,
        );
        assert!(res_up.ok);
        let pages = state.dynamic_pages.lock().unwrap();
        assert_eq!(pages[0].line1, "Updated L1");
        assert_eq!(pages[0].bar, Some(80.0));
        drop(pages);

        // 4. List pages
        let res_list = process_request(IpcRequest::ListPages, &state);
        assert!(res_list.ok);
        let data = res_list.data.unwrap();
        assert!(data.is_array());
        assert_eq!(data.as_array().unwrap().len(), 1);

        // 5. Unregister page
        let res_unreg = process_request(IpcRequest::UnregisterPage { id: "test1".into() }, &state);
        assert!(res_unreg.ok);
        assert_eq!(state.dynamic_pages.lock().unwrap().len(), 0);
    }
}
