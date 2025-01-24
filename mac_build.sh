#!/bin/bash

APP_NAME="Stegsolve"
APP_PATH="target/release/bundle/osx/${APP_NAME}.app"
BINARY_PATH="${APP_PATH}/Contents/MacOS/stegsolve-rs"
FRAMEWORKS_PATH="${APP_PATH}/Contents/Frameworks"


cargo build --release
cargo bundle --release

# 确保 Frameworks 目录存在
mkdir -p "${FRAMEWORKS_PATH}"


if [[ ! -f "${BINARY_PATH}" ]]; then
    echo "错误：未找到主程序 ${BINARY_PATH}，请检查路径或构建过程！"
    exit 1
fi

# 递归复制依赖的函数
function copy_dependencies {
    local lib=$1
    echo "处理依赖库: $lib"

    # 跳过已复制的动态库
    if [[ -f "${FRAMEWORKS_PATH}/$(basename "$lib")" ]]; then
        return
    fi

    # 复制动态库到 Frameworks
    cp "$lib" "${FRAMEWORKS_PATH}/"

    # 递归复制依赖
    for dep in $(otool -L "$lib" | grep /opt/homebrew | awk '{print $1}'); do
        copy_dependencies "$dep"
    done
}

# 复制主程序的直接依赖
echo "复制主程序的依赖..."
for dep in $(otool -L "${BINARY_PATH}" | grep /opt/homebrew | awk '{print $1}'); do
    copy_dependencies "$dep"
done

# 修复 Frameworks 中动态库的路径
echo "修复 Frameworks 中动态库的路径..."
for dylib in "${FRAMEWORKS_PATH}"/*.dylib; do
    echo "修正动态库路径: $dylib"

    # 修改动态库的自身 ID
    install_name_tool -id @rpath/$(basename "$dylib") "$dylib"

    # 修正动态库的依赖路径
    for dep in $(otool -L "$dylib" | grep /opt/homebrew | awk '{print $1}'); do
        dep_name=$(basename "$dep")
        install_name_tool -change "$dep" @rpath/"$dep_name" "$dylib"
    done
done

# 修复主程序的路径
echo "修复主程序的路径..."
install_name_tool -add_rpath @executable_path/../Frameworks "${BINARY_PATH}"
for dep in $(otool -L "${BINARY_PATH}" | grep /opt/homebrew | awk '{print $1}'); do
    dep_name=$(basename "$dep")
    install_name_tool -change "$dep" @rpath/"$dep_name" "$BINARY_PATH"
done

# 验证主程序的路径
echo "验证主程序路径..."
otool -L "${BINARY_PATH}"

# 验证 Frameworks 中的动态库路径
echo "验证动态库路径..."
for dylib in "${FRAMEWORKS_PATH}"/*.dylib; do
    otool -L "$dylib"
done

# 签名应用
echo "签名应用..."
codesign --deep --force --sign - "${APP_PATH}"

# 验证签名
echo "验证签名..."
codesign --verify --deep --strict --verbose=2 "${APP_PATH}"

echo "打包完成: ${APP_PATH}"
