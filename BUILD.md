# RustDesk 自定义构建说明

## 概述

本项目基于官方 RustDesk 1.4.9 版本进行自定义构建（已从 1.4.6 rebase 同步到上游 1.4.9 发版 tag），所有构建产物上传到 Cloudflare R2 私有存储，不依赖 GitHub Artifacts，适合公开仓库使用。

## 主要修改

### 1. 工作流同步与简化

- 以官方 rustdesk/rustdesk 1.4.9 版本 workflows 为底，保留全部自定义改动
- 移除所有自动触发条件（schedule、push、pull_request、release），仅保留手动触发（workflow_dispatch）

### 2. 产物存储方案

**问题**：GitHub Artifacts 在公开仓库中任何人都可以下载，而 `librustdesk.so` 等中间产物包含编译时嵌入的服务器配置信息。

**解决方案**：所有产物上传到 Cloudflare R2 私有存储桶。

#### R2 存储路径结构

```
{R2_BUCKET}/
├── builds/{RUN_ID}/              # 中间产物（每次构建独立）
│   ├── bridge/                   # Flutter-Rust Bridge 文件
│   │   ├── bridge_generated.rs
│   │   ├── bridge_generated.io.rs
│   │   ├── generated_bridge.dart
│   │   ├── generated_bridge.freezed.dart
│   │   └── bridge_generated.h
│   ├── topmostwindow/            # Windows 弹窗辅助 DLL
│   │   └── WindowInjection.dll
│   ├── android-libs/             # Android Rust 库（含服务器配置）
│   │   ├── librustdesk.so.aarch64-linux-android
│   │   ├── librustdesk.so.armv7-linux-androideabi
│   │   └── librustdesk.so.x86_64-linux-android
│   └── linux-debs/               # Linux DEB 包
│       ├── rustdesk-{VERSION}-x86_64.deb
│       └── rustdesk-{VERSION}-x86_64-sciter.deb
│
└── 1.4.9/{DATE}/                 # 最终产物（长期保存）
    ├── windows/x86_64/
    │   ├── rustdesk-{VERSION}-x86_64.exe
    │   └── rustdesk-{VERSION}-x86_64.msi
    ├── macos/x86_64/
    │   └── rustdesk-{VERSION}-x86_64.dmg
    ├── macos/aarch64/
    │   └── rustdesk-{VERSION}-aarch64.dmg
    ├── android/arm64-v8a/
    │   └── rustdesk-{VERSION}.apk
    ├── android/armeabi-v7a/
    │   └── rustdesk-{VERSION}.apk
    ├── android/universal/
    │   └── rustdesk-{VERSION}-universal.apk
    ├── linux/x86_64/
    │   ├── rustdesk-{VERSION}-x86_64.deb
    │   ├── rustdesk-{VERSION}-x86_64.rpm
    │   └── rustdesk-{VERSION}-x86_64.AppImage
    ├── linux/aarch64/
    │   └── ...
    └── ios/arm64/
        └── rustdesk-{VERSION}.ipa
```

## 必需的 GitHub Secrets

在仓库 Settings → Secrets and variables → Actions 中配置：

| Secret 名称 | 说明 |
|------------|------|
| `R2_BUCKET_NAME` | R2 存储桶名称（如 `rustdesk-builds`） |
| `R2_ENDPOINT_URL` | R2 端点 URL（格式：`https://<account_id>.r2.cloudflarestorage.com`） |
| `R2_ACCESS_KEY_ID` | R2 API 访问密钥 ID |
| `R2_SECRET_ACCESS_KEY` | R2 API 密钥 |

### 可选的签名相关 Secrets

| Secret 名称 | 说明 |
|------------|------|
| `ANDROID_SIGNING_KEY` | Android 签名密钥（Base64） |
| `ANDROID_ALIAS` | Android 密钥别名 |
| `ANDROID_KEY_STORE_PASSWORD` | Android 密钥库密码 |
| `ANDROID_KEY_PASSWORD` | Android 密钥密码 |
| `MACOS_P12_BASE64` | macOS 签名证书（Base64） |

## 自定义服务器配置

在构建时通过 GitHub Secrets 嵌入自定义服务器信息（编译到二进制文件中）：

