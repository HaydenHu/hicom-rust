use std::fmt::Write;
use std::io::Read;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::{Color32, CornerRadius, FontData, FontFamily, Frame, Margin, TextEdit, Vec2};

mod color {
    use egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(30, 30, 34);
    pub const PANEL: Color32 = Color32::from_rgb(38, 38, 44);
    pub const ACCENT: Color32 = Color32::from_rgb(70, 130, 220);
    pub const GREEN: Color32 = Color32::from_rgb(80, 200, 80);
    pub const RED: Color32 = Color32::from_rgb(220, 70, 70);
    pub const YELLOW: Color32 = Color32::from_rgb(220, 170, 40);
    pub const TEXT: Color32 = Color32::from_rgb(220, 220, 230);
    pub const DIM: Color32 = Color32::from_rgb(100, 100, 110);
}

enum PortCmd {
    Write(Vec<u8>),
    SetDtr(bool), SetRts(bool),
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)] enum DataBits { Five, Six, Seven, Eight }
impl DataBits {
    fn ser(self) -> serialport::DataBits { match self { DataBits::Five => serialport::DataBits::Five, DataBits::Six => serialport::DataBits::Six, DataBits::Seven => serialport::DataBits::Seven, DataBits::Eight => serialport::DataBits::Eight } }
}
impl std::fmt::Display for DataBits { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match self { DataBits::Five => 5, DataBits::Six => 6, DataBits::Seven => 7, DataBits::Eight => 8 }) } }

#[derive(Clone, Copy, PartialEq, Eq)] enum StopBits { One, Two }
impl StopBits { fn ser(self) -> serialport::StopBits { match self { StopBits::One => serialport::StopBits::One, StopBits::Two => serialport::StopBits::Two } } }
impl std::fmt::Display for StopBits { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match self { StopBits::One => 1, StopBits::Two => 2 }) } }

#[derive(Clone, Copy, PartialEq, Eq)] enum Parity { None, Odd, Even }
impl Parity { fn ser(self) -> serialport::Parity { match self { Parity::None => serialport::Parity::None, Parity::Odd => serialport::Parity::Odd, Parity::Even => serialport::Parity::Even } } }
impl std::fmt::Display for Parity { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match self { Parity::None => "None", Parity::Odd => "Odd", Parity::Even => "Even" }) } }

#[derive(Clone, Copy, PartialEq, Eq)] enum FlowCtrl { None, Hardware, Software }
impl FlowCtrl { fn ser(self) -> serialport::FlowControl { match self { FlowCtrl::None => serialport::FlowControl::None, FlowCtrl::Hardware => serialport::FlowControl::Hardware, FlowCtrl::Software => serialport::FlowControl::Software } } }
impl std::fmt::Display for FlowCtrl { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match self { FlowCtrl::None => "无", FlowCtrl::Hardware => "RTS/CTS", FlowCtrl::Software => "XON/XOFF" }) } }

#[derive(Clone, Copy, PartialEq, Eq)] enum Newline { None, CrLf, Cr, Lf }
impl std::fmt::Display for Newline { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match self { Newline::None => "无", Newline::CrLf => "\\r\\n", Newline::Cr => "\\r", Newline::Lf => "\\n" }) } }

#[derive(Clone, Copy, PartialEq, Eq)] enum View { Ascii, Hex }

// ── 共享接收缓冲区 ──
// 后台线程只写入原始字节，UI 线程取走后再格式化
struct RxBuf {
    raw: Vec<u8>,
    bytes: u64,
}

struct HicomApp {
    font_ok: bool,
    port_open: Arc<Mutex<bool>>,
    port_tx: Option<mpsc::Sender<PortCmd>>,
    rx: Arc<Mutex<RxBuf>>,
    names: Vec<String>,
    sel_port: String, baud: String,
    db: DataBits, sb: StopBits, par: Parity, fc: FlowCtrl,
    dtr: bool, rts: bool,
    view: View,
    txt: String, hex: String,
    rx_n: u64, tx_n: u64,
    ts: bool, paused: bool,
    send: String, hexmd: bool, nl: Newline,
    auto: bool, auto_t: String, auto_acc: f32,
    msg: String, msg_timer: f32,
    was_on: bool,
}

