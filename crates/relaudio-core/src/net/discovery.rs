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
    /// Kontrol kanalının TCP portu (`docs/05`, TXT alanı `cport`). Eş eski
    /// bir sürümse ya da alan yoksa 0 gelir — o zaman uzaktan başlatma
    /// denenmez, kullanıcıya "karşı taraf desteklemiyor" denir.
    pub control_port: u16,
    pub os: String,
    /// Bu eş şu anda dinliyor mu (ses alabilir mi)?
    pub listening: bool,
}

pub struct Discovery {
    daemon: ServiceDaemon,
    peers: Arc<Mutex<HashMap<String, Peer>>>,
    instance: String,
    /// İlan edilen port. Kullanıcı oynatıcı portunu değiştirdiğinde
    /// güncellenmeli, yoksa eşler eski porta yayın yapıyor ve hiçbir şey
    /// ulaşmıyor.
    port: std::sync::atomic::AtomicU16,
    /// İlan edilen kontrol portu. 59100 doluysa farklı olabiliyor.
    control_port: std::sync::atomic::AtomicU16,
    self_name: String,
    /// Bu sürece özgü kimlik. Kendimizi eş listesinden ayıklamak için.
    instance_id: String,
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

/// Bir IPv4 adresinin ilk üç sekizlisi ("192.168.1.113" → "192.168.1.").
fn subnet_of(ip: &str) -> Option<String> {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() == 4 {
        Some(format!("{}.{}.{}.", parts[0], parts[1], parts[2]))
    } else {
        None
    }
}

/// Eşin ilan ettiği adresler arasından en olası ulaşılabilir olanı seçer.
///
/// Sıra: bizimle aynı /24'te olan IPv4 → herhangi bir IPv4 → herhangi biri.
/// mDNS hem IPv4 hem IPv6, hem de VPN/sanal arayüzlerin adreslerini
/// döndürüyor; ilkini almak link-local IPv6 veya erişilemez bir VPN adresi
/// seçmeye yol açıyordu.
fn pick_address(addrs: &[std::net::IpAddr], own_subnet: Option<&String>) -> Option<String> {
    let v4: Vec<&std::net::IpAddr> = addrs.iter().filter(|a| a.is_ipv4()).collect();
    if let Some(prefix) = own_subnet {
        if let Some(a) = v4.iter().find(|a| a.to_string().starts_with(prefix.as_str())) {
            return Some(a.to_string());
        }
    }
    v4.first()
        .map(|a| a.to_string())
        .or_else(|| addrs.first().map(|a| a.to_string()))
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
    pub fn start(port: u16, control_port: u16) -> Result<Self> {
        let daemon = ServiceDaemon::new()
            .map_err(|e| Error::Stream(format!("could not start mDNS: {e}")))?;

        let name = device_name();
        // Örnek adı ağda tekil olmalı; ad + os yeterince ayırt edici.
        let instance = format!("{name}-{}", os_name());

        // Örnek adına göre kendini ayıklamak yetmiyor: mDNS aynı ad ağda
        // zaten varsa (örneğin uygulama hızlı yeniden başlatıldığında eski
        // kayıt hâlâ duruyorsa) adı değiştirerek kaydediyor. O zaman cihaz
        // kendini eş olarak listeliyordu. Sürece özgü bir kimlik TXT'ye
        // yazılıp ona göre süzülüyor.
        let instance_id = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );

        let peers: Arc<Mutex<HashMap<String, Peer>>> = Arc::new(Mutex::new(HashMap::new()));

        let receiver = daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| Error::Stream(format!("could not start mDNS browse: {e}")))?;

