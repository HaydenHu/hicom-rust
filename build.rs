fn main() {
    // 仅 Windows 需要嵌入 .ico 到 exe 文件
    // Linux/macOS 桌面图标由桌面环境管理，运行时窗口图标通过 egui::IconData 设置
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "windows" {
        return;
    }

    // 尝试查找 Windows SDK 的 rc.exe
    let rc_candidates = [
        r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe",
        r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\rc.exe",
        r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\rc.exe",
    ];

    let rc_path = rc_candidates.iter().find(|p| std::path::Path::new(p).exists());
    let rc_path = match rc_path {
        Some(p) => p,
        None => {
            // 尝试从 PATH 找 rc.exe
            if let Ok(path) = std::env::var("PATH") {
                for dir in path.split(';') {
                    let candidate = std::path::Path::new(dir).join("rc.exe");
                    if candidate.exists() {
                        // 不能在循环内 return，需要继续执行
                        let found = candidate.to_string_lossy().to_string();
                        return embed_icon(&found);
                    }
                }
            }
            println!("cargo:warning=rc.exe not found, skipping icon embed");
            return;
        }
    };

    embed_icon(rc_path);
}

fn embed_icon(rc_path: &str) {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let res_path = std::path::Path::new(&out_dir).join("icon.res");
    let rc_file = std::path::Path::new(&out_dir).join("icon.rc");

    std::fs::write(&rc_file, "1 ICON \"icon.ico\"").unwrap();

    let status = std::process::Command::new(rc_path)
        .args(&["/nologo", "/fo"])
        .arg(&res_path)
        .arg(&rc_file)
        .status()
        .expect("Failed to run rc.exe");

    if !status.success() {
        panic!("rc.exe failed");
    }

    println!("cargo:rustc-link-arg-bins={}", res_path.display());
}
