use silk_codec::{SilkError, decode_silk, encode_silk};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::slice;

#[cfg(feature = "ffmpeg")]
use silk_codec::{
    PcmError, VideoError, convert_audio_to_pcm, get_video_metadata, save_video_first_frame_png,
};
#[cfg(feature = "ffmpeg")]
use std::ffi::{CStr, c_char};
#[cfg(feature = "ffmpeg")]
use std::path::Path;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SilkCodecBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

impl SilkCodecBuffer {
    const fn empty() -> Self {
        Self {
            ptr: ptr::null_mut(),
            len: 0,
        }
    }
}

#[cfg(feature = "ffmpeg")]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SilkCodecVideoMetadata {
    pub width: u32,
    pub height: u32,
    pub duration_millis: u64,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SilkCodecStatus {
    Success = 0,
    NullPointer = 1,
    InvalidParameter = 2,
    InvalidUtf8Path = 3,
    IoError = 4,
    FfmpegError = 5,
    NoAudioStream = 6,
    MissingFilter = 7,
    MissingFilterContext = 8,
    InvalidFilteredFrame = 9,
    NoVideoStream = 10,
    MissingDuration = 11,
    MissingDimensions = 12,
    FirstFrameNotFound = 13,
    PngEncoderNotFound = 14,
    MissingPngPixelFormat = 15,
    EmptyEncodedPacket = 16,
    SilkInvalid = 100,
    SilkEncInputInvalidNoOfSamples = 101,
    SilkEncFsNotSupported = 102,
    SilkEncPacketSizeNotSupported = 103,
    SilkEncPayloadBufTooShort = 104,
    SilkEncInvalidLossRate = 105,
    SilkEncInvalidComplexitySetting = 106,
    SilkEncInvalidInbandFecSetting = 107,
    SilkEncInvalidDtxSetting = 108,
    SilkEncInternalError = 109,
    SilkDecInvalidSamplingFrequency = 110,
    SilkDecPayloadTooLarge = 111,
    SilkDecPayloadError = 112,
    SilkOther = 113,
    Panic = 1000,
}

impl From<SilkError> for SilkCodecStatus {
    fn from(value: SilkError) -> Self {
        match value {
            SilkError::Invalid => Self::SilkInvalid,
            SilkError::EncInputInvalidNoOfSamples => Self::SilkEncInputInvalidNoOfSamples,
            SilkError::EncFsNotSupported => Self::SilkEncFsNotSupported,
            SilkError::EncPacketSizeNotSupported => Self::SilkEncPacketSizeNotSupported,
            SilkError::EncPayloadBufTooShort => Self::SilkEncPayloadBufTooShort,
            SilkError::EncInvalidLossRate => Self::SilkEncInvalidLossRate,
            SilkError::EncInvalidComplexitySetting => Self::SilkEncInvalidComplexitySetting,
            SilkError::EncInvalidInbandFecSetting => Self::SilkEncInvalidInbandFecSetting,
            SilkError::EncInvalidDtxSetting => Self::SilkEncInvalidDtxSetting,
            SilkError::EncInternalError => Self::SilkEncInternalError,
            SilkError::DecInvalidSamplingFrequency => Self::SilkDecInvalidSamplingFrequency,
            SilkError::DecPayloadTooLarge => Self::SilkDecPayloadTooLarge,
            SilkError::DecPayloadError => Self::SilkDecPayloadError,
            SilkError::Other(_) => Self::SilkOther,
        }
    }
}

#[cfg(feature = "ffmpeg")]
impl From<PcmError> for SilkCodecStatus {
    fn from(value: PcmError) -> Self {
        match value {
            PcmError::Io(_) => Self::IoError,
            PcmError::Ffmpeg(_) => Self::FfmpegError,
            PcmError::NoAudioStream => Self::NoAudioStream,
            PcmError::MissingFilter(_) => Self::MissingFilter,
            PcmError::MissingFilterContext(_) => Self::MissingFilterContext,
            PcmError::InvalidFilteredFrame { .. } => Self::InvalidFilteredFrame,
        }
    }
}

#[cfg(feature = "ffmpeg")]
impl From<VideoError> for SilkCodecStatus {
    fn from(value: VideoError) -> Self {
        match value {
            VideoError::Io(_) => Self::IoError,
            VideoError::Ffmpeg(_) => Self::FfmpegError,
            VideoError::NoVideoStream => Self::NoVideoStream,
            VideoError::MissingDuration => Self::MissingDuration,
            VideoError::MissingDimensions => Self::MissingDimensions,
            VideoError::FirstFrameNotFound => Self::FirstFrameNotFound,
            VideoError::PngEncoderNotFound => Self::PngEncoderNotFound,
            VideoError::MissingPngPixelFormat => Self::MissingPngPixelFormat,
            VideoError::EmptyEncodedPacket => Self::EmptyEncodedPacket,
        }
    }
}

