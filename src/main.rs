fn main() -> eframe::Result {
    if std::env::args().any(|arg| arg == "--detect") {
        match simple_mosh_gui::mosh::find() {
            Some(server) => {
                let version = server
                    .version
                    .as_deref()
                    .map(|v| format!("mosh {v}"))
                    .unwrap_or_else(|| "version unknown".to_string());
                println!("found {} ({version})", server.path.display());
                std::process::exit(0);
            }
            None => {
                eprintln!("mosh-server not found in PATH or common locations");
                std::process::exit(1);
            }
        }
    }
    simple_mosh_gui::run_desktop()
}
