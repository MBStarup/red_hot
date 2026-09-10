use ash::vk;
use red_hot::renderer::{Mesh, Renderer};

use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    platform::pump_events::EventLoopExtPumpEvents,
    window::Window,
};

fn main() {
    unsafe {
        println!("Example 00: hello triangle");

        let window_width: u32 = 1200;
        let window_height: u32 = 800;

        #[derive(Clone, Copy)]
        #[repr(C)]
        struct Vertex {
            pos: [f32; 4],
            color: [f32; 4],
        }

        //. Fake uniforms not used by the shader, zero sized uniforms trips up the renderer atm
        #[allow(dead_code)]
        #[derive(Clone, Copy)]
        #[repr(C)]
        struct DrawUniform {
            _dummy: f32,
        }

        #[allow(dead_code)]
        #[derive(Clone, Copy)]
        #[repr(C)]
        struct RenderStageUniform {
            _dummy: f32,
        }

        #[allow(dead_code)]
        #[derive(Clone, Copy)]
        #[repr(C)]
        struct ObjectUniform {
            _dummy: f32,
        }

        let triangle = Mesh::<Vertex> {
            vertices: vec![
                Vertex { pos: [0.0, -0.5, 0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] },
                Vertex { pos: [0.5, 0.5, 0.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [-0.5, 0.5, 0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] },
            ],
            indices: vec![0, 1, 2],
        };

        let mut event_loop = EventLoop::new().expect("Failed to create new event loop? How can this even fail???");

        let window = event_loop
            .create_window(Window::default_attributes().with_title("Hello Triangle").with_inner_size(winit::dpi::LogicalSize::new(window_width, window_height)))
            .unwrap();

        let mut renderer = Renderer::<Vertex>::new(&window, window_width, window_height);

        let default_stage = renderer.register_stage(
            "Default".to_owned(),
            include_bytes!("./shader/vert.spv"),
            include_bytes!("./shader/frag.spv"),
            vk::PipelineRasterizationStateCreateInfo {
                cull_mode: vk::CullModeFlags::NONE,
                front_face: vk::FrontFace::COUNTER_CLOCKWISE,
                line_width: 1.0,
                polygon_mode: vk::PolygonMode::FILL,
                ..Default::default()
            },
            vk::PipelineDepthStencilStateCreateInfo::default(), //. No depth testing needed for a flat triangle
            vk::PipelineColorBlendStateCreateInfo::default().logic_op(vk::LogicOp::CLEAR).attachments(&[vk::PipelineColorBlendAttachmentState {
                blend_enable: 0,
                src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
                dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,
                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::ZERO,
                dst_alpha_blend_factor: vk::BlendFactor::ZERO,
                alpha_blend_op: vk::BlendOp::ADD,
                color_write_mask: vk::ColorComponentFlags::RGBA,
            }]),
            [
                vk::VertexInputAttributeDescription { location: 0, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 0 as u32 }, //. Position
                vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 4 * 32 / 8 as u32 }, //. Color
            ],
            vec![red_hot::renderer::RedHotStageImage::SwapchainImage()],
            &[vk::AttachmentDescription {
                format: renderer.swapchain_image_format,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            }],
            [vk::ClearValue { color: vk::ClearColorValue { float32: [0.09, 0.05, 0.14, 1.0] } }],
            &[vk::SubpassDescription::default()
                .color_attachments(&[vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }])
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)],
            &[vk::SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                ..Default::default()
            }],
            &[],
        );

        let triangle_meshi = renderer.register_mesh(triangle);

        let mut should_close = false;
        while !should_close {
            event_loop.pump_events(Some(std::time::Duration::ZERO), |event, _| {
                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested | WindowEvent::KeyboardInput { event: KeyEvent { state: ElementState::Pressed, physical_key: PhysicalKey::Code(KeyCode::Escape), .. }, .. },
                        ..
                    } => {
                        should_close = true; //. if we pressed close, close
                    },
                    _ => (), //. ignore other events
                }
            });

            renderer.render_begin(DrawUniform { _dummy: 0.0 });
            renderer.render_stage(default_stage, RenderStageUniform { _dummy: 0.0 }, vec![triangle_meshi], vec![ObjectUniform { _dummy: 0.0 }]);
            renderer.render_commit();
        }

        renderer.destroy();
        println!("Goodbye");
    }
}
