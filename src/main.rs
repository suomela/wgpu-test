use std::borrow::Cow;

enum Error {
    GpuError(String),
}

impl From<wgpu::RequestDeviceError> for Error {
    fn from(err: wgpu::RequestDeviceError) -> Error {
        Error::GpuError(format!("{}", err))
    }
}

impl From<wgpu::BufferAsyncError> for Error {
    fn from(err: wgpu::BufferAsyncError) -> Error {
        Error::GpuError(format!("{}", err))
    }
}

fn gpu_error(e: &str) -> Error {
    Error::GpuError(e.to_owned())
}

async fn run() -> Result<(), Error> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .ok_or_else(|| gpu_error("no adapter found"))?;
    let description = wgpu::DeviceDescriptor {
        label: None,
        features: wgpu::Features::empty(),
        limits: wgpu::Limits::downlevel_defaults(),
    };
    let (device, queue) = adapter.request_device(&description, None).await?;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });
    println!("Adapter: {}", adapter.get_info().name);

    type ResultType = u32;
    let result_size = std::mem::size_of::<ResultType>() as wgpu::BufferAddress;
    let cpu_result_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: result_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let gpu_result_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: result_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: "main",
    });
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: gpu_result_buf.as_entire_binding(),
        }],
    });
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&gpu_result_buf, 0, &cpu_result_buf, 0, result_size);

    queue.submit(Some(encoder.finish()));
    let buf_slice = cpu_result_buf.slice(..);
    let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
    buf_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

    device.poll(wgpu::Maintain::Wait);
    receiver
        .receive()
        .await
        .ok_or_else(|| gpu_error("failed to run shader on GPU"))??;

    let buf_data = buf_slice.get_mapped_range();
    let result_array: Vec<ResultType> = bytemuck::cast_slice(&buf_data).to_vec();
    let result = result_array[0];
    drop(buf_data);
    cpu_result_buf.unmap();

    println!("Result: {result}");
    Ok(())
}

fn main() {
    env_logger::init();
    match pollster::block_on(run()) {
        Ok(()) => (),
        Err(Error::GpuError(x)) => eprintln!("GPU error: {x}"),
    }
}
