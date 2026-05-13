//! Append-only on-disk log with bisect-on-overflow + cursor-delta reads.
//!
//! Spec §5.3.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::{Mutex, Notify};

use tools_core::RingRead;

#[cfg(test)]
const HEAD_KEEP_BYTES: usize = 100;
#[cfg(test)]
const TAIL_KEEP_BYTES: usize = 200;
#[cfg(test)]
const FINAL_HEAD_BYTES: usize = 64;
#[cfg(test)]
const FINAL_TAIL_BYTES: usize = 128;
#[cfg(test)]
const READ_DELTA_CAP: usize = 1_024;

#[cfg(not(test))]
const HEAD_KEEP_BYTES: usize = 1_500_000; // 1.5 MB
#[cfg(not(test))]
const TAIL_KEEP_BYTES: usize = 2_500_000; // 2.5 MB
#[cfg(not(test))]
const FINAL_HEAD_BYTES: usize = 96 * 1024; // 96 KB
#[cfg(not(test))]
const FINAL_TAIL_BYTES: usize = 160 * 1024; // 160 KB
#[cfg(not(test))]
const READ_DELTA_CAP: usize = 50 * 1024; // 50 KB per poll

#[derive(Debug)]
pub struct RingFile {
    path: PathBuf,
    writer: Mutex<BufWriter<File>>,
    bytes_written: AtomicU64,
    bisect_low_water: AtomicU64,
    bisect_generation: AtomicU64,
    cap_bytes: u64,
    notify: Notify,
}

impl RingFile {
    pub async fn create(path: impl AsRef<Path>, cap_bytes: u64) -> std::io::Result<Arc<Self>> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(&path)
            .await?;
        Ok(Arc::new(Self {
            path,
            writer: Mutex::new(BufWriter::new(file)),
            bytes_written: AtomicU64::new(0),
            bisect_low_water: AtomicU64::new(0),
            bisect_generation: AtomicU64::new(0),
            cap_bytes,
            notify: Notify::new(),
        }))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn total_bytes_emitted(&self) -> u64 {
        self.bytes_written.load(Ordering::Acquire)
    }

    pub fn bisect_generation(&self) -> u64 {
        self.bisect_generation.load(Ordering::Acquire)
    }

    pub fn bisect_count(&self) -> u64 {
        self.bisect_generation.load(Ordering::Acquire)
    }

    pub async fn append(&self, bytes: &[u8]) -> std::io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let mut writer = self.writer.lock().await;
        writer.write_all(bytes).await?;
        let new_total = self
            .bytes_written
            .fetch_add(bytes.len() as u64, Ordering::AcqRel)
            + bytes.len() as u64;

