//! Live end-to-end smoke check: starts a real `mosh-server`, reads the connect
//! line, then stops the session again.
//!
//! `cargo run --example live_smoke` — requires `mosh-server`; exits non-zero on
//! failure. The unit tests cover parsing; this covers the parts that need a
//! real process (detach, pid tracking, stop).

use std::time::{Duration, Instant};

use simple_mosh_gui::mosh;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = mosh::find().ok_or("mosh-server not found")?;
    println!("found {}", server.path.display());

    let opts = mosh::StartOptions {
        colors_256: true,
        port: None,
    };
    let mut session = mosh::Session::start(&server.path, &opts)?;

    let deadline = Instant::now() + Duration::from_secs(10);
    while (session.port.is_none() || session.detached_pid.is_none()) && Instant::now() < deadline {
        for line in session.drain() {
            println!("{line}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let port = session.port.ok_or("no MOSH CONNECT line seen")?;
    let pid = session.detached_pid.ok_or("no detached pid seen")?;
    if !session.is_running() {
        return Err("server not reported as running".into());
    }
    println!("port={port} pid={pid}");

    session.stop();
    let deadline = Instant::now() + Duration::from_secs(5);
    while session.is_running() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    if session.is_running() {
        return Err(format!("mosh-server pid {pid} still running after stop").into());
    }
    println!("stopped cleanly");
    Ok(())
}
