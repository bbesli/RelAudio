//! Eşleştirme: 6 haneli kod ve ondan sonra paylaşılan anahtar.
//!
//! Kulaklık modunu uzaktan başlatmak, karşı makineye "sesini yakala ve bana
//! gönder" dedirtiyor. Kimlik doğrulaması olmadan bunu ağdaki herkes
//! yapabilirdi. Akış:
//!
//! 1. Paylaşan makine **kodu ekranda gösteriyor** (6 hane, 3 dakika geçerli).
//! 2. Kullanıcı kodu karşı makinede yazıyor; o makine kodu sahibine gönderiyor.
//! 3. Kod tutuyorsa iki taraf 32 baytlık rastgele bir anahtarı paylaşıyor ve
//!    diske yazıyor. Kod tükeniyor.
//! 4. Bundan sonra her "başlat" isteği o anahtarla imzalanıyor; kod bir daha
//!    sorulmuyor. Tek düğme.
//!
//! ## Neyi çözüyor, neyi çözmüyor
//!
//! **Çözüyor:** eşleşmemiş bir makine artık hiçbir şey başlatamıyor. Ağdaki
//! yabancı bir RelAudio'nun elinde yalnızca reddedilen bir istek kalıyor.
//! Anahtar telde tekrarlanmıyor: her istek nonce + zaman damgası üzerinden
//! HMAC ile imzalanıyor, tekrar oynatma penceresi 2 dakika ve nonce'lar
//! hatırlanıyor.
//!
//! **Çözmüyor:** eşleştirme anındaki tek alışverişi dinleyebilen biri anahtarı
//! görür. Anahtar o tek mesajda düz metin geçiyor — anahtar değişimi (DH) yok.
//! Ses akışının kendisi de zaten şifresiz. Yani bu, ağdaki *bağlanabilen*
//! birine karşı koruma; trafiği *dinleyebilen* birine karşı değil.

use hmac_sha256::HMAC;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Kodun ekranda kalma süresi. Kısa olmalı — kullanıcı iki makinenin başında.
pub const CODE_TTL: Duration = Duration::from_secs(180);
/// İmzanın kabul edildiği saat sapması. İki LAN makinesi arasında bol.
pub const CLOCK_SKEW: u64 = 120;
/// Hatırlanan nonce sayısı; tekrar oynatmayı engelliyor.
const NONCE_MEMORY: usize = 256;
/// Arka arkaya kaç yanlış kod denenebilir. 6 hane = 10^6, ama sınırsız
/// deneme buna rağmen dakikalar içinde kırılırdı.
const MAX_CODE_ATTEMPTS: u32 = 5;

/// Ekranda duran, henüz kullanılmamış kod.
struct Pending {
    code: String,
    at: Instant,
    attempts: u32,
}

/// Bu makinenin eşleştirme durumu.
#[derive(Default)]
pub struct Pairing {
    pending: Mutex<Option<Pending>>,
    seen: Mutex<VecDeque<(String, Instant)>>,
}

/// Kriptografik rastgele bayt. `getrandom` başarısız olursa (çok olası değil)
/// eşleştirme reddediliyor — tahmin edilebilir anahtar üretmektense hiç
/// üretmemek doğru.
fn random_bytes<const N: usize>() -> Option<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf).ok()?;
    Some(buf)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Zamandan bağımsız karşılaştırma. Kod ve imza doğrularken erken çıkmak,
/// karakter karakter tahmin etmeye kapı açıyor.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// İsteğin imzalanacak hâli. İki taraf da aynı sırayla kuruyor; alan
/// eklenirse iki tarafta birden değişmeli.
fn signed_bytes(nonce: &str, ts: u64, role: &str, audio_port: u16) -> Vec<u8> {
    let mut v = Vec::with_capacity(64);
    v.extend_from_slice(b"relaudio-start-v1");
    v.push(0);
    v.extend_from_slice(nonce.as_bytes());
    v.push(0);
    v.extend_from_slice(&ts.to_be_bytes());
    v.extend_from_slice(role.as_bytes());
    v.push(0);
    v.extend_from_slice(&audio_port.to_be_bytes());
    v
}

/// Bir isteği imzalar. `key_hex` eşleştirmede paylaşılan anahtar.
pub fn sign(key_hex: &str, role: &str, audio_port: u16) -> Option<(String, u64, String)> {
    let key = unhex(key_hex)?;
    let nonce = hex(&random_bytes::<16>()?);
    let ts = now_secs();
    let mac = hex(&HMAC::mac(signed_bytes(&nonce, ts, role, audio_port), &key));
    Some((nonce, ts, mac))
}