impl HicomApp {
    fn new() -> Self {
        let names = available_ports();
        let sel = names.iter().find(|n| n.contains('[')).cloned().or_else(|| names.first().cloned()).unwrap_or_default();
        Self {
            font_ok: false, port_open: Arc::new(Mutex::new(false)), port_tx: None,
            rx: Arc::new(Mutex::new(RxBuf { raw: Vec::with_capacity(65536), bytes: 0 })),
            names, sel_port: sel, baud: "115200".into(), db: DataBits::Eight, sb: StopBits::One, par: Parity::None, fc: FlowCtrl::None,
            dtr: true, rts: true, view: View::Ascii, txt: String::new(), hex: String::new(), rx_n: 0, tx_n: 0,
            ts: true, paused: false,
            send: String::new(), hexmd: false, nl: Newline::CrLf,
            auto: true, auto_t: "200".into(), auto_acc: 0.0,
            msg: "就绪".into(), msg_timer: 0.0,
            was_on: false,
        }
    }

    // UI 线程：每帧只处理最多 65536 字节，保证实时性
    fn drain_rx(&mut self) {
        let mut rx = self.rx.lock().unwrap();
        if rx.bytes == 0 || rx.raw.is_empty() { return; }
        let n = rx.raw.len().min(65536);
        let data: Vec<u8> = rx.raw.drain(..n).collect();
        rx.bytes = rx.bytes.saturating_sub(n as u64);
        // 如果堆积太多，直接丢弃旧数据
        if rx.raw.len() > 512 * 1024 { let keep = rx.raw.len() - 256 * 1024; rx.raw.drain(..keep); }
        drop(rx);

        self.rx_n += n as u64;
        let cap = 256 * 1024;
        let max_per_frame = n;

        // 时间戳（只在每批数据开头加一次）
        if self.ts && max_per_frame > 0 { let t = ts(); self.txt.push_str(&format!("[{}] ", t)); self.hex.push_str(&format!("[{}] ", t)); }

        // ASCII
        self.txt.reserve(max_per_frame);
        for &b in &data {
            self.txt.push(if b.is_ascii_graphic() || b == b' ' || b == b'\n' || b == b'\r' || b == b'\t' { b as char } else { '.' });
        }
        if self.txt.len() > cap { let k = self.txt.len() - cap / 2; self.txt.drain(..k); }

        // HEX
        self.hex.reserve(max_per_frame * 3);
        for b in &data {
            write!(&mut self.hex, "{:02X} ", b).unwrap();
        }
        if self.hex.len() > cap { let k = self.hex.len() - cap / 2; self.hex.drain(..k); }
    }

    fn refresh(&mut self) { self.names = available_ports(); }

    fn toggle(&mut self) {
        let on = *self.port_open.lock().unwrap();
        if on {
            if let Some(tx) = self.port_tx.take() { let _ = tx.send(PortCmd::Close); }
            *self.port_open.lock().unwrap() = false; self.msg = "已关闭".into(); self.msg_timer = 2.0; self.tx_n = 0;
        } else {
            if self.sel_port.is_empty() { self.msg = "请选择串口".into(); self.msg_timer = 2.0; return; }
            let b: u32 = match self.baud.trim().parse() { Ok(v) => v, Err(_) => { self.msg = "波特率无效".into(); self.msg_timer = 2.0; return; } };
            let nm = port_name_only(&self.sel_port);
            let (tx, rx) = mpsc::channel::<PortCmd>();
            let shared = self.rx.clone(); let op = self.port_open.clone();
            let db = self.db.ser(); let sb = self.sb.ser(); let p = self.par.ser(); let fc = self.fc.ser();

            match serialport::new(&nm, b).data_bits(db).stop_bits(sb).parity(p).flow_control(fc).timeout(Duration::from_millis(10)).open() {
                Ok(mut port) => {
                    let _ = port.write_data_terminal_ready(self.dtr); let _ = port.write_request_to_send(self.rts);
                    self.port_tx = Some(tx); *op.lock().unwrap() = true; self.msg = format!("已连接 {}", nm); self.msg_timer = 0.0;

                    // ── 后台线程：只读写原始字节，不做格式化 ──
                    std::thread::spawn(move || {
                        let mut rbuf = [0u8; 8192];
                        loop {
                            match port.read(&mut rbuf) {
                                Ok(n) if n > 0 => {
                                    let mut buf = shared.lock().unwrap();
                                                    buf.raw.extend_from_slice(&rbuf[..n]);
                                    buf.bytes += n as u64;
                                    if buf.raw.len() > 512 * 1024 {
                                        let k = buf.raw.len() - 256 * 1024;
                                        buf.raw.drain(..k);
                                    }
                                }
                                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                                Err(_) => { *op.lock().unwrap() = false; break; }
                                _ => {}
                            }
                            match rx.try_recv() {
                                Ok(PortCmd::Write(d)) => { let _ = port.write_all(&d); let _ = port.flush(); continue; }
                                Ok(PortCmd::SetDtr(v)) => { let _ = port.write_data_terminal_ready(v); continue; }
                                Ok(PortCmd::SetRts(v)) => { let _ = port.write_request_to_send(v); continue; }
                                Ok(PortCmd::Close) | Err(mpsc::TryRecvError::Disconnected) => { *op.lock().unwrap() = false; break; }
                                Err(mpsc::TryRecvError::Empty) => {}
                            }
                        }
                    });
                }
                Err(e) => { self.msg = format!("打开失败: {}", e); self.msg_timer = 2.0; }
            }
        }
    }

