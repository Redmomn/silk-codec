use super::common::{TailPolicy, frame_samples, validate_encoder_config};
use super::decoder::SilkDecoder;
use super::encoder::SilkEncoder;
use super::error::SilkError;
use super::state::DecoderState;

pub struct SilkCoder;

impl SilkCoder {
    pub fn builder() -> SilkBuilder {
        SilkBuilder
    }
}

pub struct SilkBuilder;

impl SilkBuilder {
    pub fn encoder(self) -> EncoderBuilder {
        EncoderBuilder::default()
    }
    pub fn decoder(self) -> DecoderBuilder {
        DecoderBuilder::default()
    }
}

#[derive(Clone, Debug)]
pub struct EncoderBuilder {
    pub(crate) sample_rate: i32,
    pub(crate) bit_rate: i32,
    pub(crate) tail_policy: TailPolicy,
    pub(crate) complexity: i32,
    pub(crate) use_in_band_fec: bool,
    pub(crate) use_dtx: bool,
}

impl Default for EncoderBuilder {
    fn default() -> Self {
        Self {
            sample_rate: 24_000,
            bit_rate: 24_000,
            tail_policy: TailPolicy::Drop,
            complexity: 2,
            use_in_band_fec: false,
            use_dtx: false,
        }
    }
}

impl EncoderBuilder {
    pub fn sample_rate(mut self, v: i32) -> Self {
        self.sample_rate = v;
        self
    }
    pub fn bit_rate(mut self, v: i32) -> Self {
        self.bit_rate = v;
        self
    }
    pub fn tail_policy(mut self, v: TailPolicy) -> Self {
        self.tail_policy = v;
        self
    }
    pub fn complexity(mut self, v: i32) -> Self {
        self.complexity = v;
        self
    }
    pub fn use_in_band_fec(mut self, v: bool) -> Self {
        self.use_in_band_fec = v;
        self
    }
    pub fn in_band_fec(self, v: bool) -> Self {
        self.use_in_band_fec(v)
    }
    pub fn fec(self, v: bool) -> Self {
        self.use_in_band_fec(v)
    }
    pub fn use_dtx(mut self, v: bool) -> Self {
        self.use_dtx = v;
        self
    }
    pub fn dtx(self, v: bool) -> Self {
        self.use_dtx(v)
    }
    pub fn enable_dtx(self, v: bool) -> Self {
        self.use_dtx(v)
    }
    pub fn build(self) -> Result<SilkEncoder, SilkError> {
        validate_encoder_config(self.sample_rate, self.bit_rate, self.complexity)?;
        let frame_count = frame_samples(self.sample_rate)?;
        SilkEncoder::new(self, frame_count)
    }
}

#[derive(Clone, Debug)]
pub struct DecoderBuilder {
    pub(crate) output_sample_rate: i32,
}

impl Default for DecoderBuilder {
    fn default() -> Self {
        Self {
            output_sample_rate: 24_000,
        }
    }
}

impl DecoderBuilder {
    pub fn output_sample_rate(mut self, v: i32) -> Self {
        self.output_sample_rate = v;
        self
    }
    pub fn sample_rate(self, v: i32) -> Self {
        self.output_sample_rate(v)
    }
    pub fn build(self) -> Result<SilkDecoder, SilkError> {
        if !(8_000..=48_000).contains(&self.output_sample_rate) {
            return Err(SilkError::InvalidConfiguration);
        }
        Ok(SilkDecoder::new(self, DecoderState::new()?))
    }
}
