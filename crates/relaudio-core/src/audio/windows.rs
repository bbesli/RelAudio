//! Windows ses arka ucu — WASAPI (`wasapi` crate).
//!
//! Adım 1b spike'ında doğrulanan yolu izler (docs/10, Adım 1b):
//!   - Sistem sesi: **Render** aygıtı + **Capture** yönü → crate
//!     `AUDCLNT_STREAMFLAGS_LOOPBACK` bayrağını ekler.
//!   - Mikrofon: Capture aygıtı + Capture yönü.
//!
//! Aygıt kimliği olarak WASAPI'nin kararlı endpoint id'si kullanılır; görünen
//! ada göre eşleştirme yapılmaz (docs/10, Bulgu 5).

use std::collections::VecDeque;

use relaudio_proto::{CHANNELS, SAMPLE_RATE};
use wasapi::{
    initialize_mta, AudioCaptureClient, AudioClient, AudioRenderClient, Device, DeviceCollection,
    DeviceEnumerator, Direction, Handle, SampleType, StreamMode, WaveFormat,
};

use super::{Capture, DeviceInfo, DeviceKind, Playback};
use crate::error::{Error, Result};

const BITS: usize = 16;

fn com_init() {
    // Süreç başına bir kez yeterli; tekrar çağrılması zararsız.
    let _ = initialize_mta();
}

fn wasapi_err(what: &str, e: impl std::fmt::Display) -> Error {
    Error::Stream(format!("{what}: {e}"))
}

fn format() -> WaveFormat {
    WaveFormat::new(
        BITS,
        BITS,
        &SampleType::Int,
        SAMPLE_RATE as usize,
        CHANNELS as usize,
        None,
    )
}

fn collect(dir: Direction, kind: DeviceKind, default_id: &str, out: &mut Vec<DeviceInfo>) {
    let Ok(enumerator) = DeviceEnumerator::new() else {
        return;
    };
    let Ok(collection) = enumerator.get_device_collection(&dir) else {
        return;
    };
    let count = collection.get_nbr_devices().unwrap_or(0);
    for i in 0..count {
        let Ok(dev) = collection.get_device_at_index(i) else {
            continue;
        };
        let (Ok(id), Ok(name)) = (dev.get_id(), dev.get_friendlyname()) else {
            continue;
        };
        out.push(DeviceInfo {
            is_default: id == default_id,
            name,
            id,
            kind,
        });
    }
}

fn default_id(dir: Direction) -> String {
    DeviceEnumerator::new()
        .ok()
        .and_then(|e| e.get_default_device(&dir).ok())
        .and_then(|d| d.get_id().ok())
        .unwrap_or_default()
}

pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    com_init();
    let mut out = Vec::new();
    let def_render = default_id(Direction::Render);
    let def_capture = default_id(Direction::Capture);

    // Render aygıtları iki rolde birden görünür: çıkış olarak çalma hedefi,
    // monitor olarak da loopback kaynağı. Kimlik aynı, tür farklı.
    collect(Direction::Render, DeviceKind::Output, &def_render, &mut out);
    collect(Direction::Render, DeviceKind::Monitor, &def_render, &mut out);
    collect(Direction::Capture, DeviceKind::Input, &def_capture, &mut out);

    if out.is_empty() {
        return Err(Error::Enumerate("hiç ses aygıtı bulunamadı".into()));
    }
    Ok(out)
}

fn find_device(id: &str, dir: Direction) -> Result<Device> {
    let enumerator =
        DeviceEnumerator::new().map_err(|e| wasapi_err("aygıt sayıcı kurulamadı", e))?;
    if id.is_empty() {
        return enumerator
            .get_default_device(&dir)
            .map_err(|e| wasapi_err("varsayılan aygıt alınamadı", e));
    }
    let collection: DeviceCollection = enumerator
        .get_device_collection(&dir)
        .map_err(|e| wasapi_err("aygıtlar listelenemedi", e))?;
    let count = collection.get_nbr_devices().unwrap_or(0);
    for i in 0..count {
        if let Ok(dev) = collection.get_device_at_index(i) {
            if dev.get_id().map(|d| d == id).unwrap_or(false) {
                return Ok(dev);
            }
        }
    }
    Err(Error::DeviceNotFound(id.to_string()))
}

