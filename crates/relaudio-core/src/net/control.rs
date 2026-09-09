//! Kontrol kanalı — satır ayrımlı JSON üzerinden TCP.
//!
//! Ses UDP ile akıyor; bu kanal yalnızca **komut** taşıyor: "kulaklık modunu
//! başlat", "durdur". Amaç kullanıcının iki makinede ayrı ayrı düğmeye
//! basmak zorunda kalmaması.
//!
//! Port ve taşıma `docs/05-ag-protokolu.md`'de ayrılmıştı (TCP 59100).
//! Oradaki tam el sıkışma (HELLO/OFFER/ANSWER, format anlaşması, PIN) hâlâ
//! yazılmadı; burada yalnızca ihtiyaç duyulan iki komut var ve sürüm alanı
//! ileride genişletmeye yer bırakıyor.
//!
//! ## Güvenlik sınırı
//!
//! Bu, ağdaki bir makineye "sesini yakala ve gönder" dedirtebilen bir uç.
//! İki şey bilinçli olarak kısıtlandı:
//!
//! 1. **Hedef adres istekten okunmuyor.** Ses her zaman TCP bağlantısının
//!    geldiği IP'ye gönderiliyor. İsteyen taraf üçüncü bir makineyi hedef
//!    gösteremiyor — yani bu uç bir yansıtma (reflection) aracı değil.
//! 2. **Aygıtları isteyen taraf seçemiyor.** Hangi mikrofonun/hoparlörün
//!    kullanılacağına yalnızca isteği alan makinenin kendi ayarları ve
//!    [`crate::audio::headset_plan`] karar veriyor.
//!
//! 3. **Kaynak adres yerel ağla sınırlı.** Yalnızca loopback, RFC1918 ve
//!    link-local adreslerden gelen bağlantılar kabul ediliyor. Yönlendirilmiş
//!    ya da port yönlendirmesiyle dışarıdan gelen bir bağlantı reddediliyor.
//!
//! 4. **Eşleştirme zorunlu.** Başlatma isteği, iki tarafın 6 haneli bir kodla
//!    bir kez paylaştığı anahtarla imzalanmış olmalı — bkz.
//!    [`crate::net::Pairing`]. Eşleşmemiş bir makine hiçbir şey başlatamıyor.
//!
//! Geriye kalan risk: eşleştirme anındaki tek alışverişi dinleyebilen biri
//! anahtarı görür (anahtar değişimi yok). Ses akışı da zaten şifresiz.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::error::{Error, Result};

/// `docs/05-ag-protokolu.md` tablosundaki kontrol portu.
pub const CONTROL_PORT: u16 = 59100;
/// Port doluysa denenecek ardışık port sayısı. Gerçek port mDNS'te ilan edilir.
pub const PORT_TRIES: u16 = 8;
pub const CONTROL_VERSION: u8 = 1;

/// Tek bir satırın kabul edilen en büyük boyutu. Sınırsız okumak, sonu
/// gelmeyen bir satır gönderen istemcinin belleği şişirmesine izin verirdi.
const MAX_LINE: u64 = 8 * 1024;
/// Aynı anda işlenecek en fazla bağlantı. Üstü reddediliyor; bu uç saniyede
/// bir istek bile almıyor, sınır yalnızca thread patlamasını engelliyor.
const MAX_IN_FLIGHT: u32 = 8;

const IO_TIMEOUT: Duration = Duration::from_secs(3);

