#!/bin/bash

# 定义仓库的基础URL和安装目标目录
REPO_URL="https://github.com/huahuadeliaoliao/RoseSong"
RELEASE_URL="https://github.com/huahuadeliaoliao/RoseSong/releases/download/v1.0.0"
DESTINATION="$HOME/.local/bin"

# 确保目标安装目录存在
mkdir -p $DESTINATION

# 检查操作系统和架构
OS=$(uname -s)
ARCH=$(uname -m)

if [[ "$OS" == "Linux" ]]; then
    ROSESONG_URL="$RELEASE_URL/rosesong"
    RSG_URL="$RELEASE_URL/rsg"
else
    echo "不支持的操作系统：$OS"
    exit 1
fi

# 使用curl下载RoseSong二进制文件
echo "从 $ROSESONG_URL 下载 RoseSong..."
curl -L $ROSESONG_URL -o "$DESTINATION/rosesong"

echo "从 $RSG_URL 下载 rsg..."
curl -L $RSG_URL -o "$DESTINATION/rsg"

# 给予rsg文件执行权限
echo "为 rsg 设置执行权限..."
chmod +x "$DESTINATION/rsg"
chmod +x "$DESTINATION/rosesong"

# 检查rsg是否成功安装
if [ -f "$DESTINATION/rsg" ]; then
    echo "rsg 安装成功。你现在可以通过输入rsg相关命令来使用RoseSong。"
    echo "如果 $HOME/.local/bin 不在你的 PATH 中，请记得添加。"
    echo "你可以在 .bashrc 或 .zshrc 中添加以下行："
    echo 'export PATH="$HOME/.local/bin:$PATH"'
else
    echo "安装 rsg 失败。请检查上述错误。"
fi
