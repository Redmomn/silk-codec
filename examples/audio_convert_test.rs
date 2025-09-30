use silk_codec::AudioConverter;
use std::path::Path;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let converter = AudioConverter::new()
        .with_sample_rate(24000)
        .with_channels(1);

    let input_file = "test.wav";
    if Path::new(input_file).exists() {
        let output_file = "output_test.pcm";
        let start = Instant::now();
        match converter.convert_to_pcm(input_file, output_file) {
            Ok(_) => println!("sucess: {input_file} -> {output_file}"),
            Err(e) => println!("error: {e}"),
        }
        println!("time: {:.2?}", start.elapsed());
    }
    Ok(())
}
