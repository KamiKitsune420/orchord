//! A small blocking client for QEMU's machine protocol (QMP): one JSON object per line.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

/// How long a struck key stays down, and how long modifiers are given to register around it.
const KEY_HOLD: Duration = Duration::from_millis(60);
const KEY_SETTLE: Duration = Duration::from_millis(20);

/// The largest value of either axis in `QmpClient::pointer_move`.
pub const POINTER_MAX: u16 = 0x7FFF;

pub struct QmpClient {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    /// Asynchronous events that arrived while waiting for a command's reply.
    pub pending_events: Vec<Value>,
}

/// Whether the virtual CPUs are executing, as reported by `query-status`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunState {
    pub running: bool,
    pub status: String,
}

impl QmpClient {
    /// Connects to a QMP socket on 127.0.0.1, retrying until `timeout` because QEMU
    /// opens the socket a moment after the process starts.
    pub fn connect(port: u16, timeout: Duration) -> Result<Self> {
        let address = SocketAddr::from(([127, 0, 0, 1], port));
        let deadline = Instant::now() + timeout;
        let stream = loop {
            match TcpStream::connect_timeout(&address, Duration::from_millis(500)) {
                Ok(stream) => break stream,
                Err(error) if Instant::now() >= deadline => return Err(error).context("connecting to QEMU's control socket"),
                Err(_) => std::thread::sleep(Duration::from_millis(200)),
            }
        };
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let mut client = QmpClient { reader: BufReader::new(stream.try_clone()?), writer: stream, pending_events: Vec::new() };
        client.read_message().context("reading QEMU's greeting")?;
        client.execute("qmp_capabilities", None)?;
        Ok(client)
    }

    fn read_message(&mut self) -> Result<Value> {
        let mut line = String::new();
        if self.reader.read_line(&mut line)? == 0 {
            bail!("QEMU closed its control socket");
        }
        serde_json::from_str(&line).with_context(|| format!("parsing QMP message {line:?}"))
    }

    pub fn execute(&mut self, command: &str, arguments: Option<Value>) -> Result<Value> {
        let mut request = json!({ "execute": command });
        if let Some(arguments) = arguments {
            request["arguments"] = arguments;
        }
        writeln!(self.writer, "{request}")?;
        loop {
            let message = self.read_message()?;
            if message.get("event").is_some() {
                self.pending_events.push(message);
            } else if let Some(result) = message.get("return") {
                return Ok(result.clone());
            } else if let Some(error) = message.get("error") {
                return Err(anyhow!("QEMU refused {command}: {}", error["desc"].as_str().unwrap_or("unknown error")));
            }
        }
    }

    pub fn run_state(&mut self) -> Result<RunState> {
        let reply = self.execute("query-status", None)?;
        Ok(RunState { running: reply["running"].as_bool().unwrap_or(false), status: reply["status"].as_str().unwrap_or("unknown").to_string() })
    }

    /// Presses and releases a chord of keys, given as QEMU key names such as `meta_l` and `f5`.
    /// The last key is the one struck; those before it are held around it, like Shift.
    /// Returns once the keys are up again, a little over a tenth of a second later.
    ///
    /// QEMU's own `send-key` command is not used: it queues every press with a delay, and
    /// text typed faster than the queue drains overflows it, which loses key releases and
    /// leaves keys stuck down in the Mac.
    pub fn send_keys(&mut self, keys: &[&str]) -> Result<()> {
        let Some((struck, held)) = keys.split_last() else { return Ok(()) };
        let event = |key: &str, down: bool| json!({ "type": "key", "data": { "down": down, "key": { "type": "qcode", "data": key } } });
        let mut send = |events: Vec<Value>, pause: Duration| -> Result<()> {
            if !events.is_empty() {
                self.execute("input-send-event", Some(json!({ "events": events })))?;
                std::thread::sleep(pause);
            }
            Ok(())
        };
        send(held.iter().map(|key| event(key, true)).collect(), KEY_SETTLE)?;
        send(vec![event(struck, true)], KEY_HOLD)?;
        send(vec![event(struck, false)], KEY_SETTLE)?;
        send(held.iter().rev().map(|key| event(key, false)).collect(), KEY_SETTLE)
    }

    /// Presses or releases one key, given as a PC scancode with bit 7 set for extended keys
    /// (the ones a real keyboard prefixes with E0, such as the arrows and the Windows key).
    pub fn key_event(&mut self, scancode: u8, down: bool) -> Result<()> {
        let event = json!({ "type": "key", "data": { "down": down, "key": { "type": "number", "data": scancode } } });
        self.execute("input-send-event", Some(json!({ "events": [event] }))).map(|_| ())
    }

    /// Moves the Mac's pointer to an absolute place on its screen. Each axis runs from 0 at
    /// the left or top edge to `POINTER_MAX` at the right or bottom edge.
    pub fn pointer_move(&mut self, x: u16, y: u16) -> Result<()> {
        let events = json!([{ "type": "abs", "data": { "axis": "x", "value": x.min(POINTER_MAX) } }, { "type": "abs", "data": { "axis": "y", "value": y.min(POINTER_MAX) } }]);
        self.execute("input-send-event", Some(json!({ "events": events }))).map(|_| ())
    }

    /// Presses or releases a pointer button, given as a QEMU button name such as `left` or `wheel-up`.
    pub fn pointer_button(&mut self, button: &str, down: bool) -> Result<()> {
        let event = json!({ "type": "btn", "data": { "down": down, "button": button } });
        self.execute("input-send-event", Some(json!({ "events": [event] }))).map(|_| ())
    }

    /// Saves the guest's screen as a PPM image.
    pub fn screendump(&mut self, path: &std::path::Path) -> Result<()> {
        self.execute("screendump", Some(json!({ "filename": path.to_string_lossy() }))).map(|_| ())
    }

    /// Freezes the virtual CPUs. The Mac keeps its memory and resumes exactly where it was.
    pub fn pause(&mut self) -> Result<()> {
        self.execute("stop", None).map(|_| ())
    }

    pub fn resume(&mut self) -> Result<()> {
        self.execute("cont", None).map(|_| ())
    }

    pub fn quit(&mut self) -> Result<()> {
        self.execute("quit", None).map(|_| ())
    }
}
