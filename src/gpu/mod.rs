//! GPU backend for Poseidon2 permutation via wgpu compute shaders.
//!
//! This module provides GPU-accelerated batch Poseidon2 permutations
//! for bulk content hashing and BAO tree construction.
//!
//! The GPU path is optional — the CPU backend is always available as fallback.

use std::num::NonZeroU64;

use p3_field::PrimeField64;
use p3_goldilocks::Goldilocks;
use wgpu::util::DeviceExt;

use crate::params::{ROUNDS_F, ROUNDS_P, WIDTH};
use crate::sponge::Hash;

/// Pre-compiled GPU compute pipelines and device handles.
#[derive(Debug)]
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    rc_buffer: wgpu::Buffer,
}

impl GpuContext {
    /// Initialize GPU backend. Returns `None` if no suitable GPU is available.
    pub async fn new() -> Option<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok()?;

        let caps = adapter.get_downlevel_capabilities();
        if !caps
            .flags
            .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
        {
            return None;
        }

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("hemera GPU"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                ..Default::default()
            })
            .await
            .ok()?;

        let module = device.create_shader_module(wgpu::include_wgsl!("poseidon2.wgsl"));

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("poseidon2 bgl"),
                entries: &[
                    // binding 0: states (read_write storage)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: Some(NonZeroU64::new(4).unwrap()),
                        },
                        count: None,
                    },
                    // binding 1: round constants (read-only storage)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: Some(NonZeroU64::new(4).unwrap()),
                        },
                        count: None,
                    },
                    // binding 2: num_perms uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: Some(NonZeroU64::new(4).unwrap()),
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("poseidon2 layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("poseidon2 pipeline"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("poseidon2_permute"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Upload round constants (generated from the same deterministic seed as CPU)
        let rc_data = generate_round_constants_u32();
        let rc_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("round constants"),
            contents: bytemuck::cast_slice(&rc_data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        Some(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            rc_buffer,
        })
    }

    /// Run batch Poseidon2 permutations on GPU.
    ///
    /// Each state is WIDTH (16) Goldilocks elements. Returns the permuted states.
    #[allow(clippy::unused_async)] // Will use await when GPU dispatch is fully async
    pub async fn batch_permute(
        &self,
        states: &[[Goldilocks; WIDTH]],
    ) -> Vec<[Goldilocks; WIDTH]> {
        if states.is_empty() {
            return vec![];
        }

        let num_perms = states.len() as u32;

        // Flatten states to u32 pairs (lo, hi per element)
        let mut state_u32s: Vec<u32> = Vec::with_capacity(states.len() * WIDTH * 2);
        for state in states {
            for elem in state {
                let val = elem.as_canonical_u64();
                state_u32s.push(val as u32);
                state_u32s.push((val >> 32) as u32);
            }
        }

        let state_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("states"),
                contents: bytemuck::cast_slice(&state_u32s),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            });

        let num_perms_buf =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("num_perms"),
                    contents: bytemuck::bytes_of(&num_perms),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

        let download_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("download"),
            size: state_buf.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: state_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.rc_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: num_perms_buf.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let workgroups = num_perms.div_ceil(64);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&state_buf, 0, &download_buf, 0, state_buf.size());
        self.queue.submit([encoder.finish()]);

        // Read back
        let slice = download_buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self
            .device
            .poll(wgpu::PollType::wait_indefinitely());

        let mapped = slice.get_mapped_range();
        let result_u32s: &[u32] = bytemuck::cast_slice(&mapped);

        let mut result = Vec::with_capacity(states.len());
        for perm_idx in 0..states.len() {
            let mut state = [Goldilocks::new(0); WIDTH];
            for (i, elem) in state.iter_mut().enumerate() {
                let off = perm_idx * WIDTH * 2 + i * 2;
                let lo = result_u32s[off] as u64;
                let hi = result_u32s[off + 1] as u64;
                *elem = Goldilocks::new(lo | (hi << 32));
            }
            result.push(state);
        }

        drop(mapped);
        download_buf.unmap();

        result
    }

    /// Hash multiple chunks in parallel on GPU, returning their chaining values.
    #[allow(clippy::unused_async)] // Will use await when GPU sponge is implemented
    pub async fn batch_chunk_cvs(&self, data: &[u8], chunk_size: usize) -> Vec<Hash> {
        // For now, fall back to CPU. Full GPU chunk hashing requires
        // implementing the sponge absorb loop in WGSL.
        // TODO: implement GPU sponge for full chunk hashing
        data.chunks(chunk_size)
            .enumerate()
            .map(|(i, chunk)| crate::tree::chunk_cv(chunk, i as u64, false))
            .collect()
    }
}

/// Generate round constants as u32 pairs matching the CPU backend's constants.
///
/// Uses the same static round constants as the CPU permutation, ensuring
/// GPU and CPU round constants are identical.
fn generate_round_constants_u32() -> Vec<u32> {
    let values = crate::constants::ROUND_CONSTANTS;
    let mut constants = Vec::with_capacity(values.len() * 2);
    for val in values {
        constants.push(val as u32);
        constants.push((val >> 32) as u32);
    }
    constants
}
