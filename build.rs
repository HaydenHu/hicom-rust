fn main() {
    // 直接调用 Windows SDK 的 rc.exe 编译资源文件
    let rc_path = r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe";
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
