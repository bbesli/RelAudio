//! Sıra numarasına göre yeniden sıralayan, sabit hedefli jitter buffer.
//!
//! v1 sabit hedefle çalışır. Adaptif hedef (docs/05) sonraki fazda —
//! önce gerçek ağda ölçüm, sonra uyarlama.

use std::collections::BTreeMap;

pub struct JitterBuffer {
    /// Genişletilmiş sıra numarası → yük. Sessiz paketler boş vec olarak durur.
    ///
    /// Anahtar **u64**, ham u16 değil. Ham u16 ile `BTreeMap` sıralaması
    /// 65535→0 sarmalamasında tersine dönüyordu: "en eski" diye seçilen kayıt
    /// aslında en yenisi oluyor, sarmalama öncesi paketler erişilemez hâle
    /// geliyor ve tampon bir daha hiç boşalmıyordu (dolayısıyla underrun
    /// sayacı ve yeniden biriktirme histerezisi ölüyordu).
    slots: BTreeMap<u64, Vec<u8>>,
    /// Çalmaya başlamadan önce biriktirilecek paket sayısı.
    target: usize,
    /// Bir sonraki çalınacak genişletilmiş sıra numarası.
    next: Option<u64>,
    /// Son görülen genişletilmiş numara — sarmalamayı çözmek için.
    last_ext: Option<u64>,
    started: bool,
    pub lost: u64,
    pub late: u64,
    pub received: u64,
    starved: u64,
    dropped: u64,
}

/// 16 bitlik sıra numarasını, en son görülene en yakın olacak şekilde
/// 64 bite genişletir. Böylece sarmalama sıralamayı bozmuyor.
fn extend(prev: Option<u64>, seq: u16) -> u64 {
    match prev {
        None => seq as u64,
        Some(prev) => {
            let prev_low = prev as u16;
            let mut diff = seq.wrapping_sub(prev_low) as i32;
            if diff >= 0x8000 {
                diff -= 0x10000; // geriye doğru: geç gelmiş paket
            }
            (prev as i64 + diff as i64).max(0) as u64
        }
    }
}

impl JitterBuffer {
    pub fn new(target_packets: usize) -> Self {
        Self {
            slots: BTreeMap::new(),
            target: target_packets.max(1),
            next: None,
            last_ext: None,
            started: false,
            lost: 0,
            late: 0,
            received: 0,
            starved: 0,
            dropped: 0,
        }
    }

