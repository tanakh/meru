// #![windows_subsystem = "windows"]

#[async_std::main]
async fn main() {
    meru::app::main().await;
}
