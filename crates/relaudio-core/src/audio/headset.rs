//! Kulaklık modunun aygıt seçim politikası.
//!
//! Bu mantık daha önce yalnızca `App.svelte` içindeydi. Uzaktan gelen bir
//! "kulaklık modunu başlat" isteğinde arayüz hiç devrede olmuyor, dolayısıyla
//! Rust tarafının da aynı seçimi yapabilmesi gerekiyor. İki yerde ayrı ayrı
//! yazmak yerine politika buraya taşındı; arayüz de buradan okuyor.
//!
//! Tek değişmez kural: **aynı makinede yakalanan aygıt ile yazılan aygıt aynı
//! olamaz.** Aynı olursa ses kendi kuyruğunu yer ve kullanıcı kendini duyar.

use super::{paired_virtual_input, virtual_output_rank, virtual_vendor_rank, DeviceInfo, DeviceKind};

/// Kulaklık bu makinede mi, karşıda mı?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadsetRole {
    /// Kulaklık fiziksel olarak burada: mikrofonunu gönder, geleni hoparlöründen çal.
    Local,
    /// Kulaklık burada değil: geleni sanal kabloya yaz, sistem sesini gönder.
    Remote,
}

impl HeadsetRole {
    pub fn as_str(self) -> &'static str {
        match self {
            HeadsetRole::Local => "local",
            HeadsetRole::Remote => "remote",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "local" => Some(HeadsetRole::Local),
            "remote" => Some(HeadsetRole::Remote),
            _ => None,
        }
    }

    /// Bu rolü üstlenen makine karşı tarafın hangi rolü almasını bekler?
    pub fn opposite(self) -> Self {
        match self {
            HeadsetRole::Local => HeadsetRole::Remote,
            HeadsetRole::Remote => HeadsetRole::Local,
        }
    }
}

/// Plan kurulamadığında sebebi. Arayüz bunu çeviri anahtarına eşliyor,
/// bu yüzden serbest metin değil sayılabilir bir değer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadsetProblem {
    /// Gerçek (sanal olmayan) hoparlör ya da mikrofon yok.
    NeedPhysical,
    /// Sanal ses kablosu kurulu değil.
    NeedCable,
}

impl HeadsetProblem {
    pub fn as_str(self) -> &'static str {
        match self {
            HeadsetProblem::NeedPhysical => "need_physical",
            HeadsetProblem::NeedCable => "need_cable",
        }
    }

    /// Uzaktan gelen isteği reddederken karşı tarafa yollanacak açıklama.
    /// Arayüzü olmayan tarafta çeviri yapılamıyor; İngilizce sabit metin.
    pub fn message(self) -> &'static str {
        match self {
            HeadsetProblem::NeedPhysical => {
                "the other machine has no usable physical audio device"
            }
            HeadsetProblem::NeedCable => {
                "the other machine has no virtual audio cable installed"
            }
        }
    }
}

/// Bir rol için çözülmüş aygıt planı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadsetPlan {
    /// Sesin yakalanacağı aygıt (gönderilecek olan).
    pub capture: Option<DeviceInfo>,
    /// Gelen sesin yazılacağı aygıt.
    pub play: Option<DeviceInfo>,
    /// `capture` hangi türden aranıyor — `Input` (mikrofon) ya da `Monitor`
    /// (sistem sesi).
    pub capture_kind: DeviceKind,
    pub problem: Option<HeadsetProblem>,
    /// Arayüzün açılır listelerini doldurması için.
    pub capture_choices: Vec<DeviceInfo>,
    pub play_choices: Vec<DeviceInfo>,
    /// Uzak rolde: karşı taraftaki toplantı uygulamasında seçilecek mikrofon.
    pub paired_mic: Option<String>,
}

impl HeadsetPlan {
    /// Plan başlatılabilir mi?
    pub fn is_usable(&self) -> bool {
        self.problem.is_none() && self.capture.is_some() && self.play.is_some()
    }
}

