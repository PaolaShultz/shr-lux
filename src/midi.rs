//! MiniLab mkII pad feedback only. No preset store, mapping changes, firmware,
//! bootloader or DMX commands. Hardware is opened only on explicit CLI request.
use crate::preview::{OFF, Pads, Rgb};
use std::{
    io,
    sync::mpsc::{self, Receiver, SyncSender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const PALETTE: [(u8, Rgb); 8] = [
    (0, [0, 0, 0]),
    (1, [255, 0, 0]),
    (4, [0, 255, 0]),
    (5, [255, 255, 0]),
    (16, [0, 0, 255]),
    (17, [255, 0, 255]),
    (20, [0, 255, 255]),
    (127, [255, 255, 255]),
];
pub fn color(rgb: Rgb) -> u8 {
    let max = *rgb.iter().max().unwrap();
    if max < 12 {
        return 0;
    }
    let r = u8::from(u16::from(rgb[0]) * 100 >= u16::from(max) * 45);
    let g = u8::from(u16::from(rgb[1]) * 100 >= u16::from(max) * 45);
    let b = u8::from(u16::from(rgb[2]) * 100 >= u16::from(max) * 45);
    match r | (g << 2) | (b << 4) {
        0x15 => 127,
        c => c,
    }
}
/// Verified message layout from device-specific protocol research. All data bytes
/// stay seven-bit and only the pad-color parameter (0x10) is writable here.
pub fn message(pad: u8, color: u8) -> Option<[u8; 12]> {
    if pad >= 16 || !PALETTE.iter().any(|(code, _)| *code == color) {
        return None;
    }
    Some([
        0xf0,
        0,
        0x20,
        0x6b,
        0x7f,
        0x42,
        2,
        0,
        0x10,
        0x70 + pad,
        color,
        0xf7,
    ])
}

#[cfg(target_os = "linux")]
fn device() -> io::Result<String> {
    let mut devices = Vec::new();
    for entry in std::fs::read_dir("/sys/class/sound")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(numbers) = name.strip_prefix("midiC") else {
            continue;
        };
        let Some((card, port)) = numbers.split_once('D') else {
            continue;
        };
        if !card.chars().all(|c| c.is_ascii_digit()) || !port.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let path = std::fs::canonicalize(entry.path().join("device"))?;
        let minilab = path.ancestors().any(|parent| {
            std::fs::read_to_string(parent.join("idVendor")).is_ok_and(|s| s.trim() == "1c75")
                && std::fs::read_to_string(parent.join("idProduct"))
                    .is_ok_and(|s| s.trim() == "0289")
        });
        if minilab {
            devices.push(format!("hw:{card},{port},0"));
        }
    }
    if devices.len() != 1 {
        return Err(io::Error::other(
            "Connect exactly one MiniLab mkII (USB 1c75:0289) for pad feedback",
        ));
    }
    Ok(devices.remove(0))
}

#[cfg(target_os = "linux")]
struct Output {
    midi: alsa::rawmidi::Rawmidi,
    previous: [u8; 16],
}
#[cfg(target_os = "linux")]
impl Output {
    fn open() -> io::Result<Self> {
        let midi = alsa::rawmidi::Rawmidi::new(&device()?, alsa::Direction::Playback, true)
            .map_err(io::Error::other)?;
        Ok(Self {
            midi,
            previous: [255; 16],
        })
    }
    fn write(&mut self, pads: Pads) -> io::Result<()> {
        let mut bytes = [0_u8; 192];
        let mut length = 0;
        let mut next = self.previous;
        for pad in 0..16 {
            let code = color(pads[pad % 8]);
            if self.previous[pad] != code {
                let message = message(pad as u8, code).unwrap();
                bytes[length..length + 12].copy_from_slice(&message);
                length += 12;
                next[pad] = code;
            }
        }
        write_bounded(&bytes[..length], |bytes| {
            self.midi
                .write(bytes)
                .map_err(|e| io::Error::from_raw_os_error(e.errno()))
        })?;
        self.previous = next;
        Ok(())
    }
}
#[cfg(target_os = "linux")]
impl Drop for Output {
    fn drop(&mut self) {
        // Clear every pad even after a partial/failed write. Bound drain/cleanup.
        self.previous = [255; 16];
        let _ = self.write(OFF);
        let deadline = Instant::now() + Duration::from_millis(100);
        while self.midi.drain().is_err() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(2));
        }
        let _ = self.midi.drop();
    }
}

fn write_bounded(data: &[u8], mut write: impl FnMut(&[u8]) -> io::Result<usize>) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_millis(40);
    let mut position = 0;
    while position < data.len() {
        match write(&data[position..]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "MIDI write made no progress",
                ));
            }
            Ok(n) => position += n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e),
        }
        if position < data.len() {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "MIDI output stalled",
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
    Ok(())
}

