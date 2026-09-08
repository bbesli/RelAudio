# RelAudio

Send audio between computers over your local network — system audio or a
microphone, in either direction, with low latency.

**Windows and Linux.** Free and open source (MIT).

*[Türkçe README](README.tr.md)*

---

## What it does

| | |
|---|---|
| **Send system audio** | Play your PC's sound on another computer's speakers |
| **Send microphone** | Stream a mic from one machine to another |
| **Listen** | Hear the incoming audio on this computer's speakers |
| **Use as microphone** | Make a remote mic appear as a microphone to Discord, Zoom, OBS… |

Devices find each other automatically on the LAN — no IP addresses to type.
The app lives in the system tray and keeps streaming when you close the window.
Interface available in 10 languages.

**Latency.** The software path measured **12–21 ms** in a local loopback test on
Linux, depending on the audio graph quantum ([docs/10-riskler.md](docs/10-riskler.md)).
End to end between two machines with default settings, expect roughly **60–90 ms** —
the jitter buffer contributes 40 ms by default and each machine's device buffer
20 ms or so. Lower the buffer and the device period if you need less.

---

## Install

### Linux

Needs `rustup`, Node.js, and PulseAudio/PipeWire development headers.

```bash
# Arch / CachyOS
sudo pacman -S --needed base-devel rustup nodejs npm libpulse webkit2gtk-4.1 libayatana-appindicator
rustup default stable

# Debian / Ubuntu
sudo apt install build-essential curl libpulse-dev libwebkit2gtk-4.1-dev \
                 libayatana-appindicator3-dev librsvg2-dev nodejs npm

# Fedora
sudo dnf install @development-tools pulseaudio-libs-devel webkit2gtk4.1-devel \
                 libappindicator-gtk3-devel librsvg2-devel nodejs
```

Then build and install:

```bash
git clone https://github.com/bbesli/RelAudio.git
cd RelAudio/app && npm install && npx tauri build --no-bundle
cd .. && ./scripts/install-linux.sh
```

This installs to `~/.local/bin/relaudio` and adds a **RelAudio** entry to your
application menu. No root needed. Remove it with `./scripts/uninstall-linux.sh`.

> If `relaudio` isn't found in your shell, `~/.local/bin` isn't on your `PATH`.
> Either launch it from the application menu or add:
> `echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc`

