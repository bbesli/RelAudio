//! Boyutu sınırlı log dosyası.
//!
//! Uygulama tepside günlerce açık kalıyor. Boyut kontrolü yalnızca açılışta
//! yapılırsa uzun oturumlarda dosya sınırsız büyür — diskin şişmemesi için
//! kontrolün **yazma anında** olması gerekiyor.
//!
//! Tavan aşıldığında dosya silinmiyor, bir önceki kuşak olarak saklanıyor
//! (`relaudio.log` → `relaudio.log.1`). Böylece çökmeden hemen önceki
//! kayıtlar bir sonraki turda hâlâ elde oluyor. Diskte en fazla iki dosya
//! kalıyor, yani üst sınır `cap * 2`.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Dosya başına tavan. İki kuşakla birlikte diskte en fazla 4 MB.
pub const DEFAULT_CAP_BYTES: u64 = 2 * 1024 * 1024;

pub struct CappedLog {
    path: PathBuf,
    file: File,
    written: u64,
    cap: u64,
}

impl CappedLog {
    /// Dosyayı ekleme kipinde açar; mevcut boyutu sayaca alır.
    pub fn open(path: impl AsRef<Path>, cap: u64) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let written = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self { path, file, written, cap: cap.max(1024) })
    }

    fn previous_path(&self) -> PathBuf {
        let mut p = self.path.clone().into_os_string();
        p.push(".1");
        PathBuf::from(p)
    }

    /// Mevcut dosyayı bir önceki kuşak yapar ve yenisini açar.
    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        let prev = self.previous_path();
        // rename mevcut .1'in üzerine yazar; eski kuşak düşer.
        std::fs::rename(&self.path, &prev)?;
        self.file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        self.written = 0;
        Ok(())
    }
}

impl Write for CappedLog {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written + buf.len() as u64 > self.cap {
            // Devir başarısız olursa (izin, kilit) yazmaya devam et —
            // log tutulamaması uygulamayı durdurmamalı.
            let _ = self.rotate();
        }
        let n = self.file.write(buf)?;
        self.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("relaudio-log-test-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn keeps_the_file_under_the_cap_while_writing() {
        let dir = tmpdir("cap");
        let path = dir.join("relaudio.log");
        let mut log = CappedLog::open(&path, 2048).unwrap();

        // Tavanın kat kat üstünde yaz.
        for i in 0..500 {
            writeln!(log, "satır {i} {}", "x".repeat(60)).unwrap();
        }
        log.flush().unwrap();

        let live = std::fs::metadata(&path).unwrap().len();
        assert!(live <= 2048, "canlı dosya tavanı aştı: {live}");

        // Toplam disk kullanımı iki kuşakla sınırlı.
        let prev = std::fs::metadata(dir.join("relaudio.log.1")).unwrap().len();
        assert!(live + prev <= 2048 * 2 + 256, "toplam çok büyük: {}", live + prev);
    }

    #[test]
    fn preserves_the_previous_generation() {
        let dir = tmpdir("prev");
        let path = dir.join("relaudio.log");
        let mut log = CappedLog::open(&path, 1024).unwrap();
        writeln!(log, "{}", "a".repeat(900)).unwrap();
        writeln!(log, "{}", "b".repeat(900)).unwrap(); // devir tetikler
        log.flush().unwrap();

        let prev = std::fs::read_to_string(dir.join("relaudio.log.1")).unwrap();
        assert!(prev.contains("aaa"), "önceki kuşak korunmalı");
        let live = std::fs::read_to_string(&path).unwrap();
        assert!(live.contains("bbb"), "yeni kayıt canlı dosyada olmalı");
    }

    #[test]
    fn resumes_from_the_existing_size_on_reopen() {
        let dir = tmpdir("resume");
        let path = dir.join("relaudio.log");
        {
            let mut log = CappedLog::open(&path, 4096).unwrap();
            writeln!(log, "{}", "a".repeat(3000)).unwrap();
            log.flush().unwrap();
        }
        // Yeniden açıldığında sayaç sıfırdan başlarsa tavan aşılır.
        let mut log = CappedLog::open(&path, 4096).unwrap();
        writeln!(log, "{}", "b".repeat(2000)).unwrap();
        log.flush().unwrap();
        let live = std::fs::metadata(&path).unwrap().len();
        assert!(live <= 4096, "yeniden açılışta sayaç devralınmadı: {live}");
    }
}
