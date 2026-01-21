#![allow(unexpected_cfgs)]

use cocoa::base::id;
use glam::{Mat4, Vec3};
use metal::*;
use objc::{msg_send, sel, sel_impl};
use core_graphics_types::geometry::CGSize;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::path::{Path, PathBuf};
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct Uniforms {
    view_proj: [f32; 16],
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Metal OBJ")
        .build(&event_loop)
        .unwrap();

    let device = Device::system_default().expect("No Metal device found");
    let command_queue = device.new_command_queue();

    let layer = MetalLayer::new();
    layer.set_device(&device);
    layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    layer.set_presents_with_transaction(false);
    layer.set_framebuffer_only(true);

    let window_handle = window.window_handle().expect("Window handle unavailable");
    let ns_view = match window_handle.as_raw() {
        RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr() as id,
        _ => panic!("Unsupported window handle"),
    };

    unsafe {
        let _: () = msg_send![ns_view, setWantsLayer: true];
        let layer_ptr = layer.as_ref() as *const _ as id;
        let _: () = msg_send![ns_view, setLayer: layer_ptr];
    }

    let shader_source = include_str!("shaders/triangle.metal");
    let compile_options = CompileOptions::new();
    let library = device
        .new_library_with_source(shader_source, &compile_options)
        .expect("Failed to compile shaders");
    let vertex_fn = library
        .get_function("vertex_main", None)
        .expect("Missing vertex function");
    let fragment_fn = library
        .get_function("fragment_main", None)
        .expect("Missing fragment function");

    let vertex_desc = VertexDescriptor::new();
    let attributes = vertex_desc.attributes();
    attributes
        .object_at(0)
        .unwrap()
        .set_format(MTLVertexFormat::Float3);
    attributes.object_at(0).unwrap().set_offset(0);
    attributes.object_at(0).unwrap().set_buffer_index(0);
    attributes
        .object_at(1)
        .unwrap()
        .set_format(MTLVertexFormat::Float3);
    attributes.object_at(1).unwrap().set_offset(12);
    attributes.object_at(1).unwrap().set_buffer_index(0);

    let layouts = vertex_desc.layouts();
    layouts
        .object_at(0)
        .unwrap()
        .set_stride(std::mem::size_of::<Vertex>() as u64);

    let pipeline_desc = RenderPipelineDescriptor::new();
    pipeline_desc.set_vertex_function(Some(&vertex_fn));
    pipeline_desc.set_fragment_function(Some(&fragment_fn));
    pipeline_desc.set_vertex_descriptor(Some(&vertex_desc));
    pipeline_desc
        .color_attachments()
        .object_at(0)
        .unwrap()
        .set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    pipeline_desc.set_depth_attachment_pixel_format(MTLPixelFormat::Depth32Float);

    let pipeline_state = device
        .new_render_pipeline_state(&pipeline_desc)
        .expect("Failed to create pipeline state");

    let depth_desc = DepthStencilDescriptor::new();
    depth_desc.set_depth_compare_function(MTLCompareFunction::Less);
    depth_desc.set_depth_write_enabled(true);
    let depth_state = device.new_depth_stencil_state(&depth_desc);

    let (vertices, indices) = load_obj_mesh();

    let vertex_buffer = device.new_buffer_with_data(
        vertices.as_ptr() as *const _,
        (vertices.len() * std::mem::size_of::<Vertex>()) as u64,
        MTLResourceOptions::CPUCacheModeDefaultCache,
    );

    let index_buffer = device.new_buffer_with_data(
        indices.as_ptr() as *const _,
        (indices.len() * std::mem::size_of::<u32>()) as u64,
        MTLResourceOptions::CPUCacheModeDefaultCache,
    );

    let mut depth_texture = resize_drawable(&device, &layer, &window);
    let uniform_buffer = create_uniform_buffer(&device, &layer);

    let _ = event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(_) => {
                    depth_texture = resize_drawable(&device, &layer, &window);
                    update_uniform_buffer(&uniform_buffer, &layer);
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    depth_texture = resize_drawable(&device, &layer, &window);
                    update_uniform_buffer(&uniform_buffer, &layer);
                }
                WindowEvent::RedrawRequested => {
                    draw_frame(
                        &layer,
                        &command_queue,
                        &pipeline_state,
                        &vertex_buffer,
                        &index_buffer,
                        indices.len() as u64,
                        &depth_texture,
                        &depth_state,
                        &uniform_buffer,
                    );
                }
                _ => {}
            },
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        }
    });
}

fn resize_drawable(
    device: &Device,
    layer: &MetalLayer,
    window: &winit::window::Window,
) -> Texture {
    let size = window.inner_size();
    let scale = window.scale_factor();
    let drawable_size = CGSize::new(
        size.width as f64 * scale,
        size.height as f64 * scale,
    );
    layer.set_drawable_size(drawable_size);

    create_depth_texture(device, layer)
}

