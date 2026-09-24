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

/// preset(编码预设), 1-7: 数值越大越慢、画质越好
/// (1 = nvenc p1 / qsv veryfast / amf speed, 7 = nvenc p7 / qsv veryslow / amf quality)
#[allow(dead_code)]
pub const PRESET_FASTEST: i32 = 1;
#[allow(dead_code)]
pub const PRESET_MEDIUM: i32 = 4;
#[allow(dead_code)]
pub const PRESET_SLOWEST: i32 = 7;

/// rc(码率控制), 数值与 hwcodec RateControl 枚举一致
#[allow(dead_code)]
pub const RC_DEFAULT: i32 = 0;
#[allow(dead_code)]
pub const RC_CBR: i32 = 1;
#[allow(dead_code)]
pub const RC_VBR: i32 = 2;
/// 恒定 QP: nvenc rc=constqp+qp / amf rc=cqp+qp_i|p|b / qsv ICQ / mediacodec bitrate_mode=cq
#[allow(dead_code)]
pub const RC_CQ: i32 = 3;

/// nvenc multipass 取值
#[allow(dead_code)]
pub const MULTIPASS_OFF: i32 = 0;
#[allow(dead_code)]
pub const MULTIPASS_QUARTER: i32 = 1;
#[allow(dead_code)]
pub const MULTIPASS_FULL: i32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HwEncodeProfile {
    #[serde(default = "default_id")]
    pub id: String,
    /// 编码预设: 1-7, 越大越慢画质越好; None = 编码器默认预设
    #[serde(default)]
    pub preset: Option<i32>,
    /// 码率控制: 0=DEFAULT 1=CBR 2=VBR 3=CQ
    #[serde(default)]
    pub rc: Option<i32>,
    /// 固定码率 kbps; None = 自动 (base_bitrate × ratio)
    #[serde(default)]
    pub kbs: Option<u32>,
    /// QP; rc=CQ 时生效 (nvenc/amf/qsv/mediacodec), 0-51 (qsv 取 1-51);
    /// 值越小画质越好、码率越高
    #[serde(default)]
    pub q: Option<i32>,
    /// 编码器侧 fps 覆盖; None = 30
    /// 注意: 只声明编码器帧率, 不改变实际出帧节奏 —— 实际帧率由 VideoQoS 决定
    /// (起始 15fps, 按网络延迟逐步爬升, 上限 = 控制端请求的 custom_fps)。
    /// 要固定帧率需同时打开 pin_fps。
    #[serde(default)]
    pub fps: Option<i32>,
    /// 锁定串流帧率: 忽略 VideoQoS 的帧率自适应, 直接用 fps 作为出帧间隔。
    /// VideoQoS 会把帧率从 15fps 缓慢爬升, 且在延迟升高时主动降帧;
    /// 打开此项后帧率固定为 fps (网络拥塞时表现为卡顿/延迟增大, 不再自动降帧)。
    #[serde(default)]
    pub pin_fps: bool,
    /// GOP 覆盖; None = keyframe_interval / MAX_GOP (录制时 240 优先)
    #[serde(default)]
    pub gop: Option<i32>,
    /// 画质增强 (编码器内建能力, 不额外占用 CPU; None = 保持编码器默认)
    /// nvenc: spatial aq / temporal aq / multipass; amf: preanalysis
    /// 0=off 1=on (与 preset/rc 一致用数值码, 便于 UI/JSON 往返)
    #[serde(default)]
    pub spatial_aq: Option<i32>,
    /// nvenc temporal AQ; ⚠️ 部分 GPU 不支持, 此时 hwcodec 会去掉增强项重试一次
    #[serde(default)]
    pub temporal_aq: Option<i32>,
    /// nvenc multipass: 0=off 1=two pass quarter res 2=two pass full res
    #[serde(default)]
    pub multipass: Option<i32>,
    /// amf pre-analysis; 0=off 1=on
    #[serde(default)]
    pub preanalysis: Option<i32>,
    /// 厂商私有参数 (按编码器厂商分组, 只对 VRAM 通道生效)
    #[serde(default)]
    pub vendor: HwVendorOpts,
    /// 是否允许 VideoQoS 运行期动态调整码率
    #[serde(default = "default_true")]
    pub bitrate_adaptive: bool,
}