struct WasapiCapture {
    client: AudioClient,
    capture: AudioCaptureClient,
    event: Handle,
    queue: VecDeque<u8>,
}

impl Capture for WasapiCapture {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        while self.queue.len() < buf.len() {
            self.capture
                .read_from_device_to_deque(&mut self.queue)
                .map_err(|e| wasapi_err("yakalama okunamadı", e))?;
            if self.queue.len() >= buf.len() {
                break;
            }
            // Hiçbir uygulama çalmıyorken loopback HİÇ veri üretmez ve olay
            // tetiklenmez (docs/10, Bulgu 6). Zaman aşımında sessizlik üretip
            // akışı sürdürüyoruz; gönderici bunu sessizlik bayrağıyla yollar.
            if self.event.wait_for_event(200).is_err() {
                let missing = buf.len() - self.queue.len();
                self.queue.extend(std::iter::repeat(0u8).take(missing));
            }
        }
        for b in buf.iter_mut() {
            *b = self.queue.pop_front().unwrap_or(0);
        }
        Ok(buf.len())
    }
}

impl Drop for WasapiCapture {
    fn drop(&mut self) {
        let _ = self.client.stop_stream();
    }
}

pub fn open_capture(id: &str, kind: DeviceKind) -> Result<Box<dyn Capture>> {
    com_init();
    let (dir, stream_dir) = match kind {
        // Loopback: Render aygıtı, Capture yönü.
        DeviceKind::Monitor => (Direction::Render, Direction::Capture),
        DeviceKind::Input => (Direction::Capture, Direction::Capture),
        DeviceKind::Output => {
            return Err(Error::Unsupported("çıkış aygıtından yakalama yapılamaz"))
        }
    };

    let device = find_device(id, dir)?;
    let mut client = device
        .get_iaudioclient()
        .map_err(|e| wasapi_err("audio client alınamadı", e))?;
    let (_def_period, min_period) = client
        .get_device_period()
        .map_err(|e| wasapi_err("aygıt periyodu okunamadı", e))?;

    // Shared mode istenen tamponu vermiyor, motor periyodunu dayatıyor
    // (docs/10, Bulgu 7). Yine de minimumu istiyoruz.
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: min_period,
    };
    client
        .initialize_client(&format(), &stream_dir, &mode)
        .map_err(|e| Error::DeviceOpen {
            device: id.to_string(),
            source_msg: e.to_string(),
        })?;
    let event = client
        .set_get_eventhandle()
        .map_err(|e| wasapi_err("olay tanıtıcısı alınamadı", e))?;
    let capture = client
        .get_audiocaptureclient()
        .map_err(|e| wasapi_err("capture client alınamadı", e))?;
    client
        .start_stream()
        .map_err(|e| wasapi_err("akış başlatılamadı", e))?;

    log::info!("yakalama açıldı ({})", kind.as_str());
    Ok(Box::new(WasapiCapture {
        client,
        capture,
        event,
        queue: VecDeque::with_capacity(1 << 16),
    }))
}

struct WasapiPlayback {
    client: AudioClient,
    render: AudioRenderClient,
    event: Handle,
    queue: VecDeque<u8>,
    blockalign: usize,
    written: u64,
    /// İlk N yazma işlemini logla, sonra sus.
    log_until: u32,
}