    fn mk_nl(d: &[u8], nl: Newline) -> Vec<u8> { let mut v = d.to_vec(); match nl { Newline::CrLf => v.extend_from_slice(b"\r\n"), Newline::Cr => v.push(b'\r'), Newline::Lf => v.push(b'\n'), Newline::None => {} } v }

    fn mk_data(&self) -> Result<Vec<u8>, String> {
        let d = if self.hexmd {
            let h: String = self.send.chars().filter(|c| !c.is_whitespace()).collect();
            if h.is_empty() { return Ok(Vec::new()); }
            if h.len() % 2 != 0 { return Err("HEX 长度须为偶数".into()); }
            let mut v = Vec::with_capacity(h.len() / 2);
            for i in (0..h.len()).step_by(2) { match u8::from_str_radix(&h[i..i + 2], 16) { Ok(b) => v.push(b), Err(_) => return Err(format!("无效 HEX: {}", &h[i..i + 2])) } }
            v
        } else { self.send.as_bytes().to_vec() };
        Ok(Self::mk_nl(&d, self.nl))
    }

    fn do_send(&mut self) {
        if !*self.port_open.lock().unwrap() { self.msg = "未打开串口".into(); self.msg_timer = 2.0; return; }
        let d = match self.mk_data() { Ok(v) => v, Err(e) => { self.msg = e; self.msg_timer = 2.0; return; } };
        if d.is_empty() { return; }
        let n = d.len();
        if let Some(tx) = &self.port_tx { match tx.send(PortCmd::Write(d)) { Ok(_) => { self.tx_n += n as u64; self.msg = format!("已发送 {} 字节", n); self.msg_timer = 1.5; } Err(e) => { self.msg = format!("发送失败: {}", e); self.msg_timer = 2.0; } } }
    }

    fn auto_tick(&mut self) {
        let d = match self.mk_data() { Ok(v) => v, Err(_) => return }; if d.is_empty() { return; }
        if let Some(tx) = &self.port_tx { let n = d.len(); let _ = tx.send(PortCmd::Write(d)); self.tx_n += n as u64; }
    }

    fn clr(&mut self) { self.txt.clear(); self.hex.clear(); self.rx_n = 0; }
    fn dtr_set(&mut self, v: bool) { self.dtr = v; if let Some(tx) = &self.port_tx { let _ = tx.send(PortCmd::SetDtr(v)); } }
    fn rts_set(&mut self, v: bool) { self.rts = v; if let Some(tx) = &self.port_tx { let _ = tx.send(PortCmd::SetRts(v)); } }

    fn save_log(&mut self) {
        let content = if self.view == View::Ascii { self.txt.clone() } else { self.hex.clone() };
        let path = format!("hicom_log_{}.txt", chrono_now());
        match std::fs::write(&path, content) { Ok(_) => { self.msg = format!("已保存: {}", path); self.msg_timer = 2.0; } Err(e) => { self.msg = format!("保存失败: {}", e); self.msg_timer = 2.0; } }
    }
}

fn port_name_only(display: &str) -> String { display.split_whitespace().next().unwrap_or(display).to_string() }