/// Bağlantı yerel ağdan mı geliyor?
///
/// Sunucu `0.0.0.0`'a bağlanıyor (çok arayüzlü makinelerde tek adrese
/// bağlanmak çalışmıyor), dolayısıyla filtre kabul anında yapılıyor. Amaç
/// yönlendirilmiş ya da port yönlendirmesiyle gelen bir bağlantının bu ucu
/// internete açmasını engellemek.
pub fn is_link_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            let seg = v6.segments();
            // ULA (fc00::/7) ve link-local (fe80::/10) — std'de kararlı
            // karşılıkları henüz yok, bu yüzden elle.
            v6.is_loopback()
                || (seg[0] & 0xfe00) == 0xfc00
                || (seg[0] & 0xffc0) == 0xfe80
                // IPv4-eşlemli adres (::ffff:192.168.1.5) IPv6 soketinde gelir.
                || v6.to_ipv4_mapped().is_some_and(|v4| {
                    v4.is_loopback() || v4.is_private() || v4.is_link_local()
                })
        }
    }
}
/// Kabul döngüsünün kapanma bayrağına bakma sıklığı.
const ACCEPT_POLL: Duration = Duration::from_millis(250);

/// Karşı taraftan istenen iş.
/// Bir isteğin eşleştirme anahtarıyla imzası.
///
/// Anahtarın kendisi telde tekrarlanmıyor: her istek yeni bir nonce ve zaman
/// damgasıyla imzalanıyor, alıcı taraf pencereyi ve tekrarı denetliyor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Auth {
    /// Bu makinenin kalıcı kimliği (mDNS örnek adı) — anahtarı bulmak için.
    pub id: String,
    pub nonce: String,
    pub ts: u64,
    pub mac: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Request {
    /// "Şu rolü üstlen ve sesi bana gönder."
    ///
    /// `role` **isteği alan** tarafın üstleneceği rol. Hedef adres burada
    /// yok: bilerek TCP bağlantısının kaynak IP'si kullanılıyor.
    StartHeadset {
        /// "local" | "remote" — bkz. [`crate::audio::HeadsetRole`].
        role: String,
        /// İsteyen tarafın ses aldığı UDP portu.
        audio_port: u16,
        /// İsteyen tarafın görünen adı; karşıda ekranda gösteriliyor.
        name: String,
        /// Eşleştirme imzası. Yoksa istek reddediliyor; `#[serde(default)]`
        /// yalnızca eski bir eşin "imzasız" isteğinin *anlaşılır* bir hata
        /// alması için var, kabul edilmesi için değil.
        #[serde(default)]
        auth: Option<Auth>,
    },
    /// "Benim için başlattığın kulaklık modunu durdur."
    StopHeadset,
    /// Karşı tarafın ayakta ve bu sürümü konuşuyor olduğunu doğrular.
    Ping,
    /// "Ekranında duran kod bu; eşleşelim."
    ///
    /// Kodu **gösteren** taraf bu isteği alıyor. Kullanıcı kodu okuduğu
    /// makineden değil, yazdığı makineden gönderiliyor.
    Pair {
        code: String,
        /// Gönderenin kalıcı kimliği; anahtar bu kimliğe karşı saklanıyor.
        id: String,
        name: String,
    },
}

/// Telde giden istek — sürüm alanı dışarıda tutuldu ki ileride gövde
/// değişse bile karşı taraf sürümü okuyabilsin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub v: u8,
    pub body: Request,
}

impl Message {
    pub fn new(body: Request) -> Self {
        Message { v: CONTROL_VERSION, body }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    #[serde(default)]
    pub v: u8,
    pub ok: bool,
    /// Reddedildiyse sebebi. İsteyen tarafın arayüzünde gösteriliyor, bu
    /// yüzden kullanıcıya okunabilir olmalı.
    #[serde(default)]
    pub error: Option<String>,
    /// Cevaplayan makinenin adı.
    #[serde(default)]
    pub name: String,
    /// Uzak rol üstlenildiyse: karşıdaki toplantı uygulamasında seçilecek
    /// mikrofonun adı. İsteyen taraf bunu ekranda gösteriyor, böylece
    /// kullanıcı o makineye bakmadan ne seçeceğini biliyor.
    #[serde(default)]
    pub paired_mic: Option<String>,
    /// Ret sebebinin makine okunur karşılığı ("disabled", "mic_not_allowed",
    /// "need_cable", ...). Arayüz **bunu** çeviriyor: `error` alanı karşı
    /// makinede üretiliyor ve onun dili bizimkiyle aynı olmak zorunda değil.
    /// Eski bir eş göndermezse `None` gelir ve `error` olduğu gibi gösterilir.
    #[serde(default)]
    pub code: Option<String>,
    /// Eşleştirme başarılıysa paylaşılan anahtar (hex) ve kodu gösteren
    /// makinenin kimliği. Yalnızca `Pair` cevabında dolu.
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}

impl Reply {
    pub fn ok(name: String, paired_mic: Option<String>) -> Self {
        Reply {
            v: CONTROL_VERSION,
            ok: true,
            error: None,
            name,
            paired_mic,
            code: None,
            key: None,
            id: None,
        }
    }

