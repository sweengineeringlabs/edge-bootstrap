//! `RingBuffer` — fixed-capacity circular buffer for latency samples.

/// Ring buffer of latency samples in microseconds.
pub struct RingBuffer {
    pub(crate) buf: Vec<u64>,
    pub(crate) head: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0; capacity],
            head: 0,
        }
    }

    pub fn push(&mut self, val_us: u64) {
        let cap = self.buf.len();
        self.buf[self.head % cap] = val_us;
        self.head = self.head.wrapping_add(1);
    }

    /// 99th-percentile latency in milliseconds from the current window.
    pub fn p99_ms(&self) -> f64 {
        let mut samples: Vec<u64> = self.buf.iter().copied().filter(|&v| v > 0).collect();
        if samples.is_empty() {
            return 0.0;
        }
        samples.sort_unstable();
        let idx = (samples.len() * 99 / 100).saturating_sub(1);
        samples[idx] as f64 / 1_000.0
    }
}
