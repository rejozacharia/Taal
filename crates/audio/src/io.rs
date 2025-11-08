use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use symphonia::core::audio::{AudioBufferRef, SampleBuffer, Signal};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioReader {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

pub struct AudioDecoder;

impl AudioDecoder {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<AudioReader> {
        let path_ref = path.as_ref();
        let file =
            File::open(path_ref).with_context(|| format!("open audio file {:?}", path_ref))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path_ref.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;
        let mut format = probed.format;
        let track = format
            .default_track()
            .ok_or_else(|| anyhow::anyhow!("no default track found"))?;
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())?;
        let mut samples = Vec::new();
        let sample_rate = track.codec_params.sample_rate.unwrap_or(48_000);
        let channels = track
            .codec_params
            .channels
            .map(|c| c.count() as u16)
            .unwrap_or(1);

        loop {
            match format.next_packet() {
                Ok(packet) => {
                    let buffer = decoder.decode(&packet)?;
                    match buffer {
                        AudioBufferRef::F32(buf) => {
                            let channels = buf.spec().channels.count() as usize;
                            for ch in 0..channels {
                                let data = buf.chan(ch);
                                samples.extend_from_slice(data);
                            }
                        }
                        AudioBufferRef::U8(buf) => {
                            let channels = buf.spec().channels.count() as usize;
                            for ch in 0..channels {
                                let data = buf.chan(ch);
                                samples.extend(data.iter().map(|&s| (s as f32 / 255.0) * 2.0 - 1.0));
                            }
                        }
                        AudioBufferRef::S16(buf) => {
                            let channels = buf.spec().channels.count() as usize;
                            for ch in 0..channels {
                                let data = buf.chan(ch);
                                samples.extend(data.iter().map(|&s| s as f32 / i16::MAX as f32));
                            }
                        }
                        other => {
                            let spec = *other.spec();
                            let frames = other.frames() as u64;
                            let mut out = SampleBuffer::<f32>::new(frames, spec);
                            out.copy_interleaved_ref(other);
                            samples.extend_from_slice(out.samples());
                        }
                    }
                }
                Err(err) => {
                    use symphonia::core::errors::Error as SymphError;
                    match err {
                        SymphError::IoError(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                            break;
                        }
                        SymphError::DecodeError(_) => {
                            // skip undecodable packet
                        }
                        _ => return Err(err.into()),
                    }
                }
            }
        }

        Ok(AudioReader {
            sample_rate,
            channels,
            samples,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_reader_handles_missing_file() {
        let result = AudioDecoder::open("does-not-exist.wav");
        assert!(result.is_err());
    }
}