/// Kayıtlı seçim hâlâ geçerliyse onu kullan, değilse otomatik seç.
///
/// Kullanıcının elle yaptığı seçim otomatik tercihi yenmeli; aksi hâlde
/// sistem varsayılanı her açılışta seçimi geri alıyordu.
fn prefer_saved<'a>(saved: &str, list: &'a [DeviceInfo]) -> Option<&'a DeviceInfo> {
    list.iter()
        .find(|d| d.id == saved)
        .or_else(|| list.iter().find(|d| d.is_default))
        .or_else(|| list.first())
}

/// Verilen rol için yakalama ve çalma aygıtlarını çözer.
///
/// Saf fonksiyon: aygıt listesi dışarıdan verilir, böylece iki platformun
/// gerçek aygıt adlarıyla test edilebiliyor.
pub fn plan(
    role: HeadsetRole,
    devices: &[DeviceInfo],
    saved_play: &str,
    saved_capture: &str,
) -> HeadsetPlan {
    let is_virtual = |d: &DeviceInfo| super::looks_virtual(&d.name);
    let of_kind = |k: DeviceKind| -> Vec<DeviceInfo> {
        devices.iter().filter(|d| d.kind == k).cloned().collect()
    };

    match role {
        HeadsetRole::Local => {
            let play_choices: Vec<DeviceInfo> = of_kind(DeviceKind::Output)
                .into_iter()
                .filter(|d| !is_virtual(d))
                .collect();
            let capture_choices: Vec<DeviceInfo> = of_kind(DeviceKind::Input)
                .into_iter()
                .filter(|d| !is_virtual(d))
                .collect();

            let play = prefer_saved(saved_play, &play_choices).cloned();
            let capture = prefer_saved(saved_capture, &capture_choices).cloned();
            // Hoparlör yoksa gelen sesi çalacak yer yok; mikrofon yoksa
            // gönderecek bir şey yok. İkisi de bu senaryoda zorunlu.
            let problem = if play.is_none() || capture.is_none() {
                Some(HeadsetProblem::NeedPhysical)
            } else {
                None
            };
            HeadsetPlan {
                capture,
                play,
                capture_kind: DeviceKind::Input,
                problem,
                capture_choices,
                play_choices,
                paired_mic: None,
            }
        }
        HeadsetRole::Remote => {
            let mut play_choices: Vec<DeviceInfo> = of_kind(DeviceKind::Output)
                .into_iter()
                .filter(is_virtual)
                .collect();
            // Kanonik stereo uç önce ("CABLE In 16ch" gibi çok kanallı
            // varyantlar eşleşen mikrofon ucuna düzgün ulaşmıyor), sonra
            // rehberimizin kurdurduğu kablo, en sonda yabancı satıcılar.
            play_choices.sort_by_key(|d| {
                (
                    virtual_output_rank(&d.name),
                    virtual_vendor_rank(&d.name),
                    d.name.to_lowercase(),
                )
            });

            let Some(play) = prefer_saved(saved_play, &play_choices).cloned() else {
                return HeadsetPlan {
                    capture: None,
                    play: None,
                    capture_kind: DeviceKind::Monitor,
                    problem: Some(HeadsetProblem::NeedCable),
                    capture_choices: Vec::new(),
                    play_choices,
                    paired_mic: None,
                };
            };

            // Kabloya yazıp aynı kablonun monitörünü yakalamak döngü kurar.
            let capture_choices: Vec<DeviceInfo> = of_kind(DeviceKind::Monitor)
                .into_iter()
                .filter(|d| !is_virtual(d) && !d.id.starts_with(&play.id))
                .collect();
            let capture = prefer_saved(saved_capture, &capture_choices).cloned();
            let problem = capture.is_none().then_some(HeadsetProblem::NeedPhysical);
            let paired_mic = paired_virtual_input(&play.name, devices);

            HeadsetPlan {
                capture,
                play: Some(play),
                capture_kind: DeviceKind::Monitor,
                problem,
                capture_choices,
                play_choices,
                paired_mic,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(id: &str, name: &str, kind: DeviceKind, is_default: bool) -> DeviceInfo {
        DeviceInfo {
            id: id.into(),
            name: name.into(),
            kind,
            is_default,
        }
    }

    /// Windows'ta VB-CABLE kurulu tipik bir makine.
    fn windows_machine() -> Vec<DeviceInfo> {
        vec![
            dev("spk", "Speakers (Realtek(R) Audio)", DeviceKind::Output, true),
            dev("cable", "CABLE Input (VB-Audio Virtual Cable)", DeviceKind::Output, false),
            dev("cable16", "CABLE In 16ch (VB-Audio Virtual Cable)", DeviceKind::Output, false),
            dev("mic", "Microphone (Razer BlackShark)", DeviceKind::Input, true),
            dev("cableout", "CABLE Output (VB-Audio Virtual Cable)", DeviceKind::Input, false),
            dev("spk", "Speakers (Realtek(R) Audio)", DeviceKind::Monitor, true),
            dev("cable", "CABLE Input (VB-Audio Virtual Cable)", DeviceKind::Monitor, false),
        ]
    }

    /// Linux'ta null sink kurulu tipik bir makine. Kablonun mikrofon ucu
    /// burada bir monitor.
    fn linux_machine() -> Vec<DeviceInfo> {
        vec![
            dev("alsa_out.razer", "Razer BlackShark V2", DeviceKind::Output, true),
            dev("relaudio", "RelAudio-Cable", DeviceKind::Output, false),
            // Başka bir uygulamadan kalmış sanal aygıt — gerçek makinelerde
            // sık, ve adı alfabetik olarak bizimkinin önüne düşüyor.
            dev("audiorelay-virtual-mic-sink", "AudioRelay Mic&Sink", DeviceKind::Output, false),
            dev("alsa_in.razer", "Razer BlackShark V2 Mono", DeviceKind::Input, true),
            dev("alsa_out.razer.monitor", "Monitor of Razer BlackShark V2", DeviceKind::Monitor, true),
            dev("relaudio.monitor", "Monitor of RelAudio-Cable", DeviceKind::Monitor, false),
        ]
    }

    #[test]
    fn local_role_uses_physical_devices_on_both_ends() {
        let p = plan(HeadsetRole::Local, &linux_machine(), "", "");
        assert_eq!(p.capture_kind, DeviceKind::Input);
        assert_eq!(p.play.as_ref().unwrap().id, "alsa_out.razer");
        assert_eq!(p.capture.as_ref().unwrap().id, "alsa_in.razer");
        assert!(p.is_usable());
    }

    /// Sanal kablo asla "kulaklık burada" rolünde hoparlör olarak seçilmemeli;
    /// kullanıcı sessizliğe konuşup neden duyulmadığını anlamıyor.
    #[test]
    fn local_role_never_picks_a_cable_as_the_speaker() {
        let p = plan(HeadsetRole::Local, &windows_machine(), "", "");
        assert_eq!(p.play.as_ref().unwrap().id, "spk");
        assert!(p.play_choices.iter().all(|d| d.id != "cable"));
    }

    #[test]
    fn remote_role_writes_into_the_cable_and_captures_system_audio() {
        let p = plan(HeadsetRole::Remote, &windows_machine(), "", "");
        assert_eq!(p.capture_kind, DeviceKind::Monitor);
        assert_eq!(p.play.as_ref().unwrap().id, "cable");
        assert_eq!(p.capture.as_ref().unwrap().id, "spk");
        assert!(p.is_usable());
    }

    /// Alfabetik sırada "cable in 16ch" < "cable input"; sıralama rank'e
    /// göre olmazsa varsayılan seçim çok kanallı uca düşüyor.
    #[test]
    fn remote_role_prefers_the_stereo_cable_over_the_16_channel_variant() {
        let p = plan(HeadsetRole::Remote, &windows_machine(), "", "");
        assert_eq!(p.play.as_ref().unwrap().id, "cable");
        assert_eq!(p.play_choices[0].id, "cable");
    }

    /// Kabloya yazıp aynı kablonun monitörünü yakalamak geri besleme döngüsü.
    #[test]
    fn remote_role_excludes_the_cables_own_monitor_from_the_source_list() {
        let p = plan(HeadsetRole::Remote, &linux_machine(), "", "");
        assert_eq!(
            p.play.as_ref().unwrap().id,
            "relaudio",
            "başka uygulamadan kalan sanal aygıt kendi kablomuzun önüne geçmemeli"
        );
        assert!(
            p.capture_choices.iter().all(|d| d.id != "relaudio.monitor"),
            "kablonun kendi monitörü kaynak olarak sunulmamalı"
        );
        assert_eq!(p.capture.as_ref().unwrap().id, "alsa_out.razer.monitor");
    }

    #[test]
    fn remote_role_tells_the_user_which_microphone_to_pick() {
        assert_eq!(
            plan(HeadsetRole::Remote, &windows_machine(), "", "").paired_mic.as_deref(),
            Some("CABLE Output (VB-Audio Virtual Cable)")
        );
        // Linux'ta kablonun mikrofon ucu bir monitor.
        assert_eq!(
            plan(HeadsetRole::Remote, &linux_machine(), "", "").paired_mic.as_deref(),
            Some("Monitor of RelAudio-Cable")
        );
    }

    #[test]
    fn no_cable_installed_is_reported_as_such() {
        // Hiç sanal kablo yok — adına göre değil, `looks_virtual`'a göre
        // eleniyor; aksi hâlde fikstüre eklenen her yeni sanal aygıt bu
        // testi sessizce anlamsızlaştırıyor.
        let devices: Vec<DeviceInfo> = linux_machine()
            .into_iter()
            .filter(|d| !super::super::looks_virtual(&d.name))
            .collect();
        let p = plan(HeadsetRole::Remote, &devices, "", "");
        assert_eq!(p.problem, Some(HeadsetProblem::NeedCable));
        assert!(!p.is_usable());
    }

    #[test]
    fn a_machine_with_no_microphone_cannot_take_the_local_role() {
        let devices: Vec<DeviceInfo> = linux_machine()
            .into_iter()
            .filter(|d| d.kind != DeviceKind::Input)
            .collect();
        let p = plan(HeadsetRole::Local, &devices, "", "");
        assert_eq!(p.problem, Some(HeadsetProblem::NeedPhysical));
    }

    /// Kullanıcı elle seçtiyse otomatik tercih onu ezmemeli.
    #[test]
    fn a_saved_choice_beats_the_automatic_pick() {
        let devices = vec![
            dev("spk-a", "Speakers A", DeviceKind::Output, true),
            dev("spk-b", "Speakers B", DeviceKind::Output, false),
            dev("mic-a", "Mic A", DeviceKind::Input, true),
            dev("mic-b", "Mic B", DeviceKind::Input, false),
        ];
        let p = plan(HeadsetRole::Local, &devices, "spk-b", "mic-b");
        assert_eq!(p.play.as_ref().unwrap().id, "spk-b");
        assert_eq!(p.capture.as_ref().unwrap().id, "mic-b");
    }

    /// Kayıtlı aygıt çıkarılmışsa seçim otomatiğe düşmeli, plan çökmemeli.
    #[test]
    fn a_saved_choice_that_no_longer_exists_falls_back() {
        let p = plan(HeadsetRole::Local, &linux_machine(), "cikarilmis", "yok");
        assert_eq!(p.play.as_ref().unwrap().id, "alsa_out.razer");
        assert!(p.is_usable());
    }

    #[test]
    fn roles_are_opposites_and_round_trip_through_strings() {
        assert_eq!(HeadsetRole::Local.opposite(), HeadsetRole::Remote);
        assert_eq!(HeadsetRole::Remote.opposite(), HeadsetRole::Local);
        for r in [HeadsetRole::Local, HeadsetRole::Remote] {
            assert_eq!(HeadsetRole::parse(r.as_str()), Some(r));
        }
        assert_eq!(HeadsetRole::parse("kulaklik"), None);
    }

    /// Hiç aygıtı olmayan makine (sessiz sunucu, headless CI) plan kuramaz
    /// ama panic de etmemeli.
    #[test]
    fn an_empty_device_list_is_not_a_crash() {
        for role in [HeadsetRole::Local, HeadsetRole::Remote] {
            let p = plan(role, &[], "", "");
            assert!(p.problem.is_some());
            assert!(!p.is_usable());
        }
    }
}
