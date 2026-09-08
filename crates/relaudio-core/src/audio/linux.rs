//! Linux ses arka ucu — PulseAudio API'si (PipeWire bunu yerel olarak sağlar).
//!
//! Neden PipeWire API'si değil: Adım 1 spike'ında `pw-cat` ailesi aygıt adlarını
//! çözemedi ve **sessizce sessizlik yakaladı** (docs/10, Bulgu 5). Pulse API'si
//! aynı sunucuya karşı kusursuz çalıştı. `parecord`/`paplay` de bu yolu kullanıyor.
//! Bkz. docs/adr/0004-linux-pulse-api.md

use libpulse_binding::callbacks::ListResult;
use libpulse_binding::context::{Context, FlagSet as ContextFlags, State as ContextState};
use libpulse_binding::mainloop::standard::{IterateResult, Mainloop};
use libpulse_binding::def::BufferAttr;
use libpulse_binding::sample::{Format as PaFormat, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;
use relaudio_proto::{CHANNELS, PCM_PAYLOAD_LEN, SAMPLE_RATE};
use std::cell::RefCell;
use std::rc::Rc;

use super::{Capture, DeviceInfo, DeviceKind, Playback};
use crate::error::{Error, Result};

const APP_NAME: &str = "RelAudio";

fn spec() -> Spec {
    Spec {
        format: PaFormat::S16le,
        channels: CHANNELS as u8,
        rate: SAMPLE_RATE,
    }
}

/// Yakalama tamponu: bir paketlik parça (5 ms).
/// `fragsize` verilmezse sunucu büyük parçalar döndürür ve gecikme artar.
fn capture_attr() -> BufferAttr {
    BufferAttr {
        maxlength: u32::MAX,
        tlength: u32::MAX,
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize: PCM_PAYLOAD_LEN as u32,
    }
}

/// Çalma tamponu. `tlength` kritik: akışın hedef doluluğu budur ve `write()`
/// buna göre bloklar — yani **ses saati pacing'i buradan gelir**. Verilmezse
/// sunucu ~2 saniyelik tampon açıyor, `write()` hemen dönüyor ve jitter buffer
/// salınıma giriyor (dolup taşıyor, sonra aç kalıyor).
fn playback_attr(target_ms: u32) -> BufferAttr {
    let bytes_per_ms = SAMPLE_RATE / 1000 * CHANNELS as u32 * 2;
    let tlength = bytes_per_ms * target_ms;
    BufferAttr {
        maxlength: u32::MAX,
        tlength,
        prebuf: 0, // hemen başla, kendi jitter buffer'ımız birikimi yönetiyor
        minreq: PCM_PAYLOAD_LEN as u32,
        fragsize: u32::MAX,
    }
}

/// Pulse sunucusuna bağlanıp introspection çalıştırır.
///
/// Basit (blocking) mainloop kullanıyoruz; listeleme seyrek bir işlem ve ses
/// yolunda değil, bu yüzden karmaşık bir threaded mainloop'a gerek yok.
fn with_context<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut Mainloop, &mut Context) -> Result<T>,
{
    let mut mainloop =
        Mainloop::new().ok_or_else(|| Error::Enumerate("mainloop kurulamadı".into()))?;
    let mut ctx = Context::new(&mainloop, APP_NAME)
        .ok_or_else(|| Error::Enumerate("context kurulamadı".into()))?;
    ctx.connect(None, ContextFlags::NOFLAGS, None)
        .map_err(|e| Error::Enumerate(format!("sunucuya bağlanılamadı: {e}")))?;

    // Bağlantı hazır olana kadar döndür.
    loop {
        match mainloop.iterate(false) {
            IterateResult::Quit(_) | IterateResult::Err(_) => {
                return Err(Error::Enumerate("mainloop durdu".into()))
            }
            IterateResult::Success(_) => {}
        }
        match ctx.get_state() {
            ContextState::Ready => break,
            ContextState::Failed | ContextState::Terminated => {
                return Err(Error::Enumerate("bağlantı reddedildi".into()))
            }
            _ => {}
        }
    }

    let out = f(&mut mainloop, &mut ctx);
    ctx.disconnect();
    out
}

/// Bir introspection isteği bitene kadar mainloop'u döndürür.
fn pump<T>(mainloop: &mut Mainloop, done: &Rc<RefCell<bool>>, result: Rc<RefCell<T>>) -> Result<T>
where
    T: Default,
{
    while !*done.borrow() {
        match mainloop.iterate(false) {
            IterateResult::Quit(_) | IterateResult::Err(_) => {
                return Err(Error::Enumerate("listeleme yarıda kesildi".into()))
            }
            IterateResult::Success(_) => {}
        }
    }
    Ok(std::mem::take(&mut *result.borrow_mut()))
}

pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    with_context(|mainloop, ctx| {
        // Önce varsayılanların adını al.
        let defaults: Rc<RefCell<(String, String)>> = Rc::new(RefCell::new(Default::default()));
        let done = Rc::new(RefCell::new(false));
        {
            let d = defaults.clone();
            let fin = done.clone();
            ctx.introspect().get_server_info(move |info| {
                let mut d = d.borrow_mut();
                d.0 = info.default_sink_name.as_deref().unwrap_or("").to_string();
                d.1 = info.default_source_name.as_deref().unwrap_or("").to_string();
                *fin.borrow_mut() = true;
            });
        }
        let (def_sink, def_source) =
            pump::<(String, String)>(mainloop, &done, defaults.clone())?;

        let mut devices: Vec<DeviceInfo> = Vec::new();

        // Çıkışlar (sink).
        let sinks: Rc<RefCell<Vec<DeviceInfo>>> = Rc::new(RefCell::new(Vec::new()));
        let done = Rc::new(RefCell::new(false));
        {
            let out = sinks.clone();
            let fin = done.clone();
            let def = def_sink.clone();
            ctx.introspect().get_sink_info_list(move |res| match res {
                ListResult::Item(s) => {
                    let name = s.name.as_deref().unwrap_or("").to_string();
                    out.borrow_mut().push(DeviceInfo {
                        is_default: name == def,
                        name: s.description.as_deref().unwrap_or(&name).to_string(),
                        id: name,
                        kind: DeviceKind::Output,
                    });
                }
                ListResult::End | ListResult::Error => *fin.borrow_mut() = true,
            });
        }
        devices.extend(pump::<Vec<DeviceInfo>>(mainloop, &done, sinks)?);

        // Girişler ve monitörler (source).
        // monitor_of_sink dolu olan kaynak = bir çıkışın monitörü = sistem sesi.
        let sources: Rc<RefCell<Vec<DeviceInfo>>> = Rc::new(RefCell::new(Vec::new()));
        let done = Rc::new(RefCell::new(false));
        {
            let out = sources.clone();
            let fin = done.clone();
            let def = def_source.clone();
            ctx.introspect().get_source_info_list(move |res| match res {
                ListResult::Item(s) => {
                    let name = s.name.as_deref().unwrap_or("").to_string();
                    let is_monitor = s.monitor_of_sink.is_some();
                    out.borrow_mut().push(DeviceInfo {
                        is_default: !is_monitor && name == def,
                        name: s.description.as_deref().unwrap_or(&name).to_string(),
                        id: name,
                        kind: if is_monitor {
                            DeviceKind::Monitor
                        } else {
                            DeviceKind::Input
                        },
                    });
                }
                ListResult::End | ListResult::Error => *fin.borrow_mut() = true,
            });
        }
        devices.extend(pump::<Vec<DeviceInfo>>(mainloop, &done, sources)?);

        // Varsayılan çıkışın monitörünü varsayılan "sistem sesi" olarak işaretle.
        let default_monitor = format!("{def_sink}.monitor");
        for d in devices.iter_mut() {
            if d.kind == DeviceKind::Monitor && d.id == default_monitor {
                d.is_default = true;
            }
        }

        Ok(devices)
    })
}

struct PulseCapture {
    s: Simple,
}

impl Capture for PulseCapture {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.s
            .read(buf)
            .map_err(|e| Error::Stream(format!("okuma başarısız: {e:?}")))?;
        Ok(buf.len())
    }
}

struct PulsePlayback {
    s: Simple,
}

impl Playback for PulsePlayback {
    fn write(&mut self, buf: &[u8]) -> Result<()> {
        self.s
            .write(buf)
            .map_err(|e| Error::Stream(format!("yazma başarısız: {e}")))
    }
}

/// `id` boşsa türün varsayılanını çözer; doluysa **gerçekten var olduğunu
/// doğrular.**
///
/// Doğrulama şart: `pa_simple_new` olmayan bir aygıt adıyla **hata vermiyor**,
/// sessizce varsayılana düşüyor. Yani kullanıcı HDMI monitörünü seçse ve o
/// aygıt kaybolmuş olsa, uygulama hiçbir şey söylemeden kulaklıktan yayın
/// yapardı. docs/10 Bulgu 5'teki "sessiz başarısızlık" tuzağının aynısı.
fn resolve(id: &str, kind: DeviceKind) -> Result<String> {
    if id.is_empty() {
        return super::default_device(kind)?
            .map(|d| d.id)
            .ok_or_else(|| Error::DeviceNotFound(format!("varsayılan {}", kind.as_str())));
    }
    let devices = list_devices()?;
    let found = devices.iter().find(|d| d.id == id);
    match found {
        Some(d) if d.kind == kind => Ok(id.to_string()),
        Some(d) => Err(Error::DeviceNotFound(format!(
            "{id} bir '{}' aygıtı, '{}' bekleniyordu",
            d.kind.as_str(),
            kind.as_str()
        ))),
        None => Err(Error::DeviceNotFound(id.to_string())),
    }
}

pub fn open_capture(id: &str, kind: DeviceKind) -> Result<Box<dyn Capture>> {
    if kind == DeviceKind::Output {
        return Err(Error::Unsupported("çıkış aygıtından yakalama yapılamaz"));
    }
    let dev = resolve(id, kind)?;
    let s = Simple::new(
        None,
        APP_NAME,
        Direction::Record,
        Some(&dev),
        "capture",
        &spec(),
        None,
        Some(&capture_attr()),
    )
    .map_err(|e| Error::DeviceOpen {
        device: dev.clone(),
        source_msg: e.to_string().unwrap_or_else(|| format!("{e:?}")),
    })?;
    log::info!("yakalama açıldı: {dev} ({})", kind.as_str());
    Ok(Box::new(PulseCapture { s }))
}

pub fn open_playback(id: &str) -> Result<Box<dyn Playback>> {
    let dev = resolve(id, DeviceKind::Output)?;
    let s = Simple::new(
        None,
        APP_NAME,
        Direction::Playback,
        Some(&dev),
        "playback",
        &spec(),
        None,
        Some(&playback_attr(60)),
    )
    .map_err(|e| Error::DeviceOpen {
        device: dev.clone(),
        source_msg: e.to_string().unwrap_or_else(|| format!("{e:?}")),
    })?;
    log::info!("çalma açıldı: {dev}");
    Ok(Box::new(PulsePlayback { s }))
}
