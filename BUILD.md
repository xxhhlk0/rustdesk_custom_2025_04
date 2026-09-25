# RustDesk 自定义构建说明

## 概述

本项目基于官方 RustDesk **1.4.9** 版本进行自定义构建（已从 1.4.6 rebase 同步到上游 1.4.9 发版 tag），所有构建产物上传到 Cloudflare R2 私有存储，不依赖 GitHub Artifacts，适合公开仓库使用。

### 版本锚点

| 对象 | 当前值 |
|---|---|
| 上游基点 | rustdesk/rustdesk `1.4.9`（`6c578292`） |
| 自定义提交数 | **48**（`git rev-list --count 1.4.9..HEAD`） |
| 最新 tag | `1.4.9-custom.7` |
| 相关 fork | `xxhhlk0/hbb_common`、`xxhhlk0/hwcodec`（见「自定义功能 §5 依赖的 fork」） |

> tag 命名规则：`1.4.9-custom.<N>`，每个 tag 对应一次实质功能提交。
> 所有 workflow 均为 `workflow_dispatch`（无 push/tag 自动触发），**打 tag 不会触发构建**，需手动 dispatch。

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
        ├── rustdesk-{VERSION}-ios-unsigned.ipa     # 纯未签名（AltStore / Sideloadly）
        └── rustdesk-{VERSION}-ios-jailbroken.ipa   # 已注入 entitlements（越狱 / TrollStore）