impl Playback for WasapiPlayback {
    fn write(&mut self, buf: &[u8]) -> Result<()> {
        self.queue.extend(buf.iter().copied());

        // Kuyruk tavanı. Cihaz veri kabul etmeyi bırakırsa (olay zaman aşımı)
        // aşağıdaki döngü veriyi kuyrukta bırakıp çıkıyor. Tavan olmadan bu
        // kuyruk sınırsız büyür ve süreç eninde sonunda belleği tüketir —
        // "birkaç dakika sonra çöküyor" belirtisinin muhtemel kaynağı.
        let cap = SAMPLE_RATE as usize * self.blockalign; // 1 saniye
        if self.queue.len() > cap {
            let drop = self.queue.len() - cap;
            self.queue.drain(..drop);
            log::warn!("çalma kuyruğu taştı, {drop} bayt atıldı — cihaz veri kabul etmiyor");
        }

        // Kuyruğu boşaltana kadar yaz.
        //
        // Önceki sürüm yalnızca kuyrukta **tam bir cihaz tamponu kadar** veri
        // varken yazıyordu (`queue.len() < space * blockalign` ise hiç yazmıyordu).
        // Bize 5 ms'lik (960 bayt) bloklar geliyor, cihaz tamponu ise 22 ms
        // (~4224 bayt) — yani çoğu çağrıda hiçbir şey yazılmıyordu ve cihaz
        // tamponu sürekli aç kalıyordu. Sonuç: paket geliyor ama ses yok.
        //
        // Doğrusu: ne kadar yer varsa o kadar yaz, kısmi de olsa. Cihaz tamponu
        // dolduğunda olayı bekle — pacing (ses saati) buradan gelir.
        while self.queue.len() >= self.blockalign {
            let space = self
                .client
                .get_available_space_in_frames()
                .map_err(|e| wasapi_err("boş alan sorgulanamadı", e))? as usize;

            if space == 0 {
                // Cihaz tamponu dolu; bir sonraki tampon boşalmasını bekle.
                if self.event.wait_for_event(200).is_err() {
                    // Cihaz yanıt vermiyor. Kalan veri kuyrukta duruyor,
                    // bir sonraki çağrıda tekrar denenecek.
                    break;
                }
                continue;
            }

            let frames = space.min(self.queue.len() / self.blockalign);
            if frames == 0 {
                break;
            }
            self.render
                .write_to_device_from_deque(frames, &mut self.queue, None)
                .map_err(|e| wasapi_err("çalma yazılamadı", e))?;

            // İlk saniyede ne olduğunu görmek için; sonra susar.
            self.written += frames as u64;
            if self.log_until > 0 {
                self.log_until -= 1;
                log::info!(
                    "çalma: {frames} kare yazıldı (boş alan {space}), kuyrukta {} bayt kaldı",
                    self.queue.len()
                );
            }
        }
        Ok(())
    }
}

impl Drop for WasapiPlayback {
    fn drop(&mut self) {
        let _ = self.client.stop_stream();
    }
}

pub fn open_playback(id: &str) -> Result<Box<dyn Playback>> {
    com_init();
    let device = find_device(id, Direction::Render)?;
    let mut client = device
        .get_iaudioclient()
        .map_err(|e| wasapi_err("audio client alınamadı", e))?;
    let (_def, min_period) = client
        .get_device_period()
        .map_err(|e| wasapi_err("aygıt periyodu okunamadı", e))?;
    let fmt = format();
    let blockalign = fmt.get_blockalign() as usize;

    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: min_period,
    };
    client
        .initialize_client(&fmt, &Direction::Render, &mode)
        .map_err(|e| Error::DeviceOpen {
            device: id.to_string(),
            source_msg: e.to_string(),
        })?;
    let event = client
        .set_get_eventhandle()
        .map_err(|e| wasapi_err("olay tanıtıcısı alınamadı", e))?;
    let render = client
        .get_audiorenderclient()
        .map_err(|e| wasapi_err("render client alınamadı", e))?;
    client
        .start_stream()
        .map_err(|e| wasapi_err("akış başlatılamadı", e))?;

    let buffer_frames = client.get_buffer_size().unwrap_or(0);
    log::info!(
        "çalma açıldı — tampon {} kare ({:.1} ms), periyot varsayılan {:.1} ms / min {:.1} ms, blockalign {} bayt",
        buffer_frames,
        buffer_frames as f64 / SAMPLE_RATE as f64 * 1000.0,
        _def as f64 / 10_000.0,
        min_period as f64 / 10_000.0,
        blockalign
    );
    Ok(Box::new(WasapiPlayback {
        client,
        render,
        event,
        queue: VecDeque::with_capacity(1 << 16),
        blockalign,
        written: 0,
        log_until: 10,
    }))
}
