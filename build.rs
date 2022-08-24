fn main() {
    let target_os =
        std::env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS is set by cargo.");

    if target_os == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/meru.ico");
        res.compile().unwrap();
    }
}