fn with_ffi_boundary<F>(f: F) -> SilkCodecStatus
where
    F: FnOnce() -> SilkCodecStatus,
{
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(SilkCodecStatus::Panic)
}

unsafe fn read_input_bytes<'a>(
    input_ptr: *const u8,
    input_len: usize,
) -> Result<&'a [u8], SilkCodecStatus> {
    if input_len == 0 {
        return Ok(&[]);
    }
    if input_ptr.is_null() {
        return Err(SilkCodecStatus::NullPointer);
    }

    Ok(unsafe { slice::from_raw_parts(input_ptr, input_len) })
}

unsafe fn prepare_output<'a>(
    output: *mut SilkCodecBuffer,
) -> Result<&'a mut SilkCodecBuffer, SilkCodecStatus> {
    let output = unsafe { output.as_mut() }.ok_or(SilkCodecStatus::NullPointer)?;
    *output = SilkCodecBuffer::empty();
    Ok(output)
}

fn store_output(output: &mut SilkCodecBuffer, mut data: Vec<u8>) {
    if data.is_empty() {
        *output = SilkCodecBuffer::empty();
        return;
    }

    data.shrink_to_fit();
    debug_assert_eq!(data.len(), data.capacity());

    *output = SilkCodecBuffer {
        ptr: data.as_mut_ptr(),
        len: data.len(),
    };
    std::mem::forget(data);
}

