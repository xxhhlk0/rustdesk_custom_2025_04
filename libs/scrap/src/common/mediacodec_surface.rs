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
    fn new(mime: &str, window: &NativeWindow, csd: &[Vec<u8>]) -> Option<Self> {
        let codec = MediaCodec::from_decoder_type(mime)?;
        let format = MediaFormat::new();
        format.set_str("mime", mime);
        for (i, unit) in csd.iter().enumerate() {
            format.set_buffer(&format!("csd-{}", i), unit);
        }
        if let Err(e) = codec.configure(&format, Some(window), MediaCodecDirection::Decoder) {
            log::error!("surface decoder configure failed: {:?}", e);
            return None;
        }
        if let Err(e) = codec.start() {
            log::error!("surface decoder start failed: {:?}", e);
            return None;
        }
        log::info!("surface decoder started: {} csd={}", mime, csd.len());
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
    let is_h265 = mime == H265_MIME_TYPE;
    let mut lock = DECODER.lock().unwrap();
    if lock.as_ref().map(|(m, _)| m != mime).unwrap_or(true) {
        let csd = extract_csd(data, is_h265);
        if csd.is_empty() {
            log::debug!("surface decoder waits for SPS/PPS");
            return Ok(false);
        }
        match SurfaceDecoder::new(mime, &window, &csd) {
            Some(decoder) => *lock = Some((mime.to_owned(), decoder)),
            None => return Ok(false),
        }
    }
    if let Some((_, decoder)) = lock.as_mut() {
        decoder.decode(data)?;
    }
    Ok(true)
}

/// Collect the parameter sets of an Annex-B frame as MediaCodec csd buffers.
fn extract_csd(data: &[u8], is_h265: bool) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let mut current: Option<(u8, usize)> = None;
    let mut i = 0;
    while i + 3 <= data.len() {
        let start_len = if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            Some(3)
        } else if i + 4 <= data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            Some(4)
        } else {
            None
        };
        match start_len {
            Some(len) => {
                if let Some((nal_type, start)) = current {
                    push_csd(&mut result, data, start..i, nal_type, is_h265);
                }
                let header = i + len;
                if header < data.len() {
                    let nal_type = if is_h265 {
                        (data[header] >> 1) & 0x3f
                    } else {
                        data[header] & 0x1f
                    };
                    current = Some((nal_type, i));
                    i = header + 1;
                } else {
                    current = None;
                    break;
                }
            }
            None => i += 1,
        }
    }
    if let Some((nal_type, start)) = current {
        push_csd(&mut result, data, start..data.len(), nal_type, is_h265);
    }
    result
}

fn push_csd(
    result: &mut Vec<Vec<u8>>,
    data: &[u8],
    range: std::ops::Range<usize>,
    nal_type: u8,
    is_h265: bool,
) {
    let keep = if is_h265 {
        matches!(nal_type, 32..=34)
    } else {
        matches!(nal_type, 7 | 8)
    };
    if keep {
        result.push(data[range].to_vec());
    }
}