        // Check overflow against current low_water.
        let low_water = self.bisect_low_water.load(Ordering::Acquire);
        if new_total - low_water > self.cap_bytes {
            self.do_bisect(new_total, &mut writer).await?;
        }
        self.notify.notify_waiters();
        Ok(())
    }

    pub fn notify_waiters(&self) {
        self.notify.notify_waiters();
    }

    pub async fn wait_for_change(&self, timeout: std::time::Duration) {
        let _ = tokio::time::timeout(timeout, self.notify.notified()).await;
    }

    async fn do_bisect(
        &self,
        current_total: u64,
        writer: &mut BufWriter<File>,
    ) -> std::io::Result<()> {
        // Flush pending bytes so the on-disk file is complete before we read it.
        writer.flush().await?;

        let tmp_path = self.path.with_extension("log.tmp");
        let on_disk_size = tokio::fs::metadata(&self.path).await?.len();

        // Read head + tail from current file.
        let mut file = File::open(&self.path).await?;
        let head_to_read = HEAD_KEEP_BYTES.min(on_disk_size as usize);
        let mut head = vec![0u8; head_to_read];
        file.read_exact(&mut head).await?;

        let tail_to_read = TAIL_KEEP_BYTES.min(on_disk_size as usize - head_to_read);
        let tail_start = on_disk_size as i64 - tail_to_read as i64;
        file.seek(std::io::SeekFrom::Start(tail_start.max(0) as u64))
            .await?;
        let mut tail = vec![0u8; tail_to_read];
        file.read_exact(&mut tail).await?;

        let dropped = on_disk_size as i64 - head_to_read as i64 - tail_to_read as i64;
        let marker = if dropped > 0 {
            format!("\n[--- bisect: {dropped} bytes truncated from the middle ---]\n").into_bytes()
        } else {
            vec![]
        };

        // Write to .tmp and rename atomically.
        {
            let mut out = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)
                .await?;
            out.write_all(&head).await?;
            if !marker.is_empty() {
                out.write_all(&marker).await?;
            }
            out.write_all(&tail).await?;
            out.flush().await?;
        }
        tokio::fs::rename(&tmp_path, &self.path).await?;

        // Reopen the writer in append mode.
        let new_file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(&self.path)
            .await?;
        *writer = BufWriter::new(new_file);

        let new_low_water = current_total - (head_to_read + marker.len() + tail_to_read) as u64;
        self.bisect_low_water
            .store(new_low_water, Ordering::Release);
        self.bisect_generation.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    pub async fn read_delta(&self, since_offset: u64) -> std::io::Result<RingRead> {
        // Flush pending bytes so the on-disk file is up to date for reading.
        {
            let mut writer = self.writer.lock().await;
            writer.flush().await?;
        }

        let total = self.bytes_written.load(Ordering::Acquire);
        let low_water = self.bisect_low_water.load(Ordering::Acquire);
        let gen = self.bisect_generation.load(Ordering::Acquire);

        let bisect_occurred = since_offset < low_water;
        let effective_since = if bisect_occurred {
            low_water
        } else {
            since_offset
        };

        // Map cumulative offset → on-disk byte position.
        // After bisect: file = head + marker + tail. cumulative offsets [low_water .. total)
        // map to positions [0 .. on_disk_size).
        let mut file = File::open(&self.path).await?;
        let on_disk_size = file.metadata().await?.len();
        let on_disk_offset = (effective_since - low_water) as i64;
        if on_disk_offset < 0 || on_disk_offset >= on_disk_size as i64 {
            return Ok(RingRead {
                bytes: vec![],
                new_offset: total,
                bisect_generation: gen,
                bisect_occurred_since: bisect_occurred,
                total_bytes_emitted: total,
            });
        }
        file.seek(std::io::SeekFrom::Start(on_disk_offset as u64))
            .await?;
        let to_read = (on_disk_size - on_disk_offset as u64).min(READ_DELTA_CAP as u64) as usize;
        let mut buf = vec![0u8; to_read];
        let mut reader = BufReader::new(file);
        reader.read_exact(&mut buf).await?;
        let new_offset = effective_since + to_read as u64;
        Ok(RingRead {
            bytes: buf,
            new_offset,
            bisect_generation: gen,
            bisect_occurred_since: bisect_occurred,
            total_bytes_emitted: total,
        })
    }

    pub async fn finalize(&self, final_path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
        let final_path = final_path.as_ref().to_path_buf();
        // Flush any buffered bytes so the on-disk file is complete before we read it.
        {
            let mut writer = self.writer.lock().await;
            writer.flush().await?;
        }
        let on_disk_size = tokio::fs::metadata(&self.path).await?.len();
        let mut file = File::open(&self.path).await?;

        let head_to_read = FINAL_HEAD_BYTES.min(on_disk_size as usize);
        let mut head = vec![0u8; head_to_read];
        file.read_exact(&mut head).await?;

        let tail_to_read = FINAL_TAIL_BYTES.min(on_disk_size as usize - head_to_read);
        let mut final_bytes = head;
        if tail_to_read > 0 {
            let tail_start = on_disk_size - tail_to_read as u64;
            file.seek(std::io::SeekFrom::Start(tail_start)).await?;
            let dropped = on_disk_size as usize - head_to_read - tail_to_read;
            if dropped > 0 {
                final_bytes.extend_from_slice(
                    format!("\n[--- final summary: {dropped} bytes truncated ---]\n").as_bytes(),
                );
            }
            let mut tail = vec![0u8; tail_to_read];
            file.read_exact(&mut tail).await?;
            final_bytes.extend_from_slice(&tail);
        }

        let mut out = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&final_path)
            .await?;
        out.write_all(&final_bytes).await?;
        out.flush().await?;

        let _ = tokio::fs::remove_file(&self.path).await;
        Ok(final_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn append_below_cap_no_bisect() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");
        let ring = RingFile::create(&path, 4 * 1024 * 1024).await.unwrap();
        ring.append(b"hello world\n").await.unwrap();
        assert_eq!(ring.total_bytes_emitted(), 12);
        assert_eq!(ring.bisect_generation(), 0);
    }

    #[tokio::test]
    async fn read_delta_returns_full_content_at_zero_offset() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");
        let ring = RingFile::create(&path, 4 * 1024 * 1024).await.unwrap();
        ring.append(b"hello").await.unwrap();
        ring.append(b" world\n").await.unwrap();
        let rd = ring.read_delta(0).await.unwrap();
        assert_eq!(rd.bytes, b"hello world\n");
        assert_eq!(rd.new_offset, 12);
        assert!(!rd.bisect_occurred_since);
    }

    #[tokio::test]
    async fn read_delta_advances_with_cursor() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");
        let ring = RingFile::create(&path, 4 * 1024 * 1024).await.unwrap();
        ring.append(b"first chunk\n").await.unwrap();
        let rd1 = ring.read_delta(0).await.unwrap();
        assert_eq!(rd1.bytes, b"first chunk\n");
        ring.append(b"second\n").await.unwrap();
        let rd2 = ring.read_delta(rd1.new_offset).await.unwrap();
        assert_eq!(rd2.bytes, b"second\n");
    }

    #[tokio::test]
    async fn append_over_cap_triggers_bisect() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");
        // Set tiny cap to make bisect easy to test.
        let cap = 1_000;
        let ring = RingFile::create(&path, cap).await.unwrap();
        // Append enough to exceed cap.
        for i in 0..200 {
            ring.append(format!("line {i:04}\n").as_bytes())
                .await
                .unwrap();
        }
        // bisect_generation should have ticked.
        assert!(ring.bisect_generation() > 0, "bisect should have fired");
        // total_bytes_emitted is cumulative, never decreases.
        assert!(ring.total_bytes_emitted() > cap);
    }

    #[tokio::test]
    async fn read_delta_with_cursor_below_low_water_flags_bisect() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");
        let cap = 1_000;
        let ring = RingFile::create(&path, cap).await.unwrap();
        for i in 0..200 {
            ring.append(format!("line {i:04}\n").as_bytes())
                .await
                .unwrap();
        }
        // Cursor at 0 is now below low_water.
        let rd = ring.read_delta(0).await.unwrap();
        assert!(
            rd.bisect_occurred_since,
            "should flag bisect_occurred_since"
        );
        // Returned bytes still represent forward-progress content.
        assert!(!rd.bytes.is_empty());
    }

    #[tokio::test]
    async fn finalize_produces_capped_summary() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");
        let final_path = dir.path().join("test.final");
        let ring = RingFile::create(&path, 10 * 1024 * 1024).await.unwrap();
        // Write 1 MB.
        let chunk = vec![b'a'; 1024];
        for _ in 0..1024 {
            ring.append(&chunk).await.unwrap();
        }
        let out = ring.finalize(&final_path).await.unwrap();
        let size = tokio::fs::metadata(&out).await.unwrap().len();
        assert!(
            size <= (FINAL_HEAD_BYTES + FINAL_TAIL_BYTES + 256) as u64,
            "final ≤ head + tail + marker, was {size}"
        );
        assert!(!path.exists(), "{path:?} should be deleted after finalize");
    }
}