fn available_ports() -> Vec<String> {
    serialport::available_ports().map(|p| {
        let mut seen = std::collections::HashSet::new();
        p.into_iter().filter_map(|x| {
            let name = &x.port_name;
            // 相同端口名只保留第一个（优先带详细信息）
            if !seen.insert(name.clone()) { return None; }
            let display = match &x.port_type {
                serialport::SerialPortType::UsbPort(info) => {
                    let mfr = info.manufacturer.as_deref().unwrap_or("").trim();
                    let prod = info.product.as_deref().unwrap_or("").trim();
                    // 去掉制造商/产品末尾的端口名及括号包裹，避免重复显示（如 "WCH-Link SERIAL (COM23)" 中的 (COM23)）
                    let strip_port = |s: &str| -> String {
                        let s = s.trim_end();
                        let pn = name.trim();
                        // 去掉 " (COM23)" 或 "(COM23)" 或 " COM23" 结尾
                        let patterns = [
                            format!(" ({})", pn),
                            format!("({})", pn),
                            format!(" {}", pn),
                        ];
                        let mut r = s.to_string();
                        for pat in &patterns {
                            if r.ends_with(pat) { r = r[..r.len() - pat.len()].trim_end().to_string(); break; }
                        }
                        r
                    };
                    let mfr = strip_port(mfr);
                    let prod = strip_port(prod);
                    if !mfr.is_empty() || !prod.is_empty() { format!("{}  [{}{}{}]", name, mfr, if !mfr.is_empty() && !prod.is_empty() { " " } else { "" }, prod) } else { name.clone() }
                }
                serialport::SerialPortType::BluetoothPort => format!("{}  [蓝牙]", name),
                serialport::SerialPortType::PciPort => format!("{}  [PCI]", name),
                _ => name.clone(),
            };
            Some(display)
        }).collect()
    }).unwrap_or_default()
}

fn ts() -> String {
    use std::time::SystemTime;
    if let Ok(d) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) { let s = d.as_secs(); format!("{:02}:{:02}:{:02}.{:03}", (s / 3600) % 24, (s / 60) % 60, s % 60, d.subsec_millis()) } else { "--:--:--.---".into() }
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    if let Ok(d) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) { let s = d.as_secs(); format!("{:04}{:02}{:02}_{:02}{:02}{:02}", 1970 + s / 31556952, (s % 31556952) / 2629744 + 1, (s % 2629744) / 86400 + 1, (s / 3600) % 24, (s / 60) % 60, s % 60) } else { "unknown".into() }
}

fn fmtsz(n: u64) -> String { if n < 1024 { format!("{} B", n) } else if n < 1048576 { format!("{:.1} KB", n as f64 / 1024.0) } else { format!("{:.2} MB", n as f64 / 1048576.0) } }

fn combo<T: Clone + PartialEq + std::fmt::Display>(ui: &mut egui::Ui, id: &str, v: &mut T, opts: &[T], w: f32) {
    egui::ComboBox::from_id_salt(id).width(w).selected_text(v.to_string()).show_ui(ui, |ui| { for o in opts { ui.selectable_value(v, o.clone(), o.to_string()); } });
}

fn panel_frame() -> Frame { Frame { inner_margin: Margin::symmetric(8, 6), fill: color::PANEL, corner_radius: CornerRadius::same(6), ..Default::default() } }

