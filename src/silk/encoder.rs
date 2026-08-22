use std::borrow::Cow;

use super::builder::EncoderBuilder;
use super::common::{MAX_PAYLOAD_BYTES, SILK_HEADER, TailPolicy, sdk_check};
use super::error::SilkError;
use super::sdk::bindings as sdk;
use super::state::EncoderState;

pub struct SilkEncoder {
    state: EncoderState,
    sample_rate: i32,
    bit_rate: i32,
    tail_policy: TailPolicy,
    complexity: i32,
    use_in_band_fec: bool,
    use_dtx: bool,
    frame_samples: usize,
    payload: Vec<u8>,
}

impl SilkEncoder {
    pub(crate) fn new(builder: EncoderBuilder, frame_samples: usize) -> Result<Self, SilkError> {
        Ok(Self {
            state: EncoderState::new()?,
            sample_rate: builder.sample_rate,
            bit_rate: builder.bit_rate,
            tail_policy: builder.tail_policy,
            complexity: builder.complexity,
            use_in_band_fec: builder.use_in_band_fec,
            use_dtx: builder.use_dtx,
            frame_samples,
            payload: vec![0; 1250],
        })
    }

    pub fn encode(&mut self, pcm: &[i16], tencent: bool) -> Result<Vec<u8>, SilkError> {
        let complete = match self.tail_policy {
            TailPolicy::Drop => pcm.len() / self.frame_samples * self.frame_samples,
            TailPolicy::Reject => {
                if !pcm.len().is_multiple_of(self.frame_samples) {
                    return Err(SilkError::InvalidPcmLength);
                }
                pcm.len()
            }
            TailPolicy::PadWithSilence => pcm
                .len()
                .div_ceil(self.frame_samples)
                .checked_mul(self.frame_samples)
                .ok_or(SilkError::InvalidPcmLength)?,
        };
        self.reset()?;
        let input: Cow<'_, [i16]> = if complete <= pcm.len() {
            Cow::Borrowed(&pcm[..complete])
        } else {
            let mut padded = Vec::with_capacity(complete);
            padded.extend_from_slice(pcm);
            padded.resize(complete, 0);
            Cow::Owned(padded)
        };
        let mut output = Vec::with_capacity(1600);
        if tencent {
            output.push(0x02);
        }
        output.extend_from_slice(SILK_HEADER);
        let control = sdk::SKP_SILK_SDK_EncControlStruct {
            API_sampleRate: self.sample_rate,
            maxInternalSampleRate: 24_000,
            packetSize: self.frame_samples as i32,
            bitRate: self.bit_rate,
            packetLossPercentage: 0,
            complexity: self.complexity,
            useInBandFEC: i32::from(self.use_in_band_fec),
            useDTX: i32::from(self.use_dtx),
        };
        for frame in input.chunks_exact(self.frame_samples) {
            let mut payload_size = self.payload.len() as i16;
            sdk_check(unsafe {
                sdk::SKP_Silk_SDK_Encode(
                    self.state.as_ptr(),
                    &control,
                    frame.as_ptr(),
                    frame.len() as i32,
                    self.payload.as_mut_ptr(),
                    &mut payload_size,
                )
            })?;
            let payload_size =
                usize::try_from(payload_size).map_err(|_| SilkError::InvalidPacketLength)?;
            if payload_size > MAX_PAYLOAD_BYTES {
                return Err(SilkError::DecPayloadTooLarge);
            }
            let len = u16::try_from(payload_size).map_err(|_| SilkError::InvalidPacketLength)?;
            output.extend_from_slice(&len.to_le_bytes());
            output.extend_from_slice(&self.payload[..payload_size]);
        }
        if !tencent {
            output.extend_from_slice(&(-1i16).to_le_bytes());
        }
        Ok(output)
    }

    pub fn encode_bytes(&mut self, pcm_s16le: &[u8], tencent: bool) -> Result<Vec<u8>, SilkError> {
        if !pcm_s16le.len().is_multiple_of(2) {
            return Err(SilkError::InvalidPcmLength);
        }
        let mut samples = Vec::with_capacity(pcm_s16le.len() / 2);
        for b in pcm_s16le.chunks_exact(2) {
            samples.push(i16::from_le_bytes([b[0], b[1]]));
        }
        self.encode(&samples, tencent)
    }

    fn reset(&mut self) -> Result<(), SilkError> {
        let mut status = sdk::SKP_SILK_SDK_EncControlStruct {
            API_sampleRate: 0,
            maxInternalSampleRate: 0,
            packetSize: 0,
            bitRate: 0,
            packetLossPercentage: 0,
            complexity: 0,
            useInBandFEC: 0,
            useDTX: 0,
        };
        sdk_check(unsafe { sdk::SKP_Silk_SDK_InitEncoder(self.state.as_ptr(), &mut status) })
    }
}
