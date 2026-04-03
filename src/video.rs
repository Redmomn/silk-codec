use crate::ffmpeg_utils::ensure_ffmpeg_initialized;
use ffmpeg_next as ffmpeg;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

const FALLBACK_FORMAT_TIME_BASE_MICROS: i64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoMetadata {
    pub width: u32,
    pub height: u32,
    pub duration: Duration,
}

#[derive(Error, Debug)]
pub enum VideoError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg error: {0}")]
    Ffmpeg(#[from] ffmpeg::Error),
    #[error("video stream not found")]
    NoVideoStream,
    #[error("video duration is unavailable")]
    MissingDuration,
    #[error("video width or height is unavailable")]
    MissingDimensions,
    #[error("failed to decode the first video frame")]
    FirstFrameNotFound,
    #[error("png encoder is not available in ffmpeg")]
    PngEncoderNotFound,
    #[error("png encoder does not expose any supported pixel format")]
    MissingPngPixelFormat,
    #[error("png encoder produced an empty packet")]
    EmptyEncodedPacket,
}

pub fn get_video_metadata<P>(input_path: P) -> Result<VideoMetadata, VideoError>
where
    P: AsRef<Path>,
{
    ensure_ffmpeg_initialized()?;

    let input_path = input_path.as_ref();
    let format_context = ffmpeg::format::input(input_path)?;
    let input_stream = format_context
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(VideoError::NoVideoStream)?;

    let codec_context =
        ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())?;
    let decoder = codec_context.decoder().video()?;

    let width = decoder.width();
    let height = decoder.height();
    if width == 0 || height == 0 {
        return Err(VideoError::MissingDimensions);
    }

    let duration = stream_duration_to_duration(&input_stream)
        .or_else(|| format_duration_to_duration(format_context.duration()))
        .ok_or(VideoError::MissingDuration)?;

    Ok(VideoMetadata {
        width,
        height,
        duration,
    })
}

pub fn save_video_first_frame_png<P, Q>(input_path: P, output_path: Q) -> Result<(), VideoError>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    ensure_ffmpeg_initialized()?;

    let input_path = input_path.as_ref();
    let output_path = output_path.as_ref();
    let mut format_context = ffmpeg::format::input(input_path)?;
    let input_stream = format_context
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(VideoError::NoVideoStream)?;
    let video_stream_index = input_stream.index();

    let codec_context =
        ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())?;
    let mut decoder = codec_context.decoder().video()?;
    let decoded = decode_first_video_frame(&mut format_context, &mut decoder, video_stream_index)?;
    let rgb_frame = scale_frame_to_rgb24(&decoded)?;
    write_frame_as_png(&rgb_frame, output_path)?;

    Ok(())
}

fn stream_duration_to_duration(stream: &ffmpeg::Stream<'_>) -> Option<Duration> {
    let raw_duration = stream.duration();
    if raw_duration <= 0 {
        return None;
    }

    rational_units_to_duration(raw_duration, stream.time_base())
}

fn format_duration_to_duration(raw_duration: i64) -> Option<Duration> {
    if raw_duration <= 0 {
        return None;
    }

    let seconds = raw_duration as f64 / FALLBACK_FORMAT_TIME_BASE_MICROS as f64;
    duration_from_seconds(seconds)
}

fn rational_units_to_duration(value: i64, time_base: ffmpeg::Rational) -> Option<Duration> {
    let denominator = time_base.denominator();
    if denominator <= 0 {
        return None;
    }

    let seconds = value as f64 * f64::from(time_base);
    duration_from_seconds(seconds)
}

fn duration_from_seconds(seconds: f64) -> Option<Duration> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }

    Some(Duration::from_secs_f64(seconds))
}

fn decode_first_video_frame(
    format_context: &mut ffmpeg::format::context::Input,
    decoder: &mut ffmpeg::codec::decoder::Video,
    stream_index: usize,
) -> Result<ffmpeg::util::frame::Video, VideoError> {
    for (stream, packet) in format_context.packets() {
        if stream.index() != stream_index {
            continue;
        }

        decoder.send_packet(&packet)?;
        if let Some(frame) = receive_first_decoded_frame(decoder)? {
            return Ok(frame);
        }
    }

    decoder.send_eof()?;
    receive_first_decoded_frame(decoder)?.ok_or(VideoError::FirstFrameNotFound)
}

fn receive_first_decoded_frame(
    decoder: &mut ffmpeg::codec::decoder::Video,
) -> Result<Option<ffmpeg::util::frame::Video>, VideoError> {
    let mut decoded = ffmpeg::util::frame::Video::empty();

    match decoder.receive_frame(&mut decoded) {
        Ok(()) => Ok(Some(decoded)),
        Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::util::error::EAGAIN => Ok(None),
        Err(ffmpeg::Error::Eof) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn scale_frame_to_rgb24(
    frame: &ffmpeg::util::frame::Video,
) -> Result<ffmpeg::util::frame::Video, VideoError> {
    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        frame.format(),
        frame.width(),
        frame.height(),
        ffmpeg::format::Pixel::RGB24,
        frame.width(),
        frame.height(),
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )?;
    let mut rgb_frame = ffmpeg::util::frame::Video::empty();
    scaler.run(frame, &mut rgb_frame)?;
    rgb_frame.set_pts(frame.pts());
    Ok(rgb_frame)
}

fn write_frame_as_png(
    frame: &ffmpeg::util::frame::Video,
    output_path: &Path,
) -> Result<(), VideoError> {
    let codec =
        ffmpeg::encoder::find(ffmpeg::codec::Id::PNG).ok_or(VideoError::PngEncoderNotFound)?;
    let pixel_format = codec
        .video()?
        .formats()
        .and_then(|mut formats| formats.find(|format| *format == ffmpeg::format::Pixel::RGB24))
        .ok_or(VideoError::MissingPngPixelFormat)?;

    let mut encoder = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()?;
    encoder.set_width(frame.width());
    encoder.set_height(frame.height());
    encoder.set_format(pixel_format);
    encoder.set_time_base((1, 1));

    let mut encoder = encoder.open_as(codec)?;
    encoder.send_frame(frame)?;
    encoder.send_eof()?;

    let mut encoded = ffmpeg::Packet::empty();
    match encoder.receive_packet(&mut encoded) {
        Ok(()) => {
            let data = encoded.data().ok_or(VideoError::EmptyEncodedPacket)?;
            let file = File::create(output_path)?;
            let mut writer = BufWriter::new(file);
            writer.write_all(data)?;
            writer.flush()?;
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}
