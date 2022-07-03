#[cfg(windows)]
extern crate winres;

#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/meru.ico");
    res.compile().unwrap();
}

#[cfg(unix)]
fn main() {}
