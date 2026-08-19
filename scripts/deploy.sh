#!/bin/bash

set -euo pipefail

# ===============================
# IndexFlow Core Deploy Script
# ===============================

APP_NAME="indexflow-core"
BINARY_NAME="indexflow-core"

# 绝对对齐：基础路径在 /opt/indexflow/backend
BASE_DIR="/opt/indexflow/backend"
RELEASE_DIR="$BASE_DIR/releases"
CURRENT_LINK="$BASE_DIR/current"
SCRIPTS_DIR="/opt/indexflow/scripts"

PACKAGE="${1:-}"
KEEP_RELEASES=2

if [ -z "$PACKAGE" ]; then
    echo "Usage: $0 /path/to/release-package.tar.gz"
    exit 1
fi

# 1. 确保基础目录存在
mkdir -p "$RELEASE_DIR" "$SCRIPTS_DIR"

# 2. 备份当前部署脚本到 /opt/indexflow/scripts/
SCRIPT_PATH=$(readlink -f "$0")
if [ "$SCRIPT_PATH" != "$SCRIPTS_DIR/deploy.sh" ]; then
    cp "$SCRIPT_PATH" "$SCRIPTS_DIR/deploy.sh"
    chmod +x "$SCRIPTS_DIR/deploy.sh"
fi

VERSION=$(date +"%Y%m%d%H%M%S")
NEW_RELEASE="$RELEASE_DIR/$VERSION"

echo "=================================="
echo "Deploying IndexFlow Core"
echo "Version: $VERSION"
echo "=================================="

echo "[1/5] Creating release directory..."
mkdir -p "$NEW_RELEASE"

echo "[2/5] Extracting package..."
tar -xzf "$PACKAGE" -C "$NEW_RELEASE"

# 确保解压出来的二进制文件拥有可执行权限！
if [ -f "$NEW_RELEASE/$BINARY_NAME" ]; then
    chmod +x "$NEW_RELEASE/$BINARY_NAME"
fi

echo "[3/5] Updating current symlink..."
# 防御机制：如果发现有碍事的实体 current 文件夹，直接删除
if [ -d "$CURRENT_LINK" ] && [ ! -L "$CURRENT_LINK" ]; then
    rm -rf "$CURRENT_LINK"
fi
ln -sfn "releases/$VERSION" "$CURRENT_LINK"

echo "[4/5] Syncing systemd service..."
if [ -f "/tmp/$APP_NAME.service" ]; then
    cp "/tmp/$APP_NAME.service" "/etc/systemd/system/$APP_NAME.service"
    systemctl daemon-reload
    systemctl enable "$APP_NAME"
    rm -f "/tmp/$APP_NAME.service"
fi

echo "[5/5] Restarting service..."
systemctl restart "$APP_NAME"

echo "Waiting for service to start..."
sleep 3

if ! systemctl is-active --quiet "$APP_NAME"; then
    echo "ERROR: Service $APP_NAME failed to start!"
    systemctl status "$APP_NAME" --no-pager
    exit 1
fi

echo "Service OK"

# 清理历史旧版本
cd "$RELEASE_DIR"
ls -1dt */ | tail -n +$((KEEP_RELEASES+1)) | \
while read -r old
do
    echo "Removing old release: $old"
    rm -rf "$old"
done

# 清理临时包
rm -f "$PACKAGE"

echo "=================================="
echo "Deployment successful!"
echo "Current active release: $VERSION"
echo "=================================="