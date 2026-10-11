//! Tiny GUI to start `mosh-server` and surface the connect details.

pub mod mosh;
mod theme;

use std::time::{Duration, Instant};

use eframe::egui;

use theme::{AMBER, DIM, GREEN, RED, Kind};

/// How long a copy chip says "Copied".
const COPIED: Duration = Duration::from_millis(1200);

pub struct App {
    server: Option<mosh::MoshServer>,
    colors_256: bool,
    port_text: String,
    ip_text: String,
    session: Option<mosh::Session>,
    log: Vec<String>,
    started: Option<Instant>,
    start_error: Option<String>,
    copied: Option<(u8, Instant)>,
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
            copied: None,
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

    fn stop(&mut self) {
        if let Some(session) = self.session.as_mut() {
            session.stop();
        }
        self.session = None;
        self.started = None;
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

    /// What the window is doing, in three words or less.
    fn state(&self) -> (String, egui::Color32) {
        match &self.server {
            None => ("mosh-server missing".to_owned(), RED),
            Some(server) if server.exec_error.is_some() => ("cannot execute".to_owned(), RED),
            Some(_) if self.session.is_some() => {
                let secs = self.started.map_or(0, |t| t.elapsed().as_secs());
                (format!("running {:02}:{:02}", secs / 60, secs % 60), AMBER)
            }
            Some(_) => ("ready".to_owned(), GREEN),
        }
    }

    /// Copy a line, and say so for a moment so the click feels answered.
    fn copy_chip(&mut self, ui: &mut egui::Ui, text: &str, slot: u8) {
        let copied = self
            .copied
            .is_some_and(|(s, at)| s == slot && at.elapsed() < COPIED);
        let kind = if copied { Kind::Glass } else { Kind::Quiet };
        if theme::button(ui, if copied { "Copied" } else { "Copy" }, kind).clicked() {
            ui.ctx().copy_text(text.to_owned());
            self.copied = Some((slot, Instant::now()));
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
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
        match self.copied {
            Some((_, at)) if at.elapsed() < COPIED => {
                ui.ctx().request_repaint_after(Duration::from_millis(150));
            }
            Some(_) => self.copied = None,
            None => {}
        }

        theme::backdrop(ui);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 12)))
            .show(ui, |ui| {
                self.header(ui);
                ui.add_space(6.0);
                self.controls(ui);
                if self.session.is_some() {
                    ui.add_space(6.0);
                    self.connect_details(ui);
                }
                ui.add_space(6.0);
                self.output(ui);
            });
    }
}

impl App {
    fn header(&mut self, ui: &mut egui::Ui) {
        theme::glass(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Mosh");
                let (text, color) = self.state();
                theme::status_pill(ui, &text, color);
            });
            ui.horizontal(|ui| {
                match &self.server {
                    Some(server) => {
                        let version = server
                            .version
                            .as_deref()
                            .map(|v| format!("mosh {v} · "))
                            .unwrap_or_default();
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!(
                                    "{version}{}",
                                    server.path.display()
                                ))
                                .small()
                                .color(DIM),
                            )
                            .selectable(true),
                        );
                    }
                    None => {
                        ui.add(egui::Label::new(
                            egui::RichText::new(format!("not installed — {}", install_hint()))
                                .small()
                                .color(AMBER),
                        ));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::button(ui, "Re-detect", Kind::Glass).clicked() {
                        self.server = mosh::find();
                    }
                    if self.server.is_none()
                        && theme::button(ui, "Copy", Kind::Quiet)
                            .on_hover_text(format!("Copy `{}`", install_hint()))
                            .clicked()
                    {
                        ui.ctx().copy_text(install_hint().to_owned());
                    }
                });
            });
            if let Some(err) = self
                .server
                .as_ref()
                .and_then(|server| server.exec_error.as_ref())
            {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("found, but cannot be executed here: {err}"))
                            .small()
                            .color(RED),
                    )
                    .wrap_mode(egui::TextWrapMode::Wrap),
                );
            }
            if let Some(err) = &self.start_error {
                ui.add(
                    egui::Label::new(egui::RichText::new(err).small().color(RED))
                        .wrap_mode(egui::TextWrapMode::Wrap),
                );
            }
        });
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        theme::glass(ui, |ui| {
            let can_start =
                self.session.is_none() && self.server.as_ref().is_some_and(|s| s.exec_error.is_none());
            ui.horizontal(|ui| {
                if ui
                    .add_enabled_ui(can_start, |ui| theme::button(ui, "Start", Kind::Primary))
                    .inner
                    .clicked()
                {
                    self.start();
                }
                if ui
                    .add_enabled_ui(self.session.is_some(), |ui| {
                        theme::button(ui, "Stop", Kind::Glass)
                    })
                    .inner
                    .clicked()
                {
                    self.stop();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.port_text)
                            .hint_text("auto")
                            .desired_width(52.0)
                            .horizontal_align(egui::Align::Center),
                    );
                    ui.add(
                        egui::Label::new(egui::RichText::new("port").small().color(DIM)),
                    );
                    theme::switch(ui, &mut self.colors_256, "256 colors");
                });
            });
        });
    }

    fn connect_details(&mut self, ui: &mut egui::Ui) {
        let connect = self.connect_line();
        let command = self.client_command();
        theme::glass(ui, |ui| {
            ui.add(
                egui::Label::new(egui::RichText::new("Connect").small().color(DIM)),
            );
            ui.add_space(2.0);
            if let Some(connect) = &connect {
                ui.horizontal(|ui| {
                    theme::mono_row(ui, connect);
                    self.copy_chip(ui, connect, 0);
                });
            }
            if let Some(command) = &command {
                ui.horizontal(|ui| {
                    theme::mono_row(ui, command);
                    self.copy_chip(ui, command, 1);
                });
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new("Server IP").small().color(DIM)),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.ip_text)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(118.0),
                    );
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new("what the client dials")
                                .small()
                                .color(DIM),
                        ),
                    );
                });
            }
        });
    }

    fn output(&mut self, ui: &mut egui::Ui) {
        theme::glass(ui, |ui| {
            ui.add(
                egui::Label::new(egui::RichText::new("Server output").small().color(DIM)),
            );
            ui.add_space(4.0);
            let height = (ui.available_height() - 8.0).max(80.0);
            theme::well(ui, |ui| {
                ui.set_min_height(height);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if self.log.is_empty() {
                            let hint = if self.session.is_some() {
                                "Waiting for mosh-server…"
                            } else {
                                "Press Start — the handshake, port and key land here."
                            };
                            ui.vertical_centered(|ui| {
                                ui.add_space((height - 24.0).max(0.0) / 2.0);
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(hint).small().color(DIM),
                                    )
                                    .wrap_mode(egui::TextWrapMode::Wrap),
                                );
                            });
                        }
                        for line in &self.log {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(line).monospace().small().color(DIM),
                                )
                                .selectable(true),
                            );
                        }
                    });
            });
        });
    }
}

fn install_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        "brew install mosh"
    } else {
        "sudo apt install mosh"
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([480.0, 620.0])
            .with_min_inner_size([400.0, 440.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Simple Mosh GUI",
        options,
        Box::new(|cc| {
            theme::install(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        }),
    )
}