```

> iOS 两个版本均**不需要任何 Apple 证书或 provisioning profile**。
> 详见下方「自定义功能 §3 iOS 打包」。

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
| `ANDROID_SIGNING_KEY` | Android 签名密钥（keystore 的 Base64） |
| `ANDROID_ALIAS` | Android 密钥别名 |
| `ANDROID_KEY_STORE_PASSWORD` | Android 密钥库密码 |
| `ANDROID_KEY_PASSWORD` | Android 密钥密码 |
| `MACOS_P12_BASE64` | macOS 签名证书（Base64） |
| `MACOS_P12_PASSWORD` | macOS 证书密码 |
| `MACOS_CODESIGN_IDENTITY` | macOS 签名身份 |
| `MACOS_NOTARIZE_JSON` | macOS 公证凭据 |

> **Android 四个 secret 为空时不会报错**：`Sign app APK` 步骤会被跳过，
> 上传的是 **debug 签名 APK**（构建成功但**无法覆盖安装**）。
> 判"真签没签"看该步骤是 `success` 还是 `skipped`。详见「自定义功能 §4 Android 签名」。
>
> **iOS 不需要任何 secret**：未签名版与越狱版都不依赖 Apple 证书。

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

   | Workflow | 内容 | 用途 |
   |---|---|---|
   | **flutter-nightly.yml** | 全平台 + **可选 iOS** | 需要 iOS 产物时用这个 |
   | **flutter-ci.yml** | 全平台（iOS 固定关） | 不验 iOS 时 |
   | **windows-flutter-ci.yml** | 仅 Windows x64 | 快速验证 |

3. 点击 "Run workflow"
   - 需要 iOS ipa 时，勾选 **`build-ios`**（默认不勾）
   - ⚠️ `workflow_dispatch` **不能只跑某个 job**，会跑该 workflow 的全部平台；
     但所有 job 都是 `needs: [generate-bridge]`，bridge 完成后**并行执行**，
     iOS job 与其他平台互不影响，直接盯 `build rustdesk ios ipa` 即可

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

相对上游 1.4.9 共改动 38 个文件（`git diff --name-only 1.4.9 HEAD`）。

### CI / 构建

| 文件 | 修改内容 |
|-----|---------|
| `.github/workflows/flutter-build.yml` | 主构建流程：产物上传/下载改为 R2；新增 iOS 双版本打包、Android 签名前置校验；修复多处 CI 失败 |
| `.github/workflows/bridge.yml` | 上传 bridge 文件到 R2 |
| `.github/workflows/third-party-RustDeskTempTopMostWindow.yml` | 上传 DLL 到 R2 |
| `.github/workflows/flutter-ci.yml` | 更新调用方式 |
| `.github/workflows/flutter-nightly.yml` | 更新调用方式 + `build-ios` 开关 |
| `.github/workflows/flutter-tag.yml` | 更新调用方式 |
| `.github/workflows/windows-flutter-ci.yml` | 新增：Windows x64 专用构建（快速验证） |
| `.github/workflows/ci.yml`、`fdroid.yml` | 上游工作流适配 |
| `.github/actions/upload-r2/action.yml` | 新增：R2 上传 composite action |
| `.github/scripts/create-ios-ipa.sh` | 新增：产出未签名版 + 越狱版 ipa |
| `.github/scripts/r2-upload.sh`、`r2-download.sh`、`upload-to-r2.sh` | R2 上传/下载脚本 |
| `.gitignore`、`.gitmodules` | 忽略规则 + 子模块 fork 指向 |
| `build_lib_release_local.ps1` | 本机编译脚本（LLVM15 libclang） |

### 自定义功能（Rust）

| 文件 | 修改内容 |
|-----|---------|
| `src/hw_encode_profile.rs` | 新增：硬件编码 profile 模块（预置档 / JSON 解析 / per-peer 覆盖 / `disable_vram` / `pin_fps` / `bitrate_adaptive`） |
| `src/client.rs` | 远程控制会话不发送 token（`session_token` helper 覆盖 PunchHoleRequest / RequestRelay） |
| `src/server/video_service.rs` | 注入编码参数；`check_qos` 按 `bitrate_adaptive` + `pin_fps` 门控；`disable_vram` 挂载 |
| `src/server/connection.rs` | 连接建立时挂载 per-client profile |
| `src/lib.rs` | 注册 `hw_encode_profile` 模块 |
| `src/platform/windows.rs`、`windows.cc` | 选择可用用户桌面以启动 `--server`（避免 Session0 丢失） |
| `libs/scrap/src/common/codec.rs`、`hwcodec.rs`、`vram.rs` | `HwEncoderParams` 结构 + 参数下发 |
| `libs/scrap/src/dxgi/mod.rs` | 取帧后复制到私有纹理并立即释放帧，解除 IDD 反压（被控端 60 → ~95fps） |
| `libs/scrap/Cargo.toml` | hwcodec 指向 fork rev |
| `Cargo.toml`、`Cargo.lock` | 依赖与锁文件同步 |

### UI（Flutter）

| 文件 | 修改内容 |
|-----|---------|
| `flutter/lib/desktop/pages/desktop_setting_page.dart` | Hardware Encode Profile 设置 UI（显示设置页） |
| `flutter/lib/consts.dart`、`lib/models/model.dart`、`input_model.dart` | 常量、模型字段、鼠标事件节流合并（修复拖动延迟） |

### iOS 打包

| 文件 | 修改内容 |
|-----|---------|
| `flutter/ios/Runner/Runner.private.entitlements` | 新增：越狱版注入的 entitlements（`application-identifier` 等） |
| `.github/scripts/create-ios-ipa.sh` | 新增：`ldid -S` 伪签名 + 注入，产出两个 ipa |

### 文档

| 文件 | 修改内容 |
|-----|---------|
| `BUILD.md` | 本文档 |
| `libs/hbb_common`（子模块） | 指向 `xxhhlk0/hbb_common` fork |

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
| latency 低延迟 | 1 - Fastest (nvenc p1 / qsv veryfast / amf speed) | CBR | 默认 | 流畅优先 |
| balanced 均衡（默认） | 编码器默认 | CBR | 默认 | 与官方行为一致 |
| quality 画质 | 4 - Medium (nvenc p4 / qsv medium / amf balanced) | VBR | 240 | 画质优先 + nvenc spatial AQ/multipass、amf pre-analysis |
| custom 自定义 | 1-7 逐项 | 逐项 | 逐项 | 码率/QP/FPS/GOP/画质增强/厂商私有参数/自适应开关 |

preset 统一方向：**数值越大越慢、画质越好**（1 = nvenc p1 / qsv veryfast / amf speed，
7 = nvenc p7 / qsv veryslow / amf quality）；留空 = 用编码器默认预设（`balanced` 档即如此）。

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
| preset | 1-7 → p1-p7 | 1-7 → speed…quality | 1-7 → veryfast…veryslow | 仅 level | ✅ 1-7（三家原生 SDK） |
| rc=CBR | ✅ | ✅ | ✅ | ✅ | ✅ |
| rc=VBR | ✅ | ✅（vbr_latency） | ✅ | ✅ | ✅ |
| rc=CQ（恒定 QP） | ✅ `rc=constqp` + `qp` | ✅ `rc=cqp` + `qp_i/p/b` | ✅ ICQ（`global_quality`） | ✅ `bitrate_mode=cq` | ✅ |
| QP(q) 范围 | 0-51 | 0-51 | 1-51 | 0-51 | 0-51（qsv 取 1-51） |
| spatial AQ | ✅ `spatial-aq=1` | — | — | — | ✅ nvenc |
| temporal AQ | ✅ `temporal-aq=1`（部分 GPU 不支持） | — | — | — | ✅ nvenc |
| multipass | ✅ `qres` / `fullres` | — | — | — | ✅ nvenc |
| pre-analysis | — | ✅ `preanalysis=1` | — | — | ✅ amf |

- RAM（ffmpeg）通道的 preset 只有 3 档折算：1-2 → Low(nvenc p1) / 3-5 → Medium(p4) / 6-7 → High(p7)；
  完整的 1-7 只在 VRAM 通道生效（原生 SDK 与 ffmpeg_vram 均按数值直接映射）
- **rc=CQ 时码率设置被忽略**：QP 直接决定画质与带宽，值越小画质越好、码率越高
- QP 越界会被忽略并记日志；会话建立时会打印（便于核对实际生效值）：
  - `hw encode params: name=..., preset=, quality=, rc=, q=, kbs=, fps=, gop=, bit_rate=, rc_max_rate=, global_quality=`
  - `hw encode opened: name=..., bit_rate=, rc_max_rate=, global_quality=`
    （`avcodec_open2` 之后 ffmpeg 才真正选定码控模式，**以这一行为准**）
  - VRAM 原生路径各自打印一行：`mfx encode params: ...` / `nvenc encode params: ...` /
    `amf encode params: ...`，含 preset/rc/q 与全部厂商私有项的实际取值
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
- **厂商私有参数**（`vendor` 字段，仅 VRAM 通道生效，设置页按厂商标注）：
  - nvenc：`tuning`（1=ultra low latency / 2=low latency / 3=high quality）、
    `lookahead_depth`（0=关，开启会增加延迟）、`target_quality`（VBR 目标质量）、`num_ref_frame`
  - qsv：`cavlc`、`low_power`、`low_delay_brc`、`async_depth`、`num_ref_frame`
  - amf：`usage`（1=ultra low latency / 2=low latency / 4=high quality）、`vbaq`、`enforce_hrd`、
    `slices_per_frame`、`high_motion_qb`、`lowlatency_mode`、`input_queue_size`、`num_ref_frame`
  - 三家的参数可以同时配置：hwcodec 把整串下发，各厂商只读取自己认识的 key，其余忽略
  - 留空 = 不下发 = 保持编码器默认（qsv 的 `async_depth`/`low_power`/`low_delay_brc`
    未配置时仍取 §3 第 8 条的吞吐修复默认值）
- VRAM 通道（GPU 纹理直达）同样接收 preset/rc/QP/画质增强/厂商私有参数，无需回落 RAM 通道。
  Windows Flutter 构建已启用（`python build.py --flutter --hwcodec --vram`，
  `--features inline,vram,hwcodec`）

### 3. iOS 打包：未签名版 + 越狱版

iOS job 由 `flutter-nightly.yml` 的 `build-ios` 开关控制（默认关，`workflow_dispatch` 勾选后开启）。

**为什么需要两个版本**：`flutter build ipa --no-codesign` 产出的 app **没有任何签名块**，
越狱设备安装时报

```
Application is missing the application-identifier entitlement
```

（Apple TN2319：installd 要求 Mach-O 里必须有 `application-identifier` entitlement）。
纯未签名版装不上越狱设备，所以额外产出一个**已注入 entitlements 的越狱版**。

| 产物 | 做法 | 适用场景 |
|---|---|---|
| `rustdesk-{VERSION}-ios-unsigned.ipa` | 只 `cp` + `zip`，**完全不签** | AltStore / Sideloadly（用自己 Apple ID 重签） |
| `rustdesk-{VERSION}-ios-jailbroken.ipa` | `ldid -S<plist>` 伪签名 + 注入 entitlements | 越狱设备（AppSync Unified）直装；**TrollStore 亦可** |

- 打包脚本：`.github/scripts/create-ios-ipa.sh`（可用 `VERSION` / `APP_DIR` / `ENT_FILE` / `OUT_DIR` / `SIGN_FRAMEWORKS` 环境变量覆盖）
- entitlements：`flutter/ios/Runner/Runner.private.entitlements`（只含 `application-identifier`
  = `com.carriez.flutterHbb` + `platform-application` / `get-task-allow` /
  `increased-memory-limit` / mach-lookup 白名单）
- **不产出独立 `.tipa`**：TrollStore 官方文档明确其安装时会用 fake root cert 重签、
  **保留 ldid 注入的 entitlements**，故越狱版 ipa 拖进 TrollStore 即可用
- 仅对 `codesign --verify` 失败的 Mach-O 补 ad-hoc 签名，已有有效签名的原样保留
- `ldid` 由 CI 的 `brew install ldid` 提供

**刻意不含的 entitlements**：

- `aps-environment` / `com.apple.developer.networking.wifi-info`
  —— 代码中零引用（已 grep 确认），且是免费 Apple ID 重签的受限项
- `com.apple.private.cs.debugger` / `dynamic-codesigning` / `com.apple.private.skip-library-validation`
  —— TrollStore 文档明示 **iOS 15 A12+ 已封禁，带上的 app 启动即崩溃**

**其他 iOS 相关坑**：

- Flutter 3.24.5 在 `--no-codesign` 下**只产 `Runner.xcarchive`，不创建 `build/ios/ipa/`**
  （打印 `Codesigning disabled with --no-codesign, skipping IPA.`）
  → 手工打包**必须先 `mkdir -p build/ios/ipa`**，否则 `zip` 报 `Could not create output file` 并以 **exit 15** 终止
- 优先复用 xcarchive 里的 `Runner.app`，避免重复执行 `flutter build ios`（约省 2 分钟）

### 4. Android 签名

| Secret | 说明 |
|---|---|
| `ANDROID_SIGNING_KEY` | keystore 的 Base64（`base64 -w0 your.keystore`） |
| `ANDROID_ALIAS` | keystore 内的 alias |
| `ANDROID_KEY_STORE_PASSWORD` | store 密码 |
| `ANDROID_KEY_PASSWORD` | key 密码（PKCS12 要求与 store 密码**相同**） |

**两个必须知道的判据**：

1. **`Sign app APK` 步骤被 `skipped` ≠ 成功**。secret 为空时该步骤静默跳过，
   上传的是 **debug 签名 APK** —— 构建成功但**无法覆盖安装**。
   **判"真签没签"看该步骤是 success 还是 skipped。**
2. 报 `keystore password was incorrect`（`PKCS12KeyStore.engineLoad:2160`）
   = **store 密码不匹配**（不是 alias 错，alias 错走另一条代码路径）。
   已实测排除格式不兼容：cryptography/OpenSSL 产出的 PKCS12 可被 Java 17 正常加载；
   但**非 ASCII（中文）密码必定失败** → 密码须纯 ASCII。

CI 中每个 Android job 都有 `Verify signing configuration` 前置步骤，secrets 缺失或
密码/别名不匹配时**提前给出可读错误**，不再等 apksigner 抛模糊异常。

**本地自查**（脚本在**工作区**目录 `D:\T\OpenCode\github-repos\.rd-ci\`，不在本仓库内）：

```bash
cd D:/T/OpenCode/github-repos
"/c/program files/python312/python.exe" .rd-ci/verify-android-keystore.py \
    --keystore rustdesk.keystore --b64 rustdesk.keystore.b64