impl Pairing {
    /// Yeni bir kod üretir ve ekranda göstermek üzere saklar.
    /// Var olan kodun üstüne yazıyor: kullanıcı "paylaş"a yeniden bastıysa
    /// ekranda gördüğü koddur geçerli olan.
    pub fn new_code(&self) -> Option<String> {
        let bytes = random_bytes::<4>()?;
        // Modulo sapması 10^6 için ihmal edilebilir (2^32 / 10^6 ≈ 4294.97).
        let code = format!("{:06}", u32::from_be_bytes(bytes) % 1_000_000);
        *self.pending.lock().unwrap() = Some(Pending {
            code: code.clone(),
            at: Instant::now(),
            attempts: 0,
        });
        Some(code)
    }

    /// Ekranda duran kod ve kalan süresi. Süresi dolmuşsa temizleniyor.
    pub fn visible_code(&self) -> Option<(String, u64)> {
        let mut guard = self.pending.lock().unwrap();
        match guard.as_ref() {
            Some(p) if p.at.elapsed() < CODE_TTL => Some((
                p.code.clone(),
                (CODE_TTL - p.at.elapsed()).as_secs(),
            )),
            Some(_) => {
                *guard = None;
                None
            }
            None => None,
        }
    }

    pub fn clear_code(&self) {
        *self.pending.lock().unwrap() = None;
    }

    /// Karşı tarafın gönderdiği kodu doğrular ve tutuyorsa yeni bir paylaşılan
    /// anahtar üretir. Kod tek kullanımlık: doğru kod tüketiliyor.
    pub fn redeem(&self, code: &str) -> Result<String, PairError> {
        let mut guard = self.pending.lock().unwrap();
        let Some(p) = guard.as_mut() else {
            return Err(PairError::NoCode);
        };
        if p.at.elapsed() >= CODE_TTL {
            *guard = None;
            return Err(PairError::Expired);
        }
        if p.attempts >= MAX_CODE_ATTEMPTS {
            *guard = None;
            return Err(PairError::TooManyAttempts);
        }
        if !constant_time_eq(p.code.as_bytes(), code.trim().as_bytes()) {
            p.attempts += 1;
            return Err(PairError::Wrong);
        }
        *guard = None;
        random_bytes::<32>()
            .map(|k| hex(&k))
            .ok_or(PairError::NoRandom)
    }

