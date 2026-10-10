# Simple Mosh GUI

A tiny Rust/egui app that finds `mosh-server` on your machine, starts it, and
hands you the connection details — the `MOSH CONNECT <port> <key>` line and a
ready-to-copy `mosh-client` command.

Built for macOS (verified) and Android (Termux today, native-APK scaffolding).

![Simple Mosh GUI](screenshot.png)

## Why

`mosh` normally starts `mosh-server` for you over SSH. When you want a mosh
session on a machine you can't SSH into first — or you just want the port and
key handed to you — you run `mosh-server new` by hand and copy them out of the
terminal. This is a small panel around that
([background](https://v4.chriskrycho.com/2017/set-up-mosh-on-macos.html)).

## Install mosh

- macOS: `brew install mosh` (or MacPorts: `sudo port install mosh`)
- Android/Termux: `pkg install mosh`

## Run

```sh
cargo run --release
# optional: put it on your PATH
cargo install --path .
```

`simple-mosh-gui --detect` prints which `mosh-server` was found and exits
non-zero if none — handy for scripts.

## What it does

- Detects `mosh-server` in `$PATH`, plus `/opt/homebrew/bin`, `/usr/local/bin`,
  `/opt/local/bin`; on Android also `/data/data/com.termux/files/usr/bin`,
  `/data/local/tmp`, and `$HOME`.
- Starts `mosh-server new` with 256 colors (toggleable) and an optional port.
- Shows the connect line and `MOSH_KEY=<key> mosh-client <ip> <port>`, each one
  click to copy. The IP is pre-filled from the default route and is editable.
- Streams the server's raw output and can stop the session.

Notes:

- `mosh-server` prints its handshake and detaches; the app tracks the detached
  pid, so Stop kills the right process.
- A server that no client contacts exits by itself after 60 seconds; the app
  reports that as “session ended”.
- Closing the window does not kill running sessions (mosh semantics).
- If the environment has no UTF-8 locale (typical for apps launched from the
  Dock), the app passes mosh-server's `-l LANG=en_US.UTF-8` fallback — the same
  trick the mosh client uses when it starts a server over ssh.

## Android

### Termux (works today)

Run the GUI inside Termux under [termux-x11](https://github.com/termux/termux-x11)
(needs the Termux:X11 app *and* its companion package; see that project's
README):

```sh
pkg install mosh rust
git clone https://github.com/henrychea/simple-mosh-gui && cd simple-mosh-gui
cargo build --release
termux-x11 :0 & DISPLAY=:0 ./target/release/simple-mosh-gui
```

Detection finds Termux's `mosh-server` and starting works, because the app runs
with your Termux user, environment and PATH.

### Native APK (not verified on a device)

```sh
rustup target add aarch64-linux-android
cargo install cargo-apk
cargo apk run --lib
```

Android's app sandbox normally prevents a standalone app from executing
Termux's copy of `mosh-server`, so a native APK realistically needs
`mosh-server` bundled as an app asset (not shipped here yet). The app also
looks in its own `$HOME`, so a binary extracted there is detected and used.

## Development

```sh
cargo test                       # parser/unit tests
cargo run --example live_smoke   # starts and stops a real mosh-server end-to-end
```

## License

MIT — see [LICENSE](LICENSE).