`.deb` and `.rpm` packages can be built with `npx tauri build` (AppImage
generation currently fails — see [Known limits](#known-limits)).

### Windows

You need:

1. **[Rust](https://rustup.rs)** — run `rustup-init.exe`, choose the standard
   installation.
2. **Visual C++ Build Tools** with the Windows SDK. If `rustup` doesn't offer
   to install them:
   ```
   winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```
   The `--add ...VCTools` part matters — without it you get Build Tools with no
   C++ compiler and the build fails with `link.exe not found`.
3. **[Node.js](https://nodejs.org)**

Then:

```
git clone https://github.com/bbesli/RelAudio.git
cd RelAudio
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
```

The script checks prerequisites, loads the Visual Studio environment (including
preview/Insiders editions, which Rust can't find on its own), installs npm
dependencies and builds. Output:
`app\src-tauri\target\release\relaudio-app.exe`

**Firewall:** the first time you receive audio, Windows will ask to allow the
app. Say yes for private networks. To add the rule manually, in an
administrator PowerShell:

```
New-NetFirewallRule -DisplayName "RelAudio UDP 59101" -Direction Inbound -Protocol UDP -LocalPort 59101 -Action Allow
```

---

## Using it

Open RelAudio on both computers. Each one starts listening automatically and
announces itself on the network.

### Play one computer's sound on another

1. **Receiving machine** — *Player* tab, **Listen on speakers**, pick your
   speakers as the output device.
2. **Sending machine** — *Server* tab, **System audio**, pick the receiving
   machine from the **Target device** list, press **Start streaming**.

On Linux, "system audio" means the *monitor* of an output device. The default
is usually right; if you hear nothing, check which output your media player is
actually using (`pactl list sink-inputs`).

### Send a microphone

Same as above, but choose **Microphone** on the sending machine and pick the
mic in the right-hand panel.

### Headset mode — use one machine's headset on the other

This is the case RelAudio was built for: you drive machine B (over Parsec,
RDP, Sunshine, whatever) but your headset is plugged into machine A, and you
want it to behave as B's headset — microphone *and* speakers.

Open the **Headset** tab on both machines and pick a role:

| Machine | Role | What it does |
|---|---|---|
| Where the headset is plugged in | **Headset is on this machine** | Sends its microphone, plays what comes back |
| The one you're controlling | **Remote machine** | Writes the incoming mic into a virtual cable, sends its system audio back |

Pick the other device as the target, press start on both. RelAudio chooses the
devices itself, with one rule it never breaks: **on a given machine, the device
it captures from is never the device it writes to.** Break that rule and the
audio feeds itself back and you hear your own voice.

On the remote machine you still tell your meeting app which microphone to use —
the app shows you the name.

> **Turn off your remote desktop's audio streaming.** Parsec, RDP and the rest
> capture the machine's default output and send it to you. If RelAudio is also
> relaying audio, you get it twice — and if the remote desktop happens to
> capture the same cable RelAudio writes into, you hear yourself. Let RelAudio
> carry the audio; let the remote desktop carry video and input.

### Use a remote microphone as a local microphone

This is the one that needs a little setup. Windows and Linux have no built-in
way for an application to *become* a microphone, so you need a **virtual audio
cable**: a driver with a speaker end and a microphone end. Whatever is written
to the speaker end comes out of the microphone end.

**Install a virtual cable:**

| Platform | What to install |
|---|---|
| **Windows** | [VB-CABLE](https://vb-audio.com/Cable/) — free (donationware). Extract the ZIP, run `VBCABLE_Setup_x64.exe` **as administrator**, then reboot. |
| **Linux** | Nothing to install — create one with:<br>`pactl load-module module-null-sink sink_name=relaudio media.class=Audio/Sink sink_properties=device.description=RelAudio-Cable` |

**Then:**

1. **Receiving machine** — *Player* tab, **Use as microphone**. The device list
   filters itself down to virtual cables. Pick the **speaker end**
   (`CABLE Input` on Windows). A green box tells you which microphone to select
   in other apps.
2. **Sending machine** — *Server* tab, **Microphone**, pick the receiver as
   target, start streaming.
3. **In Discord / Zoom / OBS** — choose the **microphone end**
   (`CABLE Output` on Windows) as your microphone.

> **Do not change your system's default output device to the cable.** Only
> RelAudio should write to it. If you set Windows' default output to
> `CABLE Input`, everything your computer plays also goes into the microphone —
> and you'll hear yourself.

> **If you hear your own voice:** open Windows Sound settings → Recording →
> `CABLE Output` → Properties → **Listen** tab → uncheck *"Listen to this
> device"*.

---

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| **Packets counter stays at 0** | Wrong target address, or a firewall. The Player tab shows this machine's address — make sure the sender is pointed at it. |
| **Packets arrive but no sound** | Wrong output device. The Statistics panel shows which device the stream actually opened — not what the dropdown says. Change it in the right-hand panel; the stream restarts automatically. `relaudio-cli tone` tests the output on its own. |
| **You hear your own voice** | Something is monitoring the cable. See the two notes above. |
| **App won't open, nothing happens** | It's already running — look in the system tray. RelAudio allows only one instance. |
| **No tray icon on GNOME** | GNOME has no tray by default. Install the *AppIndicator and KStatusNotifierItem Support* extension. Without a tray, RelAudio disables "minimize to tray" so the app can't become unreachable. |
| **Buffer grows over time** | Clock drift between the two machines. There's no compensation yet; restart the stream. |
| **Something else** | Check the log: `%LOCALAPPDATA%\RelAudio\relaudio.log` (Windows) or `~/.local/state/RelAudio/relaudio.log` (Linux). The Settings tab shows the exact path. |

### Diagnostic commands

A command-line tool is built alongside the app:

```bash
relaudio-cli devices                  # list devices with their IDs
relaudio-cli tone                     # play a test tone — checks output, no network
relaudio-cli level --mic --device ID  # live input level meter
relaudio-cli send 192.168.1.10        # stream without the GUI
relaudio-cli recv                     # receive without the GUI
```

`relaudio` launches the app; `relaudio-cli` is the diagnostic tool.
On Windows both are under `app\src-tauri\target\release\` and
`target\release\` respectively.

`relaudio-cli level` is the fastest way to find which link in the chain is broken:
point it at the cable's microphone end and see whether audio actually arrives.

---

## Known limits

- **No clock drift compensation.** The two machines' sound cards don't run at
  exactly the same rate. Over long sessions the buffer creeps and the "dropped"
  counter grows. A cap keeps latency bounded; adaptive resampling is planned.
- **PCM only.** ~1.5 Mbit/s, though silent blocks are sent without payload so
  quiet passages cost almost nothing. Opus is planned.
- **Not encrypted.** Use it on networks you trust.
- **No packet loss concealment.** Lost packets become short silences.
- **macOS is not supported** — deliberately deferred, see
  [docs/00-genel-bakis.md](docs/00-genel-bakis.md).
- **AppImage build fails** (`linuxdeploy` error). `.deb` and `.rpm` work.

---

## How it works

```
capture ──► packetize ──► UDP ──► jitter buffer ──► playback
(WASAPI / PulseAudio)     5 ms                     (WASAPI / PulseAudio)
```

- **UI:** Tauri v2 + Svelte 5 — ~11 MB binary, uses the OS webview
- **Core:** Rust. Audio and network run on dedicated threads
- **Linux backend:** PulseAudio API (PipeWire provides it natively)
- **Windows backend:** WASAPI, loopback via a render device opened for capture
- **Discovery:** mDNS (`_relaudio._udp`)

Design notes, measurements and decision records are in [docs/](docs/) —
written in Turkish. Architecture: [docs/02-mimari.md](docs/02-mimari.md).
Decision records: [docs/adr/](docs/adr/).

### Building and testing

```bash
cargo test                                    # core tests
cd app/src-tauri && cargo test                # session layer tests
rustup target add x86_64-pc-windows-msvc      # once
cargo check --target x86_64-pc-windows-msvc   # type-check Windows code on Linux
node scripts/check-i18n.mjs                   # translation completeness
```

That third one matters: Windows-specific code is `cfg`-gated out of normal
Linux builds, so it's never checked unless you ask for it.

---

## License

MIT — see [LICENSE](LICENSE).

## Support

If RelAudio is useful to you: [buymeacoffee.com/bbesli](https://buymeacoffee.com/bbesli)