    /// İmzayı doğrular: anahtar tutuyor mu, zaman damgası penceresi içinde mi,
    /// bu nonce daha önce görüldü mü?
    pub fn verify(
        &self,
        key_hex: &str,
        nonce: &str,
        ts: u64,
        mac: &str,
        role: &str,
        audio_port: u16,
    ) -> Result<(), PairError> {
        let now = now_secs();
        if ts.abs_diff(now) > CLOCK_SKEW {
            return Err(PairError::Stale);
        }
        let (Some(key), Some(given)) = (unhex(key_hex), unhex(mac)) else {
            return Err(PairError::BadSignature);
        };
        let want = HMAC::mac(signed_bytes(nonce, ts, role, audio_port), &key);
        if !constant_time_eq(&want, &given) {
            return Err(PairError::BadSignature);
        }
        // İmza geçerli; aynı isteğin tekrar oynatılmadığından emin ol.
        let mut seen = self.seen.lock().unwrap();
        let cutoff = Duration::from_secs(CLOCK_SKEW * 2);
        seen.retain(|(_, at)| at.elapsed() < cutoff);
        if seen.iter().any(|(n, _)| n == nonce) {
            return Err(PairError::Replay);
        }
        if seen.len() >= NONCE_MEMORY {
            seen.pop_front();
        }
        seen.push_back((nonce.to_string(), Instant::now()));
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairError {
    /// Karşı makinede ekranda duran bir kod yok.
    NoCode,
    Expired,
    Wrong,
    TooManyAttempts,
    Stale,
    Replay,
    BadSignature,
    NoRandom,
}

impl PairError {
    pub fn code(self) -> &'static str {
        match self {
            PairError::NoCode => "pair_no_code",
            PairError::Expired => "pair_expired",
            PairError::Wrong => "pair_wrong",
            PairError::TooManyAttempts => "pair_too_many",
            PairError::Stale => "auth_stale",
            PairError::Replay => "auth_replay",
            PairError::BadSignature => "auth_bad",
            PairError::NoRandom => "pair_no_random",
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            PairError::NoCode => "the other machine is not showing a pairing code right now",
            PairError::Expired => "the pairing code has expired",
            PairError::Wrong => "wrong pairing code",
            PairError::TooManyAttempts => "too many wrong codes; ask for a new one",
            PairError::Stale => "the two machines' clocks are too far apart",
            PairError::Replay => "this request was already used",
            PairError::BadSignature => "this machine is not paired with you",
            PairError::NoRandom => "the system refused to provide secure randomness",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_six_digits_and_visible_until_redeemed() {
        let p = Pairing::default();
        let code = p.new_code().unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(p.visible_code().unwrap().0, code);
        assert!(p.redeem(&code).is_ok());
        assert!(p.visible_code().is_none(), "kod tek kullanımlık olmalı");
    }

    #[test]
    fn the_wrong_code_is_refused_and_the_right_one_still_works() {
        let p = Pairing::default();
        let code = p.new_code().unwrap();
        let wrong = if code == "000000" { "111111" } else { "000000" };
        assert_eq!(p.redeem(wrong), Err(PairError::Wrong));
        assert!(p.redeem(&code).is_ok(), "yanlış deneme doğru kodu yakmamalı");
    }

    /// 6 hane sınırsız denemeye karşı hiçbir şey ifade etmiyor.
    #[test]
    fn guessing_is_capped() {
        let p = Pairing::default();
        let code = p.new_code().unwrap();
        let wrong = if code == "000000" { "111111" } else { "000000" };
        for _ in 0..MAX_CODE_ATTEMPTS {
            assert_eq!(p.redeem(wrong), Err(PairError::Wrong));
        }
        assert_eq!(p.redeem(wrong), Err(PairError::TooManyAttempts));
        // Kod yakıldı: doğrusu bile artık geçmiyor, kullanıcı yenisini alacak.
        assert_eq!(p.redeem(&code), Err(PairError::NoCode));
    }

    #[test]
    fn pairing_twice_yields_different_keys() {
        let p = Pairing::default();
        let a = { let c = p.new_code().unwrap(); p.redeem(&c).unwrap() };
        let b = { let c = p.new_code().unwrap(); p.redeem(&c).unwrap() };
        assert_ne!(a, b);
        assert_eq!(a.len(), 64, "32 bayt = 64 hex karakter");
    }

    #[test]
    fn redeeming_without_a_visible_code_fails() {
        assert_eq!(Pairing::default().redeem("123456"), Err(PairError::NoCode));
    }

    #[test]
    fn a_signed_request_verifies_with_the_shared_key() {
        let p = Pairing::default();
        let key = { let c = p.new_code().unwrap(); p.redeem(&c).unwrap() };
        let (nonce, ts, mac) = sign(&key, "remote", 59101).unwrap();
        assert!(p.verify(&key, &nonce, ts, &mac, "remote", 59101).is_ok());
    }

    /// Anahtarı olmayan biri geçerli imza üretemez.
    #[test]
    fn a_different_key_does_not_verify() {
        let p = Pairing::default();
        let mine = { let c = p.new_code().unwrap(); p.redeem(&c).unwrap() };
        let theirs = { let c = p.new_code().unwrap(); p.redeem(&c).unwrap() };
        let (nonce, ts, mac) = sign(&theirs, "remote", 59101).unwrap();
        assert_eq!(
            p.verify(&mine, &nonce, ts, &mac, "remote", 59101),
            Err(PairError::BadSignature)
        );
    }

    /// İmza isteğin kendisini de kapsamalı; yoksa dinleyen biri rolü
    /// "remote"tan "local"a çevirip mikrofonu açtırabilirdi.
    #[test]
    fn the_signature_covers_the_role_and_the_port() {
        let p = Pairing::default();
        let key = { let c = p.new_code().unwrap(); p.redeem(&c).unwrap() };
        let (nonce, ts, mac) = sign(&key, "remote", 59101).unwrap();
        assert_eq!(
            p.verify(&key, &nonce, ts, &mac, "local", 59101),
            Err(PairError::BadSignature),
            "rol değiştirilirse imza tutmamalı"
        );
        assert_eq!(
            p.verify(&key, &nonce, ts, &mac, "remote", 60000),
            Err(PairError::BadSignature),
            "port değiştirilirse imza tutmamalı"
        );
    }

    #[test]
    fn the_same_request_cannot_be_replayed() {
        let p = Pairing::default();
        let key = { let c = p.new_code().unwrap(); p.redeem(&c).unwrap() };
        let (nonce, ts, mac) = sign(&key, "remote", 59101).unwrap();
        assert!(p.verify(&key, &nonce, ts, &mac, "remote", 59101).is_ok());
        assert_eq!(
            p.verify(&key, &nonce, ts, &mac, "remote", 59101),
            Err(PairError::Replay)
        );
    }

    #[test]
    fn an_old_timestamp_is_refused_before_the_signature_is_even_checked() {
        let p = Pairing::default();
        let key = { let c = p.new_code().unwrap(); p.redeem(&c).unwrap() };
        let (nonce, _, mac) = sign(&key, "remote", 59101).unwrap();
        let old = now_secs() - CLOCK_SKEW - 10;
        assert_eq!(
            p.verify(&key, &nonce, old, &mac, "remote", 59101),
            Err(PairError::Stale)
        );
    }

    #[test]
    fn hex_round_trips_and_rejects_garbage() {
        let bytes = [0u8, 15, 16, 255];
        assert_eq!(hex(&bytes), "000f10ff");
        assert_eq!(unhex("000f10ff").unwrap(), bytes);
        assert_eq!(unhex("abc"), None, "tek uzunluk geçersiz");
        assert_eq!(unhex("zz"), None);
    }

    #[test]
    fn constant_time_eq_still_compares_correctly() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