    /// Eşleştirme başarılı: paylaşılan anahtarı ve kimliğimizi döndür.
    pub fn paired(name: String, id: String, key: String) -> Self {
        Reply { key: Some(key), id: Some(id), ..Reply::ok(name, None) }
    }

    /// `code` çevrilebilir sebep, `error` insan okunur yedek. İkisi birden
    /// gönderiliyor: kod bilinmeyen bir sürümden geliyorsa metin kalıyor.
    pub fn failed(name: String, code: &str, error: impl Into<String>) -> Self {
        Reply {
            v: CONTROL_VERSION,
            ok: false,
            error: Some(error.into()),
            name,
            paired_mic: None,
            code: Some(code.to_string()),
            key: None,
            id: None,
        }
    }
}

/// Çalışan kontrol sunucusu. Düşürüldüğünde kabul döngüsü durur.
pub struct ControlServer {
    stop: Arc<AtomicBool>,
    port: u16,
}

impl ControlServer {
    /// Fiilen bağlanılan port. 59100 doluysa farklı olabilir; mDNS'te bu
    /// ilan edilmeli, yoksa karşı taraf yanlış kapıyı çalar.
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// `port`'tan başlayarak boş bir port bulur.
fn bind_any(port: u16) -> Result<TcpListener> {
    let mut last = None;
    for offset in 0..PORT_TRIES {
        let p = port.saturating_add(offset);
        match TcpListener::bind(("0.0.0.0", p)) {
            Ok(l) => return Ok(l),
            Err(e) => last = Some((p, e)),
        }
    }
    let (p, e) = last.expect("PORT_TRIES > 0");
    Err(Error::Stream(format!(
        "control port {port}..{p} is not available: {e}"
    )))
}

/// Kontrol sunucusunu başlatır.
///
/// `handler` her istek için çağrılır; ikinci argüman bağlantının geldiği
/// IP'dir ve sesin gönderileceği adres **yalnızca** budur.
pub fn serve<F>(port: u16, handler: F) -> Result<ControlServer>
where
    F: Fn(&Request, IpAddr) -> Reply + Send + Sync + 'static,
{
    let listener = bind_any(port)?;
    let bound = listener
        .local_addr()
        .map(|a| a.port())
        .map_err(|e| Error::Stream(format!("control port could not be read: {e}")))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| Error::Stream(format!("control socket could not be configured: {e}")))?;

    let stop = Arc::new(AtomicBool::new(false));
    let handler = Arc::new(handler);
    let in_flight = Arc::new(AtomicU32::new(0));
    let loop_stop = stop.clone();

