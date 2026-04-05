use ffmpeg_next as ffmpeg;
#[cfg(feature = "ffmpeg-tracing")]
use std::cell::Cell;
#[cfg(feature = "ffmpeg-tracing")]
use std::ffi::CStr;
#[cfg(feature = "ffmpeg-tracing")]
use std::os::raw::{c_char, c_int, c_void};
use std::sync::OnceLock;

#[cfg(all(feature = "ffmpeg-tracing", target_os = "linux"))]
type FfmpegVaList = *mut ffmpeg::ffi::__va_list_tag;
#[cfg(all(feature = "ffmpeg-tracing", not(target_os = "linux")))]
type FfmpegVaList = ffmpeg::ffi::va_list;

pub(crate) fn ensure_ffmpeg_initialized() -> Result<(), ffmpeg::Error> {
    static FFMPEG_INIT: OnceLock<Result<(), ffmpeg::Error>> = OnceLock::new();

    match FFMPEG_INIT.get_or_init(|| {
        ffmpeg::init()?;
        ffmpeg::log::set_level(ffmpeg::log::Level::Error);
        Ok(())
    }) {
        Ok(()) => Ok(()),
        Err(err) => Err(*err),
    }
}

#[cfg(feature = "ffmpeg-tracing")]
pub fn install_ffmpeg_tracing(level: ffmpeg::log::Level) -> Result<(), ffmpeg::Error> {
    static FFMPEG_TRACING_INIT: OnceLock<()> = OnceLock::new();

    ensure_ffmpeg_initialized()?;
    FFMPEG_TRACING_INIT.get_or_init(|| unsafe {
        ffmpeg::ffi::av_log_set_callback(Some(ffmpeg_tracing_callback));
    });
    ffmpeg::log::set_level(level);
    Ok(())
}

#[cfg(feature = "ffmpeg-tracing")]
thread_local! {
    static FFMPEG_LOG_PREFIX_STATE: Cell<c_int> = const { Cell::new(1) };
}

#[cfg(feature = "ffmpeg-tracing")]
unsafe extern "C" fn ffmpeg_tracing_callback(
    avcl: *mut c_void,
    level: c_int,
    fmt: *const c_char,
    vl: FfmpegVaList,
) {
    if fmt.is_null() {
        return;
    }

    let configured_level = match ffmpeg::log::get_level() {
        Ok(level) => c_int::from(level),
        Err(_) => c_int::from(ffmpeg::log::Level::Error),
    };
    if level > configured_level {
        return;
    }

    let mut line = [0 as c_char; 4096];
    let mut formatted = String::new();

    FFMPEG_LOG_PREFIX_STATE.with(|print_prefix| {
        let mut prefix = print_prefix.get();
        unsafe {
            ffmpeg::ffi::av_log_format_line2(
                avcl,
                level,
                fmt,
                vl,
                line.as_mut_ptr(),
                line.len() as c_int,
                &mut prefix,
            );
        }
        print_prefix.set(prefix);
        formatted = unsafe { CStr::from_ptr(line.as_ptr()) }
            .to_string_lossy()
            .trim()
            .to_owned();
    });

    if formatted.is_empty() {
        return;
    }

    match ffmpeg::log::Level::try_from(level).unwrap_or(ffmpeg::log::Level::Trace) {
        ffmpeg::log::Level::Quiet => {}
        ffmpeg::log::Level::Panic | ffmpeg::log::Level::Fatal | ffmpeg::log::Level::Error => {
            tracing::error!(target: "ffmpeg", "{}", formatted);
        }
        ffmpeg::log::Level::Warning => {
            tracing::warn!(target: "ffmpeg", "{}", formatted);
        }
        ffmpeg::log::Level::Info => {
            tracing::info!(target: "ffmpeg", "{}", formatted);
        }
        ffmpeg::log::Level::Verbose | ffmpeg::log::Level::Debug => {
            tracing::debug!(target: "ffmpeg", "{}", formatted);
        }
        ffmpeg::log::Level::Trace => {
            tracing::trace!(target: "ffmpeg", "{}", formatted);
        }
    }
}
