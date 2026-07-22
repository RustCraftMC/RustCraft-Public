#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unreachable_patterns)]
mod abstractions;
mod assets;
mod audio;
mod auth;
mod client;
mod entity;
mod logging;
mod net;
mod render;
mod scripting;
mod ui;
mod util;
mod world;

fn main() {
    logging::init();
    configure_runtime_directory();
    logging::log_startup_context();

    // Disable NVIDIA/ third-party implicit Vulkan layers that can corrupt
    // swapchain flags and cause access violations at queue_submit time.
    // These layers inject VK_SWAPCHAIN_CREATE_MUTABLE_FORMAT_BIT_KHR into
    // swapchain creation and can crash inside vk_gr2608GetInstanceProcAddr.
    unsafe {
        std::env::set_var("DISABLE_LAYER_NV_OPTIMUS_1", "1");
        std::env::set_var("DISABLE_LAYER_NV_PRESENT_1", "1");
        std::env::set_var("DISABLE_VULKAN_OBS_CAPTURE", "1");
        std::env::set_var("DISABLE_GAMEPP_LAYER", "1");
    }
    log::debug!("disabled known conflicting implicit Vulkan layers");

    // Limit Rayon's global thread pool to leave at least one logical core for
    // the render/main thread. Without this, Rayon's mesh builders compete with
    // the event loop for CPU time, which inflifies `outside` frame spikes under
    // unlimited-framerate polling.
    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let rayon_threads = (cpu_count - 1).max(1);
    if let Err(error) = rayon::ThreadPoolBuilder::new()
        .num_threads(rayon_threads)
        .thread_name(|index| format!("rayon-mesh-{index}"))
        .build_global()
    {
        log::warn!("failed to configure rayon thread pool: {error}");
    } else {
        log::info!("rayon thread pool configured: {rayon_threads} workers (cpu_count={cpu_count})");
    }

    let exit_code = client::app::run();
    log::info!("RustCraft shutting down with exit code {exit_code}");
    log::logger().flush();
    std::process::exit(exit_code);
}

fn configure_runtime_directory() {
    let current = std::env::current_dir().ok();
    if current.as_deref().map(has_game_assets).unwrap_or(false) {
        if let Some(path) = current {
            log::debug!("using current runtime directory: {}", path.display());
        }
        return;
    }

    let executable_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(std::path::Path::to_path_buf));
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let runtime_dir = executable_dir
        .filter(|path| has_game_assets(path))
        .or_else(|| has_game_assets(&manifest_dir).then_some(manifest_dir));

    if let Some(path) = runtime_dir {
        if let Err(error) = std::env::set_current_dir(&path) {
            log::error!(
                "failed to select runtime directory '{}': {}",
                path.display(),
                error
            );
        } else {
            log::info!("selected runtime directory: {}", path.display());
        }
    } else {
        log::warn!("no runtime directory containing assets/minecraft was found");
    }
}

fn has_game_assets(path: &std::path::Path) -> bool {
    path.join("assets/minecraft").is_dir()
}
