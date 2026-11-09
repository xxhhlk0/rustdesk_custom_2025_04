use hbb_common::{log, ResultType};
use ndk::media::media_codec::{MediaCodec, MediaCodecDirection, MediaFormat};
use ndk::native_window::NativeWindow;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const H264_MIME_TYPE: &str = "video/avc";
const H265_MIME_TYPE: &str = "video/hevc";

static WINDOW: Mutex<Option<Arc<NativeWindow>>> = Mutex::new(None);
static DECODER: Mutex<Option<(String, SurfaceDecoder)>> = Mutex::new(None);
static FRAME_RENDERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SURFACE_SIZE: Mutex<(i32, i32)> = Mutex::new((0, 0));

/// True once a decoded frame has been handed to the surface.
pub fn frame_rendered() -> bool {
    FRAME_RENDERED.load(std::sync::atomic::Ordering::SeqCst)
}

/// Real decoded frame size, reported to the client so it can lay the texture out
/// instead of guessing from the peer display list.
pub fn surface_size() -> (i32, i32) {
    *SURFACE_SIZE.lock().unwrap()
}

struct SurfaceDecoder {
    codec: MediaCodec,
    window: Arc<NativeWindow>,
    pts: u64,
    size_reported: bool,
}

// The codec handle is only touched from the video decode thread behind the
// decoder mutex.
unsafe impl Send for SurfaceDecoder {}

impl SurfaceDecoder {
    fn new(mime: &str, window: &Arc<NativeWindow>, csd: &[Vec<u8>]) -> Option<Self> {
        let w = if window.width() > 0 {
            window.width()
        } else {
            2560
        };
        let h = if window.height() > 0 {
            window.height()
        } else {
            1440
        };
        // Device codecs differ in what they accept for csd: Annex-B parameter
        // sets, AVCC (no start code) or a single merged buffer. Try the common
        // layouts instead of guessing one.
        for variant in 0..3 {
            let units: Vec<Vec<u8>> = match variant {
                0 => csd.to_vec(),
                1 => csd.iter().map(|u| strip_start_code(u)).collect(),
                _ => vec![csd.concat()],
            };
            let codec = match MediaCodec::from_decoder_type(mime) {
                Some(c) => c,
                None => return None,
            };
            let format = MediaFormat::new();
            format.set_str("mime", mime);
            format.set_i32("width", w);
            format.set_i32("height", h);
            for (i, unit) in units.iter().enumerate() {
                format.set_buffer(&format!("csd-{}", i), unit);
            }
            match codec.configure(&format, Some(window), MediaCodecDirection::Decoder) {
                Ok(()) => match codec.start() {
                    Ok(()) => {
                        log::info!(
                            "surface decoder started: {} variant={} csd={} size={}x{}",
                            mime,
                            variant,
                            units.len(),
                            w,
                            h
                        );
                        return Some(Self {
                            codec,
                            window: window.clone(),
                            pts: 0,
                            size_reported: false,
                        });
                    }
                    Err(e) => log::error!("surface start failed variant={}: {:?}", variant, e),
                },
                Err(e) => log::error!("surface configure failed variant={}: {:?}", variant, e),
            }
        }
        None
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
            if !self.size_reported {
                // surface 模式下 output_format 的 width/height 常常是 configure 时的
                // 窗口尺寸 (TextureView 初始布局), 不是码流真实尺寸; 真实尺寸在
                // crop 字段里。拿窗口尺寸去布局会把 2560x1440 的画面拉成手机屏
                // 比例 (截图实测内容区比例 0.695, 正确应为 1.78, 横向严重压扁)。
                // 因此 crop 优先。
                let format = self.codec.output_format();
                let crop_w = match (format.i32("crop-right"), format.i32("crop-left")) {
                    (Some(r), Some(l)) if r >= l => Some(r - l + 1),
                    _ => None,
                };
                let crop_h = match (format.i32("crop-bottom"), format.i32("crop-top")) {
                    (Some(b), Some(t)) if b >= t => Some(b - t + 1),
                    _ => None,
                };
                let fmt_w = format.i32("width").filter(|v| *v > 0);
                let fmt_h = format.i32("height").filter(|v| *v > 0);
                if let (Some(w), Some(h)) = (crop_w.or(fmt_w), crop_h.or(fmt_h)) {
                    if w > 0 && h > 0 {
                        *SURFACE_SIZE.lock().unwrap() = (w, h);
                        self.size_reported = true;
                        log::info!(
                            "surface frame size {}x{} (crop w={:?} h={:?}, fmt {}x{})",
                            w,
                            h,
                            crop_w,
                            crop_h,
                            fmt_w.unwrap_or(0),
                            fmt_h.unwrap_or(0)
                        );
                    }
                }
            }
            self.codec.release_output_buffer(out, true)?;
            FRAME_RENDERED.store(true, std::sync::atomic::Ordering::SeqCst);
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
    FRAME_RENDERED.store(false, std::sync::atomic::Ordering::SeqCst);
    *SURFACE_SIZE.lock().unwrap() = (0, 0);
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
            static PROBED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            if !PROBED.swap(true, std::sync::atomic::Ordering::SeqCst) {
                let n = data.len().min(32);
                log::info!("surface probe len={} head={:02x?}", data.len(), &data[..n]);
            }
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

/// Collect the parameter sets of a frame as MediaCodec csd buffers. Handles
/// both Annex-B start codes and length-prefixed (AVCC) NAL units.
fn extract_csd(data: &[u8], is_h265: bool) -> Vec<Vec<u8>> {
    let annexb = extract_annexb_csd(data, is_h265);
    if !annexb.is_empty() {
        return annexb;
    }
    extract_avcc_csd(data, is_h265)
}

fn strip_start_code(unit: &[u8]) -> Vec<u8> {
    if unit.starts_with(&[0, 0, 0, 1]) {
        unit[4..].to_vec()
    } else if unit.starts_with(&[0, 0, 1]) {
        unit[3..].to_vec()
    } else {
        unit.to_vec()
    }
}

fn keep_nal(nal_type: u8, is_h265: bool) -> bool {    if is_h265 {
        matches!(nal_type, 32..=34)
    } else {
        matches!(nal_type, 7 | 8)
    }
}

fn extract_annexb_csd(data: &[u8], is_h265: bool) -> Vec<Vec<u8>> {
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
                    if keep_nal(nal_type, is_h265) {
                        result.push(data[start..i].to_vec());
                    }
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
        if keep_nal(nal_type, is_h265) {
            result.push(data[start..data.len()].to_vec());
        }
    }
    result
}

fn extract_avcc_csd(data: &[u8], is_h265: bool) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let mut i = 0;
    while i + 4 <= data.len() {
        let len = u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
        if len == 0 || i + 4 + len > data.len() {
            break;
        }
        let nal = &data[i + 4..i + 4 + len];
        let nal_type = if is_h265 {
            (nal[0] >> 1) & 0x3f
        } else {
            nal[0] & 0x1f
        };
        if keep_nal(nal_type, is_h265) {
            let mut with_sc = vec![0u8, 0, 0, 1];
            with_sc.extend_from_slice(nal);
            result.push(with_sc);
        }
        i += 4 + len;
    }
    result
}
