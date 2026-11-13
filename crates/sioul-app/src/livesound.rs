// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The noises and sounds of nature made as they play (sioul_core::sounds,
//! docs/sounds.md), played through Qt's audio output (cpp/livesound.cpp): one
//! at a time, started and stopped on the window's thread, read by the output
//! on whichever thread it reads from.

use std::cell::RefCell;
use std::ffi::c_void;

use sioul_core::sounds::Stream;

unsafe extern "C" {
    /// `stream` played at `rate` frames a second, both ears, 16 bits, at
    /// `volume` (cpp/livesound.cpp); null when the system's output takes no
    /// such sound.
    fn sioul_live_start(stream: *const c_void, rate: i32, volume: f64) -> *mut c_void;
    /// The volume of `sound`, from `sioul_live_start`, 0 to 1.
    fn sioul_live_volume(sound: *mut c_void, volume: f64);
    /// `sound` stopped and freed; its stream is read no more once this returns.
    fn sioul_live_stop(sound: *mut c_void);
}

/// The sound playing: its output, which reads its stream until it is stopped,
/// then the stream, kept for the output to read and freed after it.
struct Playing {
    output: *mut c_void,
    _stream: Box<Stream>,
}

impl Drop for Playing {
    fn drop(&mut self) {
        // SAFETY: `output` came from `sioul_live_start` and is stopped once, here; the
        // stream it reads is freed after, with the other fields.
        unsafe { sioul_live_stop(self.output) };
    }
}

thread_local! {
    /// The sound playing, on the window's thread.
    static PLAYING: RefCell<Option<Playing>> = const { RefCell::new(None) };
}

/// `kind` ("waves", "pink") played as it is made, from a new seed, at `volume`,
/// in place of the one playing; whether it plays.
pub(crate) fn start(kind: &str, volume: f64) -> bool {
    stop();
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_nanos() as u64);
    let Some(stream) = Stream::start(kind, seed).map(Box::new) else { return false };
    let rate = i32::try_from(stream.rate()).unwrap_or(i32::MAX);
    // SAFETY: the output reads the stream through `sioul_live_read` until `sioul_live_stop`,
    // which `Playing` calls before the stream, boxed and never moved, is freed.
    let output = unsafe { sioul_live_start(std::ptr::from_ref::<Stream>(&stream).cast(), rate, volume) };
    if output.is_null() {
        return false;
    }
    PLAYING.with(|playing| *playing.borrow_mut() = Some(Playing { output, _stream: stream }));
    true
}

/// The volume of the sound playing, at once, 0 to 1.
pub(crate) fn volume(volume: f64) {
    PLAYING.with(|playing| {
        if let Some(playing) = &*playing.borrow() {
            // SAFETY: `output` is playing until `Playing` is dropped.
            unsafe { sioul_live_volume(playing.output, volume) };
        }
    });
}

/// The sound playing stopped.
pub(crate) fn stop() {
    let stopped = PLAYING.with(|playing| playing.borrow_mut().take());
    drop(stopped);
}

/// Qt's audio output asking for `length` bytes of `stream` at `out`
/// (cpp/livesound.cpp): native 16-bit samples, the two ears in turn.
///
/// # Safety
/// `stream` is a `Stream` alive for the call (`Playing` keeps it until its
/// output is stopped), and `out` holds `length` writable bytes.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_live_read(stream: *const c_void, out: *mut u8, length: usize) {
    if stream.is_null() || out.is_null() {
        return;
    }
    // SAFETY: as this function's contract says.
    let (stream, out) = unsafe { (&*stream.cast::<Stream>(), std::slice::from_raw_parts_mut(out, length)) };
    let mut samples = vec![0i16; length / 2];
    stream.read(&mut samples);
    for (bytes, sample) in out.chunks_exact_mut(2).zip(&samples) {
        bytes.copy_from_slice(&sample.to_ne_bytes());
    }
}
