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

## Before you start

Two things to know, because almost every problem people hit comes from one of them.

**1. A "virtual audio cable" is what lets an app become a microphone.**
Neither Windows nor Linux lets a normal program appear in another program's
microphone list. A virtual cable is a driver with two ends: one end looks like a
**speaker**, the other looks like a **microphone**. Whatever is written to the
speaker end comes out of the microphone end.

```
RelAudio writes here                 your meeting app reads here
        │                                        │
        ▼                                        ▼
  CABLE Input  ══════ the cable ══════►  CABLE Output
  (a speaker)                            (a microphone)
```

The names are confusing on purpose-ish: they are named from *the cable's* point
of view, not Windows'. So in Windows' Sound panel, `CABLE Input` is on the
**Playback** tab and `CABLE Output` is on the **Recording** tab. If you go
looking for `CABLE Output` in a list of speakers, you will not find it.

**2. One machine must never capture the same device it writes to.**
If it does, the audio feeds itself in a circle and you hear your own voice.
RelAudio's Headset mode picks devices for you and never breaks this rule; if you
configure things by hand, this is the rule to keep in mind.

---

## Using it

Open RelAudio on **both** computers. Each one starts listening automatically and
announces itself on the network, so they find each other — you never type an IP.

Below, "machine A" and "machine B" just mean your two computers.

---

### Scenario 1 — Play one computer's sound on another

*Example: your PC plays music, you want to hear it on the laptop in the kitchen.*

**On the machine that should make the sound (B):**

1. Open RelAudio.
2. Click the **Player** tab.
3. Make sure **Listen on speakers** is selected.
4. On the right, under **Output device**, choose your speakers or headphones.
5. Press **Start listening**. (It usually already started by itself.)

**On the machine whose sound you want to send (A):**

1. Open RelAudio.
2. Click the **Server** tab.
3. Choose **System audio**.
4. In **Target device**, pick machine B from the list.
   *If the list is empty, wait a few seconds. Still empty? See Troubleshooting.*
5. Press **Start streaming**.

Play something on A. You should hear it on B within a second.

> On Linux, "system audio" means the *monitor* of an output device. RelAudio
> picks the default one, which is right in most cases. If you hear nothing,
> check which output your media player is actually using — some apps are pinned
> to a specific device.

---

### Scenario 2 — Send a microphone to another computer's speakers

Exactly like Scenario 1, except in step 3 on machine A you choose
**Microphone** instead of **System audio**, and on the right you pick which
microphone to send.

This only makes the mic *audible* on B. If you want B's apps (Discord, Zoom…)
to treat it as a real microphone, that's Scenario 4.

---

### Scenario 3 — Headset mode (the main event)

*Example: you are sitting at your Linux machine, controlling a Windows machine
over Parsec / RDP / Sunshine. Your headset is plugged into the Linux machine.
You want it to work as the Windows machine's headset — you speak into it, the
meeting hears you; the meeting talks, you hear it.*

This needs **both directions at once**, and RelAudio sets both up for you.

#### Step 1 — Install a virtual audio cable on the remote machine

Only the machine **without** the headset needs one.

**Windows:**

1. Go to <https://vb-audio.com/Cable/>.
2. Download **VBCABLE_Driver_Pack** (the ZIP on the left, under Windows).
3. **Extract the ZIP** to a folder. Do not run it from inside the ZIP.
4. Right-click `VBCABLE_Setup_x64.exe` → **Run as administrator**.
5. Click **Install Driver**, accept the Windows prompt.
6. **Reboot.** The driver is not fully usable until you do.

After rebooting you should see `CABLE Input` in your speaker list and
`CABLE Output` in your microphone list.

**Linux:** nothing to install. Run this once (it lasts until reboot):

```bash
pactl load-module module-null-sink sink_name=relaudio \
  media.class=Audio/Sink sink_properties=device.description=RelAudio-Cable
```

#### Step 2 — Turn off your remote desktop's audio

**Do this.** Parsec, RDP, AnyDesk and friends capture the remote machine's
default speaker and stream it to you. If RelAudio is also carrying audio you get
everything twice — and if the remote desktop happens to capture the very cable
RelAudio writes into, you will hear your own voice and nothing you change in
RelAudio will fix it.

- **Parsec:** Settings → Host (or Client) → **Audio** → off.
- **Windows RDP:** in the connection settings, Local Resources → Remote audio →
  **Do not play**.

RelAudio replaces that audio channel, with lower latency.

#### Step 3 — Set the remote machine's default speaker to a *real* speaker

On the remote machine, open Sound settings and make sure the default output is
your normal speakers (e.g. `Speakers (Realtek(R) Audio)`) — **not** the cable.

Why: RelAudio captures the system sound from a real output device and sends it
to you. If the default were the cable, the machine's audio would go into the
cable instead, mix with the relayed microphone, and you would hear yourself.

Windows shortcut: press <kbd>Win</kbd>+<kbd>R</kbd>, type `mmsys.cpl`, Enter.
**Playback** tab → click your speakers → **Set Default**.

#### Step 4 — Start Headset mode on both machines

**On the machine where the headset is plugged in:**

1. **Headset** tab.
2. Choose **Headset is on this machine**.
3. **Other device** → pick the remote machine.
4. Press **Start headset mode**.

**On the remote machine:**

1. **Headset** tab.
2. Choose **Remote machine**.
3. **Other device** → pick the machine with the headset.
4. Press **Start headset mode**.

Before you press start, RelAudio shows which two devices it picked. On the
remote machine that should look like:

