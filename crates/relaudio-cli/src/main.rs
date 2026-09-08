//! RelAudio CLI — arayüz olmadan yayın başlatıp durdurmak için.
//!
//! Arayüz (Tauri) bu çekirdeği ayrı süreç olarak kullanacak (docs/adr/0002);
//! bu binary aynı çekirdeği komut satırından sürer.

use relaudio_core::audio::{self, DeviceKind};
use relaudio_core::net::{receive_loop, send_loop, ReceiverStats, SenderStats, Stopper};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_PORT: u16 = 59101;

fn usage() -> ! {
    eprintln!(
        r#"RelAudio {ver}

KULLANIM
  relaudio devices
      Ses aygıtlarını listeler.

  relaudio send <ip[:port]> [--device <id>] [--mic]
      Bu makinenin sesini gönderir.
      Varsayılan: sistem sesi. --mic ile mikrofon gönderilir.

  relaudio tone [--device <id>] [--seconds <n>]
      Çıkış aygıtına test tonu çalar. Ağı devre dışı bırakıp yalnızca
      çalma yolunu sınar. Ses duyulmuyorsa sorun ağda değil, çalmada.

  relaudio level [--device <id>] [--mic]
      Bir giriş aygıtındaki ses seviyesini canlı gösterir.
      Zincirin neresinin koptuğunu bulmak için: sanal kablonun mikrofon
      ucunu dinleyip sinyal gelip gelmediğini görürsün.
      Varsayılan: sistem sesi (monitor). --mic ile mikrofon girişleri.

  relaudio recv [--port <n>] [--device <id>] [--buffer <paket>]
      Gelen sesi bu makinede çalar.
      --buffer: jitter buffer hedefi, paket cinsinden (1 paket = 5 ms).
                Varsayılan 8 = 40 ms.

ÖRNEK
  # A makinesinde:
  relaudio recv
  # B makinesinde:
  relaudio send 192.168.1.113
"#,
        ver = env!("CARGO_PKG_VERSION")
    );
    std::process::exit(2)
}