    std::thread::Builder::new()
        .name("relaudio-control".into())
        .spawn(move || {
            log::info!("kontrol kanalı dinlemede — port {bound}");
            while !loop_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, peer)) => {
                        if !is_link_local(peer.ip()) {
                            log::warn!("kontrol isteği reddedildi (yerel ağ dışı): {peer}");
                            continue;
                        }
                        // Sınırı aşan bağlantı sessizce kapatılıyor: thread
                        // sayısını bir isteğin belirlemesine izin vermiyoruz.
                        if in_flight.load(Ordering::Relaxed) >= MAX_IN_FLIGHT {
                            log::warn!("kontrol isteği reddedildi (meşgul): {peer}");
                            continue;
                        }
                        in_flight.fetch_add(1, Ordering::Relaxed);
                        let h = handler.clone();
                        let counter = in_flight.clone();
                        let spawned = std::thread::Builder::new()
                            .name("relaudio-control-conn".into())
                            .spawn(move || {
                                if let Err(e) = handle(stream, peer, h.as_ref()) {
                                    log::warn!("kontrol isteği işlenemedi ({peer}): {e}");
                                }
                                counter.fetch_sub(1, Ordering::Relaxed);
                            });
                        if spawned.is_err() {
                            in_flight.fetch_sub(1, Ordering::Relaxed);
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(ACCEPT_POLL);
                    }
                    Err(e) => {
                        log::warn!("kontrol kanalı accept hatası: {e}");
                        std::thread::sleep(ACCEPT_POLL);
                    }
                }
            }
            log::info!("kontrol kanalı durdu");
        })
        .map_err(|e| Error::Stream(format!("could not spawn control thread: {e}")))?;

    Ok(ControlServer { stop, port: bound })
}

/// Tek bir bağlantı: bir satır oku, cevapla, kapat.
fn handle<F>(stream: TcpStream, peer: SocketAddr, handler: &F) -> Result<()>
where
    F: Fn(&Request, IpAddr) -> Reply,
{
    let io = |e: std::io::Error| Error::Stream(e.to_string());
    stream.set_read_timeout(Some(IO_TIMEOUT)).map_err(io)?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).map_err(io)?;

    let mut writer = stream.try_clone().map_err(io)?;
    let mut reader = BufReader::new(stream).take(MAX_LINE);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(io)?;

    // Sürüm gövdeden ÖNCE okunuyor. Doğrudan `Message`'a çözmek gövdeyi de
    // çözmeye çalışıyor; ileride gövde şekli değişirse eski sürüm "malformed
    // request" der ve kullanıcı asıl sebebi (sürüm uyuşmazlığı) hiç görmez.
    let reply = match serde_json::from_str::<serde_json::Value>(line.trim()) {
        Err(e) => Reply::failed(String::new(), "malformed", format!("malformed request: {e}")),
        Ok(value) => match value.get("v").and_then(|v| v.as_u64()) {
            Some(v) if v != CONTROL_VERSION as u64 => Reply::failed(
                String::new(),
                "version",
                format!("unsupported control protocol version {v}"),
            ),
            None => Reply::failed(String::new(), "malformed", "malformed request: no version field"),
            Some(_) => match serde_json::from_value::<Message>(value) {
                Ok(m) => handler(&m.body, peer.ip()),
                Err(e) => Reply::failed(String::new(), "malformed", format!("malformed request: {e}")),
            },
        },
    };

    let mut text = serde_json::to_string(&reply)
        .map_err(|e| Error::Stream(format!("reply could not be encoded: {e}")))?;
    text.push('\n');
    writer.write_all(text.as_bytes()).map_err(io)?;
    writer.flush().map_err(io)?;
    Ok(())
}

