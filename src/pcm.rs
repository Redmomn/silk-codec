#![cfg(feature = "symphonia")]
use std::io::{Read, Write};
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PcmError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Symphonia error: {0}")]
    Symphonia(#[from] SymphoniaError),
    #[error("unsupported audio format")]
    UnsupportedFormat,
    #[error("audio track not found")]
    NoAudioTrack,
    #[error("decoder creation failed")]
    DecoderCreationFailed,
}

/// 音频转换器，支持streaming处理
pub struct AudioConverter {
    target_sample_rate: u32,
    target_channels: u32,
}

impl AudioConverter {
    pub fn new() -> Self {
        Self {
            target_sample_rate: 24000,
            target_channels: 1,
        }
    }

    pub fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.target_sample_rate = sample_rate;
        self
    }

    pub fn with_channels(mut self, channels: u32) -> Self {
        self.target_channels = channels;
        self
    }

    pub fn convert_to_pcm<P: AsRef<Path>>(
        &self,
        input_path: P,
        output_path: P,
    ) -> Result<(), PcmError> {
        let input_file = std::fs::File::open(&input_path)?;
        let output_file = std::fs::File::create(&output_path)?;
        let format_hint = input_path.as_ref().extension().and_then(|ext| ext.to_str());
        self.convert_streaming(input_file, output_file, format_hint)?;
        Ok(())
    }

    pub fn convert_streaming<R, W>(
        &self,
        input: R,
        mut output: W,
        format_hint: Option<&str>,
    ) -> Result<(), PcmError>
    where
        R: Read + Send + MediaSource + 'static,
        W: Write,
    {
        let media_source = MediaSourceStream::new(Box::new(input), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = format_hint {
            hint.with_extension(ext);
        }

        // 探测格式
        let meta_opts = MetadataOptions::default();
        let fmt_opts = FormatOptions::default();

        let probed =
            symphonia::default::get_probe().format(&hint, media_source, &fmt_opts, &meta_opts)?;

        let mut format = probed.format;

        // 取第一个音轨
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or(PcmError::NoAudioTrack)?;

        let track_id = track.id;

        let dec_opts = DecoderOptions::default();
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &dec_opts)
            .map_err(|_| PcmError::DecoderCreationFailed)?;

        // 源音频参数
        let source_sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let source_channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2) as u32;

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::ResetRequired) => {
                    decoder.reset();
                    continue;
                }
                Err(SymphoniaError::IoError(err)) => {
                    if err.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    }
                    return Err(PcmError::Symphonia(SymphoniaError::IoError(err)));
                }
                Err(err) => return Err(PcmError::Symphonia(err)),
            };

            if packet.track_id() != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    let pcm_data =
                        self.process_audio_buffer(&decoded, source_sample_rate, source_channels)?;
                    output.write_all(&pcm_data)?;
                }
                Err(SymphoniaError::IoError(err)) => {
                    if err.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    }
                    return Err(PcmError::Symphonia(SymphoniaError::IoError(err)));
                }
                Err(SymphoniaError::DecodeError(_)) => {
                    continue;
                }
                Err(err) => return Err(PcmError::Symphonia(err)),
            }
        }

        output.flush()?;
        Ok(())
    }

    pub fn convert_bytes_to_pcm(
        &self,
        input_data: &[u8],
        format_hint: Option<&str>,
    ) -> Result<Vec<u8>, PcmError> {
        let mut output = Vec::new();
        let input = std::io::Cursor::new(input_data.to_vec());
        self.convert_streaming(input, &mut output, format_hint)?;
        Ok(output)
    }

    fn convert_buffer_to_f32(&self, decoded: &AudioBufferRef, source_channels: u32) -> Vec<f32> {
        let spec = *decoded.spec();
        let duration = decoded.frames();
        let channels_count = spec.channels.count();
        let need_mix_down = source_channels > 1 && self.target_channels == 1 && channels_count > 1;

        let mut samples = Vec::with_capacity(duration);

        match decoded {
            AudioBufferRef::S16(buf) => {
                let left_chan = buf.chan(0);
                let right_chan = if need_mix_down && channels_count > 1 {
                    Some(buf.chan(1))
                } else {
                    None
                };

                for i in 0..duration.min(left_chan.len()) {
                    let left = left_chan[i] as f32 / 32768.0;
                    if let Some(right_data) = right_chan {
                        if i < right_data.len() {
                            let right = right_data[i] as f32 / 32768.0;
                            samples.push(self.stereo_to_mono_mix(left, right));
                        } else {
                            samples.push(left);
                        }
                    } else {
                        samples.push(left);
                    }
                }
            }
            AudioBufferRef::F32(buf) => {
                let left_chan = buf.chan(0);
                let right_chan = if need_mix_down && channels_count > 1 {
                    Some(buf.chan(1))
                } else {
                    None
                };

                for i in 0..duration.min(left_chan.len()) {
                    let left = left_chan[i];
                    if let Some(right_data) = right_chan {
                        if i < right_data.len() {
                            let right = right_data[i];
                            samples.push(self.stereo_to_mono_mix(left, right));
                        } else {
                            samples.push(left);
                        }
                    } else {
                        samples.push(left);
                    }
                }
            }
            AudioBufferRef::U8(buf) => {
                let left_chan = buf.chan(0);
                let right_chan = if need_mix_down && channels_count > 1 {
                    Some(buf.chan(1))
                } else {
                    None
                };

                for i in 0..duration.min(left_chan.len()) {
                    let left = (left_chan[i] as f32 - 128.0) / 128.0;
                    if let Some(right_data) = right_chan {
                        if i < right_data.len() {
                            let right = (right_data[i] as f32 - 128.0) / 128.0;
                            samples.push(self.stereo_to_mono_mix(left, right));
                        } else {
                            samples.push(left);
                        }
                    } else {
                        samples.push(left);
                    }
                }
            }
            AudioBufferRef::S32(buf) => {
                let left_chan = buf.chan(0);
                let right_chan = if need_mix_down && channels_count > 1 {
                    Some(buf.chan(1))
                } else {
                    None
                };

                for i in 0..duration.min(left_chan.len()) {
                    let left = left_chan[i] as f32 / 2147483648.0;
                    if let Some(right_data) = right_chan {
                        if i < right_data.len() {
                            let right = right_data[i] as f32 / 2147483648.0;
                            samples.push(self.stereo_to_mono_mix(left, right));
                        } else {
                            samples.push(left);
                        }
                    } else {
                        samples.push(left);
                    }
                }
            }
            // 其他格式通用处理
            _ => {
                for i in 0..duration {
                    let sample = match decoded {
                        AudioBufferRef::U16(buf) => {
                            if i < buf.chan(0).len() {
                                (buf.chan(0)[i] as f32 - 32768.0) / 32768.0
                            } else {
                                0.0
                            }
                        }
                        AudioBufferRef::U32(buf) => {
                            if i < buf.chan(0).len() {
                                (buf.chan(0)[i] as f32 - 2147483648.0) / 2147483648.0
                            } else {
                                0.0
                            }
                        }
                        AudioBufferRef::S8(buf) => {
                            if i < buf.chan(0).len() {
                                buf.chan(0)[i] as f32 / 128.0
                            } else {
                                0.0
                            }
                        }
                        AudioBufferRef::F64(buf) => {
                            if i < buf.chan(0).len() {
                                buf.chan(0)[i] as f32
                            } else {
                                0.0
                            }
                        }
                        _ => 0.0,
                    };

                    if need_mix_down {
                        let right_sample = match decoded {
                            AudioBufferRef::U16(buf) => {
                                if i < buf.chan(1).len() {
                                    (buf.chan(1)[i] as f32 - 32768.0) / 32768.0
                                } else {
                                    0.0
                                }
                            }
                            AudioBufferRef::U32(buf) => {
                                if i < buf.chan(1).len() {
                                    (buf.chan(1)[i] as f32 - 2147483648.0) / 2147483648.0
                                } else {
                                    0.0
                                }
                            }
                            AudioBufferRef::S8(buf) => {
                                if i < buf.chan(1).len() {
                                    buf.chan(1)[i] as f32 / 128.0
                                } else {
                                    0.0
                                }
                            }
                            AudioBufferRef::F64(buf) => {
                                if i < buf.chan(1).len() {
                                    buf.chan(1)[i] as f32
                                } else {
                                    0.0
                                }
                            }
                            _ => 0.0,
                        };
                        samples.push(self.stereo_to_mono_mix(sample, right_sample));
                    } else {
                        samples.push(sample);
                    }
                }
            }
        }

        samples
    }

    fn process_audio_buffer(
        &self,
        decoded: &AudioBufferRef,
        source_sample_rate: u32,
        source_channels: u32,
    ) -> Result<Vec<u8>, PcmError> {
        // 转换为f32样本
        let samples = self.convert_buffer_to_f32(decoded, source_channels);

        let final_samples = if source_sample_rate != self.target_sample_rate {
            self.resample(&samples, source_sample_rate, self.target_sample_rate)
        } else {
            samples
        };

        let pcm_data = self.clean_samples_to_pcm_bytes(&final_samples);
        Ok(pcm_data)
    }

    fn clean_samples_to_pcm_bytes(&self, samples: &[f32]) -> Vec<u8> {
        let mut pcm_data = Vec::with_capacity(samples.len() * 2);

        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let sample_i16 = if clamped >= 0.0 {
                (clamped * 32767.0 + 0.5) as i16
            } else {
                (clamped * 32768.0 - 0.5) as i16
            };

            pcm_data.extend_from_slice(&sample_i16.to_le_bytes());
        }

        pcm_data
    }

    /// resample
    fn resample(&self, samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
        if source_rate == target_rate {
            return samples.to_vec();
        }

        // 线性插值重采样
        let ratio = source_rate as f64 / target_rate as f64;
        let target_len = (samples.len() as f64 / ratio) as usize;
        let mut resampled = Vec::with_capacity(target_len);

        for i in 0..target_len {
            let source_pos = i as f64 * ratio;
            let idx = source_pos as usize;

            if idx + 1 < samples.len() {
                let frac = source_pos - idx as f64;
                let interpolated =
                    samples[idx] * (1.0 - frac as f32) + samples[idx + 1] * frac as f32;
                resampled.push(interpolated);
            } else if idx < samples.len() {
                resampled.push(samples[idx]);
            }
        }

        resampled
    }

    /// 混音
    fn stereo_to_mono_mix(&self, left: f32, right: f32) -> f32 {
        // 平均混音
        (left + right) * 0.5
    }
}

impl Default for AudioConverter {
    fn default() -> Self {
        Self::new()
    }
}

/// resample 24000 to pcm
pub fn convert_audio_to_pcm<P: AsRef<Path>>(input_path: P, output_path: P) -> Result<(), PcmError> {
    let converter = AudioConverter::new();
    converter.convert_to_pcm(input_path, output_path)
}

pub fn convert_audio_bytes_to_pcm(
    input_data: &[u8],
    format_hint: Option<&str>,
) -> Result<Vec<u8>, PcmError> {
    let converter = AudioConverter::new();
    converter.convert_bytes_to_pcm(input_data, format_hint)
}
