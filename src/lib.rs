//! Tiny GUI to start `mosh-server` and surface the connect details.

pub mod mosh;

use std::time::{Duration, Instant};

use eframe::egui;

pub struct App {
    server: Option<mosh::MoshServer>,
    colors_256: bool,
    port_text: String,
    ip_text: String,
    session: Option<mosh::Session>,
    log: Vec<String>,
    started: Option<Instant>,
    start_error: Option<String>,
    ticks: u8,
}

impl App {
    pub fn new() -> Self {
        Self {
            server: mosh::find(),
            colors_256: true,
            port_text: String::new(),
            ip_text: local_ip().unwrap_or_default(),
            session: None,
            log: Vec::new(),
            started: None,
            start_error: None,
            ticks: 0,
        }
    }

    fn start(&mut self) {
        let Some(server) = self.server.as_ref() else {
            return;
        };
        if let Some(err) = &server.exec_error {
            self.start_error = Some(format!("cannot execute {}: {err}", server.path.display()));
            return;
        }
        let opts = mosh::StartOptions {
            colors_256: self.colors_256,
            port: self.port_text.trim().parse().ok(),
        };
        match mosh::Session::start(&server.path, &opts) {
            Ok(session) => {
                self.log.clear();
                self.session = Some(session);
                self.started = Some(Instant::now());
                self.start_error = None;
            }
            Err(err) => self.start_error = Some(err.to_string()),
        }
    }

    fn poll(&mut self) {
        let mut ended = false;
        if let Some(session) = self.session.as_mut() {
            let lines = session.drain();
            self.log.extend(lines);
            ended = !session.is_running();
        }
        if ended {
            self.log.push(
                "session ended (mosh-server exits by itself if no client connects within 60s)"
                    .into(),
            );
            self.session = None;
            self.started = None;
        }
    }

    fn connect_line(&self) -> Option<String> {
        let session = self.session.as_ref()?;
        let (port, key) = (session.port?, session.key.as_deref()?);
        Some(format!("MOSH CONNECT {port} {key}"))
    }

    fn client_command(&self) -> Option<String> {
        let session = self.session.as_ref()?;
        let (port, key) = (session.port?, session.key.as_deref()?);
        let ip = match self.ip_text.trim() {
            "" => "<YOUR-IP>",
            ip => ip,
        };
        Some(format!("MOSH_KEY={key} mosh-client {ip} {port}"))
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn copy_button(ui: &mut egui::Ui, text: &str) {
    if ui.button("Copy").clicked() {
        ui.ctx().copy_text(text.to_string());
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        // Panels size themselves from the previous frame; force a few repaints so
        // the first frames settle without waiting for user input.
        if self.ticks < 3 {
            self.ticks += 1;
            ui.ctx().request_repaint();
        }
        if self.session.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }

        egui::Panel::top("header").show(ui, |ui| {
            if cfg!(target_os = "android") {
                ui.add_space(28.0); // keep clear of the Android status bar
            }
            ui.add_space(4.0);
            match &self.server {
                Some(server) => {
                    let version = server
                        .version
                        .as_deref()
                        .map(|v| format!(" (mosh {v})"))
                        .unwrap_or_default();
                    ui.label(format!(
                        "mosh-server{version} found at {}",
                        server.path.display()
                    ));
                }
                None => {
                    ui.label("mosh-server not found.");
                    ui.horizontal(|ui| {
                        let hint = install_hint();
                        ui.monospace(hint);
                        copy_button(ui, hint);
                    });
                }
            }
            if let Some(err) = self
                .server
                .as_ref()
                .and_then(|server| server.exec_error.as_ref())
            {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!("found, but cannot be executed here: {err}"),
                );
                ui.label("On Android, run `mosh-server new` inside Termux instead.");
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Re-detect").clicked() {
                    self.server = mosh::find();
                }
                ui.checkbox(&mut self.colors_256, "256 colors");
                ui.label("port:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.port_text)
                        .hint_text("auto")
                        .desired_width(60.0),
                );
            });
            ui.horizontal(|ui| {
                let can_start = self.session.is_none()
                    && self.server.as_ref().is_some_and(|s| s.exec_error.is_none());
                if ui
                    .add_enabled(can_start, egui::Button::new("Start"))
                    .clicked()
                {
                    self.start();
                }
                let can_stop = self.session.is_some();
                if ui
                    .add_enabled(can_stop, egui::Button::new("Stop"))
                    .clicked()
                {
                    if let Some(session) = self.session.as_mut() {
                        session.stop();
                    }
                    self.session = None;
                    self.started = None;
                }
                if let Some(started) = self.started {
                    let secs = started.elapsed().as_secs();
                    ui.label(format!("running {:02}:{:02}", secs / 60, secs % 60));
                }
            });
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(connect) = self.connect_line() {
                ui.horizontal(|ui| {
                    ui.monospace(&connect);
                    copy_button(ui, &connect);
                });
            }
            if let Some(command) = self.client_command() {
                ui.horizontal(|ui| {
                    ui.monospace(&command);
                    copy_button(ui, &command);
                });
                ui.horizontal(|ui| {
                    ui.label("server IP (for the client):");
                    ui.add(egui::TextEdit::singleline(&mut self.ip_text).desired_width(140.0));
                });
            }
            if let Some(err) = &self.start_error {
                ui.colored_label(egui::Color32::RED, err);
            }
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for line in &self.log {
                    ui.monospace(line);
                }
            });
        });
    }
}

fn install_hint() -> &'static str {
    if cfg!(target_os = "android") {
        "install inside Termux: pkg install mosh"
    } else if cfg!(target_os = "macos") {
        "install with Homebrew: brew install mosh"
    } else {
        "install mosh with your package manager"
    }
}

/// Best-effort local IP: the source address of the default route.
/// A connected UDP socket sends no packets.
fn local_ip() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

pub fn run_desktop() -> eframe::Result {
    eframe::run_native(
        "Simple Mosh GUI",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    let options = eframe::NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };
    eframe::run_native(
        "Simple Mosh GUI",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
    .unwrap();
}
