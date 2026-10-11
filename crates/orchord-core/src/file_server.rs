//! The live shared folder: a small WebDAV server that lets a Mac use a Windows folder as a
//! network drive.
//!
//! The server listens on this PC's loopback address only, so nothing else on the network can
//! reach it and no firewall or administrator rights are involved. QEMU's built-in network
//! makes the PC's loopback address appear to the Mac as 10.0.2.2, so the Mac connects to
//! `http://10.0.2.2:<port>/`. A file dropped in the folder on Windows shows up on the Mac at
//! once, and the other way round.
//!
//! The server also hands out one small script, at `/orchord-setup.sh`, which makes the Mac
//! connect the folder by itself every time its user logs in.

use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr, TcpListener as StdTcpListener};
use std::path::Path;
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use dav_server::body::Body;
use dav_server::fakels::FakeLs;
use dav_server::localfs::LocalFs;
use dav_server::{DavHandler, DavMethodSet};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// The address at which a Mac sees this PC through QEMU's built-in network.
pub const HOST_AS_SEEN_BY_MAC: &str = "10.0.2.2";
/// The folder in the Mac user's home where the shared folder appears.
pub const MAC_FOLDER_NAME: &str = "Windows";
const SETUP_SCRIPT_PATH: &str = "/orchord-setup.sh";

/// A running file server. Dropping it stops the server.
pub struct FileServer {
    port: u16,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl FileServer {
    /// Serves `folder` (created if missing) on `port` of the loopback address. Port 0 asks
    /// Windows for any free port; use `port()` to learn which one it gave.
    pub fn start(folder: &Path, port: u16) -> Result<FileServer> {
        std::fs::create_dir_all(folder).with_context(|| format!("creating the shared folder {}", folder.display()))?;
        // Bound here, not on the server's thread, so that a port in use is reported to the caller.
        let listener = StdTcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).with_context(|| format!("opening port {port} for the shared folder"))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        // macOS writes "._" companion files and .DS_Store everywhere; the last flag keeps those out of the Windows folder.
        let handler = DavHandler::builder().filesystem(LocalFs::new(folder, false, false, true)).locksystem(FakeLs::new()).methods(DavMethodSet::all()).build_handler();
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().thread_name("orchord-file-server").build()?;
        let (stop, stopped) = oneshot::channel();
        let thread = std::thread::spawn(move || runtime.block_on(serve(listener, handler, port, stopped)));
        Ok(FileServer { port, stop: Some(stop), thread: Some(thread) })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The address to give the Mac, for Finder's Connect to Server.
    pub fn mac_url(&self) -> String {
        mac_url(self.port)
    }
}

impl Drop for FileServer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn mac_url(port: u16) -> String {
    format!("http://{HOST_AS_SEEN_BY_MAC}:{port}/")
}

/// The one line to run in the Mac's Terminal to make it connect the folder at every login.
pub fn setup_command(port: u16) -> String {
    format!("curl -s {HOST_AS_SEEN_BY_MAC}:{port}{SETUP_SCRIPT_PATH} | sh")
}

async fn serve(listener: StdTcpListener, handler: DavHandler, port: u16, mut stopped: oneshot::Receiver<()>) {
    let Ok(listener) = TcpListener::from_std(listener) else { return };
    loop {
        let stream = tokio::select! {
            _ = &mut stopped => return,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => stream,
                Err(_) => continue,
            },
        };
        let handler = handler.clone();
        tokio::spawn(async move {
            let service = service_fn(move |request: Request<hyper::body::Incoming>| {
                let handler = handler.clone();
                async move { Ok::<_, Infallible>(respond(&handler, port, request).await) }
            });
            let _ = http1::Builder::new().serve_connection(TokioIo::new(stream), service).await;
        });
    }
}

async fn respond(handler: &DavHandler, port: u16, request: Request<hyper::body::Incoming>) -> Response<Body> {
    if request.method() == Method::GET && request.uri().path() == SETUP_SCRIPT_PATH {
        return Response::builder().header("Content-Type", "text/x-shellscript").body(Body::from(setup_script(port))).expect("a plain response is always valid");
    }
    handler.handle(request).await
}

/// The script behind `setup_command`. It installs a per-user launch agent that mounts the
/// shared folder at `~/Windows` at login and again whenever the connection is lost, and
/// starts it straight away.
fn setup_script(port: u16) -> String {
    let url = mac_url(port);
    format!(
        r#"#!/bin/sh
# Installed by Orchord: connects the folder shared with Windows at every login.
AGENT="$HOME/Library/LaunchAgents/org.orchord.shared-folder.plist"
mkdir -p "$HOME/Library/LaunchAgents" "$HOME/{folder}"
cat > "$AGENT" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>org.orchord.shared-folder</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/sh</string>
        <string>-c</string>
        <string>mount | grep -q " on $HOME/{folder} " || /sbin/mount_webdav -S {url} "$HOME/{folder}"</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>StartInterval</key>
    <integer>20</integer>
</dict>
</plist>
PLIST
launchctl unload "$AGENT" 2>/dev/null
launchctl load "$AGENT"
echo "Orchord: the folder shared with Windows is now the {folder} folder in your home folder."
"#,
        folder = MAC_FOLDER_NAME,
        url = url
    )
}

#[cfg(test)]
mod tests {
    use super::{FileServer, setup_command};
    use std::io::{Read, Write};
    use std::net::TcpStream;

    fn request(port: u16, text: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(text.as_bytes()).unwrap();
        let mut answer = String::new();
        let _ = stream.read_to_string(&mut answer);
        answer
    }

    #[test]
    fn a_file_put_on_the_server_lands_in_the_folder_and_can_be_listed_and_read() {
        let folder = std::env::temp_dir().join(format!("orchord-share-test-{}", std::process::id()));
        let server = FileServer::start(&folder, 0).unwrap();
        let port = server.port();

        let put = request(port, "PUT /hello.txt HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello");
        assert!(put.starts_with("HTTP/1.1 201"), "{put}");
        assert_eq!(std::fs::read_to_string(folder.join("hello.txt")).unwrap(), "hello");

        std::fs::write(folder.join("from-windows.txt"), "made on Windows").unwrap();
        let list = request(port, "PROPFIND / HTTP/1.1\r\nHost: x\r\nDepth: 1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        assert!(list.starts_with("HTTP/1.1 207"), "{list}");
        assert!(list.contains("from-windows.txt") && list.contains("hello.txt"), "{list}");

        let get = request(port, "GET /from-windows.txt HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
        assert!(get.ends_with("made on Windows"), "{get}");

        // macOS only mounts a server read-write if it says it supports locking (class 2).
        let options = request(port, "OPTIONS / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
        assert!(options.to_lowercase().contains("dav: 1,2"), "{options}");

        let script = request(port, "GET /orchord-setup.sh HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
        assert!(script.contains(&format!("mount_webdav -S http://10.0.2.2:{port}/")), "{script}");
        assert_eq!(setup_command(port), format!("curl -s 10.0.2.2:{port}/orchord-setup.sh | sh"));

        drop(server);
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err(), "the server should have stopped");
        let _ = std::fs::remove_dir_all(folder);
    }
}
