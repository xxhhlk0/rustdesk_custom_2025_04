# RustDesk 自定义构建说明

## 概述

本项目基于官方 RustDesk 1.4.6 版本进行自定义构建，所有构建产物上传到 Cloudflare R2 私有存储，不依赖 GitHub Artifacts，适合公开仓库使用。

## 主要修改

### 1. 工作流同步与简化

- 从官方 rustdesk/rustdesk 1.4.6 版本同步所有 workflow 文件
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
└── 1.4.6/{DATE}/                 # 最终产物（长期保存）
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
aws s3 ls s3://{R2_BUCKET}/1.4.6/ --endpoint-url {R2_ENDPOINT_URL} --region auto

# 下载文件
aws s3 cp s3://{R2_BUCKET}/1.4.6/20260425/windows/x86_64/rustdesk-1.4.6-x86_64.exe . \
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
