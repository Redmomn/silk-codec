use super::error::SilkError;

pub const SILK_HEADER: &[u8] = b"#!SILK_V3";
pub const MAX_PAYLOAD_BYTES: usize = 1024;
pub const MAX_INTERNAL_FRAMES: usize = 5;
pub const SUPPORTED_SAMPLE_RATES: &[i32] = &[8_000, 12_000, 16_000, 24_000, 32_000, 44_100, 48_000];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TailPolicy {
    #[default]
    Drop,
    Reject,
    PadWithSilence,
}

pub fn validate_encoder_config(
    sample_rate: i32,
    bit_rate: i32,
    complexity: i32,
) -> Result<(), SilkError> {
    if !SUPPORTED_SAMPLE_RATES.contains(&sample_rate)
        || !(5_000..=100_000).contains(&bit_rate)
        || !(0..=2).contains(&complexity)
    {
        return Err(SilkError::InvalidConfiguration);
    }
    Ok(())
}

pub fn frame_samples(sample_rate: i32) -> Result<usize, SilkError> {
    let rate = u64::try_from(sample_rate).map_err(|_| SilkError::InvalidConfiguration)?;
    usize::try_from(
        rate.checked_mul(20)
            .and_then(|v| v.checked_div(1000))
            .ok_or(SilkError::InvalidConfiguration)?,
    )
    .map_err(|_| SilkError::InvalidConfiguration)
}

pub fn sdk_check(code: i32) -> Result<(), SilkError> {
    if code == 0 {
        Ok(())
    } else {
        Err(SilkError::from(code))
    }
}