/// Karşı tarafa bir istek gönderip cevabı bekler.
///
/// İki ayrı zaman aşımı var ve ayrı olmaları önemli:
///
/// - `connect`: makine kapalıysa ne kadar bekleneceği. Kısa olmalı, aksi
///   hâlde kullanıcı düğmeye basıp donmuş bir arayüze bakar.
/// - `io`: cevabın ne kadar bekleneceği. Uzun olmalı, çünkü karşı taraf
///   cevap vermeden önce ses aygıtlarını açıyor ve bu saniyeler sürebilir.
///
/// İkisini tek değere bağlamak ya erken vazgeçmeye ya da uzun donmaya yol
/// açıyordu.
pub fn send(addr: SocketAddr, req: &Request, connect: Duration, io_timeout: Duration) -> Result<Reply> {
    let io = |e: std::io::Error| Error::Stream(e.to_string());
    let stream = TcpStream::connect_timeout(&addr, connect)
        .map_err(|e| Error::Stream(format!("{addr} could not be reached: {e}")))?;
    stream.set_read_timeout(Some(io_timeout)).map_err(io)?;
    stream.set_write_timeout(Some(io_timeout)).map_err(io)?;

    let mut writer = stream.try_clone().map_err(io)?;
    let mut text = serde_json::to_string(&Message::new(req.clone()))
        .map_err(|e| Error::Stream(format!("request could not be encoded: {e}")))?;
    text.push('\n');
    writer.write_all(text.as_bytes()).map_err(io)?;
    writer.flush().map_err(io)?;

    let mut reader = BufReader::new(stream).take(MAX_LINE);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(io)?;
    if line.trim().is_empty() {
        return Err(Error::Stream(format!("{addr} closed the connection without replying")));
    }
    serde_json::from_str::<Reply>(line.trim())
        .map_err(|e| Error::Stream(format!("malformed reply from {addr}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn local(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    #[test]
    fn a_start_request_reaches_the_handler_and_the_reply_comes_back() {
        let seen = Arc::new(Mutex::new(Vec::<(Request, IpAddr)>::new()));
        let recorder = seen.clone();
        let server = serve(0, move |req, ip| {
            recorder.lock().unwrap().push((req.clone(), ip));
            Reply::ok("windows-box".into(), Some("CABLE Output".into()))
        })
        .unwrap();

        let req = Request::StartHeadset {
            role: "remote".into(),
            audio_port: 59101,
            name: "linux-box".into(),
            auth: None,
        };
        let reply = send(local(server.port()), &req, Duration::from_secs(2), Duration::from_secs(5)).unwrap();

        assert!(reply.ok);
        assert_eq!(reply.name, "windows-box");
        assert_eq!(reply.paired_mic.as_deref(), Some("CABLE Output"));
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, req);
        assert!(seen[0].1.is_loopback(), "kaynak IP handler'a verilmeli");
    }

    /// Reddedilen istek istisna değil, cevap. İsteyen taraf sebebi
    /// kullanıcıya gösterebilmeli.
    #[test]
    fn a_refusal_is_delivered_as_a_reply_not_a_dropped_connection() {
        let server = serve(0, |_, _| {
Reply::failed("windows-box".into(), "need_cable", "no virtual audio cable installed")
        })
        .unwrap();
        let reply = send(local(server.port()), &Request::Ping, Duration::from_secs(2), Duration::from_secs(5)).unwrap();
        assert!(!reply.ok);
        assert_eq!(reply.error.as_deref(), Some("no virtual audio cable installed"));
        assert_eq!(reply.code.as_deref(), Some("need_cable"), "arayüz sebebi çevirebilmeli");
    }

    /// Gövdesi bozuk bir istek sunucuyu düşürmemeli; sonraki istek çalışmalı.
    #[test]
    fn garbage_does_not_kill_the_server() {
        let server = serve(0, |_, _| Reply::ok("box".into(), None)).unwrap();
        let addr = local(server.port());

        let mut s = TcpStream::connect(addr).unwrap();
        s.write_all(b"bu json degil\n").unwrap();
        let mut line = String::new();
        BufReader::new(s).read_line(&mut line).unwrap();
        let reply: Reply = serde_json::from_str(line.trim()).unwrap();
        assert!(!reply.ok);
        assert!(reply.error.unwrap().contains("malformed"));

        assert!(send(addr, &Request::Ping, Duration::from_secs(2), Duration::from_secs(5)).unwrap().ok);
    }

    /// İleride protokol değişirse eski sürüm sessizce yanlış davranmamalı.
    #[test]
    fn a_future_protocol_version_is_refused_clearly() {
        let server = serve(0, |_, _| Reply::ok("box".into(), None)).unwrap();
        let ask = |line: &str| {
            let mut s = TcpStream::connect(local(server.port())).unwrap();
            s.write_all(line.as_bytes()).unwrap();
            s.write_all(b"\n").unwrap();
            let mut out = String::new();
            BufReader::new(s).read_line(&mut out).unwrap();
            serde_json::from_str::<Reply>(out.trim()).unwrap()
        };
        let reply = ask(r#"{"v":99,"body":"ping"}"#);
        assert!(!reply.ok);
        assert!(reply.error.unwrap().contains("version"));

        // Asıl mesele bu: ileride gövde şekli de değişecek. Sürüm gövdeden
        // sonra okunsaydı bu "malformed request" derdi ve kullanıcı
        // güncelleme gerektiğini anlayamazdı.
        let reply = ask(r#"{"v":2,"body":{"start_headset_v2":{"whatever":1}}}"#);
        assert!(!reply.ok);
        let err = reply.error.unwrap();
        assert!(err.contains("version"), "sürüm hatası bekleniyordu, gelen: {err}");
    }

    /// Kapalı bir porta istek, arayüzü kilitlemek yerine hata dönmeli.
    #[test]
    fn an_unreachable_peer_fails_fast_instead_of_hanging() {
        let started = std::time::Instant::now();
        // 1 numaralı port ayrıcalıklı ve dinlenmiyor: bağlantı hemen reddedilir.
        let r = send(local(1), &Request::Ping, Duration::from_secs(2), Duration::from_secs(5));
        assert!(r.is_err());
        assert!(started.elapsed() < Duration::from_secs(3), "zaman aşımına uyulmalı");
    }

    #[test]
    fn shutdown_stops_the_accept_loop() {
        let server = serve(0, |_, _| Reply::ok("box".into(), None)).unwrap();
        let port = server.port();
        server.shutdown();
        // Döngü bayrağı en geç bir yoklama aralığında görüyor.
        std::thread::sleep(ACCEPT_POLL * 3);
        // Port serbest kalmış olmalı: aynı porta yeniden bağlanabilmeliyiz.
        drop(server);
        std::thread::sleep(ACCEPT_POLL * 2);
        assert!(TcpListener::bind(("0.0.0.0", port)).is_ok());
    }

    /// Yönlendirilmiş bir adresten gelen bağlantı bu ucu internete açardı.
    #[test]
    fn only_link_local_sources_are_accepted() {
        for ok in ["127.0.0.1", "192.168.1.113", "10.0.0.2", "172.16.5.9", "169.254.1.1", "::1"] {
            assert!(is_link_local(ok.parse().unwrap()), "{ok} kabul edilmeliydi");
        }
        for bad in ["8.8.8.8", "203.0.113.7", "172.32.0.1", "2606:4700::1111"] {
            assert!(!is_link_local(bad.parse().unwrap()), "{bad} reddedilmeliydi");
        }
        // IPv6 soketine düşen IPv4 bağlantısı eşlemli adres olarak geliyor.
        assert!(is_link_local("::ffff:192.168.1.5".parse().unwrap()));
        assert!(!is_link_local("::ffff:8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn the_wire_format_round_trips() {
        let m = Message::new(Request::StartHeadset {
            role: "local".into(),
            audio_port: 59101,
            name: "linux".into(),
            auth: None,
        });
        let text = serde_json::to_string(&m).unwrap();
        let back: Message = serde_json::from_str(&text).unwrap();
        assert_eq!(back.v, CONTROL_VERSION);
        assert_eq!(back.body, m.body);
        assert_eq!(
            serde_json::to_string(&Message::new(Request::StopHeadset)).unwrap(),
            r#"{"v":1,"body":"stop_headset"}"#
        );
    }
}