```

会报告密码是否可打开、keystore 内**真实 alias**、私钥位数、证书 SHA256
（可与 `apksigner verify --print-certs` 比对），并警告密码含首尾空白或非 ASCII。

> ⚠️ keystore 与密码**必须离线备份**（丢失则永远无法发布同签名更新）；
> **绝不提交进仓库** —— 公开仓库等于公开签名私钥。

### 5. 依赖的 fork

| 依赖 | 上游 | 本仓库指向 | 原因 |
|---|---|---|---|
| `libs/hbb_common`（子模块） | rustdesk/hbb_common | xxhhlk0/hbb_common | 编译期 `CUSTOM_*` 服务器配置注入 |
| `hwcodec`（cargo git 依赖） | rustdesk-org/hwcodec | xxhhlk0/hwcodec @ `d34f21d`（分支 `custom-1.4.9`） | 恢复 encoder preset 生效 + constant QP（CQ）码率控制 + 可选画质增强 + qsv 编码吞吐修复（`async_depth`、`low_power`/`low_delay_brc`）+ VRAM 编码参数透传（preset 1-7 / rc / QP / 厂商私有项） |

hwcodec fork 的改动（`cpp/common/util.{h,cpp}`、`cpp/ffmpeg_ram/ffmpeg_ram_{ffi.h,encode.cpp}`、
`cpp/ffmpeg_vram/ffmpeg_vram_{ffi.h,encode.cpp}`、`cpp/mfx/mfx_{ffi.h,encode.cpp}`、
`cpp/nv/nv_{ffi.h,encode.cpp}`、`cpp/amf/amf_{ffi.h,encode.cpp}`、`src/vram/{mod,inner,encode}.rs`）：

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
6. **把"实际生效的码控"打进日志**：QSV 没有 `rc` 选项，模式由 ffmpeg 的
   `qsvenc.c select_rc_mode` 从 AVCodecContext 字段推导，可能出现请求 CBR/CQ 而实际
   走 VBR/CQP 的静默失效，因此 `set_rate_control()` 会回读并打印实际模式、不一致时告警；
   `set_encode_enhance()` 在请求了编码器不支持的增强项时打印 `encode enhance ignored`；
   编码器日志补 `bit_rate`/`rc_max_rate`/`global_quality`，并在 `avcodec_open2` 后再打印一次
7. **qsv 编码吞吐修复**：上游把最低延迟参数写死（`async_depth=1`，且不设 VDENC 相关项），
   iGPU 无法重叠"取帧-编码-回读"，编码吞吐被腰斩。Intel UHD 750 @2560x1440
   （testsrc2 60fps / preset=veryfast / ICQ20）实测：
   `async_depth=1` 78~89fps、`=2` 99~116fps、`=1 + low_power=1` 56fps（单独开反而更慢）、
   **`=2 + low_power=1` 154fps（6.5ms/帧）← 采用**，并同时设 `low_delay_brc=1`
   （与 Sunshine 的 qsv 配置同思路）。改动前 60fps 会话会被编码耗时卡在 ~45fps
   （采集 5ms + 编码 14ms > 16.7ms 预算）。
   - `async_depth` 由 `hw_async_depth()` 提供（qsv 与 vaapi 共用），代价是多 1 帧管线延迟（60fps 下约 16ms）
   - `low_power`/`low_delay_brc` 只对 qsv 由 `apply_qsv_low_latency()` 下发；老核显/低版本驱动可能不支持，
     `avcodec_open2` 失败时会调用 `revert_qsv_low_latency()` 关掉这两项再重试一次，不会让会话建不起来
   - `do_encode()` 容忍首帧 `EAGAIN`（等包而不是立刻返回失败，否则首帧会被上层当成编码错误并切掉
     硬件编码器），交付完当前帧的包后即正常收工、不空等到超时
   - 三个参数都能用环境变量在运行期覆盖（改完重启 RustDesk 服务生效，无需重新构建）：
     `HWCODEC_ASYNC_DEPTH`（默认 2，设 1 退回上游行为）、`HWCODEC_QSV_LOW_POWER`（默认 1）、
     `HWCODEC_QSV_LOW_DELAY_BRC`（默认 1），设 `0` 即关闭
8. **VRAM 编码参数透传**：新增 `opts` 通道（`key=value;key=value`），
   `DynamicContext.opts` → 四个 `*_new_encoder()` FFI 新增 `const char *opts` 参数 →
   `util_encode::parse_opts()` / `has_opt()` / `opt_int()` / `opt_flag()` 拆表，
   各厂商只读自己认识的 key（未出现的 key 不下发，保持编码器默认值，即改动前的写死行为）
   - mfx：`TargetUsage = 8 - preset`（`MFX_TARGETUSAGE_BEST_QUALITY=1`…`BEST_SPEED=7`）、
     `rc` 1/2/3 → CBR/VBR/ICQ（ICQ 需 `q∈[1,51]`，否则回落 VBR）、`GopRefDist` 保持 1（低延迟）、
     `async_depth`、`num_ref_frame`、`low_power`、`cavlc`、`low_delay_brc`
   - nvenc：preset 1-7 → `NV_ENC_PRESET_P1..P7_GUID`、`tuning` 1/2/3 →
     ULTRA_LOW_LATENCY/LOW_LATENCY/HIGH_QUALITY、`rc` 1/2/3 → CBR/VBR/CONSTQP（越界回落 CBR）、
     `target_quality`、`spatial_aq`、`temporal_aq`、`multipass`、`lookahead_depth`、`num_ref_frame`
   - amf：AVC 与 HEVC 各一张 quality preset 表（两套枚举值不同）、`usage` 1/2/4、
     `rc` 1/2/3 → CBR/VBR/CQP、`vbaq`、`enforce_hrd`、`preanalysis`、`slices_per_frame`、
     `high_motion_qb`、`lowlatency_mode`、`input_queue_size`、`num_ref_frame`
     （HEVC 无 `MAX_NUM_REF_FRAMES`，该项只对 AVC 生效）
   - ffmpeg_vram：`preset` 按编码器名映射（nvenc p1-p7 / qsv veryfast-veryslow / amf speed-quality），
     qsv 额外支持 `low_power`/`low_delay_brc`/`async_depth`/`cavlc`
   - 三家原生路径各新增一行 `mfx/nvenc/amf encode params:` 日志


