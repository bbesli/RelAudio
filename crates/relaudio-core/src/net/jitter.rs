//! Sıra numarasına göre yeniden sıralayan, sabit hedefli jitter buffer.
//!
//! v1 sabit hedefle çalışır. Adaptif hedef (docs/05) sonraki fazda —
//! önce gerçek ağda ölçüm, sonra uyarlama.

use std::collections::BTreeMap;

pub struct JitterBuffer {
    /// seq → yük. Sessiz paketler boş vec olarak durur.
    slots: BTreeMap<u16, Vec<u8>>,
    /// Çalmaya başlamadan önce biriktirilecek paket sayısı.
    target: usize,
    /// Bir sonraki çalınacak sıra numarası.
    next: Option<u16>,
    started: bool,
    pub lost: u64,
    pub late: u64,
    pub received: u64,
    starved: u64,
    dropped: u64,
}

/// `a`, `b`'den sonra mı? 16-bit sarmalamaya karşı güvenli karşılaştırma.
fn seq_gt(a: u16, b: u16) -> bool {
    a != b && a.wrapping_sub(b) < 0x8000
}

impl JitterBuffer {
    pub fn new(target_packets: usize) -> Self {
        Self {
            slots: BTreeMap::new(),
            target: target_packets.max(1),
            next: None,
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
        if let Some(next) = self.next {
            // Oynatma noktasını geçmiş paket işe yaramaz.
            if seq_gt(next, seq) {
                self.late += 1;
                return;
            }
        }
        self.slots.insert(seq, payload);

        // Tampon tavanı. Saat kayması telafisi (adaptif yeniden örnekleme,
        // docs/03) henüz yok; onsuz tampon yavaşça büyüyor ve gecikme zamanla
        // artıyor. Bu tavan gecikmeyi sınırlar — kalıcı çözüm değil, koruma.
        let cap = self.target * 4;
        while self.slots.len() > cap {
            if let Some(&oldest) = self.slots.keys().next() {
                self.slots.remove(&oldest);
                self.dropped += 1;
                self.next = Some(oldest.wrapping_add(1));
            }
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
        self.next = Some(next.wrapping_add(1));
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
    fn sequence_comparison_survives_wraparound() {
        assert!(seq_gt(0, 65535), "sarmalama sonrası 0, 65535'ten sonradır");
        assert!(!seq_gt(65535, 0));
        assert!(seq_gt(100, 99));
    }
}