| Secret 名称 | 说明 | 示例值 |
|------------|------|--------|
| `CUSTOM_RENDEZVOUS_SERVER` | 自定义服务器地址 | `my-server.example.com` |
| `CUSTOM_RS_PUB_KEY` | 自定义公钥 | `OeVuKk5nlHiXp+APNn0Y3pC1Iwpwn44JGqrQCsWqmBw=` |
| `CUSTOM_RENDEZVOUS_PORT` | Rendezvous 端口 | `21116`（默认） |
| `CUSTOM_RELAY_PORT` | Relay 端口 | `21117`（默认） |
| `CUSTOM_WS_RENDEZVOUS_PORT` | WebSocket Rendezvous 端口 | `21118`（默认） |
| `CUSTOM_WS_RELAY_PORT` | WebSocket Relay 端口 | `21119`（默认） |

**重要：** 这些值在编译时嵌入，如果不设置则使用 RustDesk 官方默认值。

## 使用方法

### 手动触发构建

1. 进入 GitHub 仓库 → Actions
2. 选择要运行的 workflow：
   - **flutter-ci.yml** - 完整构建（所有平台）
   - **flutter-nightly.yml** - 每日构建
   - **flutter-tag.yml** - 标签构建
3. 点击 "Run workflow"

### 下载构建产物

访问 Cloudflare R2 控制台或使用 AWS CLI：

```bash
# 列出所有构建
aws s3 ls s3://{R2_BUCKET}/1.4.9/ --endpoint-url {R2_ENDPOINT_URL} --region auto

# 下载文件
aws s3 cp s3://{R2_BUCKET}/1.4.9/20260425/windows/x86_64/rustdesk-1.4.9-x86_64.exe . \
  --endpoint-url {R2_ENDPOINT_URL} \
  --region auto
```

## 修改的文件

| 文件 | 修改内容 |
|-----|---------|
| `.github/workflows/bridge.yml` | 上传 bridge 文件到 R2 |
| `.github/workflows/third-party-RustDeskTempTopMostWindow.yml` | 上传 DLL 到 R2 |
| `.github/workflows/flutter-build.yml` | 主要构建流程，所有产物上传/下载改为 R2 |
| `.github/workflows/flutter-ci.yml` | 更新调用方式 |
| `.github/workflows/flutter-nightly.yml` | 更新调用方式 |
| `.github/workflows/flutter-tag.yml` | 更新调用方式 |
| `.github/scripts/r2-upload.sh` | R2 上传脚本（备用） |
| `.github/scripts/r2-download.sh` | R2 下载脚本（备用） |

## 清理中间产物

构建完成后，可以手动清理 `builds/{RUN_ID}/` 目录中的中间产物：

```bash
# 清理特定构建的中间产物
aws s3 rm s3://{R2_BUCKET}/builds/{RUN_ID}/ --recursive \
  --endpoint-url {R2_ENDPOINT_URL} \
  --region auto
```

## 注意事项

1. **中间产物敏感**：`librustdesk.so.*` 和最终产物包含服务器配置，不应泄露
2. **清理策略**：建议定期清理 `builds/` 目录下的旧构建中间产物
3. **R2 费用**：关注存储量和流量，Cloudflare R2 免费额度为 10GB 存储/月
4. **playground.yml**：此工作流未修改，如需使用需单独配置

## 自定义功能

### 1. 远程控制会话不发送 token

当且仅当建立远程控制会话（`ConnType::DEFAULT_CONN`）时，客户端不向 hbbs 发送 token：
`secure_tcp` 门控、`PunchHoleRequest`、`RequestRelay` 均不再携带（与上游 `other_server` 分支行为一致）。
其他场景保留：HC 心跳通道 token、账号 API `Authorization: Bearer`、
文件传输 / 端口转发 / 摄像头 / 终端会话 token。

### 2. 硬件编码参数 profile（视频流）

被控端（主机）编码参数可选，位于 设置 → 显示 → Hardware Encode Profile：

| 预置档 | 编码预设(preset) | 码率控制(rc) | GOP | 说明 |
|---|---|---|---|---|
| latency 低延迟 | Low (nvenc p1 / qsv veryfast / amf speed) | CBR | 默认 | 流畅优先 |
| balanced 均衡（默认） | Default | CBR | 默认 | 与官方行为一致 |
| quality 画质 | Medium (nvenc p4 / qsv medium / amf balanced) | VBR | 240 | 画质优先 + nvenc spatial AQ/multipass、amf pre-analysis |
| custom 自定义 | 逐项 | 逐项 | 逐项 | 码率/QP/FPS/GOP/画质增强/自适应开关 |

- 配置存储：全局 option `hw-encode-profile`（预置档 id 或 JSON），修改后对新会话生效
- 按客户端覆盖：PeerConfig option `hw-encode-profile`（按 peer id），
  最近连接的带覆盖客户端生效，其断开后回落全局默认
