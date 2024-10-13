
<p align="center">
    <img src="img/RoseSong.png" width="200" height="200" alt="RoseSong Logo">
</p>

<h1 align="center">RoseSong</h1>

# 简介

**RoseSong** 是一个基于 Rust 构建的命令行播放器，依赖 GStreamer 解码播放 Bilibili 音频。它通过 D-Bus 进行进程间通信，可以快速导入 B 站收藏夹（注意：目标收藏夹在导入时需要设置为公开状态）。

---
<details>
  <summary><strong style="font-size: 1.5em;">安装说明</strong></summary>

## 1. Linux 系统

### 1.1 Debian/Ubuntu 用户
你可以直接下载 [Release 页面](https://github.com/huahuadeliaoliao/RoseSong/releases) 中提供的 `.deb` 文件进行安装。

### 1.2 其他 Linux 发行版
- RoseSong 依赖 GStreamer 和 D-Bus，绝大多数 Linux 系统默认已经安装这些依赖。如果运行遇到问题，请确保这两个依赖项已经安装。
- 使用以下命令安装 RoseSong，这将会把 `rosesong` 和 `rsg` 二进制可执行文件（仅支持 Linux amd64）安装到当前用户的 `.local/bin` 目录中：
  
```bash
curl -s https://raw.githubusercontent.com/huahuadeliaoliao/RoseSong/main/installation_script/install_rosesong.sh | bash
```

- 也可以直接使用cargo安装RoseSong：

```bash
cargo install rosesong
```

## 2. MacOS
- 如果安装了GStreamer和D-Bus可以使用cargo安装RoseSong

## 3. Windows
- **暂不支持**

</details>

---

# 构建说明

在 Linux 上构建 RoseSong 需要安装 Rust 以及 [GStreamer 开发包](https://gstreamer.freedesktop.org/documentation/installing/on-linux.html?gi-language=c#)。构建命令如下：

```bash
cargo b --release
```

构建完成后的二进制文件位于 `target/release` 目录下。

---

# PR 贡献指南

感谢您对 RoseSong 项目的贡献！

- 本项目使用 `cargo clippy` 管理代码质量及风格。在提交 PR 之前，请确保 `cargo clippy` 没有任何警告或错误信息。
- 提交前，使用 `cargo fmt` 统一格式化代码。

再次感谢您的贡献！

---

<details>
  <summary><strong style="font-size: 1.5em;">使用示例</strong></summary>

## 基本命令

- 使用 `rsg -h` 获取帮助信息：

<p align="center">
    <img src="img/v1.0.0rsg-h.png" width="350" height="400" alt="rsg -h help">
</p>

- 使用 `rsg add -f fid` 通过 fid 导入收藏夹（fid 是 B 站收藏夹网址中的数字，导入收藏夹可能需要等待一段时间）：

<p align="center">
    <img src="img/v1.0.0rsg-add-playlist.png" width="600" height="320" alt="rsg add playlist">
</p>

- 使用 `rsg add -b bvid` 通过 bvid 导入歌曲（bvid 是 B 站视频网址中的 BV 开头的字符串）：

<p align="center">
    <img src="img/v1.0.0rsg-add-b.png" width="260" height="90" alt="rsg add bvid">
</p>

- 使用 `rsg delete` 删除导入的歌曲：

<p align="center">
    <img src="img/v1.0.0rsg-delete.png" width="300" height="280" alt="rsg delete">
</p>

- 使用 `rsg find` 查找导入歌曲的信息：

<p align="center">
    <img src="img/v1.0.0rsg-find.png" width="300" height="280" alt="rsg find">
</p>

</details>

---

# 版本历史

## 版本 1.0.0
- [查看版本信息](https://github.com/huahuadeliaoliao/RoseSong/releases/tag/v1.0.0)

---
