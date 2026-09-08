//! Çalışan yayın oturumlarının yaşam döngüsü.
//!
//! Her oturum kendi thread'inde çalışır. Ses döngüleri bloklayıcı olduğu için
//! (Pulse `Simple`, WASAPI event) thread başına bir akış modeli doğru olanı.
//! Durdurma [`Stopper`] üzerinden; thread bir sonraki blok sınırında çıkar.

use relaudio_core::audio::DeviceKind;
use relaudio_core::net::{receive_loop, send_loop, ReceiverStats, SenderStats, Stopper};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::StatsDto;

/// Kimlikten görünen ada çevirir. Boş kimlik = o türün varsayılanı.
fn device_label(id: &str, kind: DeviceKind) -> (String, String) {
    let devices = relaudio_core::audio::list_devices().unwrap_or_default();
    let found = if id.is_empty() {
        devices.iter().find(|d| d.kind == kind && d.is_default)
    } else {
        devices.iter().find(|d| d.id == id && d.kind == kind)
    };
    match found {
        Some(d) => (d.id.clone(), d.name.clone()),
        None => (id.to_string(), id.to_string()),
    }
}

/// Bir saniyelik pencerede bit hızı hesaplamak için.
struct Rate {
    last_bytes: u64,
    last_at: Instant,
    kbps: f64,
}

impl Rate {
    fn new() -> Self {
        Self { last_bytes: 0, last_at: Instant::now(), kbps: 0.0 }
    }
    fn update(&mut self, bytes: u64) -> f64 {
        let dt = self.last_at.elapsed().as_secs_f64();
        if dt >= 0.5 {
            self.kbps = (bytes.saturating_sub(self.last_bytes)) as f64 * 8.0 / 1000.0 / dt;
            self.last_bytes = bytes;
            self.last_at = Instant::now();
        }
        self.kbps
    }
}

struct Running<S> {
    stop: Stopper,
    stats: Arc<S>,
    handle: Option<std::thread::JoinHandle<()>>,
    label: String,
    rate: Rate,
    /// Akışın açtığı aygıtın kimliği ve görünen adı — seçili olan değil,
    /// fiilen kullanılan.
    device_id: String,
    device_name: String,
    /// Thread'in başlangıçtan sonra ölmesi hâlinde hatayı buraya yazar.
    /// Aygıt çıkarılması gibi durumlarda arayüz "Yayında" demeye devam
    /// etmemeli — [`Running::died`] bunu yakalar.
    err: Arc<Mutex<Option<String>>>,
}

impl<S> Running<S> {
    /// Thread bittiyse (yani yayın fiilen durduysa) sebebini döndürür.
    fn died(&self) -> Option<Option<String>> {
        match &self.handle {
            Some(h) if h.is_finished() => Some(self.err.lock().unwrap().clone()),
            None => Some(None),
            _ => None,
        }
    }
}

impl<S> Running<S> {
    fn shut_down(&mut self) {
        self.stop.stop();
        if let Some(h) = self.handle.take() {
            // Ses döngüleri bloklayıcı okuma yapıyor; join burada birkaç yüz ms
            // bekleyebilir. Kullanıcı "durdur"a bastığında bu kabul edilebilir.
            let _ = h.join();
        }
    }
}

#[derive(Default)]
pub struct ServerSession {
    inner: Mutex<Option<Running<SenderStats>>>,
    error: Mutex<Option<String>>,
}