    pub fn push(&mut self, seq: u16, payload: Vec<u8>) {
        self.received += 1;
        let ext = extend(self.last_ext, seq);
        // İleri giden en yüksek numarayı takip et; geç gelen paket referansı
        // geriye çekmemeli.
        self.last_ext = Some(self.last_ext.map_or(ext, |p| p.max(ext)));

        if let Some(next) = self.next {
            // Oynatma noktasını geçmiş paket işe yaramaz.
            if ext < next {
                self.late += 1;
                return;
            }
        }
        self.slots.insert(ext, payload);

        // Tampon tavanı. Saat kayması telafisi (adaptif yeniden örnekleme,
        // docs/03) henüz yok; onsuz tampon yavaşça büyüyor ve gecikme zamanla
        // artıyor. Bu tavan gecikmeyi sınırlar — kalıcı çözüm değil, koruma.
        let cap = self.target * 4;
        while self.slots.len() > cap {
            let Some(&oldest) = self.slots.keys().next() else { break };
            self.slots.remove(&oldest);
            self.dropped += 1;
            self.next = Some(oldest + 1);
        }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Çalınacak bir sonraki blok.
    /// `None`        = tampon hedefe ulaşmadı; arayan sessizlik çalmalı
    /// `Some(None)`  = paket kayıp; arayan sessizlik üretmeli
    /// `Some(Some)`  = yük
    ///
    /// Tampon tamamen boşalırsa yeniden biriktirme moduna döner. Bu histerezis
    /// olmadan üretim ve tüketim hızı eşit olduğunda tampon sıfırda takılıyor ve
    /// jitter'a karşı hiçbir koruma kalmıyor.
    pub fn pop(&mut self) -> Option<Option<Vec<u8>>> {
        if !self.started {
            if self.slots.len() < self.target {
                return None;
            }
            self.started = true;
            self.next = self.slots.keys().next().copied();
        }
        if self.slots.is_empty() {
            // Aç kaldık: yastığı yeniden kur.
            self.started = false;
            self.next = None;
            self.starved += 1;
            return None;
        }
        let next = self.next?;
        self.next = Some(next + 1);
        match self.slots.remove(&next) {
            Some(p) => Some(Some(p)),
            None => {
                self.lost += 1;
                Some(None)
            }
        }
    }

    /// Kaç kez tamponun tamamen boşaldığı. Gerçek underrun ölçüsü budur;
    /// "henüz doldurmadı" durumu bundan ayrıdır.
    pub fn starved(&self) -> u64 {
        self.starved
    }

    /// Tampon tavanı aşıldığı için atılan paketler. Sıfırdan büyükse gecikme
    /// birikiyor demektir — saat kayması telafisi gerektiğinin işareti.
    pub fn dropped_count(&self) -> u64 {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_target_then_plays_in_order() {
        let mut j = JitterBuffer::new(3);
        j.push(10, vec![1]);
        j.push(11, vec![2]);
        assert!(j.pop().is_none(), "hedefe ulaşmadan çalmamalı");
        j.push(12, vec![3]);
        assert_eq!(j.pop(), Some(Some(vec![1])));
        assert_eq!(j.pop(), Some(Some(vec![2])));
        assert_eq!(j.pop(), Some(Some(vec![3])));
    }

    #[test]
    fn reorders_out_of_order_arrivals() {
        let mut j = JitterBuffer::new(3);
        j.push(2, vec![2]);
        j.push(1, vec![1]);
        j.push(3, vec![3]);
        assert_eq!(j.pop(), Some(Some(vec![1])));
        assert_eq!(j.pop(), Some(Some(vec![2])));
    }

    #[test]
    fn reports_gap_for_lost_packet() {
        let mut j = JitterBuffer::new(2);
        j.push(1, vec![1]);
        j.push(3, vec![3]);
        assert_eq!(j.pop(), Some(Some(vec![1])));
        assert_eq!(j.pop(), Some(None), "2 kayıp, boşluk bildirilmeli");
        assert_eq!(j.lost, 1);
        assert_eq!(j.pop(), Some(Some(vec![3])));
    }

    #[test]
    fn drops_packets_that_arrive_too_late() {
        let mut j = JitterBuffer::new(1);
        j.push(5, vec![5]);
        j.pop();
        j.push(4, vec![4]); // oynatma noktasını geçti
        assert_eq!(j.late, 1);
    }

    #[test]
    fn rebuffers_after_starving() {
        let mut j = JitterBuffer::new(2);
        j.push(1, vec![1]);
        j.push(2, vec![2]);
        assert_eq!(j.pop(), Some(Some(vec![1])));
        assert_eq!(j.pop(), Some(Some(vec![2])));
        // tampon boşaldı → yeniden biriktirmeye dönmeli
        assert_eq!(j.pop(), None);
        assert_eq!(j.starved(), 1);
        // tek paket hedefi karşılamıyor, hâlâ beklemeli
        j.push(3, vec![3]);
        assert_eq!(j.pop(), None);
        j.push(4, vec![4]);
        assert_eq!(j.pop(), Some(Some(vec![3])));
    }

    #[test]
    fn caps_buffer_growth_to_bound_latency() {
        let mut j = JitterBuffer::new(2); // tavan = 8
        for seq in 0..20u16 {
            j.push(seq, vec![seq as u8]);
        }
        assert!(j.len() <= 8, "tampon tavanı aşıldı: {}", j.len());
        assert!(j.dropped_count() > 0, "atılan paket sayılmalı");
    }

    #[test]
    fn extends_sequence_numbers_across_the_wrap() {
        assert_eq!(extend(None, 5), 5);
        assert_eq!(extend(Some(65535), 0), 65536, "sarmalama ileri gitmeli");
        assert_eq!(extend(Some(65536), 1), 65537);
        assert_eq!(extend(Some(65536), 65535), 65535, "geç gelen geriye gitmeli");
        assert_eq!(extend(Some(100), 99), 99);
    }

    /// Sarmalama anında tavan tetiklenirse eski kayıtlar erişilemez hâle
    /// geliyordu: "en eski" ham u16 sırasına göre seçildiği için sarmalama
    /// sonrası paket atılıyor, öncekiler sonsuza dek tamponda kalıyordu.
    /// Sonuç: tampon hiç boşalmıyor, underrun sayacı ölüyor, yastık
    /// bir daha kurulamıyor.
    #[test]
    fn survives_the_sequence_wrap_with_the_cap_active() {
        let mut j = JitterBuffer::new(4); // tavan 16
        // Sarmalamayı kapsayan bir pencere doldur.
        for k in 0..24u32 {
            let seq = (65530u32.wrapping_add(k) % 65536) as u16;
            j.push(seq, vec![k as u8]);
        }
        assert!(j.len() <= 16, "tavan aşıldı: {}", j.len());

        // Hepsi çalınabilmeli: sırayla ve boşluk vermeden.
        let mut played = 0;
        for _ in 0..40 {
            match j.pop() {
                Some(Some(_)) => played += 1,
                Some(None) => {}
                None => break,
            }
        }
        assert_eq!(played, j_expected_playable(), "sarmalama sonrası paketler erişilemez kaldı");
        assert!(j.is_empty(), "tampon boşalabilmeli");
        assert_eq!(j.starved(), 1, "boşalınca yeniden biriktirmeye dönmeli");
    }

    fn j_expected_playable() -> usize {
        16 // tavan kadar; gerisi drift eviction ile atıldı
    }
}
