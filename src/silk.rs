use bytes::{Buf, BufMut};
use std::ffi::c_void;
use thiserror::Error;

#[allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]
pub(crate) mod sdk {
    include!(concat!(env!("OUT_DIR"), "/silk_bindings.rs"));
}

macro_rules! fast_check {
    ($call:expr) => {{
        unsafe {
            let code = $call;
            if code != 0 {
                return Err(SilkError::from(code));
            }
        }
    }};
}

pub fn decode_silk<R: AsRef<[u8]>>(src: R, sample_rate: i32) -> Result<Vec<u8>, SilkError> {
    unsafe { _decode_silk(src.as_ref(), sample_rate) }
}

unsafe fn _decode_silk(mut src: &[u8], sample_rate: i32) -> Result<Vec<u8>, SilkError> {
    // skip tencent flag
    if src.starts_with(&[0x02]) {
        src.advance(1);
    };

    const SILK_HEADER: &[u8] = b"#!SILK_V3";
    if src.starts_with(SILK_HEADER) {
        src.advance(SILK_HEADER.len());
    } else {
        return Err(SilkError::Invalid);
    };

    let mut dec_control = sdk::SKP_SILK_SDK_DecControlStruct {
        API_sampleRate: sample_rate,
        frameSize: 0,
        framesPerPacket: 1,
        moreInternalDecoderFrames: 0,
        inBandFECOffset: 0,
    };

    let mut decoder_size = 0;

    fast_check!(sdk::SKP_Silk_SDK_Get_Decoder_Size(&mut decoder_size));

    let mut decoder = vec![0u8; decoder_size as usize];

    fast_check!(sdk::SKP_Silk_SDK_InitDecoder(
        decoder.as_mut_ptr() as *mut c_void
    ));

    let mut result = vec![];
    let frame_size = sample_rate as usize / 1000 * 40;
    let mut buf = vec![0u8; frame_size];
    loop {
        if src.remaining() < 2 {
            break;
        }
        let input_size = src.get_i16_le();
        if input_size > frame_size as i16 {
            return Err(SilkError::Invalid);
        }
        if src.remaining() < input_size as usize {
            return Err(SilkError::Invalid);
        }

        let input;
        (input, src) = src.split_at(input_size as usize);

        let mut output_size = 0i16;

        fast_check!(sdk::SKP_Silk_SDK_Decode(
            decoder.as_mut_ptr() as *mut c_void,
            &mut dec_control,
            0,
            input.as_ptr(),
            input_size as i32,
            buf.as_mut_ptr() as *mut i16,
            &mut output_size,
        ));

        result.extend_from_slice(&buf[0..output_size as usize * 2])
    }
    Ok(result)
}

pub fn encode_silk<R: AsRef<[u8]>>(
    src: R,
    sample_rate: i32,
    bit_rate: i32,
    tencent: bool,
) -> Result<Vec<u8>, SilkError> {
    unsafe { _encode_silk(src.as_ref(), sample_rate, bit_rate, tencent) }
}

unsafe fn _encode_silk(
    src: &[u8],
    sample_rate: i32,
    bit_rate: i32,
    tencent: bool,
) -> Result<Vec<u8>, SilkError> {
    let enc_control = sdk::SKP_SILK_SDK_EncControlStruct {
        API_sampleRate: sample_rate,
        maxInternalSampleRate: 24000,
        packetSize: (20 * sample_rate) / 1000,
        bitRate: bit_rate,
        packetLossPercentage: 0,
        complexity: 2,
        useInBandFEC: 0,
        useDTX: 0,
    };

    let mut enc_status = sdk::SKP_SILK_SDK_EncControlStruct {
        API_sampleRate: 0,
        maxInternalSampleRate: 0,
        packetSize: 0,
        bitRate: bit_rate,
        packetLossPercentage: 0,
        complexity: 0,
        useInBandFEC: 0,
        useDTX: 0,
    };

    let mut encoder_size = 0;
    fast_check!(sdk::SKP_Silk_SDK_Get_Encoder_Size(&mut encoder_size));

    let mut encoder = vec![0u8; encoder_size as usize];

    fast_check!(sdk::SKP_Silk_SDK_InitEncoder(
        encoder.as_mut_ptr() as *mut c_void,
        &mut enc_status,
    ));

    let mut result = vec![];
    if tencent {
        result.put_u8(b'\x02');
    }
    result.extend_from_slice(b"#!SILK_V3");

    let frame_size = sample_rate as usize / 1000 * 40;
    let mut output_size = 1250i16;
    let mut buf = vec![0u8; output_size as usize];
    for chunk in src.chunks(frame_size) {
        output_size = 1250;
        if chunk.len() < frame_size {
            break;
        }
        fast_check!(sdk::SKP_Silk_SDK_Encode(
            encoder.as_mut_ptr() as *mut c_void,
            &enc_control,
            chunk.as_ptr() as *const i16,
            chunk.len() as i32 / 2,
            buf.as_mut_ptr(),
            &mut output_size,
        ));
        result.put_i16_le(output_size);
        result.extend_from_slice(&buf[0..output_size as usize]);
    }

    Ok(result)
}

#[derive(Error, Debug)]
pub enum SilkError {
    #[error("Invalid")]
    Invalid,
    #[error("EncInputInvalidNoOfSamples")]
    EncInputInvalidNoOfSamples,
    #[error("EncFsNotSupported")]
    EncFsNotSupported,
    #[error("EncPacketSizeNotSupported")]
    EncPacketSizeNotSupported,
    #[error("EncPayloadBufTooShort")]
    EncPayloadBufTooShort,
    #[error("EncInvalidLossRate")]
    EncInvalidLossRate,
    #[error("EncInvalidComplexitySetting")]
    EncInvalidComplexitySetting,
    #[error("EncInvalidInbandFecSetting")]
    EncInvalidInbandFecSetting,
    #[error("EncInvalidDtxSetting")]
    EncInvalidDtxSetting,
    #[error("EncInternalError")]
    EncInternalError,
    #[error("DecInvalidSamplingFrequency")]
    DecInvalidSamplingFrequency,
    #[error("DecPayloadTooLarge")]
    DecPayloadTooLarge,
    #[error("DecPayloadError")]
    DecPayloadError,
    #[error("OTHER {0}")]
    Other(i32),
}

impl From<i32> for SilkError {
    fn from(code: i32) -> Self {
        match code {
            -1 => Self::EncInputInvalidNoOfSamples,
            -2 => Self::EncFsNotSupported,
            -3 => Self::EncPacketSizeNotSupported,
            -4 => Self::EncPayloadBufTooShort,
            -5 => Self::EncInvalidLossRate,
            -6 => Self::EncInvalidComplexitySetting,
            -7 => Self::EncInvalidInbandFecSetting,
            -8 => Self::EncInvalidDtxSetting,
            -9 => Self::EncInternalError,
            -10 => Self::DecInvalidSamplingFrequency,
            -11 => Self::DecPayloadTooLarge,
            -12 => Self::DecPayloadError,
            _ => Self::Other(code),
        }
    }
}