impl ServerSession {
    pub fn start(&self, device_id: &str, kind: DeviceKind, target: &str) -> Result<(), String> {
        self.stop();
        let stop = Stopper::new();
        let stats = Arc::new(SenderStats::default());

        // Aygıtı burada değil thread'de açıyoruz; ama hatayı kullanıcıya
        // gösterebilmek için ilk hatayı paylaşılan alana yazıyoruz.
        let (t_stop, t_stats) = (stop.clone(), stats.clone());
        let (dev, target_s) = (device_id.to_string(), target.to_string());
        let err_slot = Arc::new(Mutex::new(None::<String>));
        let err_thread = err_slot.clone();

        let handle = std::thread::Builder::new()
            .name("relaudio-server".into())
            .spawn(move || {
                if let Err(e) = send_loop(&dev, kind, &target_s, t_stop, t_stats) {
                    log::error!("gönderim hatası: {e}");
                    *err_thread.lock().unwrap() = Some(e.to_string());
                }
            })
            .map_err(|e| format!("thread başlatılamadı: {e}"))?;

        // Aygıt açma hatası ilk 300 ms içinde ortaya çıkar; kullanıcıya
        // "başladı" deyip sonra sessizce ölmemek için kısa süre bekliyoruz.
        std::thread::sleep(std::time::Duration::from_millis(300));
        if let Some(e) = err_slot.lock().unwrap().clone() {
            return Err(e);
        }

        let (device_id, device_name) = device_label(device_id, kind);
        *self.inner.lock().unwrap() = Some(Running {
            stop,
            stats,
            handle: Some(handle),
            label: target.to_string(),
            rate: Rate::new(),
            err: err_slot,
            device_id,
            device_name,
        });
        *self.error.lock().unwrap() = None;
        Ok(())
    }

    pub fn stop(&self) {
        if let Some(mut r) = self.inner.lock().unwrap().take() {
            r.shut_down();
        }
    }

    pub fn fill(&self, out: &mut StatsDto) {
        let mut guard = self.inner.lock().unwrap();
        // Thread beklenmedik şekilde öldüyse durumu temizle; aksi hâlde arayüz
        // sonsuza dek "Yayında" gösterir ve kullanıcı neden ses gelmediğini
        // anlamaz.
        if guard.as_ref().and_then(|r| r.died()).is_some() {
            // take() + shut_down(): sadece bırakmak yetmiyor, Stopper'ın
            // Drop'u yok. Thread ölmüş olsa bile bayrağı set edip join
            // ediyoruz ki hiçbir yol açık kaynak bırakmasın.
            let mut r = guard.take().expect("az önce Some'du");
            let reason = r.err.lock().unwrap().clone();
            r.shut_down();
            *self.error.lock().unwrap() =
                Some(reason.unwrap_or_else(|| "Gönderim beklenmedik şekilde durdu".into()));
        }
        if let Some(r) = guard.as_mut() {
            let (packets, bytes, silent, errors) = r.stats.snapshot();
            out.server_send_errors = errors;
            out.server_running = true;
            out.server_target = r.label.clone();
            out.server_packets = packets;
            out.server_kbps = r.rate.update(bytes);
            out.server_device_name = r.device_name.clone();
            out.server_device_id = r.device_id.clone();
            out.server_silent_ratio = if packets > 0 {
                silent as f64 / packets as f64
            } else {
                0.0
            };
        }
        if let Some(e) = self.error.lock().unwrap().take() {
            out.last_error = Some(e);
        }
    }
}

#[derive(Default)]
pub struct PlayerSession {
    inner: Mutex<Option<Running<ReceiverStats>>>,
    error: Mutex<Option<String>>,
}

impl PlayerSession {
    pub fn start(&self, port: u16, device_id: &str, buffer_packets: usize) -> Result<(), String> {
        self.stop();
        let stop = Stopper::new();
        let stats = Arc::new(ReceiverStats::default());
        let (t_stop, t_stats) = (stop.clone(), stats.clone());
        let dev = device_id.to_string();
        let dev_for_label = device_id.to_string();
        let buf = buffer_packets.clamp(2, 200);

        let err_slot = Arc::new(Mutex::new(None::<String>));
        let err_thread = err_slot.clone();

        let handle = std::thread::Builder::new()
            .name("relaudio-player".into())
            .spawn(move || {
                if let Err(e) = receive_loop(port, &dev, buf, t_stop, t_stats) {
                    log::error!("alım hatası: {e}");
                    *err_thread.lock().unwrap() = Some(e.to_string());
                }
            })
            .map_err(|e| format!("thread başlatılamadı: {e}"))?;

        std::thread::sleep(std::time::Duration::from_millis(300));
        if let Some(e) = err_slot.lock().unwrap().clone() {
            return Err(e);
        }

        let (device_id, device_name) = device_label(&dev_for_label, DeviceKind::Output);
        *self.inner.lock().unwrap() = Some(Running {
            stop,
            stats,
            handle: Some(handle),
            label: port.to_string(),
            rate: Rate::new(),
            err: err_slot,
            device_id,
            device_name,
        });
        *self.error.lock().unwrap() = None;
        Ok(())
    }

