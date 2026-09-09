//! Eşleştirmenin gerçek soket üzerinden uçtan uca sınanması.
//!
//! Birim testleri `Pairing`'i tek başına doğruluyor; buradaki test iki ucu
//! gerçek bir TCP bağlantısıyla konuşturuyor: kod üret → karşı taraf kodu
//! gönderip anahtarı alsın → o anahtarla imzalanmış bir "başlat" isteği
//! kabul edilsin, imzasız ya da kurcalanmış olan reddedilsin.
//!
//! Bunun ayrı bir dosyada olmasının sebebi: JSON gövdesi, imza ve doğrulama
//! ayrı modüllerde ve birbirlerinin varsayımlarını sessizce bozabiliyorlar.

use relaudio_core::net::{
    control_send, control_serve, sign_request, ControlAuth, ControlReply, ControlRequest, Pairing,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

fn local(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

const QUICK: Duration = Duration::from_secs(3);

/// Kodu gösteren tarafı taklit eden sunucu.
///
/// Gerçek uygulamadaki `handle_control` ile aynı sırayı izliyor: `Pair`
/// kodu bozdurur, `StartHeadset` imzayı doğrular.
fn serve_pairing(pairing: Arc<Pairing>, key_slot: Arc<std::sync::Mutex<Option<String>>>) -> u16 {
    let server = control_serve(0, move |req, _from| match req {
        ControlRequest::Pair { code, .. } => match pairing.redeem(code) {
            Ok(key) => {
                *key_slot.lock().unwrap() = Some(key.clone());
                ControlReply::paired("responder".into(), "responder-linux".into(), key)
            }
            Err(e) => ControlReply::failed("responder".into(), e.code(), e.message()),
        },
        ControlRequest::StartHeadset { role, audio_port, auth, .. } => {
            let Some(auth) = auth else {
                return ControlReply::failed("responder".into(), "needs_pairing", "no signature");
            };
            let stored = key_slot.lock().unwrap().clone();
            let Some(key) = stored else {
                return ControlReply::failed("responder".into(), "needs_pairing", "not paired");
            };
            match pairing.verify(&key, &auth.nonce, auth.ts, &auth.mac, role, *audio_port) {
                Ok(()) => ControlReply::ok("responder".into(), Some("CABLE Output".into())),
                Err(e) => ControlReply::failed("responder".into(), e.code(), e.message()),
            }
        }
        _ => ControlReply::ok("responder".into(), None),
    })
    .expect("kontrol sunucusu açılmalı");
    let port = server.port();
    // Sunucu testin sonuna kadar yaşasın.
    std::mem::forget(server);
    port
}

fn start_request(key: &str, role: &str, port: u16) -> ControlRequest {
    let (nonce, ts, mac) = sign_request(key, role, port).expect("imza üretilebilmeli");
    ControlRequest::StartHeadset {
        role: role.into(),
        audio_port: port,
        name: "requester".into(),
        auth: Some(ControlAuth { id: "requester-linux".into(), nonce, ts, mac }),
    }
}

#[test]
fn pairing_then_a_signed_start_is_accepted_and_an_unsigned_one_is_not() {
    let pairing = Arc::new(Pairing::default());
    let key_slot = Arc::new(std::sync::Mutex::new(None::<String>));
    let port = serve_pairing(pairing.clone(), key_slot.clone());
    let addr = local(port);

    // Eşleşmeden önce hiçbir şey başlatılamaz.
    let unsigned = ControlRequest::StartHeadset {
        role: "remote".into(),
        audio_port: 59101,
        name: "requester".into(),
        auth: None,
    };
    let reply = control_send(addr, &unsigned, QUICK, QUICK).unwrap();
    assert!(!reply.ok);
    assert_eq!(reply.code.as_deref(), Some("needs_pairing"));

    // Kullanıcı kodu okuyup karşı makinede giriyor.
    let code = pairing.new_code().unwrap();
    let reply = control_send(
        addr,
        &ControlRequest::Pair {
            code: code.clone(),
            id: "requester-linux".into(),
            name: "requester".into(),
        },
        QUICK,
        QUICK,
    )
    .unwrap();
    assert!(reply.ok, "doğru kod kabul edilmeli");
    let key = reply.key.expect("anahtar dönmeli");
    assert_eq!(key.len(), 64);
    assert_eq!(reply.id.as_deref(), Some("responder-linux"));

    // Artık imzalı istek geçiyor — kullanıcı için tek düğme.
    let reply = control_send(addr, &start_request(&key, "remote", 59101), QUICK, QUICK).unwrap();
    assert!(reply.ok, "imzalı istek kabul edilmeli: {:?}", reply.error);
    assert_eq!(reply.paired_mic.as_deref(), Some("CABLE Output"));

    // Aynı kod ikinci kez kullanılamaz.
    let reply = control_send(
        addr,
        &ControlRequest::Pair { code, id: "x".into(), name: "x".into() },
        QUICK,
        QUICK,
    )
    .unwrap();
    assert!(!reply.ok);
    assert_eq!(reply.code.as_deref(), Some("pair_no_code"));
}

/// İmza isteğin içeriğini bağlamalı. Bağlamasaydı, geçerli bir "sistem sesi
/// gönder" isteğini yakalayan biri onu "mikrofonu gönder"e çevirebilirdi.
#[test]
fn a_tampered_request_is_refused() {
    let pairing = Arc::new(Pairing::default());
    let key_slot = Arc::new(std::sync::Mutex::new(None::<String>));
    let addr = local(serve_pairing(pairing.clone(), key_slot));

    let code = pairing.new_code().unwrap();
    let key = control_send(
        addr,
        &ControlRequest::Pair { code, id: "r".into(), name: "r".into() },
        QUICK,
        QUICK,
    )
    .unwrap()
    .key
    .unwrap();

    // "remote" için imzalanmış isteği "local"a çevir: mikrofon açtırma girişimi.
    let ControlRequest::StartHeadset { auth, .. } = start_request(&key, "remote", 59101) else {
        unreachable!()
    };
    let tampered = ControlRequest::StartHeadset {
        role: "local".into(),
        audio_port: 59101,
        name: "requester".into(),
        auth,
    };
    let reply = control_send(addr, &tampered, QUICK, QUICK).unwrap();
    assert!(!reply.ok, "kurcalanmış istek kabul edilmemeli");
    assert_eq!(reply.code.as_deref(), Some("auth_bad"));
}

/// Yakalanan geçerli bir istek tekrar oynatılamamalı.
#[test]
fn a_captured_request_cannot_be_replayed() {
    let pairing = Arc::new(Pairing::default());
    let key_slot = Arc::new(std::sync::Mutex::new(None::<String>));
    let addr = local(serve_pairing(pairing.clone(), key_slot));

    let code = pairing.new_code().unwrap();
    let key = control_send(
        addr,
        &ControlRequest::Pair { code, id: "r".into(), name: "r".into() },
        QUICK,
        QUICK,
    )
    .unwrap()
    .key
    .unwrap();

    let req = start_request(&key, "remote", 59101);
    assert!(control_send(addr, &req, QUICK, QUICK).unwrap().ok);
    let again = control_send(addr, &req, QUICK, QUICK).unwrap();
    assert!(!again.ok);
    assert_eq!(again.code.as_deref(), Some("auth_replay"));
}

/// Yanlış anahtarla imzalanan istek — yani eşleşmemiş bir cihaz — geçmemeli.
#[test]
fn a_stranger_with_their_own_key_is_refused() {
    let pairing = Arc::new(Pairing::default());
    let key_slot = Arc::new(std::sync::Mutex::new(None::<String>));
    let addr = local(serve_pairing(pairing.clone(), key_slot));

    // Gerçek bir eşleşme kur (sunucunun anahtarı olsun).
    let code = pairing.new_code().unwrap();
    control_send(
        addr,
        &ControlRequest::Pair { code, id: "r".into(), name: "r".into() },
        QUICK,
        QUICK,
    )
    .unwrap();

    // Yabancı kendi uydurduğu anahtarla imzalıyor.
    let stranger_key = "ab".repeat(32);
    let reply = control_send(addr, &start_request(&stranger_key, "remote", 59101), QUICK, QUICK)
        .unwrap();
    assert!(!reply.ok);
    assert_eq!(reply.code.as_deref(), Some("auth_bad"));
}
