#!/bin/bash
#
# create-ios-ipa.sh — 从已构建的 Runner.app 产出两个 ipa
#
#   1) rustdesk-<VERSION>-ios-unsigned.ipa   纯未签名
#      用途: AltStore / Sideloadly 等「用自己的 Apple ID 重签」的场景
#
#   2) rustdesk-<VERSION>-ios-jailbroken.ipa 已 ldid 伪签名 + 注入 entitlements
#      用途: 越狱设备 (AppSync Unified) 直接安装; TrollStore 亦可
#            (TrollStore 安装时会用假根证书重签, 但会保留 ldid 注入的 entitlements)
#
# 为什么需要第 2 个: no-codesign 构建出的 app 完全没有签名块, installd 会以
#   "Application is missing the application-identifier entitlement"
# 拒绝安装 (Apple TN2319)。注入 application-identifier 后即可安装。
#
# 参考实现: minh-ton/reynard-browser 的 tools/release/create-ipa.sh
#   (同样是 一次 archive -> 分别产出未签名 / 越狱版, 差异只在 ldid 注入)
#
# 用法 (在仓库任意位置均可):
#   VERSION=1.4.9 bash .github/scripts/create-ios-ipa.sh
#
# 可用环境变量覆盖:
#   VERSION           产物文件名里的版本号 (默认 0.0.0)
#   APP_DIR           直接指定 Runner.app 路径 (默认自动在 flutter/build/ios 下查找)
#   ENT_FILE          越狱版注入的 entitlements
#                     (默认 flutter/ios/Runner/Runner.private.entitlements)
#   OUT_DIR           输出目录 (默认 flutter/build/ios/ipa)
#   SIGN_FRAMEWORKS   1 = 对 app 内「未签名」的 Mach-O 补 ad-hoc 签名 (默认 1)。
#                     只处理 codesign --verify 失败的那些，已有有效签名的原样保留。

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
FLUTTER_DIR="$REPO_ROOT/flutter"

VERSION="${VERSION:-0.0.0}"
ENT_FILE="${ENT_FILE:-$FLUTTER_DIR/ios/Runner/Runner.private.entitlements}"
OUT_DIR="${OUT_DIR:-$FLUTTER_DIR/build/ios/ipa}"
SIGN_FRAMEWORKS="${SIGN_FRAMEWORKS:-1}"

cd "$FLUTTER_DIR"
mkdir -p "$OUT_DIR"
OUT_DIR="$(cd "$OUT_DIR" && pwd)"

# ---------------------------------------------------------------- 1. 找 Runner.app
if [ -z "${APP_DIR:-}" ]; then
  APP_DIR="$(ls -d build/ios/archive/Runner.xcarchive/Products/Applications/Runner.app 2>/dev/null \
             || ls -d build/ios/iphoneos/Runner.app 2>/dev/null \
             || ls -d build/ios/Release-iphoneos/Runner.app 2>/dev/null || true)"
fi
if [ -z "$APP_DIR" ] || [ ! -d "$APP_DIR" ]; then
  echo "ERROR: 未找到 Runner.app (试过 xcarchive / iphoneos / Release-iphoneos)"
  ls -la build/ios/ 2>/dev/null || true
  exit 1
fi
APP_DIR="$(cd "$(dirname "$APP_DIR")" && pwd)/$(basename "$APP_DIR")"
MAIN_BIN="$APP_DIR/Runner"

echo "APP_DIR         = $APP_DIR"
echo "MAIN_BIN        = $MAIN_BIN"
echo "ENT_FILE        = $ENT_FILE"
echo "OUT_DIR         = $OUT_DIR"
echo "VERSION         = $VERSION"
echo "SIGN_FRAMEWORKS = $SIGN_FRAMEWORKS"
echo

if [ ! -f "$MAIN_BIN" ]; then
  echo "ERROR: 缺少主二进制 $MAIN_BIN"
  exit 1
fi

# ---------------------------------------------------------------- 2. 工具检查
if ! command -v ldid >/dev/null 2>&1; then
  echo "ERROR: 未找到 ldid (macOS: brew install ldid)"
  exit 1
fi
if ! command -v zip >/dev/null 2>&1; then
  echo "ERROR: 未找到 zip"
  exit 1
fi
echo "ldid: $(command -v ldid)"

# ---------------------------------------------------------------- 3. 打包函数
# $1 = 输出文件名
# $2 = yes 时注入 entitlements (越狱版)
make_ipa() {
  local name="$1"
  local inject="$2"
  local out="$OUT_DIR/$name"
  local work
  local pkg_bin
  work="$(mktemp -d /tmp/ios_ipa.XXXXXX)"
  pkg_bin="$work/Payload/Runner.app/Runner"

  rm -f "$out"
  mkdir -p "$work/Payload"
  cp -R "$APP_DIR" "$work/Payload/"

  if [ "$inject" = "yes" ]; then
    if [ ! -f "$ENT_FILE" ]; then
      echo "ERROR: 缺少 entitlements 文件 $ENT_FILE"
      rm -rf "$work"
      exit 1
    fi

    # 主二进制: ad-hoc fakesign + 注入 entitlements。
    # ldid -S<plist> 会把 entitlements 写进签名块, TrollStore 安装时重签会保留它。
    ldid -S"$ENT_FILE" "$pkg_bin"

    echo "== $name: Runner 注入的 entitlements =="
    ldid -e "$pkg_bin" 2>/dev/null | sed 's/^/   /' || true

    # 其余 Mach-O: 只给「没有有效签名」的补一个 ad-hoc 签名。
    # 已有有效签名的原样保留, 避免破坏 Flutter / Xcode 的产物。
    if [ "$SIGN_FRAMEWORKS" = "1" ]; then
      local n=0
      local bin
      while IFS= read -r bin; do
        if [ "$bin" = "$pkg_bin" ]; then
          continue
        fi
        if ! file -b "$bin" 2>/dev/null | grep -q '^Mach-O'; then
          continue
        fi
        if codesign --verify "$bin" >/dev/null 2>&1; then
          continue
        fi
        if ldid -S "$bin" 2>/dev/null; then
          n=$((n + 1))
        fi
      done < <(find "$work/Payload/Runner.app" -type f)
      echo "   另有 $n 个未签名的 Mach-O 已补 ad-hoc 签名"
    fi

    # 自证: 签名块与 entitlements 确实写进去了
    echo "== $name: codesign 自证 =="
    codesign -dv --entitlements - "$pkg_bin" 2>&1 | sed 's/^/   /' || true
  fi

  ( cd "$work" && zip -qry "$out" Payload -x '._*' -x '.DS_Store' -x '__MACOSX' )
  rm -rf "$work"

  echo "--> $name  ($(du -h "$out" | cut -f1))"
  echo
}

# ---------------------------------------------------------------- 4. 产出两个版本
make_ipa "rustdesk-${VERSION}-ios-unsigned.ipa" no
make_ipa "rustdesk-${VERSION}-ios-jailbroken.ipa" yes

# ---------------------------------------------------------------- 5. 汇总
echo "== $OUT_DIR =="
ls -la "$OUT_DIR"
