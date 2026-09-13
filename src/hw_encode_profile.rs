// 硬件编码参数 profile (需求: 视频流硬件编码参数选择, 含预置 profile 与自定义参数)。
//
// - 全局默认: Config option "hw-encode-profile" (内容为预置档 id 或完整 JSON)
// - 按客户端覆盖: Config::get_peer_option(peer_id, "hw-encode-profile")
//   (sunshine 基地版"客户端独立配置"模式; 编码器为被控端共享, 以最近连接的
//    带 override 客户端为准, 其断开后回落到全局默认)
// - 生效时机: 编码器随会话建立/编码协商创建 (video_service setup_encoder),
//   修改配置后对新会话生效。
use hbb_common::{
    config::Config, log, serde_derive::{Deserialize, Serialize},
};
use std::collections::HashMap;

/// 与 hbb_common keys::OPTION_HW_ENCODE_PROFILE 相同的键名;
/// 同时作为 per-peer option 的键。
pub const OPTION_HW_ENCODE_PROFILE: &str = "hw-encode-profile";

/// 预置档 id
pub const PRESET_LATENCY: &str = "latency";
pub const PRESET_BALANCED: &str = "balanced";
pub const PRESET_QUALITY: &str = "quality";
pub const PRESET_CUSTOM: &str = "custom";

/// preset(编码预设), 数值与 hwcodec Quality 枚举一致
#[allow(dead_code)]
pub const PRESET_DEFAULT: i32 = 0;
#[allow(dead_code)]
pub const PRESET_HIGH: i32 = 1;
#[allow(dead_code)]
pub const PRESET_MEDIUM: i32 = 2;
#[allow(dead_code)]
pub const PRESET_LOW: i32 = 3;

/// rc(码率控制), 数值与 hwcodec RateControl 枚举一致
#[allow(dead_code)]
pub const RC_DEFAULT: i32 = 0;
#[allow(dead_code)]
pub const RC_CBR: i32 = 1;
#[allow(dead_code)]
pub const RC_VBR: i32 = 2;
#[allow(dead_code)]
pub const RC_CQ: i32 = 3; // 仅 mediacodec 生效

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HwEncodeProfile {
    #[serde(default = "default_id")]
    pub id: String,
    /// 编码预设: 0=Default 1=High 2=Medium 3=Low
    #[serde(default)]
    pub preset: Option<i32>,
    /// 码率控制: 0=DEFAULT 1=CBR 2=VBR 3=CQ
    #[serde(default)]
    pub rc: Option<i32>,
    /// 固定码率 kbps; None = 自动 (base_bitrate × ratio)
    #[serde(default)]
    pub kbs: Option<u32>,
    /// QP; 仅 rc=CQ (mediacodec) 时生效, 0-51
    #[serde(default)]
    pub q: Option<i32>,
    /// 编码器侧 fps 覆盖; None = 30
    #[serde(default)]
    pub fps: Option<i32>,
    /// GOP 覆盖; None = keyframe_interval / MAX_GOP (录制时 240 优先)
    #[serde(default)]
    pub gop: Option<i32>,
    /// 是否允许 VideoQoS 运行期动态调整码率
    #[serde(default = "default_true")]
    pub bitrate_adaptive: bool,
}

fn default_id() -> String {
    PRESET_CUSTOM.to_owned()
}

fn default_true() -> bool {
    true
}

impl Default for HwEncodeProfile {
    fn default() -> Self {
        Self {
            id: PRESET_BALANCED.to_owned(),
            preset: None,
            rc: None,
            kbs: None,
            q: None,
            fps: None,
            gop: None,
            bitrate_adaptive: true,
        }
    }
}