/// 厂商私有编码参数。各厂商只读取自己认识的 key, 因此三家的参数可以同时下发,
/// 未被当前硬件选中的厂商的参数会被忽略 (主机日志 "hw encode params" 可见实际取值)。
/// 统一约定: 0/1 为开关 (1=on), 其余按各自枚举语义。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HwVendorOpts {
    /// nvenc 调优方向: 1=ultra low latency 2=low latency (编码器默认) 3=high quality
    #[serde(default)]
    pub tuning: Option<i32>,
    /// nvenc 前向预测深度; 0=关闭 (默认, 不增加延迟), 越大越慢画质越好
    #[serde(default)]
    pub lookahead_depth: Option<i32>,
    /// nvenc VBR 目标质量 0-51 (0=自动); 仅 VBR 生效, 越大画质越差
    #[serde(default)]
    pub target_quality: Option<i32>,
    /// 参考帧数 (nvenc/mfx/amf); 越大画质越好、延迟略增
    #[serde(default)]
    pub num_ref_frame: Option<i32>,
    /// qsv 关闭 CABAC 用 CAVLC: 0=off 1=on (码率换兼容性, 一般不开)
    #[serde(default)]
    pub cavlc: Option<i32>,
    /// qsv 低功耗编码 (VDENC): 0=off 1=on
    #[serde(default)]
    pub low_power: Option<i32>,
    /// qsv 低延迟码控: 0=off 1=on
    #[serde(default)]
    pub low_delay_brc: Option<i32>,
    /// qsv 编码队列深度; 越大吞吐越好但延迟越高
    #[serde(default)]
    pub async_depth: Option<i32>,
    /// amf 用途: 1=ultra low latency 2=low latency 4=high quality
    #[serde(default)]
    pub usage: Option<i32>,
    /// amf 自适应量化: 0=off 1=on
    #[serde(default)]
    pub vbaq: Option<i32>,
    /// amf 强制 HRD (码率严格合规): 0=off 1=on
    #[serde(default)]
    pub enforce_hrd: Option<i32>,
    /// amf 每帧 slice 数; 越多越利于丢包恢复, 码率略增
    #[serde(default)]
    pub slices_per_frame: Option<i32>,
    /// amf 高运动画面质量增强: 0=off 1=on
    #[serde(default)]
    pub high_motion_qb: Option<i32>,
    /// amf 低延迟模式: 0=off 1=on (默认 on)
    #[serde(default)]
    pub lowlatency_mode: Option<i32>,
    /// amf 输入队列深度; 越大吞吐越好但延迟越高
    #[serde(default)]
    pub input_queue_size: Option<i32>,
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
            pin_fps: false,
            gop: None,
            spatial_aq: None,
            temporal_aq: None,
            multipass: None,
            preanalysis: None,
            vendor: HwVendorOpts::default(),
            bitrate_adaptive: true,
        }
    }
}