fn draw_frame(
    layer: &MetalLayer,
    command_queue: &CommandQueue,
    pipeline_state: &RenderPipelineState,
    vertex_buffer: &Buffer,
    index_buffer: &Buffer,
    index_count: u64,
    depth_texture: &Texture,
    depth_state: &DepthStencilState,
    uniform_buffer: &Buffer,
) {
    let drawable = match layer.next_drawable() {
        Some(drawable) => drawable,
        None => return,
    };

    let pass_desc = RenderPassDescriptor::new();
    let color_attachment = pass_desc.color_attachments().object_at(0).unwrap();
    color_attachment.set_texture(Some(drawable.texture()));
    color_attachment.set_load_action(MTLLoadAction::Clear);
    color_attachment.set_store_action(MTLStoreAction::Store);
    color_attachment.set_clear_color(MTLClearColor::new(0.1, 0.12, 0.16, 1.0));

    let depth_attachment = pass_desc.depth_attachment().unwrap();
    depth_attachment.set_texture(Some(depth_texture));
    depth_attachment.set_load_action(MTLLoadAction::Clear);
    depth_attachment.set_store_action(MTLStoreAction::DontCare);
    depth_attachment.set_clear_depth(1.0);

    let command_buffer = command_queue.new_command_buffer();
    let encoder = command_buffer.new_render_command_encoder(&pass_desc);
    encoder.set_render_pipeline_state(pipeline_state);
    encoder.set_depth_stencil_state(depth_state);
    encoder.set_vertex_buffer(0, Some(vertex_buffer), 0);
    encoder.set_vertex_buffer(1, Some(uniform_buffer), 0);
    encoder.draw_indexed_primitives(
        MTLPrimitiveType::Triangle,
        index_count,
        MTLIndexType::UInt32,
        index_buffer,
        0,
    );
    encoder.end_encoding();

    command_buffer.present_drawable(drawable);
    command_buffer.commit();
}

fn create_uniform_buffer(device: &Device, layer: &MetalLayer) -> Buffer {
    let uniforms = build_uniforms(layer);
    device.new_buffer_with_data(
        (&uniforms as *const Uniforms) as *const _,
        std::mem::size_of::<Uniforms>() as u64,
        MTLResourceOptions::CPUCacheModeDefaultCache,
    )
}

fn update_uniform_buffer(buffer: &Buffer, layer: &MetalLayer) {
    let uniforms = build_uniforms(layer);
    unsafe {
        let ptr = buffer.contents() as *mut Uniforms;
        *ptr = uniforms;
    }
}

fn build_uniforms(layer: &MetalLayer) -> Uniforms {
    let drawable_size = layer.drawable_size();
    let aspect = (drawable_size.width as f32).max(1.0) / (drawable_size.height as f32).max(1.0);

    let eye = Vec3::new(0.0, 0.0, 2.0);
    let target = Vec3::new(0.0, 0.0, 0.0);
    let up = Vec3::new(0.0, 1.0, 0.0);

    let view = Mat4::look_at_rh(eye, target, up);
    let proj = Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 100.0);
    let view_proj = proj * view;

    Uniforms {
        view_proj: view_proj.to_cols_array(),
    }
}

fn create_depth_texture(device: &Device, layer: &MetalLayer) -> Texture {
    let drawable_size = layer.drawable_size();
    let width = drawable_size.width as u64;
    let height = drawable_size.height as u64;

    let desc = TextureDescriptor::new();
    desc.set_texture_type(MTLTextureType::D2);
    desc.set_pixel_format(MTLPixelFormat::Depth32Float);
    desc.set_width(width.max(1));
    desc.set_height(height.max(1));
    desc.set_storage_mode(MTLStorageMode::Private);
    desc.set_usage(MTLTextureUsage::RenderTarget);

    device.new_texture(&desc)
}

fn load_obj_mesh() -> (Vec<Vertex>, Vec<u32>) {
    let obj_path = find_obj_path().unwrap_or_else(|| {
        panic!("未找到 OBJ：请放在 src/Models 或 Models 目录下")
    });

    let load_options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };

    let (models, _) =
        tobj::load_obj(&obj_path, &load_options).expect("读取 OBJ 失败");

    let mut all_positions: Vec<[f32; 3]> = Vec::new();
    for model in &models {
        let mesh = &model.mesh;
        for i in (0..mesh.positions.len()).step_by(3) {
            all_positions.push([
                mesh.positions[i],
                mesh.positions[i + 1],
                mesh.positions[i + 2],
            ]);
        }
    }

    if all_positions.is_empty() {
        panic!("OBJ 没有顶点位置数据");
    }

    let (min, max) = bounds(&all_positions);
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let extent = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    let max_extent = extent[0].max(extent[1]).max(extent[2]);
    let scale = if max_extent > 0.0 { 2.0 / max_extent } else { 1.0 };

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut present_index: u32 = 0;

    for model in &models {
        let mesh = &model.mesh;
        for &idx in &mesh.indices {
            let base = idx as usize * 3;
            let pos = [
                mesh.positions[base],
                mesh.positions[base + 1],
                mesh.positions[base + 2],
            ];
            let normalized = [
                (pos[0] - center[0]) * scale,
                (pos[1] - center[1]) * scale,
                (pos[2] - center[2]) * scale,
            ];

            vertices.push(Vertex {
                position: normalized,
                color: [0.8, 0.85, 0.9],
            });
            indices.push(present_index);
            present_index += 1;
        }
    }

    if indices.is_empty() {
        panic!("OBJ 没有可绘制的索引数据");
    }

    (vertices, indices)
}

fn find_first_obj(models_dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(models_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("obj"))
            .unwrap_or(false)
        {
            return Some(path);
        }
    }
    None
}

fn find_obj_path() -> Option<PathBuf> {
    let preferred = Path::new("src/Models/Bunny.obj");
    if preferred.exists() {
        return Some(preferred.to_path_buf());
    }

    let alt_preferred = Path::new("Models/Bunny.obj");
    if alt_preferred.exists() {
        return Some(alt_preferred.to_path_buf());
    }

    find_first_obj(Path::new("src/Models"))
        .or_else(|| find_first_obj(Path::new("Models")))
}

fn bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in positions {
        min[0] = min[0].min(p[0]);
        min[1] = min[1].min(p[1]);
        min[2] = min[2].min(p[2]);
        max[0] = max[0].max(p[0]);
        max[1] = max[1].max(p[1]);
        max[2] = max[2].max(p[2]);
    }
    (min, max)
}