impl eframe::App for HicomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals { dark_mode: true, panel_fill: color::PANEL, window_fill: color::PANEL, ..Default::default() });

        if !self.font_ok {
            let mut fs = egui::FontDefinitions::default();
            if let Ok(d) = std::fs::read("C:/Windows/Fonts/NotoSansSC-VF.ttf") {
                fs.font_data.insert("noto".into(), FontData::from_owned(d).into());
                for f in [FontFamily::Proportional, FontFamily::Monospace] { if let Some(v) = fs.families.get_mut(&f) { v.insert(0, "noto".into()); } }
            }
            ctx.set_fonts(fs); self.font_ok = true;
        }

        self.drain_rx();
        let on = *self.port_open.lock().unwrap();

        let dt = ctx.input(|i| i.stable_dt);
        if self.msg_timer > 0.0 { self.msg_timer -= dt; if self.msg_timer <= 0.0 { self.msg.clear(); } }

        if self.auto && on && !self.send.is_empty() {
            let iv = (self.auto_t.trim().parse::<f32>().unwrap_or(200.0).max(10.0)) / 1000.0;
            self.auto_acc += dt;
            if self.auto_acc >= iv { self.auto_acc -= iv; self.auto_tick(); }
        }

        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter)) && on && !self.send.is_empty() { self.do_send(); }

        // 有数据待处理时高频刷新，提升实时性
        if on { ctx.request_repaint_after(Duration::from_millis(20)); }

        // ═══ 一个 CentralPanel，内部用 vertical 分三块 ═══
        egui::CentralPanel::default()
            .frame(Frame::NONE.inner_margin(Margin::symmetric(8, 8)))
            .show(ctx, |ui| {
            ui.vertical(|ui| {
                // ── 第一块：串口配置 ──
                Frame { fill: color::PANEL, corner_radius: CornerRadius::same(6), inner_margin: Margin::symmetric(8, 6), ..Default::default() }
                    .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (lbl, clr) = if on { ("关闭", color::RED) } else { ("打开", color::GREEN) };
                        if ui.add_sized([60.0, 24.0], egui::Button::new(egui::RichText::new(lbl).color(Color32::WHITE)).fill(clr).corner_radius(6)).clicked() { self.toggle(); }
                        if on { let pn = port_name_only(&self.sel_port); ui.colored_label(color::GREEN, "● 已连接"); ui.colored_label(color::TEXT, format!("{} @ {} {} {} {}", pn, self.baud, self.db, self.sb, self.par)); }
                        else { ui.colored_label(color::DIM, "○ 未连接"); }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { if ui.small_button("刷新").clicked() { self.refresh(); } });
                    });
                    let force_open = if self.was_on != on { Some(!on) } else { None };
                    egui::CollapsingHeader::new("串口设置").id_salt("cfg").default_open(!on).open(force_open).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(!on, |ui| { ui.label("端口"); combo(ui, "p", &mut self.sel_port, &self.names.clone(), 160.0); });
                            ui.separator();
                            ui.add_enabled_ui(!on, |ui| { ui.label("波特率"); combo(ui, "b", &mut self.baud, &["300","1200","2400","4800","9600","19200","38400","57600","115200","230400","460800","921600"].map(|s| s.to_string()), 90.0); });
                            ui.separator();
                            ui.add_enabled_ui(!on, |ui| { ui.label("数据"); combo(ui, "D", &mut self.db, &[DataBits::Eight, DataBits::Seven, DataBits::Six, DataBits::Five], 40.0); ui.label("停止"); combo(ui, "S", &mut self.sb, &[StopBits::One, StopBits::Two], 35.0); ui.label("校验"); combo(ui, "P", &mut self.par, &[Parity::None, Parity::Odd, Parity::Even], 60.0); ui.label("流控"); combo(ui, "F", &mut self.fc, &[FlowCtrl::None, FlowCtrl::Hardware, FlowCtrl::Software], 70.0); });
                            ui.separator();
                            ui.add_enabled_ui(on, |ui| { ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0); let mut d = self.dtr; if ui.checkbox(&mut d, "DTR").changed() { self.dtr_set(d); } let mut r = self.rts; if ui.checkbox(&mut r, "RTS").changed() { self.rts_set(r); } });
                        });
                    });
                });
                // 跟踪连接状态变化
                self.was_on = on;

                ui.add_space(4.0);

                // ── 第二块：接收区（填满剩余空间，但为发送区留出固定高度） ──
                let rx_avail_h = ui.available_height().max(100.0) - 185.0;
                Frame { fill: color::PANEL, corner_radius: CornerRadius::same(6), inner_margin: Margin::symmetric(8, 6), ..Default::default() }
                    .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(color::TEXT, egui::RichText::new("接收数据").size(13.0));
                        if self.paused { ui.colored_label(color::YELLOW, "[已暂停]"); }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.selectable_label(self.view == View::Ascii, "ASCII").clicked() { self.view = View::Ascii; }
                            if ui.selectable_label(self.view == View::Hex, "HEX").clicked() { self.view = View::Hex; }
                            ui.separator();
                            if ui.small_button("保存日志").clicked() { self.save_log(); }
                            if ui.small_button("清空").clicked() { self.clr(); }
                            ui.checkbox(&mut self.ts, "时间戳");
                            ui.checkbox(&mut self.paused, "暂停");
                        });
                    });
                    ui.separator();
                    // 接收区填满剩余垂直空间但不超过 rx_avail_h
                    let rx_id = ui.next_auto_id();
                    let rx_bg_rect = egui::Rect::from_min_size(
                        ui.next_widget_position(),
                        egui::vec2(ui.available_width(), rx_avail_h),
                    );
                    // 提前画接收区的黑色背景（固定在 ScrollArea 后面的层）
                    ui.painter().rect_filled(rx_bg_rect, CornerRadius::ZERO, Color32::BLACK);
                    egui::ScrollArea::vertical()
                        .id_salt(rx_id)
                        .auto_shrink([false; 2])
                        .stick_to_bottom(true)
                        .max_height(rx_avail_h)
                        .show(ui, |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                            let c = match self.view { View::Ascii => &self.txt, View::Hex => &self.hex };
                            Frame { fill: Color32::BLACK, inner_margin: Margin::symmetric(4, 4), ..Default::default() }
                                .show(ui, |ui| {
                                    ui.set_min_size(egui::vec2(ui.available_width().max(0.0), rx_avail_h.max(0.0)));
                                    // 逐行显示，奇偶行交替文字颜色
                                    let full_w = ui.available_width().max(1.0);
                                    for (i, line) in c.split('\n').enumerate() {
                                        let fg = match i % 3 {
                                            0 => Color32::from_rgb(220, 220, 230),
                                            1 => Color32::from_rgb(100, 200, 100),
                                            _ => Color32::from_rgb(100, 180, 220),
                                        };
                                        let (id, painter) = ui.allocate_painter(egui::vec2(full_w, 18.0), egui::Sense::hover());
                                        painter.text(id.rect.min + egui::vec2(2.0, 1.0), egui::Align2::LEFT_TOP, line, egui::FontId::monospace(14.0), fg);
                                    }
                                });
                        });
                });

                ui.add_space(4.0);

                // ── 第三块：发送区 + 状态（固定高度） ──
                Frame { fill: color::PANEL, corner_radius: CornerRadius::same(6), inner_margin: Margin::symmetric(8, 6), ..Default::default() }
                    .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(color::TEXT, egui::RichText::new("发送").size(13.0));
                        if ui.selectable_label(!self.hexmd, "TXT").clicked() { self.hexmd = false; }
                        if ui.selectable_label(self.hexmd, "HEX").clicked() { self.hexmd = true; }
                        ui.separator();
                        ui.label("换行"); combo(ui, "N", &mut self.nl, &[Newline::None, Newline::CrLf, Newline::Cr, Newline::Lf], 55.0);
                        ui.separator();
                        if ui.checkbox(&mut self.auto, "定时").changed() && !self.auto { self.auto_acc = 0.0; }
                        if self.auto { ui.add(egui::TextEdit::singleline(&mut self.auto_t).desired_width(50.0)); ui.label("ms"); }
                    });
                    ui.horizontal(|ui| {
                        let h = if self.hexmd { "HEX (如 A0 1B 2C)" } else { "输入内容... (Ctrl+Enter 发送)" };
                        ui.add_sized([ui.available_width() - 90.0, 72.0], TextEdit::multiline(&mut self.send).font(egui::FontId::monospace(14.0)).hint_text(h));
                        ui.add_sized([80.0, 72.0], egui::Button::new(egui::RichText::new("发送").size(15.0).color(Color32::WHITE)).fill(color::ACCENT).corner_radius(6)).clicked().then(|| self.do_send());
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.colored_label(color::TEXT, &self.msg);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.colored_label(color::GREEN, format!("RX: {}", fmtsz(self.rx_n)));
                            ui.add_space(8.0);
                            ui.colored_label(color::ACCENT, format!("TX: {}", fmtsz(self.tx_n)));
                            ui.add_space(12.0);
                            if on { let pn = port_name_only(&self.sel_port); ui.colored_label(color::DIM, format!("{} @ {} {} {} {}", pn, self.baud, self.db, self.sb, self.par)); ui.add_space(4.0); ui.colored_label(color::GREEN, "●"); }
                            else { ui.colored_label(color::DIM, "○ 未连接"); }
                        });
                    });
                });
            });
        });
    }
}

fn main() -> eframe::Result {
    eframe::run_native("hicom - 串口助手", eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([960.0, 700.0]), ..Default::default() }, Box::new(|_cc| Ok(Box::new(HicomApp::new()))))
}
