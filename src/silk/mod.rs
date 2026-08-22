mod builder;
mod common;
mod decoder;
mod encoder;
mod error;
mod sdk;
mod state;

pub use builder::{DecoderBuilder, EncoderBuilder, SilkBuilder, SilkCoder};
pub use common::TailPolicy;
pub use decoder::SilkDecoder;
pub use encoder::SilkEncoder;
pub use error::SilkError;

pub fn decode_silk<R: AsRef<[u8]>>(src: R, sample_rate: i32) -> Result<Vec<u8>, SilkError> {
    let mut decoder = SilkCoder::builder()
        .decoder()
        .output_sample_rate(sample_rate)
        .build()?;
    decoder.decode_bytes(src.as_ref())
}

pub fn encode_silk<R: AsRef<[u8]>>(
    src: R,
    sample_rate: i32,
    bit_rate: i32,
    tencent: bool,
) -> Result<Vec<u8>, SilkError> {
    let mut encoder = SilkCoder::builder()
        .encoder()
        .sample_rate(sample_rate)
        .bit_rate(bit_rate)
        .build()?;
    encoder.encode_bytes(src.as_ref(), tencent)
}