        let map = peers.clone();
        let own = instance.clone();
        let own_iid = instance_id.clone();
        // Kendi adresimizin /24'ü — eşin hangi arayüzünü seçeceğimize karar
        // vermek için. VPN adaptörü olan makineler birden fazla adres ilan
        // ediyor ve yanlışını seçersek paketler hiçbir yere gitmiyor.
        let own_subnet = super::local_address().and_then(|a| subnet_of(&a));
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
                            // Kendimizi listeleme — önce kimliğe, sonra ada bak.
                            let peer_iid = info
                                .get_property_val_str("iid")
                                .unwrap_or_default()
                                .to_string();
                            if peer_iid == own_iid || inst == own {
                                continue;
                            }
                            let addrs: Vec<std::net::IpAddr> =
                                info.get_addresses().iter().map(|a| a.to_ip_addr()).collect();
                            let Some(addr) = pick_address(&addrs, own_subnet.as_ref()) else {
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
                                control_port: get("cport").parse().unwrap_or(0),
                                address: addr,
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
            .map_err(|e| Error::Stream(format!("could not spawn mDNS thread: {e}")))?;

        let d = Discovery {
            daemon,
            peers,
            instance,
            port: std::sync::atomic::AtomicU16::new(port),
            control_port: std::sync::atomic::AtomicU16::new(control_port),
            self_name: name,
            instance_id,
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
            (
                "cport".to_string(),
                self.control_port.load(std::sync::atomic::Ordering::Relaxed).to_string(),
            ),
            ("iid".to_string(), self.instance_id.clone()),
        ]
        .into_iter()
        .collect();

        let info = ServiceInfo::new(
            SERVICE_TYPE,
            &self.instance,
            &format!("{}.local.", self.instance),
            (),
            self.port.load(std::sync::atomic::Ordering::Relaxed),
            props,
        )
        .map_err(|e| Error::Stream(format!("could not build mDNS service: {e}")))?
        // Adresleri işletim sisteminden otomatik al; elle IP vermek
        // çok arayüzlü makinelerde yanlış adres ilan etmeye yol açıyor.
        .enable_addr_auto();

        self.daemon
            .register(info)
            .map_err(|e| Error::Stream(format!("could not register mDNS service: {e}")))?;
        Ok(())
    }

    /// İlan edilen portu değiştirir ve yeniden ilan eder.
    pub fn set_port(&self, port: u16, listening: bool) -> Result<()> {
        self.port.store(port, std::sync::atomic::Ordering::Relaxed);
        self.announce(listening)
    }

    /// İlan edilen kontrol portunu değiştirir. Kontrol sunucusu ilandan
    /// sonra başlarsa (port çakışması yüzünden farklı bir porta düşmüşse)
    /// bu çağrılmazsa eşler yanlış kapıyı çalar.
    pub fn set_control_port(&self, port: u16, listening: bool) -> Result<()> {
        self.control_port.store(port, std::sync::atomic::Ordering::Relaxed);
        self.announce(listening)
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


#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn prefers_an_address_on_our_own_subnet() {
        // VPN'li bir makine hem 10.16.x hem 192.168.1.x ilan ediyor.
        let addrs = vec![ip("10.16.48.2"), ip("192.168.1.110")];
        let own = "192.168.1.".to_string();
        assert_eq!(
            pick_address(&addrs, Some(&own)).as_deref(),
            Some("192.168.1.110")
        );
    }

    #[test]
    fn prefers_ipv4_over_ipv6() {
        let addrs = vec![ip("fe80::1"), ip("192.168.1.110")];
        assert_eq!(
            pick_address(&addrs, None).as_deref(),
            Some("192.168.1.110")
        );
    }

    #[test]
    fn falls_back_to_whatever_exists() {
        assert_eq!(pick_address(&[ip("fe80::1")], None).as_deref(), Some("fe80::1"));
        assert_eq!(pick_address(&[], None), None);
    }

    #[test]
    fn extracts_the_subnet_prefix() {
        assert_eq!(subnet_of("192.168.1.113").as_deref(), Some("192.168.1."));
        assert_eq!(subnet_of("::1"), None);
    }
}
