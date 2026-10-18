use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use rayon::prelude::*;

pub struct BenchmarkRunner {
    pub running: Arc<AtomicBool>,
    pub total_bytes: Arc<AtomicU64>,
    pub start_time: Instant,
}

impl BenchmarkRunner {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            total_bytes: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }

    pub fn start(&mut self) {
        self.running.store(true, Ordering::SeqCst);
        self.total_bytes.store(0, Ordering::SeqCst);
        self.start_time = Instant::now();

        let running = Arc::clone(&self.running);
        let total_bytes = Arc::clone(&self.total_bytes);

        std::thread::spawn(move || {
            let num_threads = rayon::current_num_threads();
            let chunk_size = 64 * 1024; // 64 KB

            while running.load(Ordering::Relaxed) {
                (0..num_threads).into_par_iter().for_each(|id| {
                    if !running.load(Ordering::Relaxed) {
                        return;
                    }

                    // Generate pseudo-random compressible data buffer
                    let mut buffer = vec![0u8; chunk_size];
                    let seed = (id as u64).wrapping_mul(1103515245).wrapping_add(12345);
                    for (i, b) in buffer.iter_mut().enumerate() {
                        *b = ((seed ^ (i as u64)) % 64) as u8;
                    }

                    // Compute CRC32
                    let mut hasher = crc32fast::Hasher::new();
                    hasher.update(&buffer);
                    let _crc = hasher.finalize();

                    // Simulate LZ sliding window compression dictionary match
                    let mut matches = 0u64;
                    for w in buffer.windows(4) {
                        if w[0] == w[2] && w[1] == w[3] {
                            matches += 1;
                        }
                    }
                    std::hint::black_box(matches);

                    total_bytes.fetch_add(chunk_size as u64, Ordering::Relaxed);
                });
            }
        });
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn current_speed_kb_s(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed < 0.1 {
            return 0.0;
        }
        let bytes = self.total_bytes.load(Ordering::Relaxed) as f64;
        (bytes / 1024.0) / elapsed
    }

    pub fn total_processed_mb(&self) -> f64 {
        let bytes = self.total_bytes.load(Ordering::Relaxed) as f64;
        bytes / (1024.0 * 1024.0)
    }

    pub fn elapsed_seconds(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }
}