fn feedback_loop(
    receiver: Receiver<(Pads, Instant)>,
    mut write: impl FnMut(Pads) -> io::Result<()>,
) -> io::Result<()> {
    let mut last_frame = Instant::now();
    loop {
        match receiver.recv_timeout(Duration::from_millis(40)) {
            Ok((pads, at)) => {
                write(if at.elapsed() <= Duration::from_millis(250) {
                    pads
                } else {
                    OFF
                })?;
                last_frame = at;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if last_frame.elapsed() > Duration::from_millis(250) {
                    write(OFF)?;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return write(OFF),
        }
    }
}

/// One queued frame at most; DSP never waits for MIDI. Frames expire after 250 ms.
/// Drop closes the channel, joins the bounded worker and clears both pad banks.
pub struct Feedback {
    sender: Option<SyncSender<(Pads, Instant)>>,
    result: Receiver<io::Result<()>>,
    worker: Option<JoinHandle<()>>,
}
impl Feedback {
    #[cfg(target_os = "linux")]
    pub fn open() -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<(Pads, Instant)>(1);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (result_tx, result) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let mut output = match Output::open() {
                Ok(o) => {
                    let _ = ready_tx.send(Ok(()));
                    o
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let operation = feedback_loop(receiver, |pads| output.write(pads));
            drop(output);
            let _ = result_tx.send(operation);
        });
        match ready_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => Ok(Self {
                sender: Some(sender),
                result,
                worker: Some(worker),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(error) => Err(io::Error::other(error)),
        }
    }
    #[cfg(not(target_os = "linux"))]
    pub fn open() -> io::Result<Self> {
        Err(io::Error::other(
            "MiniLab output currently requires Linux ALSA",
        ))
    }
    pub fn send(&self, pads: Pads) -> io::Result<()> {
        if let Ok(result) = self.result.try_recv() {
            return result.and(Err(io::Error::other("MIDI worker ended")));
        }
        match self
            .sender
            .as_ref()
            .unwrap()
            .try_send((pads, Instant::now()))
        {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(_)) => Err(io::Error::other("MIDI disconnected")),
        }
    }
}
impl Drop for Feedback {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn test_pads() -> io::Result<()> {
    let stop = crate::live::Stop::new()?;
    let feedback = Feedback::open()?;
    let pads = [
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [0, 0, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255; 3],
        [255, 0, 0],
    ];
    for _ in 0..100 {
        if stop.flag.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        feedback.send(pads)?;
        thread::sleep(Duration::from_millis(40));
    }
    drop(feedback);
    Ok(())
}

/// Explicit manual audition only; never called by the automatic show.
pub fn test_strobe() -> io::Result<()> {
    let stop = crate::live::Stop::new()?;
    let feedback = Feedback::open()?;
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos() as u64;
    let storm = crate::preview::WhiteStorm::new(seed);
    let started = Instant::now();
    while started.elapsed() < crate::preview::WHITE_AUDITION_DURATION
        && !stop.flag.load(std::sync::atomic::Ordering::Relaxed)
    {
        feedback.send(storm.frame(started.elapsed()))?;
        thread::sleep(Duration::from_millis(10));
    }
    drop(feedback);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_protocol_and_no_other_commands() {
        assert_eq!(
            message(0, 1).unwrap(),
            [240, 0, 32, 107, 127, 66, 2, 0, 16, 112, 1, 247]
        );
        for pad in 0..16 {
            for (code, _) in PALETTE {
                let m = message(pad, code).unwrap();
                assert!(m[1..11].iter().all(|x| *x < 128));
                assert_eq!(m[8], 16);
            }
        }
        assert!(message(16, 1).is_none());
        assert!(message(0, 2).is_none());
    }
    #[test]
    fn discrete_palette_preserves_hue_without_claiming_brightness() {
        for (code, rgb) in PALETTE {
            assert_eq!(color(rgb), code);
        }
        assert_eq!(color([0, 0, 50]), 16);
        assert_eq!(color([1; 3]), 0);
    }
    #[test]
    fn partial_writes_are_completed_and_errors_propagated() {
        let mut received = Vec::new();
        write_bounded(&[1, 2, 3, 4], |bytes| {
            received.push(bytes[0]);
            Ok(1)
        })
        .unwrap();
        assert_eq!(received, [1, 2, 3, 4]);
        assert!(write_bounded(&[1], |_| Ok(0)).is_err());
        assert_eq!(
            write_bounded(&[1], |_| Err(io::ErrorKind::BrokenPipe.into()))
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }
    #[test]
    fn stalled_output_is_bounded() {
        let start = Instant::now();
        let error = write_bounded(&[1], |_| Err(io::ErrorKind::WouldBlock.into())).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn stale_frames_and_closed_channel_clear_pads() {
        let (tx, rx) = mpsc::sync_channel(1);
        tx.send(([[255; 3]; 8], Instant::now() - Duration::from_secs(1)))
            .unwrap();
        drop(tx);
        let mut seen = Vec::new();
        feedback_loop(rx, |pads| {
            seen.push(pads);
            Ok(())
        })
        .unwrap();
        assert_eq!(seen, vec![OFF, OFF]);
    }
    #[test]
    fn stalled_producer_is_blacked_out() {
        let (tx, rx) = mpsc::sync_channel(1);
        let (out_tx, out_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            feedback_loop(rx, |pads| {
                out_tx.send(pads).unwrap();
                Ok(())
            })
        });
        let on = [[255; 3]; 8];
        tx.send((on, Instant::now())).unwrap();
        assert_eq!(out_rx.recv_timeout(Duration::from_secs(2)).unwrap(), on);
        assert_eq!(out_rx.recv_timeout(Duration::from_secs(2)).unwrap(), OFF);
        drop(tx);
        worker.join().unwrap().unwrap();
    }
}
