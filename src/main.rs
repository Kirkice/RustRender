use cocoa::base::id;
use metal::*;
use objc::{msg_send, sel, sel_impl};
use core_graphics_types::geometry::CGSize;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 3],
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Metal Triangle")
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
        .set_format(MTLVertexFormat::Float2);
    attributes.object_at(0).unwrap().set_offset(0);
    attributes.object_at(0).unwrap().set_buffer_index(0);
    attributes
        .object_at(1)
        .unwrap()
        .set_format(MTLVertexFormat::Float3);
    attributes.object_at(1).unwrap().set_offset(8);
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

    let pipeline_state = device
        .new_render_pipeline_state(&pipeline_desc)
        .expect("Failed to create pipeline state");

    let vertices = [
        Vertex {
            position: [0.0, 0.6],
            color: [1.0, 0.0, 0.0],
        },
        Vertex {
            position: [-0.6, -0.6],
            color: [0.0, 1.0, 0.0],
        },
        Vertex {
            position: [0.6, -0.6],
            color: [0.0, 0.0, 1.0],
        },
    ];

    let vertex_buffer = device.new_buffer_with_data(
        vertices.as_ptr() as *const _,
        (vertices.len() * std::mem::size_of::<Vertex>()) as u64,
        MTLResourceOptions::CPUCacheModeDefaultCache,
    );

    resize_drawable(&layer, &window);

    let _ = event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(_) => resize_drawable(&layer, &window),
                WindowEvent::ScaleFactorChanged { .. } => resize_drawable(&layer, &window),
                WindowEvent::RedrawRequested => {
                    draw_frame(
                        &layer,
                        &command_queue,
                        &pipeline_state,
                        &vertex_buffer,
                    );
                }
                _ => {}
            },
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        }
    });
}

fn resize_drawable(layer: &MetalLayer, window: &winit::window::Window) {
    let size = window.inner_size();
    let scale = window.scale_factor();
    let drawable_size = CGSize::new(
        size.width as f64 * scale,
        size.height as f64 * scale,
    );
    layer.set_drawable_size(drawable_size);
}

fn draw_frame(
    layer: &MetalLayer,
    command_queue: &CommandQueue,
    pipeline_state: &RenderPipelineState,
    vertex_buffer: &Buffer,
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

    let command_buffer = command_queue.new_command_buffer();
    let encoder = command_buffer.new_render_command_encoder(&pass_desc);
    encoder.set_render_pipeline_state(pipeline_state);
    encoder.set_vertex_buffer(0, Some(vertex_buffer), 0);
    encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
    encoder.end_encoding();

    command_buffer.present_drawable(drawable);
    command_buffer.commit();
}
