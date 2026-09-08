//! Yerel ağ keşfi — mDNS / DNS-SD.
//!
//! Her RelAudio örneği kendini ilan eder ve diğerlerini dinler. Böylece
//! kullanıcı IP yazmak zorunda kalmaz — yanlış IP'nin belirtisi "hiç paket
//! gelmiyor" oluyor ve sebebi anlaşılmıyor.
//!
//! Servis tipi ve TXT alanları `docs/05-ag-protokolu.md` ile uyumlu.

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{Error, Result};

pub const SERVICE_TYPE: &str = "_relaudio._udp.local.";

/// Ağda bulunan bir RelAudio örneği.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    /// Kalıcı kimlik (mDNS örnek adı). Aynı cihaz yeniden başlasa da aynı kalır.
    pub id: String,
    /// Kullanıcıya gösterilecek ad.
    pub name: String,
    pub address: String,
    pub port: u16,
    pub os: String,
    /// Bu eş şu anda dinliyor mu (ses alabilir mi)?
    pub listening: bool,
}

pub struct Discovery {
    daemon: ServiceDaemon,
    peers: Arc<Mutex<HashMap<String, Peer>>>,
    instance: String,
    port: u16,
    self_name: String,
}

fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// Bu makinenin kullanıcıya gösterilecek adı.
pub fn device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| os_name().to_string())
}

impl Discovery {
    /// Keşfi başlatır: kendini ilan eder ve diğerlerini dinlemeye başlar.
    pub fn start(port: u16) -> Result<Self> {
        let daemon = ServiceDaemon::new()
            .map_err(|e| Error::Stream(format!("mDNS başlatılamadı: {e}")))?;

        let name = device_name();
        // Örnek adı ağda tekil olmalı; ad + os yeterince ayırt edici.
        let instance = format!("{name}-{}", os_name());

        let peers: Arc<Mutex<HashMap<String, Peer>>> = Arc::new(Mutex::new(HashMap::new()));

        let receiver = daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| Error::Stream(format!("mDNS taraması başlatılamadı: {e}")))?;

        let map = peers.clone();
        let own = instance.clone();
        std::thread::Builder::new()
            .name("relaudio-mdns".into())
            .spawn(move || {
                while let Ok(event) = receiver.recv() {
                    match event {
                        ServiceEvent::ServiceResolved(info) => {
                            let inst = info
                                .get_fullname()
                                .split('.')
                                .next()
                                .unwrap_or("")
                                .to_string();
                            // Kendimizi listeleme.
                            if inst == own {
                                continue;
                            }
                            // IPv4 tercih et. mDNS hem IPv4 hem IPv6 döndürüyor;
                            // ilkini almak link-local IPv6 seçmeye yol açıyordu
                            // ve kullanıcıya anlamsız bir adres gösteriyordu.
                            let addrs = info.get_addresses();
                            let Some(addr) = addrs
                                .iter()
                                .find(|a| a.to_ip_addr().is_ipv4())
                                .or_else(|| addrs.iter().next())
                                .cloned()
                            else {
                                continue;
                            };
                            let get = |k: &str| {
                                info.get_property_val_str(k).unwrap_or_default().to_string()
                            };
                            let peer = Peer {
                                name: {
                                    let n = get("name");
                                    if n.is_empty() { inst.clone() } else { n }
                                },
                                os: get("os"),
                                listening: get("listening") == "1",
                                address: addr.to_string(),
                                port: info.get_port(),
                                id: inst.clone(),
                            };
                            log::info!("eş bulundu: {} ({}:{})", peer.name, peer.address, peer.port);
                            map.lock().unwrap().insert(inst, peer);
                        }
                        ServiceEvent::ServiceRemoved(_, fullname) => {
                            let inst = fullname.split('.').next().unwrap_or("").to_string();
                            if map.lock().unwrap().remove(&inst).is_some() {
                                log::info!("eş kayboldu: {inst}");
                            }
                        }
                        _ => {}
                    }
                }
                log::info!("mDNS tarama döngüsü bitti");
            })
            .map_err(|e| Error::Stream(format!("mDNS thread'i başlatılamadı: {e}")))?;

        let d = Discovery {
            daemon,
            peers,
            instance,
            port,
            self_name: name,
        };
        d.announce(false)?;
        Ok(d)
    }

    /// Kendini ağa ilan eder. `listening`, şu anda ses alıp alamadığımızı söyler;
    /// karşı taraf kime gönderebileceğini böyle biliyor.
    pub fn announce(&self, listening: bool) -> Result<()> {
        let props: HashMap<String, String> = [
            ("v".to_string(), "1".to_string()),
            ("name".to_string(), self.self_name.clone()),
            ("os".to_string(), os_name().to_string()),
            ("listening".to_string(), if listening { "1" } else { "0" }.to_string()),
        ]
        .into_iter()
        .collect();

        let info = ServiceInfo::new(
            SERVICE_TYPE,
            &self.instance,
            &format!("{}.local.", self.instance),
            (),
            self.port,
            props,
        )
        .map_err(|e| Error::Stream(format!("mDNS servisi kurulamadı: {e}")))?
        // Adresleri işletim sisteminden otomatik al; elle IP vermek
        // çok arayüzlü makinelerde yanlış adres ilan etmeye yol açıyor.
        .enable_addr_auto();

        self.daemon
            .register(info)
            .map_err(|e| Error::Stream(format!("mDNS kaydı yapılamadı: {e}")))?;
        Ok(())
    }

    pub fn peers(&self) -> Vec<Peer> {
        let mut v: Vec<Peer> = self.peers.lock().unwrap().values().cloned().collect();
        v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        v
    }

    pub fn shutdown(&self) {
        let _ = self.daemon.unregister(&format!("{}.{}", self.instance, SERVICE_TYPE));
        let _ = self.daemon.shutdown();
    }
}
