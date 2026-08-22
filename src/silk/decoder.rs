use super::builder::DecoderBuilder;
use super::common::{MAX_INTERNAL_FRAMES, MAX_PAYLOAD_BYTES, SILK_HEADER, sdk_check};
use super::error::SilkError;
use super::sdk::bindings as sdk;
use super::state::DecoderState;

pub struct SilkDecoder {
    state: DecoderState,
    output_sample_rate: i32,
    scratch: Vec<i16>,
}

impl SilkDecoder {
    pub(crate) fn new(builder: DecoderBuilder, state: DecoderState) -> Self {
        Self {
            state,
            output_sample_rate: builder.output_sample_rate,
            scratch: vec![0; 960],
        }
    }

    pub fn decode(&mut self, silk: &[u8]) -> Result<Vec<i16>, SilkError> {
        let mut src = silk;
        if src.first() == Some(&0x02) {
            src = &src[1..];
        }
        if !src.starts_with(SILK_HEADER) {
            return Err(SilkError::InvalidHeader);
        }
        src = &src[SILK_HEADER.len()..];
        self.reset()?;
        let mut output = Vec::new();
        let mut control = sdk::SKP_SILK_SDK_DecControlStruct {
            API_sampleRate: self.output_sample_rate,
            frameSize: 0,
            framesPerPacket: 1,
            moreInternalDecoderFrames: 0,
            inBandFECOffset: 0,
        };
        while !src.is_empty() {
            if src.len() < 2 {
                return Err(SilkError::TruncatedPayload);
            }
            let length = i16::from_le_bytes([src[0], src[1]]);
            src = &src[2..];
            if length == -1 {
                break;
            }
            if length < 0 {
                return Err(SilkError::InvalidPacketLength);
            }
            let length = length as usize;
            if length > MAX_PAYLOAD_BYTES {
                return Err(SilkError::DecPayloadTooLarge);
            }
            if src.len() < length {
                return Err(SilkError::TruncatedPayload);
            }
            let payload = &src[..length];
            src = &src[length..];
            let mut frames = 0;
            loop {
                if frames >= MAX_INTERNAL_FRAMES {
                    return Err(SilkError::TooManyInternalFrames);
                }
                let mut sample_count = self.scratch.len() as i16;
                sdk_check(unsafe {
                    sdk::SKP_Silk_SDK_Decode(
                        self.state.as_ptr(),
                        &mut control,
                        0,
                        payload.as_ptr(),
                        payload.len() as i32,
                        self.scratch.as_mut_ptr(),
                        &mut sample_count,
                    )
                })?;
                let sample_count =
                    usize::try_from(sample_count).map_err(|_| SilkError::DecPayloadError)?;
                output.extend_from_slice(&self.scratch[..sample_count]);
                frames += 1;
                if control.moreInternalDecoderFrames == 0 {
                    break;
                }
            }
        }
        Ok(output)
    }

    pub fn decode_bytes(&mut self, silk: &[u8]) -> Result<Vec<u8>, SilkError> {
        let samples = self.decode(silk)?;
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        Ok(bytes)
    }

    fn reset(&mut self) -> Result<(), SilkError> {
        sdk_check(unsafe { sdk::SKP_Silk_SDK_InitDecoder(self.state.as_ptr()) })
    }
}