impl HwEncodeProfile {
    fn validate(mut self) -> Option<Self> {
        if let Some(v) = self.preset {
            if !(PRESET_DEFAULT..=PRESET_LOW).contains(&v) {
                log::warn!("hw-encode-profile: preset {v} 越界, 忽略");
                self.preset = None;
            }
        }
        if let Some(v) = self.rc {
            if !(RC_DEFAULT..=RC_CQ).contains(&v) {
                log::warn!("hw-encode-profile: rc {v} 越界, 忽略");
                self.rc = None;
            }
        }
        if let Some(v) = self.q {
            if !(0..=51).contains(&v) {
                log::warn!("hw-encode-profile: q {v} 越界(0-51), 忽略");
                self.q = None;
            }
        }
        if let Some(v) = self.fps {
            if !(1..=120).contains(&v) {
                log::warn!("hw-encode-profile: fps {v} 越界(1-120), 忽略");
                self.fps = None;
            }
        }
        if let Some(v) = self.gop {
            if !(1..=100000).contains(&v) {
                log::warn!("hw-encode-profile: gop {v} 越界, 忽略");
                self.gop = None;
            }
        }
        if let Some(v) = self.kbs {
            // BR_MAX=40Mbps, 下限 16kbps; scrap 侧 check_bitrate_range 会再做一次钳制
            if !(16..=40000).contains(&v) {
                log::warn!("hw-encode-profile: kbs {v} 越界(16-40000), 忽略");
                self.kbs = None;
            }
        }
        Some(self)
    }
}

/// 预置档定义
pub fn preset(id: &str) -> Option<HwEncodeProfile> {
    match id {
        PRESET_LATENCY => Some(HwEncodeProfile {
            id: PRESET_LATENCY.to_owned(),
            preset: Some(PRESET_LOW),    // nvenc p1 / qsv veryfast / amf speed
            rc: Some(RC_CBR),
            gop: None,
            ..Default::default()
        }),
        PRESET_BALANCED => Some(HwEncodeProfile::default()), // 与官方默认行为一致
        PRESET_QUALITY => Some(HwEncodeProfile {
            id: PRESET_QUALITY.to_owned(),
            preset: Some(PRESET_MEDIUM), // nvenc p4 / qsv medium / amf balanced
            rc: Some(RC_VBR),
            gop: Some(240),
            ..Default::default()
        }),
        PRESET_CUSTOM => Some(HwEncodeProfile {
            id: PRESET_CUSTOM.to_owned(),
            ..Default::default()
        }),
        _ => None,
    }
}

/// 解析配置值: 支持预置档 id ("latency"/"balanced"/"quality"/"custom") 或完整 JSON
pub fn parse_profile(v: &str) -> Option<HwEncodeProfile> {
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    if !v.starts_with('{') {
        return preset(v).filter(|p| p.id != PRESET_CUSTOM);
    }
    match serde_json::from_str::<HwEncodeProfile>(v) {
        Ok(p) => p.validate(),
        Err(e) => {
            log::warn!("hw-encode-profile: JSON 解析失败: {e}");
            None
        }
    }
}

fn global_profile() -> Option<HwEncodeProfile> {
    parse_profile(&Config::get_option(OPTION_HW_ENCODE_PROFILE))
}

fn peer_profile(peer_id: &str) -> Option<HwEncodeProfile> {
    if peer_id.is_empty() {
        return None;
    }
    parse_profile(&crate::ui_interface::get_peer_option(
        peer_id.to_owned(),
        OPTION_HW_ENCODE_PROFILE.to_owned(),
    ))
}

#[derive(Debug)]
struct ActiveOverride {
    conn_id: i32,
    peer_id: String,
    profile: HwEncodeProfile,
}

lazy_static::lazy_static! {
    // 按客户端覆盖: 最近连接的带 override 客户端生效
    static ref ACTIVE: std::sync::Mutex<Option<ActiveOverride>> = Default::default();
    // conn_id -> peer_id (活跃的远程控制/摄像头连接)
    static ref CONNS: std::sync::Mutex<HashMap<i32, String>> = Default::default();
}