    pub fn stop(&self) {
        if let Some(mut r) = self.inner.lock().unwrap().take() {
            r.shut_down();
        }
    }

    pub fn fill(&self, out: &mut StatsDto) {
        let mut guard = self.inner.lock().unwrap();
        if guard.as_ref().and_then(|r| r.died()).is_some() {
            let mut r = guard.take().expect("az önce Some'du");
            let reason = r.err.lock().unwrap().clone();
            r.shut_down();
            *self.error.lock().unwrap() =
                Some(reason.unwrap_or_else(|| "Alım beklenmedik şekilde durdu".into()));
        }
        if let Some(r) = guard.as_mut() {
            let s = &r.stats;
            out.player_running = true;
            out.player_port = r.label.parse().unwrap_or(0);
            out.player_packets = s.packets.load(Ordering::Relaxed);
            out.player_kbps = r.rate.update(s.bytes.load(Ordering::Relaxed));
            out.player_lost = s.lost.load(Ordering::Relaxed);
            out.player_late = s.late.load(Ordering::Relaxed);
            out.player_underruns = s.underruns.load(Ordering::Relaxed);
            out.player_dropped = s.dropped.load(Ordering::Relaxed);
            out.player_buffer_ms = s.buffer_depth.load(Ordering::Relaxed) * 5;
            out.player_peak = s.peak.load(Ordering::Relaxed);
            out.player_device_name = r.device_name.clone();
            out.player_device_id = r.device_id.clone();
        }
        if let Some(e) = self.error.lock().unwrap().take() {
            out.last_error = Some(e);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use relaudio_core::audio::DeviceKind;

    /// Aygıt açılamadığında kullanıcıya "başladı" denmemeli.
    /// Bu, oturum katmanındaki en kritik davranış: `send_loop` thread içinde
    /// çalıştığı için hata doğrudan `start`'tan dönmüyor, kısa bir bekleme ile
    /// yakalanıyor. O mekanizma bozulursa arayüz sessizce yalan söyler.
    #[test]
    fn server_start_reports_bad_device_instead_of_pretending_to_run() {
        let s = ServerSession::default();
        let r = s.start("boyle-bir-aygit-yok", DeviceKind::Monitor, "127.0.0.1:59999");
        assert!(r.is_err(), "olmayan aygıtla başarı dönmemeli");

        let mut stats = StatsDto::default();
        s.fill(&mut stats);
        assert!(!stats.server_running, "başarısız başlangıç çalışıyor görünmemeli");
    }

    #[test]
    fn player_start_reports_bad_device() {
        let p = PlayerSession::default();
        let r = p.start(59998, "boyle-bir-aygit-yok", 8);
        assert!(r.is_err());
        let mut stats = StatsDto::default();
        p.fill(&mut stats);
        assert!(!stats.player_running);
    }

    #[test]
    fn stopping_an_idle_session_is_harmless() {
        ServerSession::default().stop();
        PlayerSession::default().stop();
    }

    #[test]
    fn fill_on_idle_session_reports_nothing_running() {
        let mut stats = StatsDto::default();
        ServerSession::default().fill(&mut stats);
        PlayerSession::default().fill(&mut stats);
        assert!(!stats.server_running && !stats.player_running);
        assert!(stats.last_error.is_none());
    }
}