```
Source   Speakers (Realtek(R) Audio)          ← its system sound, sent to you
Output   CABLE Input (VB-Audio Virtual Cable) ← your mic, written into the cable
```

Two different devices. That is the rule from the top of this page, enforced.

#### Step 5 — Tell your meeting app what to use

On the **remote** machine, in Discord / Zoom / Teams / Meet:

- **Microphone:** `CABLE Output (VB-Audio Virtual Cable)`
  RelAudio tells you this name on screen after you press start.
- **Speaker / output:** leave it as your normal speakers. Do not pick the cable.
  Its sound is already being relayed to you.

That's it. Talk into your headset — the meeting hears you. The meeting talks —
you hear it in your headset.

---

### Scenario 4 — Use a remote microphone as a local microphone (one direction only)

Same as Scenario 3 but you only care about the microphone direction (you already
have sound some other way).

1. Install a virtual cable on the receiving machine (Step 1 above).
2. **Receiving machine:** *Player* tab → **Use as microphone** → the device list
   filters itself to cables → pick `CABLE Input`. Press **Start listening**.
   A green box appears telling you which microphone to select elsewhere.
3. **Sending machine:** *Server* tab → **Microphone** → pick the target →
   **Start streaming**.
4. **In your app:** choose `CABLE Output` as the microphone.

> `CABLE Output` will **not** appear in RelAudio's list. RelAudio shows
> speakers; `CABLE Output` is a microphone. It appears in the *other* app's
> microphone list. This trips up almost everyone once.

## Troubleshooting

Work down this list in order. Each step tells you where the chain is broken.

### "I hear my own voice"

This is the most common complaint, and it is almost never RelAudio playing the
audio back at you. Check in this order:

1. **Is your remote desktop still streaming audio?** Parsec / RDP / AnyDesk send
   you the remote machine's speaker output. If RelAudio is relaying too, the
   same sound reaches you twice — and if the remote desktop captures the cable
   RelAudio writes into, you hear yourself no matter what you change.
   → Turn its audio off (Scenario 3, Step 2).

2. **Is the remote machine's default speaker set to the cable?** Then everything
   that machine plays goes into the cable, mixes with your relayed microphone,
   and comes back.
   → `mmsys.cpl` → Playback → pick your real speakers → **Set Default**.

3. **Is "Listen to this device" enabled on the cable's microphone end?** That
   setting pipes the cable straight to your speakers.
   → `mmsys.cpl` → **Recording** tab → `CABLE Output` → Properties →
   **Listen** tab → uncheck *Listen to this device*.

4. **Is the same meeting open on both machines?** If Discord is in the same
   voice channel on both, one of them transmits your relayed microphone and the
   other plays it back to you. Leave the call on the machine that isn't relaying.

5. **Is RelAudio itself looping?** If the Server captures the same device the
   Player writes to, RelAudio shows a red **Feedback loop** warning. Headset
   mode prevents this; manual configuration can hit it.

### "The Packets counter stays at 0"

Nothing is arriving. Either the sender is aiming at the wrong address, or a
firewall is in the way.

- The **Player** tab shows this machine's address. Make sure the sender is
  pointed at exactly that.
- If the sending side shows a red **Send errors** count, the packets are being
  refused — wrong address. A machine with several network adapters (VPN, WSL,
  Hyper-V) announces more than one; pick the target again, or type the address
  by hand.
- Windows firewall: allow the app when prompted, or add the rule manually:
  ```
  New-NetFirewallRule -DisplayName "RelAudio UDP 59101" -Direction Inbound -Protocol UDP -LocalPort 59101 -Action Allow
  ```

### "Packets arrive but I hear nothing"

The audio is going somewhere you're not listening.

- Look at the **Output** row in the Statistics panel. That is the device the
  stream *actually opened* — not what the dropdown shows. If it says the wrong
  thing, change the device on the right; the stream restarts by itself.
- Test the output on its own, with no network involved:
  ```
  relaudio-cli tone
  ```
  If you don't hear a 440 Hz tone, the problem is the output device, not the network.
- In Headset/microphone mode the audio goes into a *cable*, so you are not
  supposed to hear it. Check the far end instead:
  ```
  relaudio-cli level --mic --device "<CABLE Output id>"
  ```
  Get the id from `relaudio-cli devices`. The bar should move when the other
  side speaks.

### "My meeting app doesn't show the cable as a microphone"

- Did you **reboot** after installing VB-CABLE? It is not fully registered
  until you do.
- You are probably looking at a list of speakers. `CABLE Output` is a
  microphone; it appears under microphone/input settings, never under speakers.
- Some apps cache their device list — restart the meeting app.

### Other

| Symptom | Cause |
|---|---|
| **App won't open, nothing happens** | It's already running — check the system tray. RelAudio allows one instance. |
| **No tray icon on GNOME** | GNOME has no tray by default. Install the *AppIndicator and KStatusNotifierItem Support* extension. Without a tray, RelAudio disables "minimize to tray" so it can't become unreachable. |
| **Buffer grows over time / "Dropped" counter climbs** | Clock drift between the two machines. There is no compensation yet; restart the stream. |
| **Sound is choppy** | Raise **Buffer** in the Player tab. Each unit is 5 ms; the default 8 is 40 ms. Wi-Fi usually wants 15–20. |
| **Anything else** | Read the log. Settings tab shows the exact path — `%LOCALAPPDATA%\RelAudio\relaudio.log` on Windows, `~/.local/state/RelAudio/relaudio.log` on Linux. |

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
