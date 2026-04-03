use ffmpeg_next::log::Level as FfmpegLogLevel;
use silk_codec::{get_video_metadata, install_ffmpeg_tracing, save_video_first_frame_png};
use std::path::Path;
use tracing::info;
use tracing_subscriber::FmtSubscriber;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    install_ffmpeg_tracing(FfmpegLogLevel::Warning)?;

    let input_file = "test.mp4";
    let output_file = "first-frame.png";

    if !Path::new(input_file).exists() {
        eprintln!("input file not found: {input_file}");
        return Ok(());
    }

    let metadata = get_video_metadata(input_file)?;
    info!("video: {input_file}");
    info!("width: {}", metadata.width);
    info!("height: {}", metadata.height);
    info!("duration: {:.3}s", metadata.duration.as_secs_f64());

    save_video_first_frame_png(input_file, output_file)?;
    info!("first frame saved to: {output_file}");

    Ok(())
}
