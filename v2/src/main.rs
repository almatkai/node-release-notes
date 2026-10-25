#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#[tokio::main(worker_threads = 2)]
async fn main() {
    let result = match node_release_notes_native::Adapter::production() {
        Ok(adapter) => tezbar_extension_sdk::serve(adapter).await,
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