- `bitrate_adaptive=false` 时抑制 VideoQoS 运行期动态码率调整
- **`pin_fps=true` 时才真正固定串流帧率**：profile 的 `fps` 字段只是声明编码器帧率，
  实际出帧节奏由 VideoQoS 决定（会话开始为 `INIT_FPS=15`，随网络延迟逐步爬升，
  上限取控制端请求的 `custom_fps`，延迟升高时主动降帧）。打开 `pin_fps` 后直接用
  `fps` 作帧间隔，忽略该自适应（拥塞时表现为卡顿而非自动降帧）
- 录制会话的 240 帧关键帧间隔优先于 profile 的 GOP 覆盖

**各编码器实际生效矩阵**（依赖下方 §3 的 hwcodec fork）

| 参数 | nvenc | amf | qsv | mediacodec | VRAM 通道 |
|---|---|---|---|---|---|
| preset | p1 / p4 / p7 | speed / balanced / quality | veryfast / medium / veryslow | 仅 level | 不支持（SDK 内写死） |
| rc=CBR | ✅ | ✅ | ✅ | ✅ | 不支持（固定 CBR） |
| rc=VBR | ✅ | ✅（vbr_latency） | ✅ | ✅ | 不支持 |
| rc=CQ（恒定 QP） | ✅ `rc=constqp` + `qp` | ✅ `rc=cqp` + `qp_i/p/b` | ✅ ICQ（`global_quality`） | ✅ `bitrate_mode=cq` | 不支持 |
| QP(q) 范围 | 0-51 | 0-51 | 1-51 | 0-51 | — |
| spatial AQ | ✅ `spatial-aq=1` | — | — | — | — |
| temporal AQ | ✅ `temporal-aq=1`（部分 GPU 不支持） | — | — | — | — |
| multipass | ✅ `qres` / `fullres` | — | — | — | — |
| pre-analysis | — | ✅ `preanalysis=1` | — | — | — |

- **rc=CQ 时码率设置被忽略**：QP 直接决定画质与带宽，值越小画质越好、码率越高
- QP 越界会被忽略并记日志；会话建立时会打印（便于核对实际生效值）：
  - `hw encode params: name=..., quality=, rc=, q=, kbs=, fps=, gop=, bit_rate=, rc_max_rate=, global_quality=`
  - `hw encode opened: name=..., bit_rate=, rc_max_rate=, global_quality=`
    （`avcodec_open2` 之后 ffmpeg 才真正选定码控模式，**以这一行为准**）
  - `qsv rate control: ICQ(global_quality=20)`；
    若请求的模式与实际不符会记为 `qsv rate control mismatch: requested rc=... but ffmpeg will use ...`
    （QSV 没有 `rc` 选项，模式由 ffmpeg 的 `qsvenc.c select_rc_mode` 从 AVCodecContext 字段推导，
    所以可能出现"设了 CBR/CQ 实际却是 VBR"）
  - `encode enhance: ...` = 增强项已应用；
    `encode enhance ignored: <name> supports none of the requested options ...` =
    该编码器不支持所选项（**QSV 等非 nvenc/amf 编码器上 AQ/multipass/pre-analysis 一律无效**）
- **画质增强**（编码器内建能力，不需要 CPU 侧滤镜，均为可选、默认关）：
  - nvenc：`spatial-aq`（空间自适应量化）、`temporal-aq`（时间自适应量化）、
    `multipass`（两次编码，1/4 分辨率或全分辨率）
  - amf：`preanalysis`（预分析）
  - ⚠️ `temporal-aq` 在部分 GPU 上不受支持（ffmpeg 的 `NV_ENC_CAPS_SUPPORT_TEMPORAL_AQ`
    能力检查会返回 ENOSYS），因此 hwcodec fork 在 `avcodec_open2` 失败且应用过增强项时
    **会去掉全部增强项自动重试一次**并记日志，不会让远程会话建不起来
  - `quality` 预置档默认开启 spatial AQ + multipass(quarter res) + pre-analysis；
    `temporal-aq` 默认关（能力门槛），仅在「自定义」档可选
- VRAM 通道（GPU 纹理直达）仅支持 码率/FPS/GOP。VRAM 只在 legacy linux-sciter 构建中启用
- **禁用 VRAM**：profile 自定义档可勾选 `disable_vram`（设置页"强制 RAM 硬编"），
  被控端视频服务启动时对显示器调 `VRamEncoder::set_not_use(monitor, true)`，
  编码协商回落 RAM 通道，preset/rc/QP/AQ 全参数生效；服务停止时自动恢复。
  代价：GPU 纹理多一次回读到内存，延迟略增
  （`--features inline,vram,hwcodec`）；Windows / macOS / Linux Flutter 构建的命令均未启用 vram，
  因此上表参数对正式产物全部生效

