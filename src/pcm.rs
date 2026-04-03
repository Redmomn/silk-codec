use crate::ffmpeg_utils::ensure_ffmpeg_initialized;
use ffmpeg_next as ffmpeg;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use thiserror::Error;

const FILTER_INPUT_NAME: &str = "in";
const FILTER_OUTPUT_NAME: &str = "out";
const TARGET_CHANNEL_LAYOUT: ffmpeg::ChannelLayout = ffmpeg::ChannelLayout::MONO;
const PCM_BYTES_PER_SAMPLE: usize = 2;
const FILTER_SPEC: &str = "aformat=sample_fmts=s16:sample_rates=24000:channel_layouts=mono";

#[derive(Error, Debug)]
pub enum PcmError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg error: {0}")]
    Ffmpeg(#[from] ffmpeg::Error),
    #[error("audio stream not found")]
    NoAudioStream,
    #[error("required ffmpeg filter `{0}` not found")]
    MissingFilter(&'static str),
    #[error("filter context `{0}` not found")]
    MissingFilterContext(&'static str),
    #[error("invalid filtered pcm frame: expected at least {expected} bytes, got {actual} bytes")]
    InvalidFilteredFrame { expected: usize, actual: usize },
}

struct AudioInput {
    format_context: ffmpeg::format::context::Input,
    stream_index: usize,
    decoder: ffmpeg::codec::decoder::Audio,
}

#[derive(Debug, Clone, Copy)]
pub struct AudioConverter;

impl AudioConverter {
    pub fn new() -> Result<Self, PcmError> {
        ensure_ffmpeg_initialized()?;
        Ok(Self)
    }

    pub fn convert_to_pcm<P, Q>(&self, input_path: P, output_path: Q) -> Result<(), PcmError>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let mut input = open_audio_input(input_path.as_ref())?;
        let mut filter_graph = build_pcm_filter_graph(&input.decoder)?;
        let output_file = File::create(output_path)?;
        let mut output = BufWriter::new(output_file);

        for (stream, packet) in input.format_context.packets() {
            if stream.index() != input.stream_index {
                continue;
            }

            input.decoder.send_packet(&packet)?;
            receive_decoded_frames(
                &mut input.decoder,
                &mut filter_graph,
                &mut output,
                TARGET_CHANNEL_LAYOUT,
            )?;
        }

        input.decoder.send_eof()?;
        receive_decoded_frames(
            &mut input.decoder,
            &mut filter_graph,
            &mut output,
            TARGET_CHANNEL_LAYOUT,
        )?;

        flush_filter_graph(&mut filter_graph, &mut output, TARGET_CHANNEL_LAYOUT)?;
        output.flush()?;
        Ok(())
    }
}

pub fn convert_audio_to_pcm<P, Q>(input_path: P, output_path: Q) -> Result<(), PcmError>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    AudioConverter::new()?.convert_to_pcm(input_path, output_path)
}

fn open_audio_input(path: &Path) -> Result<AudioInput, PcmError> {
    let format_context = ffmpeg::format::input(path)?;
    let (stream_index, parameters) = {
        let input_stream = format_context
            .streams()
            .best(ffmpeg::media::Type::Audio)
            .ok_or(PcmError::NoAudioStream)?;
        (input_stream.index(), input_stream.parameters().clone())
    };

    let codec_context = ffmpeg::codec::context::Context::from_parameters(parameters)?;
    let decoder = codec_context.decoder().audio()?;

    Ok(AudioInput {
        format_context,
        stream_index,
        decoder,
    })
}

fn build_pcm_filter_graph(
    decoder: &ffmpeg::codec::decoder::Audio,
) -> Result<ffmpeg::filter::Graph, PcmError> {
    let mut filter_graph = ffmpeg::filter::Graph::new();
    let input_channel_layout = decoder_input_channel_layout(decoder);
    let input_args = format!(
        "time_base={}:sample_rate={}:sample_fmt={}:channel_layout=0x{:x}",
        decoder.time_base(),
        decoder.rate(),
        decoder.format().name(),
        input_channel_layout.bits()
    );

    let abuffer = ffmpeg::filter::find("abuffer").ok_or(PcmError::MissingFilter("abuffer"))?;
    let abuffersink =
        ffmpeg::filter::find("abuffersink").ok_or(PcmError::MissingFilter("abuffersink"))?;

    filter_graph.add(&abuffer, FILTER_INPUT_NAME, &input_args)?;
    filter_graph.add(&abuffersink, FILTER_OUTPUT_NAME, "")?;
    filter_graph
        .output(FILTER_INPUT_NAME, 0)?
        .input(FILTER_OUTPUT_NAME, 0)?
        .parse(FILTER_SPEC)?;
    filter_graph.validate()?;

    Ok(filter_graph)
}