#[cfg(feature = "ffmpeg")]
unsafe fn read_c_path<'a>(path: *const c_char) -> Result<&'a Path, SilkCodecStatus> {
    if path.is_null() {
        return Err(SilkCodecStatus::NullPointer);
    }

    let path = unsafe { CStr::from_ptr(path) }
        .to_str()
        .map_err(|_| SilkCodecStatus::InvalidUtf8Path)?;
    Ok(Path::new(path))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn silk_codec_encode(
    input_ptr: *const u8,
    input_len: usize,
    sample_rate: i32,
    bit_rate: i32,
    tencent: bool,
    output: *mut SilkCodecBuffer,
) -> SilkCodecStatus {
    with_ffi_boundary(|| {
        let output = match unsafe { prepare_output(output) } {
            Ok(output) => output,
            Err(err) => return err,
        };
        let input = match unsafe { read_input_bytes(input_ptr, input_len) } {
            Ok(input) => input,
            Err(err) => return err,
        };

        match encode_silk(input, sample_rate, bit_rate, tencent) {
            Ok(data) => {
                store_output(output, data);
                SilkCodecStatus::Success
            }
            Err(err) => err.into(),
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn silk_codec_decode(
    input_ptr: *const u8,
    input_len: usize,
    sample_rate: i32,
    output: *mut SilkCodecBuffer,
) -> SilkCodecStatus {
    with_ffi_boundary(|| {
        let output = match unsafe { prepare_output(output) } {
            Ok(output) => output,
            Err(err) => return err,
        };
        let input = match unsafe { read_input_bytes(input_ptr, input_len) } {
            Ok(input) => input,
            Err(err) => return err,
        };

        match decode_silk(input, sample_rate) {
            Ok(data) => {
                store_output(output, data);
                SilkCodecStatus::Success
            }
            Err(err) => err.into(),
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn silk_codec_free_buffer(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }

    unsafe {
        drop(Vec::from_raw_parts(ptr, len, len));
    }
}

#[cfg(feature = "ffmpeg")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn silk_codec_convert_audio_to_pcm_file(
    input_path: *const c_char,
    output_path: *const c_char,
) -> SilkCodecStatus {
    with_ffi_boundary(|| {
        let input_path = match unsafe { read_c_path(input_path) } {
            Ok(path) => path,
            Err(err) => return err,
        };
        let output_path = match unsafe { read_c_path(output_path) } {
            Ok(path) => path,
            Err(err) => return err,
        };

        match convert_audio_to_pcm(input_path, output_path) {
            Ok(()) => SilkCodecStatus::Success,
            Err(err) => err.into(),
        }
    })
}

#[cfg(feature = "ffmpeg")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn silk_codec_get_video_metadata(
    input_path: *const c_char,
    metadata: *mut SilkCodecVideoMetadata,
) -> SilkCodecStatus {
    with_ffi_boundary(|| {
        let input_path = match unsafe { read_c_path(input_path) } {
            Ok(path) => path,
            Err(err) => return err,
        };
        let metadata = match unsafe { metadata.as_mut() } {
            Some(metadata) => metadata,
            None => return SilkCodecStatus::NullPointer,
        };

        match get_video_metadata(input_path) {
            Ok(video) => {
                *metadata = SilkCodecVideoMetadata {
                    width: video.width,
                    height: video.height,
                    duration_millis: video.duration.as_millis().min(u128::from(u64::MAX)) as u64,
                };
                SilkCodecStatus::Success
            }
            Err(err) => err.into(),
        }
    })
}

#[cfg(feature = "ffmpeg")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn silk_codec_save_video_first_frame_png(
    input_path: *const c_char,
    output_path: *const c_char,
) -> SilkCodecStatus {
    with_ffi_boundary(|| {
        let input_path = match unsafe { read_c_path(input_path) } {
            Ok(path) => path,
            Err(err) => return err,
        };
        let output_path = match unsafe { read_c_path(output_path) } {
            Ok(path) => path,
            Err(err) => return err,
        };

        match save_video_first_frame_png(input_path, output_path) {
            Ok(()) => SilkCodecStatus::Success,
            Err(err) => err.into(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{SilkCodecBuffer, SilkCodecStatus};
    use std::ptr;
    #[cfg(feature = "ffmpeg")]
    use std::path::Path;

    fn sample_pcm_frame() -> Vec<u8> {
        vec![0u8; 24_000 / 1_000 * 40]
    }

    #[test]
    fn ffi_encode_decode_round_trip() {
        let input = sample_pcm_frame();
        let mut encoded = SilkCodecBuffer::empty();

        let encode_status = unsafe {
            super::silk_codec_encode(
                input.as_ptr(),
                input.len(),
                24_000,
                24_000,
                false,
                &mut encoded,
            )
        };
        assert_eq!(encode_status, SilkCodecStatus::Success);
        assert!(!encoded.ptr.is_null());
        assert!(encoded.len > 0);

        let mut decoded = SilkCodecBuffer::empty();
        let decode_status = unsafe {
            super::silk_codec_decode(encoded.ptr, encoded.len, 24_000, &mut decoded)
        };
        assert_eq!(decode_status, SilkCodecStatus::Success);
        assert_eq!(decoded.len, input.len());

        unsafe {
            super::silk_codec_free_buffer(encoded.ptr, encoded.len);
            super::silk_codec_free_buffer(decoded.ptr, decoded.len);
        }
    }

    #[test]
    fn ffi_encode_rejects_null_input_pointer_when_length_is_non_zero() {
        let mut output = SilkCodecBuffer::empty();
        let status =
            unsafe { super::silk_codec_encode(ptr::null(), 8, 24_000, 24_000, false, &mut output) };
        assert_eq!(status, SilkCodecStatus::NullPointer);
        assert!(output.ptr.is_null());
        assert_eq!(output.len, 0);
    }

    #[test]
    fn ffi_decode_rejects_invalid_payload() {
        let invalid = [1u8, 2, 3, 4];
        let mut output = SilkCodecBuffer::empty();
        let status =
            unsafe { super::silk_codec_decode(invalid.as_ptr(), invalid.len(), 24_000, &mut output) };
        assert_eq!(status, SilkCodecStatus::SilkInvalid);
        assert!(output.ptr.is_null());
        assert_eq!(output.len, 0);
    }

    #[cfg(feature = "ffmpeg")]
    #[test]
    fn ffi_convert_audio_to_pcm_file_from_test_wav() {
        let output_path = std::env::temp_dir().join(format!(
            "silk-codec-ffi-pcm-{}-{}.pcm",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let input_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("test.wav");
        let input_path = std::ffi::CString::new(input_path.to_string_lossy().into_owned()).unwrap();
        let output_path =
            std::ffi::CString::new(output_path.to_string_lossy().into_owned()).unwrap();

        let status = unsafe {
            super::silk_codec_convert_audio_to_pcm_file(input_path.as_ptr(), output_path.as_ptr())
        };
        assert_eq!(status, SilkCodecStatus::Success);
        assert!(std::fs::metadata(output_path.to_str().unwrap()).unwrap().len() > 0);
        std::fs::remove_file(output_path.to_str().unwrap()).unwrap();
    }
}