/// 连接建立 (认证后) 时调用; peer_id 为控制端 ID
pub fn on_connection_open(conn_id: i32, peer_id: &str) {
    CONNS.lock().unwrap().insert(conn_id, peer_id.to_owned());
    if let Some(p) = peer_profile(peer_id) {
        log::info!("hw-encode-profile: 应用客户端 {peer_id} 的覆盖 profile: {:?}", p.id);
        *ACTIVE.lock().unwrap() = Some(ActiveOverride {
            conn_id,
            peer_id: peer_id.to_owned(),
            profile: p,
        });
    }
}

/// 连接断开时调用
pub fn on_connection_close(conn_id: i32) {
    let peer = CONNS.lock().unwrap().remove(&conn_id);
    let mut active = ACTIVE.lock().unwrap();
    let is_owner = active.as_ref().map(|a| a.conn_id) == Some(conn_id);
    if is_owner {
        *active = None;
        drop(active);
        // 回落到其他仍在线、带 override 的客户端 (如有)
        if let Some(pid) = peer.as_ref() {
            log::info!("hw-encode-profile: 客户端 {pid} 断开, 覆盖 profile 失效");
        }
        let candidates: Vec<String> = CONNS.lock().unwrap().values().cloned().collect();
        for pid in candidates {
            if let Some(p) = peer_profile(&pid) {
                *ACTIVE.lock().unwrap() = Some(ActiveOverride {
                    conn_id,
                    peer_id: pid.clone(),
                    profile: p,
                });
                break;
            }
        }
    }
}

/// 当前生效的 profile: 客户端覆盖 > 全局默认
pub fn active_profile() -> Option<HwEncodeProfile> {
    if let Some(a) = ACTIVE.lock().unwrap().as_ref() {
        return Some(a.profile.clone());
    }
    global_profile()
}

/// 当前是否允许 VideoQoS 动态码率 (默认允许)
pub fn bitrate_adaptive() -> bool {
    active_profile().map(|p| p.bitrate_adaptive).unwrap_or(true)
}

/// 转换为 scrap 编码器参数; record=true 时 gop 覆盖让位给录制用的 240 帧关键帧间隔。
/// VRAM 通道仅支持 kbs/fps/gop (preset/rc 由 C 库写死)。
#[cfg(feature = "hwcodec")]
pub fn hw_params(record: bool) -> Option<scrap::codec::HwEncoderParams> {
    let p = active_profile()?;
    let gop = if record { None } else { p.gop };
    Some(scrap::codec::HwEncoderParams {
        preset: p.preset,
        rc: p.rc,
        kbs: p.kbs,
        q: p.q,
        fps: p.fps,
        gop,
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_preset_id() {
        let p = parse_profile("latency").unwrap();
        assert_eq!(p.id, "latency");
        assert_eq!(p.preset, Some(PRESET_LOW));
        assert_eq!(p.rc, Some(RC_CBR));
        assert!(parse_profile("balanced").unwrap().preset.is_none());
        assert!(parse_profile("unknown-preset").is_none());
        assert!(parse_profile("").is_none());
    }

    #[test]
    fn test_parse_json_and_validate() {
        let p = parse_profile(r#"{"id":"custom","preset":2,"rc":2,"kbs":8000,"fps":60}"#).unwrap();
        assert_eq!(p.preset, Some(PRESET_MEDIUM));
        assert_eq!(p.rc, Some(RC_VBR));
        assert_eq!(p.kbs, Some(8000));
        assert_eq!(p.fps, Some(60));
        assert!(p.bitrate_adaptive);
        // 越界回退
        let p = parse_profile(r#"{"preset":9,"kbs":999999}"#).unwrap();
        assert_eq!(p.preset, None);
        assert_eq!(p.kbs, None);
        // 非法 JSON
        assert!(parse_profile("{bad json").is_none());
    }

    #[test]
    fn test_record_gop_priority() {
        let gop_backup = parse_profile(r#"{"gop":500}"#).unwrap();
        assert_eq!(gop_backup.gop, Some(500));
        // hw_params 的 record 分支由 video_service 集成, 这里只验字段
    }
}
