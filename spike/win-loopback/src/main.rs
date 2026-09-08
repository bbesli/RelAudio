// Adım 1b — Windows WASAPI loopback spike. ATILACAK KOD.
//
// Ne yapar:
//   1. Varsayılan çıkış (render) aygıtını loopback modunda yakalar
//   2. Aygıt periyodunu ve gerçek tampon boyutunu raporlar
//   3. SESSİZLİKTE paket üretiyor mu, silent bayrağı geliyor mu — ölçer
//   4. İsteğe bağlı olarak 960 baytlık UDP paketleri hâlinde gönderir
//
// Kullanım:
//   cargo run --release                          (sadece ölçüm, ağ yok)
//   cargo run --release -- 192.168.1.113:59101   (ölçüm + UDP gönderim)

use std::collections::VecDeque;
use std::error::Error;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

use wasapi::*;

const RATE: usize = 48000;
const CHANNELS: usize = 2;
const BITS: usize = 16;
const PACKET_BYTES: usize = 960; // 240 kare x 2 kanal x 2 bayt = 5 ms

fn main() -> Result<(), Box<dyn Error>> {
    initialize_mta().ok()?;

    let target = std::env::args().nth(1);
    let socket = match &target {
        Some(t) => {
            let s = UdpSocket::bind("0.0.0.0:0")?;
            s.connect(t.as_str())?;
            println!("UDP hedefi : {t}");
            Some(s)
        }
        None => {
            println!("UDP hedefi : yok (sadece ölçüm)");
            None
        }
    };

    let enumerator = DeviceEnumerator::new()?;
    let device = enumerator.get_default_device(&Direction::Render)?;
    println!("aygıt      : {}", device.get_friendlyname()?);

    let mut audio_client = device.get_iaudioclient()?;

    let (def_hns, min_hns) = audio_client.get_device_period()?;
    println!(
        "aygıt periyodu: varsayılan {:.2} ms, minimum {:.2} ms",
        def_hns as f64 / 10_000.0,
        min_hns as f64 / 10_000.0
    );

    let format = WaveFormat::new(BITS, BITS, &SampleType::Int, RATE, CHANNELS, None);
    let blockalign = format.get_blockalign() as usize;

    // Render aygıtı + Capture yönü => crate AUDCLNT_STREAMFLAGS_LOOPBACK ekler
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: min_hns,
    };
    audio_client.initialize_client(&format, &Direction::Capture, &mode)?;

    let h_event = audio_client.set_get_eventhandle()?;
    let buffer_frames = audio_client.get_buffer_size()?;
    println!(
        "tampon     : {} kare ({:.2} ms)",
        buffer_frames,
        buffer_frames as f64 / RATE as f64 * 1000.0
    );
    println!("format     : {} bit, {} kanal, {} Hz\n", BITS, CHANNELS, RATE);

    let capture_client = audio_client.get_audiocaptureclient()?;
    let mut queue: VecDeque<u8> = VecDeque::with_capacity(blockalign * buffer_frames as usize * 16);

    audio_client.start_stream()?;
    println!("yakalama başladı. Ctrl+C ile çık.");
    println!("SESSİZLİK TESTİ: ilk 5 saniye hiçbir şey çalma, sonra müzik aç.\n");

    let start = Instant::now();
    let mut last_report = Instant::now();
    let mut frames_total: u64 = 0;
    let mut silent_buffers: u64 = 0;
    let mut data_buffers: u64 = 0;
    let mut discontinuities: u64 = 0;
    let mut bytes_sent: u64 = 0;
    let mut peak: i16 = 0;

    loop {
        let info = capture_client.read_from_device_to_deque(&mut queue)?;
        if info.flags.silent {
            silent_buffers += 1;
        } else {
            data_buffers += 1;
        }
        if info.flags.data_discontinuity {
            discontinuities += 1;
        }

        while queue.len() >= PACKET_BYTES {
            let mut pkt = [0u8; PACKET_BYTES];
            for b in pkt.iter_mut() {
                *b = queue.pop_front().unwrap();
            }
            for c in pkt.chunks_exact(2) {
                let v = i16::from_le_bytes([c[0], c[1]]).saturating_abs();
                if v > peak {
                    peak = v;
                }
            }
            frames_total += (PACKET_BYTES / blockalign) as u64;
            if let Some(s) = &socket {
                if s.send(&pkt).is_ok() {
                    bytes_sent += PACKET_BYTES as u64;
                }
            }
        }

        if last_report.elapsed() >= Duration::from_secs(1) {
            let secs = start.elapsed().as_secs_f64();
            println!(
                "[{:5.1}s] kare {:>9} ({:5.2}s ses)  veri-tamponu {:>5}  SESSİZ-tampon {:>5}  kopukluk {}  tepe {:>6}  gönderilen {:.2} MB",
                secs,
                frames_total,
                frames_total as f64 / RATE as f64,
                data_buffers,
                silent_buffers,
                discontinuities,
                peak,
                bytes_sent as f64 / 1_048_576.0
            );
            peak = 0;
            last_report = Instant::now();
        }

        if h_event.wait_for_event(2000).is_err() {
            eprintln!("\nUYARI: 2 sn olay gelmedi — loopback sessizlikte akış üretmiyor olabilir.");
            eprintln!("Bu, docs/10'daki açık sorulardan biri. Müzik açıp tekrar dene.");
        }
    }
}
