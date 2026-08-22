mod silk;
pub use silk::{
    DecoderBuilder, EncoderBuilder, SilkBuilder, SilkCoder, SilkDecoder, SilkEncoder, SilkError,
    TailPolicy, decode_silk, encode_silk,
};

pub use silk::{DecoderBuilder as SilkDecoderBuilder, EncoderBuilder as SilkEncoderBuilder};

#[cfg(feature = "ffmpeg")]
mod ffmpeg_utils;
#[cfg(feature = "ffmpeg")]
mod pcm;
#[cfg(feature = "ffmpeg")]
mod video;

#[cfg(feature = "ffmpeg-tracing")]
pub use ffmpeg_utils::install_ffmpeg_tracing;
#[cfg(feature = "ffmpeg")]
pub use pcm::{AudioConverter, PcmError, convert_audio_to_pcm};
#[cfg(feature = "ffmpeg")]
pub use video::{VideoError, VideoMetadata, get_video_metadata, save_video_first_frame_png};