impl HwEncodeProfile {
    fn validate(mut self) -> Option<Self> {
        if let Some(v) = self.preset {
            if !(PRESET_FASTEST..=PRESET_SLOWEST).contains(&v) {
                log::warn!("hw-encode-profile: preset {v} 越界(1-7), 忽略");
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
        if self.pin_fps && self.fps.is_none() {
            // 没填 fps 时 pin 无意义, 直接关掉, 避免"打开了却没效果"
            log::warn!("hw-encode-profile: pin_fps 需要同时设置 fps, 已关闭 pin_fps");
            self.pin_fps = false;
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
        if let Some(v) = self.multipass {
            if !(0..=2).contains(&v) {
                log::warn!("hw-encode-profile: multipass {v} 越界(0-2), 忽略");
                self.multipass = None;
            }
        }
        // 0/1 开关类增强项越界即丢弃 (multipass 上面已单独校验)
        fn check01(v: Option<i32>, name: &str) -> Option<i32> {
            if let Some(v) = v {
                if !(0..=1).contains(&v) {
                    log::warn!("hw-encode-profile: {name} {v} 越界(0-1), 忽略");
                    return None;
                }
            }
            v
        }
        self.spatial_aq = check01(self.spatial_aq, "spatial_aq");
        self.temporal_aq = check01(self.temporal_aq, "temporal_aq");
        self.preanalysis = check01(self.preanalysis, "preanalysis");
        self.vendor.validate();
        Some(self)
    }
}

impl HwVendorOpts {
    fn validate(&mut self) {
        fn check(v: Option<i32>, name: &str, min: i32, max: i32) -> Option<i32> {
            if let Some(v) = v {
                if !(min..=max).contains(&v) {
                    log::warn!("hw-encode-profile: {name} {v} 越界({min}-{max}), 忽略");
                    return None;
                }
            }
            v
        }
        self.tuning = check(self.tuning, "tuning", 1, 3);
        self.lookahead_depth = check(self.lookahead_depth, "lookahead_depth", 0, 64);
        self.target_quality = check(self.target_quality, "target_quality", 0, 51);
        self.num_ref_frame = check(self.num_ref_frame, "num_ref_frame", 0, 16);
        self.cavlc = check(self.cavlc, "cavlc", 0, 1);
        self.low_power = check(self.low_power, "low_power", 0, 1);
        self.low_delay_brc = check(self.low_delay_brc, "low_delay_brc", 0, 1);
        self.async_depth = check(self.async_depth, "async_depth", 1, 64);
        if let Some(v) = self.usage {
            if !matches!(v, 1 | 2 | 4) {
                log::warn!("hw-encode-profile: usage {v} 非法(1/2/4), 忽略");
                self.usage = None;
            }
        }
        self.vbaq = check(self.vbaq, "vbaq", 0, 1);
        self.enforce_hrd = check(self.enforce_hrd, "enforce_hrd", 0, 1);
        self.slices_per_frame = check(self.slices_per_frame, "slices_per_frame", 0, 32);
        self.high_motion_qb = check(self.high_motion_qb, "high_motion_qb", 0, 1);
        self.lowlatency_mode = check(self.lowlatency_mode, "lowlatency_mode", 0, 1);
        self.input_queue_size = check(self.input_queue_size, "input_queue_size", 1, 64);
    }
}

/// 预置档定义
pub fn preset(id: &str) -> Option<HwEncodeProfile> {
    match id {
        PRESET_LATENCY => Some(HwEncodeProfile {
            id: PRESET_LATENCY.to_owned(),
            preset: Some(PRESET_FASTEST), // nvenc p1 / qsv veryfast / amf speed
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
            // 零成本画质增强 (编码器内建): nvenc spatial AQ + 两次编码(1/4 分辨率),
            // amf pre-analysis。temporal AQ 有 GPU 能力门槛, 仅自定义档可选。
            spatial_aq: Some(1),
            multipass: Some(MULTIPASS_QUARTER),
            preanalysis: Some(1),
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

/// 需要锁定的出帧帧率。Some(fps) 表示忽略 VideoQoS 的帧率自适应, 固定以此帧率出帧;
/// None = 保持上游行为 (起始 15fps, 由 VideoQoS 按延迟爬升/下降)。
pub fn pinned_fps() -> Option<u32> {
    let p = active_profile()?;
    if !p.pin_fps {
        return None;
    }
    p.fps.filter(|v| *v > 0).map(|v| v as u32)
}

/// 转换为 scrap 编码器参数; record=true 时 gop 覆盖让位给录制用的 240 帧关键帧间隔。
/// VRAM 通道 (本 hwcodec fork) 同样接收 preset/rc/q/画质增强/厂商私有参数。
#[cfg(feature = "hwcodec")]
pub fn hw_params(record: bool) -> Option<scrap::codec::HwEncoderParams> {
    let p = active_profile()?;
    let gop = if record { None } else { p.gop };
    let v = &p.vendor;
    let mut vendor = Vec::<(String, i32)>::new();
    for (k, val) in [
        ("tuning", v.tuning),
        ("lookahead_depth", v.lookahead_depth),
        ("target_quality", v.target_quality),
        ("num_ref_frame", v.num_ref_frame),
        ("cavlc", v.cavlc),
        ("low_power", v.low_power),
        ("low_delay_brc", v.low_delay_brc),
        ("async_depth", v.async_depth),
        ("usage", v.usage),
        ("vbaq", v.vbaq),
        ("enforce_hrd", v.enforce_hrd),
        ("slices_per_frame", v.slices_per_frame),
        ("high_motion_qb", v.high_motion_qb),
        ("lowlatency_mode", v.lowlatency_mode),
        ("input_queue_size", v.input_queue_size),
    ] {
        if let Some(val) = val {
            vendor.push((k.to_owned(), val));
        }
    }
    Some(scrap::codec::HwEncoderParams {
        preset: p.preset,
        rc: p.rc,
        kbs: p.kbs,
        q: p.q,
        fps: p.fps,
        gop,
        spatial_aq: p.spatial_aq.map(|v| v > 0),
        temporal_aq: p.temporal_aq.map(|v| v > 0),
        multipass: p.multipass,
        preanalysis: p.preanalysis.map(|v| v > 0),
        vendor,
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_preset_id() {
        let p = parse_profile("latency").unwrap();
        assert_eq!(p.id, "latency");
        assert_eq!(p.preset, Some(PRESET_FASTEST));
        assert_eq!(p.rc, Some(RC_CBR));
        assert!(parse_profile("balanced").unwrap().preset.is_none());
        assert!(parse_profile("unknown-preset").is_none());
        assert!(parse_profile("").is_none());
    }

    #[test]
    fn test_parse_json_and_validate() {
        let p = parse_profile(r#"{"id":"custom","preset":4,"rc":2,"kbs":8000,"fps":60}"#).unwrap();
        assert_eq!(p.preset, Some(PRESET_MEDIUM));
        assert_eq!(p.rc, Some(RC_VBR));
        assert_eq!(p.kbs, Some(8000));
        assert_eq!(p.fps, Some(60));
        assert!(p.bitrate_adaptive);
        // 越界回退 (preset 只有 1-7)
        let p = parse_profile(r#"{"preset":9,"kbs":999999}"#).unwrap();
        assert_eq!(p.preset, None);
        assert_eq!(p.kbs, None);
        let p = parse_profile(r#"{"preset":0}"#).unwrap();
        assert_eq!(p.preset, None);
        // 非法 JSON
        assert!(parse_profile("{bad json").is_none());
    }

    #[test]
    fn test_record_gop_priority() {
        let gop_backup = parse_profile(r#"{"gop":500}"#).unwrap();
        assert_eq!(gop_backup.gop, Some(500));
        // hw_params 的 record 分支由 video_service 集成, 这里只验字段
    }

    #[test]
    fn test_quality_preset_enhance() {
        let p = parse_profile("quality").unwrap();
        assert_eq!(p.spatial_aq, Some(1));
        assert_eq!(p.multipass, Some(MULTIPASS_QUARTER));
        assert_eq!(p.preanalysis, Some(1));
        // temporal AQ 有 GPU 能力门槛, 默认不开 (仅自定义档可选)
        assert_eq!(p.temporal_aq, None);
        // 越界 multipass 丢弃
        let p = parse_profile(r#"{"id":"custom","multipass":9}"#).unwrap();
        assert_eq!(p.multipass, None);
        // 越界增强开关丢弃
        let p = parse_profile(r#"{"id":"custom","spatial_aq":5}"#).unwrap();
        assert_eq!(p.spatial_aq, None);
    }

    #[test]
    fn test_cq_profile() {
        // CQ / 恒定 QP: nvenc/amf/qsv 也支持 (依赖 hwcodec fork)
        let p = parse_profile(r#"{"id":"custom","rc":3,"q":23}"#).unwrap();
        assert_eq!(p.rc, Some(RC_CQ));
        assert_eq!(p.q, Some(23));
        // q 越界丢弃
        let p = parse_profile(r#"{"id":"custom","rc":3,"q":60}"#).unwrap();
        assert_eq!(p.q, None);
        assert_eq!(p.rc, Some(RC_CQ));
        // rc 越界丢弃
        let p = parse_profile(r#"{"id":"custom","rc":9}"#).unwrap();
        assert_eq!(p.rc, None);
    }

    #[test]
    fn test_vendor_opts() {
        // 缺省不下发任何厂商私有参数
        let p = parse_profile("latency").unwrap();
        assert_eq!(p.vendor, HwVendorOpts::default());
        // 三家可以同时配置
        let p = parse_profile(
            r#"{"id":"custom","vendor":{"tuning":3,"cavlc":1,"usage":4,"async_depth":4}}"#,
        )
        .unwrap();
        assert_eq!(p.vendor.tuning, Some(3));
        assert_eq!(p.vendor.cavlc, Some(1));
        assert_eq!(p.vendor.usage, Some(4));
        assert_eq!(p.vendor.async_depth, Some(4));
        // 越界丢弃
        let p = parse_profile(r#"{"id":"custom","vendor":{"tuning":9,"cavlc":5,"usage":3}}"#)
            .unwrap();
        assert_eq!(p.vendor.tuning, None);
        assert_eq!(p.vendor.cavlc, None);
        assert_eq!(p.vendor.usage, None);
    }

    #[test]
    fn test_pin_fps() {
        // 默认不锁定
        let p = parse_profile(r#"{"id":"custom","fps":60}"#).unwrap();
        assert!(!p.pin_fps);
        assert_eq!(p.fps, Some(60));
        // 显式锁定
        let p = parse_profile(r#"{"id":"custom","fps":60,"pin_fps":true}"#).unwrap();
        assert!(p.pin_fps);
        assert_eq!(p.fps, Some(60));
        // pin_fps 但没填 fps -> 自动关闭
        let p = parse_profile(r#"{"id":"custom","pin_fps":true}"#).unwrap();
        assert!(!p.pin_fps);
        // fps 越界被丢弃时 pin 同样关闭
        let p = parse_profile(r#"{"id":"custom","fps":999,"pin_fps":true}"#).unwrap();
        assert_eq!(p.fps, None);
        assert!(!p.pin_fps);
    }
}