fn decoder_input_channel_layout(decoder: &ffmpeg::codec::decoder::Audio) -> ffmpeg::ChannelLayout {
    let layout = decoder.channel_layout();
    if !layout.is_empty() {
        return layout;
    }

    match decoder.channels() {
        0 | 1 => ffmpeg::ChannelLayout::MONO,
        _ => ffmpeg::ChannelLayout::STEREO,
    }
}

fn receive_decoded_frames<W: Write>(
    decoder: &mut ffmpeg::codec::decoder::Audio,
    filter_graph: &mut ffmpeg::filter::Graph,
    output: &mut W,
    target_channel_layout: ffmpeg::ChannelLayout,
) -> Result<(), PcmError> {
    let mut decoded = ffmpeg::util::frame::Audio::empty();
    loop {
        match decoder.receive_frame(&mut decoded) {
            Ok(()) => {
                let timestamp = decoded.timestamp();
                decoded.set_pts(timestamp);
                add_frame_to_filter(filter_graph, &decoded)?;
                drain_filtered_frames(filter_graph, output, target_channel_layout)?;
            }
            Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::util::error::EAGAIN => break,
            Err(ffmpeg::Error::Eof) => break,
            Err(err) => return Err(err.into()),
        }
    }

    Ok(())
}

fn add_frame_to_filter(
    filter_graph: &mut ffmpeg::filter::Graph,
    frame: &ffmpeg::util::frame::Audio,
) -> Result<(), PcmError> {
    let mut input = filter_graph
        .get(FILTER_INPUT_NAME)
        .ok_or(PcmError::MissingFilterContext(FILTER_INPUT_NAME))?;
    input.source().add(frame)?;
    Ok(())
}

fn flush_filter_graph<W: Write>(
    filter_graph: &mut ffmpeg::filter::Graph,
    output: &mut W,
    target_channel_layout: ffmpeg::ChannelLayout,
) -> Result<(), PcmError> {
    let mut input = filter_graph
        .get(FILTER_INPUT_NAME)
        .ok_or(PcmError::MissingFilterContext(FILTER_INPUT_NAME))?;
    input.source().flush()?;
    drain_filtered_frames(filter_graph, output, target_channel_layout)
}

fn drain_filtered_frames<W: Write>(
    filter_graph: &mut ffmpeg::filter::Graph,
    output: &mut W,
    target_channel_layout: ffmpeg::ChannelLayout,
) -> Result<(), PcmError> {
    let mut filtered = ffmpeg::util::frame::Audio::empty();
    loop {
        let mut sink = filter_graph
            .get(FILTER_OUTPUT_NAME)
            .ok_or(PcmError::MissingFilterContext(FILTER_OUTPUT_NAME))?;

        match sink.sink().frame(&mut filtered) {
            Ok(()) => write_pcm_frame(output, &filtered, target_channel_layout)?,
            Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::util::error::EAGAIN => break,
            Err(ffmpeg::Error::Eof) => break,
            Err(err) => return Err(err.into()),
        }
    }

    Ok(())
}

fn write_pcm_frame<W: Write>(
    output: &mut W,
    frame: &ffmpeg::util::frame::Audio,
    target_channel_layout: ffmpeg::ChannelLayout,
) -> Result<(), PcmError> {
    let expected_bytes =
        frame.samples() * target_channel_layout.channels() as usize * PCM_BYTES_PER_SAMPLE;
    let pcm_bytes = frame.data(0);

    if pcm_bytes.len() < expected_bytes {
        return Err(PcmError::InvalidFilteredFrame {
            expected: expected_bytes,
            actual: pcm_bytes.len(),
        });
    }

    output.write_all(&pcm_bytes[..expected_bytes])?;
    Ok(())
}
