# HiCOM - 串口调试助手

基于 Rust + egui 构建的跨平台串口调试工具。

## 功能特性

- 串口收发 (ASCII/HEX)
- 多种串口参数配置 (波特率/数据位/停止位/校验/流控)
- 定时发送 / HEX发送 / 换行设置
- 接收数据实时波形显示
- 接收区搜索定位
- 亮色/暗色主题切换
- 日志保存 (支持选择保存目录)
- 跨平台支持 (Windows / Linux / macOS)

## 截图

## HiCOM 主界面 <img width="962" height="732" alt="image" src="https://github.com/user-attachments/assets/8f37839f-5e9d-484b-9692-4a50f491d76d" />


## 下载

从 [Releases](https://github.com/HaydenHu/hicom-rust/releases) 下载最新版本。

### Windows

直接运行 `hicom_v1.0_windows_x86_64.exe` 即可，无需安装。

### Linux / macOS

```bash
cargo build --release
./target/release/hicom
```

## 编译

需要安装 Rust 工具链：

```bash
git clone https://github.com/HaydenHu/hicom-rust.git
cd hicom-rust
cargo build --release
```

编译产物在 `target/release/hicom`（或 `hicom.exe`）。

## 技术栈

- [Rust](https://www.rust-lang.org/)
- [egui](https://github.com/emilk/egui) - 即时模式 GUI 框架
- [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) - egui 应用框架
- [serialport](https://github.com/serialport/serialport-rs) - 串口通信库

## 作者

**Hayden** ([GitHub](https://github.com/HaydenHu)) · hayhi@qq.com

## License

MIT
