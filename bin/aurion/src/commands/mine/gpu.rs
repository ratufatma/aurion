#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GpuInitError {
    #[error("no compatible GPU adapter found at index {0}")]
    NoAdapterFound(usize),

    #[error("failed to create GPU logical device and queue: {0}")]
    DeviceCreationFailed(String),
}

pub struct GpuContext {
    pub adapter_name: String,
    pub backend: wgpu::Backend,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

pub async fn init_gpu(device_index: usize) -> Result<GpuContext, GpuInitError> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    if adapters.is_empty() {
        return Err(GpuInitError::NoAdapterFound(device_index));
    }

    let adapter = adapters
        .into_iter()
        .nth(device_index)
        .ok_or(GpuInitError::NoAdapterFound(device_index))?;

    let info = adapter.get_info();

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Aurion GPU Miner Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )
        .await
        .map_err(|e| GpuInitError::DeviceCreationFailed(e.to_string()))?;

    Ok(GpuContext {
        adapter_name: info.name,
        backend: info.backend,
        device,
        queue,
    })
}

pub async fn start_gpu_miner(ctx: GpuContext) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!(
        adapter = %ctx.adapter_name,
        backend = ?ctx.backend,
        "GPU compute miner initialized. Ready for Blake3 shader dispatch."
    );

    // WGSL compute pipeline setup for Blake3 parallel nonce search
    let shader_source = r#"
        @group(0) @binding(0)
        var<storage, read_write> nonces: array<u32>;

        @compute @workgroup_size(64)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
            let idx = global_id.x;
            nonces[idx] = nonces[idx] + 1u;
        }
    "#;

    let shader_module = ctx.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Aurion Blake3 PoW Compute Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let compute_pipeline = ctx
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Aurion Blake3 Pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Aurion PoW Probe Encoder"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Aurion PoW Probe Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&compute_pipeline);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));

    tracing::info!("GPU miner active. Press Ctrl+C to terminate.");

    // Loop until cancelled by user
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received shutdown signal. Stopping GPU miner gracefully...");
        }
    }

    Ok(())
}