### 3. 依赖的 fork

| 依赖 | 上游 | 本仓库指向 | 原因 |
|---|---|---|---|
| `libs/hbb_common`（子模块） | rustdesk/hbb_common | xxhhlk0/hbb_common | 编译期 `CUSTOM_*` 服务器配置注入 |
| `hwcodec`（cargo git 依赖） | rustdesk-org/hwcodec | xxhhlk0/hwcodec @ `516577d` | 恢复 encoder preset 生效 + constant QP（CQ）码率控制 + 可选画质增强 + qsv 编码吞吐修复（`async_depth` 1→2） |

hwcodec fork 的改动（`cpp/common/util.{h,cpp}`、`cpp/ffmpeg_ram/ffmpeg_ram_{ffi.h,encode.cpp}`、
`cpp/ffmpeg_vram/ffmpeg_vram_encode.cpp`）：

1. **恢复 preset 生效**：上游把 `util_encode::set_quality()` 调用注释掉了，导致 profile 的
   `preset` 字段传进 C 后完全没被使用；现已恢复（`Quality_Default` 仍是 no-op，默认行为不变）
2. **nvenc 补 `Quality_High → preset p7`**：上游只映射了 Medium→p4、Low→p1，选 High 在 N 卡上无任何效果
3. **新增 constant QP（CQ）**：nvenc `rc=constqp`+`qp`、amf `rc=cqp`+`qp_i/qp_p/qp_b`、
   qsv ICQ（清掉 `rc_max_rate`/`bit_rate` 并设 `global_quality`）、mediacodec 保持 `bitrate_mode=cq`
4. **qsv 支持 CBR**：上游 `set_av_codec_ctx()` 把 `bit_rate` 减 1 以走 VBR 分支，
   现按 rc=CBR 令 `bit_rate = rc_max_rate` 走真正的 `MFX_RATECONTROL_CBR`
5. **新增 `set_encode_enhance()`**（`spatial-aq`/`temporal-aq`/`multipass`/`preanalysis`）：
   `EncodeContext` 与 `ffmpeg_ram_new_encoder()` FFI 相应扩参；该函数永不返回失败
   （可选项不该拖垮会话），并在 `avcodec_open2` 失败且应用过增强项时去增强重试一次
6. **新增 `set_encode_enhance()`**（`spatial-aq`/`temporal-aq`/`multipass`/`preanalysis`）：
   `EncodeContext` 与 `ffmpeg_ram_new_encoder()` FFI 相应扩参；该函数永不返回失败
   （可选项不该拖垮会话），并在 `avcodec_open2` 失败且应用过增强项时去增强重试一次
7. **把"实际生效的码控"打进日志**：QSV 没有 `rc` 选项，模式由 ffmpeg 的
   `qsvenc.c select_rc_mode` 从 AVCodecContext 字段推导，可能出现请求 CBR/CQ 而实际
   走 VBR/CQP 的静默失效，因此 `set_rate_control()` 会回读并打印实际模式、不一致时告警；
   `set_encode_enhance()` 在请求了编码器不支持的增强项时打印 `encode enhance ignored`；
   编码器日志补 `bit_rate`/`rc_max_rate`/`global_quality`，并在 `avcodec_open2` 后再打印一次
8. **qsv/vaapi `async_depth` 由写死的 1 改为 2（`HWCODEC_ASYNC_DEPTH`）**：上游为了"最低延迟"
   把流水线深度锁成 1，iGPU 无法重叠"取帧-编码-回读"，编码吞吐直接腰斩——
   Intel UHD 750 @2560x1440 实测 `async_depth=1` 只有 89fps（11.2ms/帧），
   `=2` 为 116fps（8.6ms/帧），`=4` 与默认值同量级。写死 1 时 60fps 会话必然被编码耗时卡在
   45fps 左右（采集 5ms + 编码 14ms > 16.7ms 预算）。代价是多 1 帧管线延迟（60fps 下约 16ms），
   同时 `do_encode()` 容忍首帧 `EAGAIN`（等包而不是立刻返回失败，否则首帧会被上层当成编码错误
   并切掉硬件编码器），交付完当前帧的包后即正常收工、不空等到超时。qsv 与 vaapi 两条路都生效，
   RAM 与 VRAM 通道共用该函数


