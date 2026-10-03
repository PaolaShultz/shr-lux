//! File-backed substitute for four AUX inputs. No sound card or hardware I/O.
//! Prepared WAV channel order is kick, bass, guitar 1, guitar 2.
use std::{
    error::Error,
    fmt,
    io::{Read, Seek},
    time::Duration,
};

pub const SOURCE_NAMES: [&str; 4] = ["kick", "bass", "guitar_1", "guitar_2"];
pub const SAMPLE_RATE: u32 = 48_000;
pub type AuxFrame = [f32; 4];

#[derive(Debug)]
pub struct ReplayError(String);

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for ReplayError {}
impl From<hound::Error> for ReplayError {
    fn from(error: hound::Error) -> Self {
        Self(error.to_string())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Block {
    pub first_frame: u32,
    pub frames: usize,
}

impl Block {
    pub fn timestamp(&self) -> Duration {
        frame_time(self.first_frame)
    }
}

pub fn frame_time(frame: u32) -> Duration {
    Duration::from_nanos(u64::from(frame) * 1_000_000_000 / u64::from(SAMPLE_RATE))
}

/// Owns only the decoder and reader, never the entire song. Caller supplies a
/// reusable block buffer. EOF is an empty block; decode errors are not silence.
/// Seek starts a new playback timeline: downstream analysis must reset its history.
pub struct AuxReplay<R: Read + Seek> {
    reader: hound::WavReader<R>,
    position: u32,
    failed: bool,
}

impl<R: Read + Seek> AuxReplay<R> {
    pub fn new(source: R) -> Result<Self, ReplayError> {
        let reader = hound::WavReader::new(source)?;
        let spec = reader.spec();
        if spec.channels != 4
            || spec.sample_rate != SAMPLE_RATE
            || spec.bits_per_sample != 16
            || spec.sample_format != hound::SampleFormat::Int
            || reader.duration() == 0
        {
            return Err(ReplayError(
                "Expected nonempty 4-channel 48000 Hz signed 16-bit PCM WAV; run scripts/prepare-simulation.py"
                    .into(),
            ));
        }
        Ok(Self {
            reader,
            position: 0,
            failed: false,
        })
    }

    pub fn total_frames(&self) -> u32 {
        self.reader.duration()
    }

    pub fn seek(&mut self, frame: u32) -> Result<(), ReplayError> {
        if frame > self.total_frames() {
            return Err(ReplayError("Seek exceeds song length".into()));
        }
        self.reader
            .seek(frame)
            .map_err(|e| ReplayError(e.to_string()))?;
        self.position = frame;
        self.failed = false;
        Ok(())
    }

    pub fn read_block(&mut self, output: &mut [AuxFrame]) -> Result<Block, ReplayError> {
        if self.failed {
            return Err(ReplayError(
                "Replay failed; reopen or seek before continuing".into(),
            ));
        }
        if output.is_empty() {
            return Err(ReplayError("Replay buffer must not be empty".into()));
        }
        let count = output
            .len()
            .min((self.total_frames() - self.position) as usize);
        let block = Block {
            first_frame: self.position,
            frames: count,
        };
        let mut samples = self.reader.samples::<i16>();
        for frame in &mut output[..count] {
            for channel in frame {
                match samples.next() {
                    Some(Ok(sample)) => *channel = f32::from(sample) / 32768.0,
                    result => {
                        self.failed = true;
                        return Err(ReplayError(match result {
                            Some(Err(error)) => format!("Cannot read AUX audio: {error}"),
                            _ => "Truncated AUX audio".into(),
                        }));
                    }
                }
            }
        }
        self.position += count as u32;
        Ok(block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn wav(channels: u16, rate: u32, samples: &[i16]) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(
            &mut buffer,
            hound::WavSpec {
                channels,
                sample_rate: rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for s in samples {
            writer.write_sample(*s).unwrap();
        }
        writer.finalize().unwrap();
        buffer.into_inner()
    }

    #[test]
    fn preserves_channel_order_signed_scale_and_final_partial_block() {
        let mut replay = AuxReplay::new(Cursor::new(wav(
            4,
            SAMPLE_RATE,
            &[16384, -16384, 0, -32768, 100, 200, 300, 400, 10, 20, 30, 40],
        )))
        .unwrap();
        let mut out = [[0.0; 4]; 2];
        assert_eq!(
            replay.read_block(&mut out).unwrap(),
            Block {
                first_frame: 0,
                frames: 2
            }
        );
        assert_eq!(out[0], [0.5, -0.5, 0.0, -1.0]);
        assert_eq!(
            out[1],
            [
                100.0 / 32768.0,
                200.0 / 32768.0,
                300.0 / 32768.0,
                400.0 / 32768.0
            ]
        );
        assert_eq!(
            replay.read_block(&mut out).unwrap(),
            Block {
                first_frame: 2,
                frames: 1
            }
        );
        assert_eq!(out[0][3], 40.0 / 32768.0);
        assert_eq!(replay.read_block(&mut out).unwrap().frames, 0);
        assert_eq!(replay.read_block(&mut out).unwrap().frames, 0);
    }

    #[test]
    fn seek_and_timestamps_are_sample_based() {
        let samples = vec![1; 4 * 48_002];
        let mut replay = AuxReplay::new(Cursor::new(wav(4, SAMPLE_RATE, &samples))).unwrap();
        replay.seek(48_000).unwrap();
        let block = replay.read_block(&mut [[0.0; 4]; 10]).unwrap();
        assert_eq!(block.timestamp(), Duration::from_secs(1));
        assert_eq!(block.frames, 2);
        assert!(replay.seek(48_003).is_err());
        replay.seek(0).unwrap();
        assert_eq!(
            replay.read_block(&mut [[0.0; 4]; 1]).unwrap().first_frame,
            0
        );
        replay.seek(48_002).unwrap();
        assert_eq!(replay.read_block(&mut [[0.0; 4]; 1]).unwrap().frames, 0);
    }

    #[test]
    fn rejects_wrong_format_empty_audio_and_empty_buffer() {
        for (channels, rate, samples) in [
            (2, 48_000, vec![0; 8]),
            (4, 44_100, vec![0; 8]),
            (4, 48_000, vec![]),
        ] {
            assert!(AuxReplay::new(Cursor::new(wav(channels, rate, &samples))).is_err());
        }
        let mut replay = AuxReplay::new(Cursor::new(wav(4, SAMPLE_RATE, &[0; 4]))).unwrap();
        assert!(replay.read_block(&mut []).is_err());
    }

    #[test]
    fn truncated_audio_is_an_error_and_stays_failed() {
        let mut bytes = wav(4, SAMPLE_RATE, &[1; 12]);
        bytes.truncate(bytes.len() - 3);
        let mut replay = AuxReplay::new(Cursor::new(bytes)).unwrap();
        assert!(replay.read_block(&mut [[0.0; 4]; 3]).is_err());
        assert!(replay.read_block(&mut [[0.0; 4]; 1]).is_err());
    }

    #[test]
    fn different_block_sizes_produce_identical_frames() {
        let samples: Vec<i16> = (0..404).map(|i| i - 202).collect();
        let bytes = wav(4, SAMPLE_RATE, &samples);
        let collect = |size| {
            let mut replay = AuxReplay::new(Cursor::new(bytes.clone())).unwrap();
            let mut buffer = vec![[0.0; 4]; size];
            let mut frames = Vec::new();
            loop {
                let block = replay.read_block(&mut buffer).unwrap();
                if block.frames == 0 {
                    break;
                }
                frames.extend_from_slice(&buffer[..block.frames]);
            }
            frames
        };
        assert_eq!(collect(1), collect(17));
    }
}
