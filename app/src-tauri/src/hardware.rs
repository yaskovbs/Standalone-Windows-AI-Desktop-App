//! Hardware detection: NVIDIA GPU / VRAM, CPU threads and AVX support.

use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct HardwareInfo {
    pub gpu_name: Option<String>,
    pub vram_mb: u64,
    pub cpu_threads: u32,
    pub avx2: bool,
    pub avx512: bool,
}

/// Query nvidia-smi (ships with the NVIDIA driver). Returns (name, vram_mb).
fn query_nvidia() -> Option<(String, u64)> {
    let mut cmd = Command::new("nvidia-smi");
    cmd.args(["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?;
    let mut parts = line.split(',');
    let name = parts.next()?.trim().to_string();
    let vram = parts.next()?.trim().parse::<u64>().ok()?;
    Some((name, vram))
}

pub fn detect() -> HardwareInfo {
    let (gpu_name, vram_mb) = match query_nvidia() {
        Some((n, v)) => (Some(n), v),
        None => (None, 0),
    };
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    let (avx2, avx512) = (
        std::is_x86_feature_detected!("avx2"),
        std::is_x86_feature_detected!("avx512f"),
    );
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    let (avx2, avx512) = (false, false);

    HardwareInfo {
        gpu_name,
        vram_mb,
        cpu_threads: std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(4),
        avx2,
        avx512,
    }
}

/// Decide how many layers to offload. `model_mb` is the GGUF file size.
/// Keeps ~1.5 GB of VRAM headroom for the KV cache and the display.
pub fn gpu_layers_for(info: &HardwareInfo, model_mb: u64, total_layers: u32) -> u32 {
    if info.gpu_name.is_none() || info.vram_mb == 0 || !cfg!(feature = "cuda") {
        return 0;
    }
    let budget = info.vram_mb.saturating_sub(1536);
    if budget >= model_mb {
        return 999; // offload everything
    }
    let per_layer = (model_mb / total_layers.max(1) as u64).max(1);
    ((budget / per_layer) as u32).min(total_layers)
}

/// Threads for CPU compute: leave one core for the UI.
pub fn cpu_threads(info: &HardwareInfo) -> i32 {
    info.cpu_threads.saturating_sub(1).max(1) as i32
}