/// `--anahtar deger` çiftini arar.
fn opt(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn has(args: &[String], key: &str) -> bool {
    args.iter().any(|a| a == key)
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().map(String::as_str) else {
        usage()
    };

    let result = match cmd {
        "devices" => cmd_devices(),
        "send" => cmd_send(&args),
        "recv" => cmd_recv(&args),
        "tone" => cmd_tone(&args),
        "level" => cmd_level(&args),
        "-h" | "--help" | "help" => usage(),
        other => {
            eprintln!("bilinmeyen komut: {other}\n");
            usage()
        }
    };

    if let Err(e) = result {
        eprintln!("\nHATA: {e}");
        std::process::exit(1);
    }
}

fn cmd_devices() -> relaudio_core::Result<()> {
    let devices = audio::list_devices()?;
    let mut last = None;
    for d in &devices {
        if last != Some(d.kind) {
            let title = match d.kind {
                DeviceKind::Output => "ÇIKIŞ (sesin çalınacağı yer)",
                DeviceKind::Input => "MİKROFON",
                DeviceKind::Monitor => "SİSTEM SESİ (loopback)",
            };
            println!("\n{title}");
            last = Some(d.kind);
        }
        println!(
            "  {} {}\n      id: {}",
            if d.is_default { "*" } else { " " },
            d.name,
            d.id
        );
    }
    println!("\n(* = varsayılan)");
    Ok(())
}

/// Bir giriş aygıtındaki seviyeyi canlı gösterir.
///
/// "Ses geliyor mu?" sorusunu zincirin herhangi bir noktasında cevaplamak
/// için. Sanal kablo senaryosunda kablonun mikrofon ucunu dinleyip
/// gerçekten sinyal taşıyıp taşımadığını görmeyi sağlıyor.
fn cmd_level(args: &[String]) -> relaudio_core::Result<()> {
    use relaudio_proto::PCM_PAYLOAD_LEN;

    let device = opt(args, "--device").unwrap_or_default();
    let kind = if has(args, "--mic") { DeviceKind::Input } else { DeviceKind::Monitor };

    println!("aygıt : {}", if device.is_empty() { "<varsayılan>" } else { &device });
    println!("tür   : {}", if kind == DeviceKind::Input { "mikrofon" } else { "sistem sesi" });
    println!("Ctrl+C ile çık.\n");

    let mut cap = audio::open_capture(&device, kind)?;
    let mut buf = vec![0u8; PCM_PAYLOAD_LEN];
    let mut peak_hold: i32 = 0;
    let mut silent_blocks: u64 = 0;
    let mut total_blocks: u64 = 0;
    // RMS de tepe gibi tüm pencere boyunca birikmeli; yalnızca son bloktan
    // hesaplanırsa "tepe yüksek ama dB -99" gibi tutarsız satırlar çıkıyor.
    let mut window_sq: f64 = 0.0;
    let mut window_samples: u64 = 0;

    loop {
        cap.read(&mut buf)?;
        total_blocks += 1;

        let mut peak: i32 = 0;
        let mut sum_sq: f64 = 0.0;
        for c in buf.chunks_exact(2) {
            let v = i16::from_le_bytes([c[0], c[1]]) as i32;
            peak = peak.max(v.abs());
            sum_sq += (v as f64) * (v as f64);
        }
        if peak < 32 {
            silent_blocks += 1;
        }
        peak_hold = peak_hold.max(peak);
        window_sq += sum_sq;
        window_samples += (buf.len() / 2) as u64;

        // 100 ms'de bir çiz (20 blok x 5 ms)
        if total_blocks % 20 != 0 {
            continue;
        }
        let rms = (window_sq / window_samples.max(1) as f64).sqrt();
        let db = if rms > 1.0 { 20.0 * (rms / 32768.0).log10() } else { -99.0 };
        let bars = ((peak_hold as f64 / 32768.0) * 40.0).round() as usize;
        let meter: String = "█".repeat(bars.min(40)) + &"·".repeat(40 - bars.min(40));

        print!(
            "\r[{meter}] tepe {:>5}  {:>6.1} dB  sessiz %{:>3}",
            peak_hold,
            db,
            silent_blocks * 100 / total_blocks
        );
        use std::io::Write;
        let _ = std::io::stdout().flush();
        peak_hold = 0;
        window_sq = 0.0;
        window_samples = 0;
    }
}

/// Ağdan bağımsız çalma testi. 440 Hz sinüs.
fn cmd_tone(args: &[String]) -> relaudio_core::Result<()> {
    use relaudio_proto::{CHANNELS, PCM_FRAMES_PER_PACKET, PCM_PAYLOAD_LEN, SAMPLE_RATE};

    let device = opt(args, "--device").unwrap_or_default();
    let seconds: f64 = opt(args, "--seconds")
        .and_then(|s| s.parse().ok())
        .unwrap_or(3.0);

    println!("çıkış aygıtı : {}", if device.is_empty() { "<varsayılan>" } else { &device });
    println!("süre         : {seconds} sn, 440 Hz");
    println!();

    let mut out = audio::open_playback(&device)?;

    let blocks = (seconds * SAMPLE_RATE as f64 / PCM_FRAMES_PER_PACKET as f64) as usize;
    let mut buf = vec![0u8; PCM_PAYLOAD_LEN];
    let mut phase: f64 = 0.0;
    let step = 2.0 * std::f64::consts::PI * 440.0 / SAMPLE_RATE as f64;

    for i in 0..blocks {
        for f in 0..PCM_FRAMES_PER_PACKET {
            // Başta ve sonda yumuşak geçiş — klik olmasın.
            let t = (i * PCM_FRAMES_PER_PACKET + f) as f64 / (blocks * PCM_FRAMES_PER_PACKET) as f64;
            let env = (t * 40.0).min(1.0).min((1.0 - t) * 40.0);
            let v = (12000.0 * env * phase.sin()) as i16;
            phase += step;
            for c in 0..CHANNELS as usize {
                let idx = (f * CHANNELS as usize + c) * 2;
                buf[idx..idx + 2].copy_from_slice(&v.to_le_bytes());
            }
        }
        out.write(&buf)?;
        if i % 40 == 0 {
            print!("\r  çalınıyor... {:.1} sn", i as f64 * 0.005);
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    }
    println!("\r  bitti. {seconds} sn ton yazıldı.        ");
    println!();
    println!("Ses duydun mu? Duymadıysan sorun çalma yolunda, ağda değil.");
    Ok(())
}

fn cmd_send(args: &[String]) -> relaudio_core::Result<()> {
    let Some(target) = args.get(1).filter(|s| !s.starts_with("--")) else {
        usage()
    };
    let target = if target.contains(':') {
        target.clone()
    } else {
        format!("{target}:{DEFAULT_PORT}")
    };

    let kind = if has(args, "--mic") {
        DeviceKind::Input
    } else {
        DeviceKind::Monitor
    };
    let device = opt(args, "--device").unwrap_or_default();

    println!(
        "kaynak : {}",
        if kind == DeviceKind::Input { "mikrofon" } else { "sistem sesi" }
    );
    println!("hedef  : {target}");
    println!("Ctrl+C ile durdur.\n");

    let stop = Stopper::new();
    let stats = Arc::new(SenderStats::default());
    spawn_reporter_send(stats.clone(), stop.clone());
    install_sigint(stop.clone());

    send_loop(&device, kind, &target, stop, stats)
}

fn cmd_recv(args: &[String]) -> relaudio_core::Result<()> {
    let port: u16 = opt(args, "--port")
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let device = opt(args, "--device").unwrap_or_default();
    let target_packets: usize = opt(args, "--buffer")
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);

    println!("dinleniyor : 0.0.0.0:{port}");
    println!("tampon     : {target_packets} paket ({} ms)", target_packets * 5);
    println!("Ctrl+C ile durdur.\n");

    let stop = Stopper::new();
    let stats = Arc::new(ReceiverStats::default());
    spawn_reporter_recv(stats.clone(), stop.clone());
    install_sigint(stop.clone());

    receive_loop(port, &device, target_packets, stop, stats)
}

fn install_sigint(stop: Stopper) {
    // Basit yaklaşım: ayrı bir signal crate'i eklemek yerine ctrl+c'yi
    // varsayılan davranışına bırakıyoruz. Spike aşamasında yeterli.
    // Arayüz entegrasyonunda IPC üzerinden düzgün kapatma gelecek.
    let _ = stop;
}

fn spawn_reporter_send(stats: Arc<SenderStats>, stop: Stopper) {
    std::thread::spawn(move || {
        let mut last = (0u64, 0u64, 0u64);
        while !stop.stopped() {
            std::thread::sleep(Duration::from_secs(1));
            let now = stats.snapshot();
            let dp = now.0 - last.0;
            let db = now.1 - last.1;
            let ds = now.2 - last.2;
            println!(
                "gönderilen {:>7} paket ({:>5.1} s ses)  {:>6.1} kbit/s  sessiz {:>4}/s  toplam {:.2} MB",
                now.0,
                now.0 as f64 * 0.005,
                db as f64 * 8.0 / 1000.0,
                ds,
                now.1 as f64 / 1_048_576.0
            );
            let _ = dp;
            last = now;
        }
    });
}

fn spawn_reporter_recv(stats: Arc<ReceiverStats>, stop: Stopper) {
    std::thread::spawn(move || {
        while !stop.stopped() {
            std::thread::sleep(Duration::from_secs(1));
            println!(
                "alınan {:>7} paket  kayıp {:>4}  geç {:>4}  underrun {:>4}  atılan {:>4}  tampon {:>3} paket ({:>3} ms)  {:.2} MB",
                stats.packets.load(Ordering::Relaxed),
                stats.lost.load(Ordering::Relaxed),
                stats.late.load(Ordering::Relaxed),
                stats.underruns.load(Ordering::Relaxed),
                stats.dropped.load(Ordering::Relaxed),
                stats.buffer_depth.load(Ordering::Relaxed),
                stats.buffer_depth.load(Ordering::Relaxed) * 5,
                stats.bytes.load(Ordering::Relaxed) as f64 / 1_048_576.0
            );
        }
    });
}
