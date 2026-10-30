use hbb_common::{log, ResultType};
use ndk::media::media_codec::{MediaCodec, MediaCodecDirection, MediaFormat};
use ndk::native_window::NativeWindow;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const H264_MIME_TYPE: &str = "video/avc";
const H265_MIME_TYPE: &str = "video/hevc";

static WINDOW: Mutex<Option<Arc<NativeWindow>>> = Mutex::new(None);
static DECODER: Mutex<Option<(String, SurfaceDecoder)>> = Mutex::new(None);

struct SurfaceDecoder {
    codec: MediaCodec,
    pts: u64,
}

// The codec handle is only touched from the video decode thread behind the
// decoder mutex.
unsafe impl Send for SurfaceDecoder {}

impl SurfaceDecoder {
    fn new(mime: &str, window: &NativeWindow) -> Option<Self> {
        let codec = MediaCodec::from_decoder_type(mime)?;
        let format = MediaFormat::new();
        format.set_str("mime", mime);
        if let Err(e) = codec.configure(&format, Some(window), MediaCodecDirection::Decoder) {
            log::error!("surface decoder configure failed: {:?}", e);
            return None;
        }
        if let Err(e) = codec.start() {
            log::error!("surface decoder start failed: {:?}", e);
            return None;
        }
        log::info!("surface decoder started: {}", mime);
        Some(Self { codec, pts: 0 })
    }

    fn decode(&mut self, data: &[u8]) -> ResultType<()> {
        if let Some(mut input) = self.codec.dequeue_input_buffer(Duration::from_millis(10))? {
            let len = data.len();
            let buf = input.buffer_mut();
            if len <= buf.len() {
                buf[..len].copy_from_slice(data);
                self.pts += 10_000;
                self.codec
                    .queue_input_buffer(input, 0, len, self.pts, 0)?;
            } else {
                log::warn!("surface frame {} bigger than input {}", len, buf.len());
            }
        }
        while let Some(out) = self.codec.dequeue_output_buffer(Duration::from_millis(1))? {
            self.codec.release_output_buffer(out, true)?;
        }
        Ok(())
    }
}

pub fn set_window_from_java(env: *mut jni::sys::JNIEnv, surface: jni::sys::jobject) {
    let window = if surface.is_null() {
        None
    } else {
        unsafe { NativeWindow::from_surface(env, surface) }
    };
    *WINDOW.lock().unwrap() = window.map(Arc::new);
    *DECODER.lock().unwrap() = None;
    log::info!("video surface {}", if surface.is_null() { "cleared" } else { "set" });
}

pub fn enabled() -> bool {
    WINDOW.lock().unwrap().is_some()
}

pub fn decode_h264(data: &[u8]) -> ResultType<bool> {
    decode(H264_MIME_TYPE, data)
}

pub fn decode_h265(data: &[u8]) -> ResultType<bool> {
    decode(H265_MIME_TYPE, data)
}

fn decode(mime: &str, data: &[u8]) -> ResultType<bool> {
    let window = match WINDOW.lock().unwrap().clone() {
        Some(w) => w,
        None => return Ok(false),
    };
    let mut lock = DECODER.lock().unwrap();
    if lock.as_ref().map(|(m, _)| m != mime).unwrap_or(true) {
        match SurfaceDecoder::new(mime, &window) {
            Some(decoder) => *lock = Some((mime.to_owned(), decoder)),
            None => return Ok(false),
        }
    }
    if let Some((_, decoder)) = lock.as_mut() {
        decoder.decode(data)?;
    }
    Ok(true)
}
