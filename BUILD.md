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
| quality 画质 | Medium (nvenc p4 / qsv medium / amf balanced) | VBR | 240 | 画质优先 |
| custom 自定义 | 逐项 | 逐项 | 逐项 | 码率/QP/FPS/GOP/自适应开关 |

- 配置存储：全局 option `hw-encode-profile`（预置档 id 或 JSON），修改后对新会话生效
- 按客户端覆盖：PeerConfig option `hw-encode-profile`（按 peer id），
  最近连接的带覆盖客户端生效，其断开后回落全局默认
- `bitrate_adaptive=false` 时抑制 VideoQoS 运行期动态码率调整
- 录制会话的 240 帧关键帧间隔优先于 profile 的 GOP 覆盖
- VRAM 通道（GPU 纹理直达）仅支持 码率/FPS/GOP；preset 与 rc 仅作用于 RAM 硬编路径
  （hwcodec C 库写死）
