//! Single-producer single-consumer f32 ring buffer. The audio callback is the only consumer
//! and never locks or allocates.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Ring {
    buf: Box<[UnsafeCell<f32>]>,
    /// Total samples ever read / written; indices are taken modulo capacity.
    read: AtomicUsize,
    write: AtomicUsize,
}

// SAFETY: one producer writes only the free region, one consumer reads only the filled region;
// the Acquire/Release pairs on the indices publish the data between them.
unsafe impl Sync for Ring {}

impl Ring {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: (0..capacity).map(|_| UnsafeCell::new(0.0)).collect(),
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
        }
    }

    fn cap(&self) -> usize {
        self.buf.len()
    }

    pub fn len(&self) -> usize {
        self.write.load(Ordering::Acquire).wrapping_sub(self.read.load(Ordering::Acquire))
    }

    pub fn free(&self) -> usize {
        self.cap() - self.len()
    }

    /// Producer: push as many whole `align`-sized frames as fit. Returns samples pushed.
    pub fn push(&self, data: &[f32], align: usize) -> usize {
        let w = self.write.load(Ordering::Relaxed);
        let r = self.read.load(Ordering::Acquire);
        let free = self.cap() - w.wrapping_sub(r);
        let n = data.len().min(free) / align * align;
        for (i, &s) in data[..n].iter().enumerate() {
            // SAFETY: the slot is in the free region, which only this producer writes; the
            // consumer reads it only after the Release store of `write` below publishes it.
            unsafe { *self.buf[(w + i) % self.cap()].get() = s };
        }
        self.write.store(w.wrapping_add(n), Ordering::Release);
        n
    }

    /// Consumer: pop into `out`. Returns samples popped.
    pub fn pop(&self, out: &mut [f32]) -> usize {
        let r = self.read.load(Ordering::Relaxed);
        let w = self.write.load(Ordering::Acquire);
        let n = out.len().min(w.wrapping_sub(r));
        for (i, o) in out[..n].iter_mut().enumerate() {
            // SAFETY: the slot is in the filled region, published by the producer's Release
            // store of `write` (read with Acquire above), and only this consumer reads it.
            *o = unsafe { *self.buf[(r + i) % self.cap()].get() };
        }
        self.read.store(r.wrapping_add(n), Ordering::Release);
        n
    }

    /// Consumer: drop everything currently buffered. Only the consumer may call it: from any
    /// other thread it races with `pop` over the read index.
    pub fn clear(&self) {
        self.read.store(self.write.load(Ordering::Acquire), Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_and_aligns() {
        let ring = Ring::new(8);
        assert_eq!(ring.push(&[1.0; 7], 2), 6);
        let mut out = [0.0; 4];
        assert_eq!(ring.pop(&mut out), 4);
        assert_eq!(ring.push(&[2.0, 3.0, 4.0, 5.0, 6.0, 7.0], 2), 6);
        let mut out = [0.0; 8];
        assert_eq!(ring.pop(&mut out), 8);
        assert_eq!(&out[..4], &[1.0, 1.0, 2.0, 3.0]);
        assert_eq!(ring.len(), 0);
    }

    /// A producer pushing a counting sequence while the consumer pops and now and then clears:
    /// whatever comes out is always in order, never a slot overwritten while being read.
    #[test]
    fn clearing_from_the_consumer_keeps_the_order() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
        let ring = Arc::new(Ring::new(64));
        let done = Arc::new(AtomicBool::new(false));
        let producer = {
            let (ring, done) = (ring.clone(), done.clone());
            std::thread::spawn(move || {
                let mut next = 0.0f32;
                while next < 200_000.0 {
                    let chunk: Vec<f32> = (0..16).map(|i| next + i as f32).collect();
                    next += ring.push(&chunk, 1) as f32;
                }
                done.store(true, Ordering::Release);
            })
        };
        let (mut last, mut out, mut n) = (-1.0f32, [0.0f32; 8], 0u64);
        while !done.load(Ordering::Acquire) || ring.len() > 0 {
            n += 1;
            if n % 97 == 0 {
                ring.clear();
                continue;
            }
            let got = ring.pop(&mut out);
            for &v in &out[..got] {
                assert!(v > last, "{v} came after {last}");
                last = v;
            }
        }
        producer.join().unwrap();
    }
}
