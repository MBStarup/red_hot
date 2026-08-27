use std::{
    ascii,
    f32::consts::{PI, TAU},
    fs::read,
    println,
    time::{SystemTime, UNIX_EPOCH},
};

use img::parse_bmp;
use rand::{RngExt, SeedableRng};

use ash::vk;
use red_hot::{
    math::{perspective_matrix, Mat4x4, Quaternion, Transform, Vec3},
    renderer::{ImageSize, Mesh, MeshIndex, RedHotImageCreateInfo, Renderer},
};

use winit::{
    event::{DeviceEvent::MouseMotion, ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    platform::pump_events::EventLoopExtPumpEvents,
    window::Window,
};

fn texture_atlas_letter_coords(c: char) -> (i32, i32) {
    const ROWS: &[&str] = &["abcdefghijklmnopqrst", "uvxyz1234567890+-?=!", ".:,; {[]}w*'"];

    let c = c.to_ascii_lowercase();

    ROWS.iter()
        .enumerate()
        .find_map(|(row, chars)| chars.chars().position(|x| x == c).map(|col| (col as i32, row as i32)))
        .unwrap_or_else(|| panic!("Unsupported character: {c:?}"))
}

fn main() {
    unsafe {
        println!("Example 06: shadows");
        let mut window_width: u32 = 2000;
        let mut window_height: u32 = 1200;

        #[derive(Clone, Debug, Copy)]
        #[repr(C)]
        struct Vertex {
            pos: [f32; 4],
            normal: [f32; 4],
            color: [f32; 4],
        }

        #[allow(dead_code)]
        #[derive(Clone, Debug, Copy)]
        #[repr(C)]
        struct DrawUniform {
            _dummy: f32,
        }

        #[allow(dead_code)]
        #[derive(Clone, Debug, Copy)]
        #[repr(C)]
        struct RenderStageUniform {
            view_mat: Mat4x4<f32>,
            proj_mat: Mat4x4<f32>,
            light_view_proj_mat: Mat4x4<f32>,
            light_view_proj_mat2: Mat4x4<f32>,
            light_dir: [f32; 4],
            light_dir2: [f32; 4],
        }

        #[allow(dead_code)]
        #[derive(Clone, Debug, Copy)]
        #[repr(C)]
        struct ShadowStageUniform {
            view_mat: Mat4x4<f32>,
            proj_mat: Mat4x4<f32>,
        }

        #[allow(dead_code)]
        #[derive(Clone, Debug, Copy)]
        #[repr(C)]
        struct ObjectUniform {
            model_mat: Mat4x4<f32>,
            is_light: u32,
        }

        #[allow(dead_code)]
        #[derive(Clone, Debug, Copy)]
        #[repr(C)]
        struct UiObjectUniform {
            model_mat: Mat4x4<f32>,
            color: Vec3<f32>,
            use_color: u32,
            texture_offset: [f32; 2],
            texture_area: [f32; 2],
        }

        #[rustfmt::skip]
        let pyramid_mesh = {
            let vertices = vec![
                Vertex { pos: [-1.0,  0.0, -1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] },
                Vertex { pos: [ 1.0,  0.0, -1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] }, //. DOWN (RIGHT/BACK)
                Vertex { pos: [ 1.0,  0.0,  1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] },
                Vertex { pos: [-1.0,  0.0,  1.0, 1.0], normal: [-1.0,  1.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0] },
                Vertex { pos: [ 0.0,  1.0,  0.0, 1.0], normal: [-1.0,  1.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0] }, //. LEFT
                Vertex { pos: [-1.0,  0.0, -1.0, 1.0], normal: [-1.0,  1.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0] },
                Vertex { pos: [-1.0,  0.0, -1.0, 1.0], normal: [ 0.0,  1.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [ 0.0,  1.0,  0.0, 1.0], normal: [ 0.0,  1.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] }, //. FRONT
                Vertex { pos: [ 1.0,  0.0, -1.0, 1.0], normal: [ 0.0,  1.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [ 1.0,  0.0, -1.0, 1.0], normal: [ 1.0,  1.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [ 0.0,  1.0,  0.0, 1.0], normal: [ 1.0,  1.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0] }, //. RIGHT
                Vertex { pos: [ 1.0,  0.0,  1.0, 1.0], normal: [ 1.0,  1.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [ 1.0,  0.0,  1.0, 1.0], normal: [ 0.0,  1.0,  1.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] },
                Vertex { pos: [ 0.0,  1.0,  0.0, 1.0], normal: [ 0.0,  1.0,  1.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] }, //. BACKWARDS
                Vertex { pos: [-1.0,  0.0,  1.0, 1.0], normal: [ 0.0,  1.0,  1.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] },
                Vertex { pos: [ 1.0,  0.0,  1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] },
                Vertex { pos: [-1.0,  0.0,  1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] }, //. DOWN (LEFT/FRONT)
                Vertex { pos: [-1.0,  0.0, -1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] },
            ];
            let indices = (0..vertices.len() as u32).collect();
            Mesh::<Vertex> {vertices, indices}
        };

        #[rustfmt::skip]
        let (cube_mesh, inverse_cube_mesh) = {
            let vertices = vec![
                Vertex { pos: [-1.0, -1.0, -1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] },
                Vertex { pos: [ 1.0, -1.0, -1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] }, //. DOWN (LEFT/BACK)
                Vertex { pos: [-1.0, -1.0,  1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] },
                Vertex { pos: [ 1.0, -1.0, -1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] },
                Vertex { pos: [ 1.0, -1.0,  1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] }, //. DOWN (RIGHT/FRONT)
                Vertex { pos: [-1.0, -1.0,  1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] },
                Vertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] },
                Vertex { pos: [-1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] }, //. UP (LEFT/FRONT)
                Vertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] },
                Vertex { pos: [ 1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] },
                Vertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] }, //. UP (RIGHT/BACK)
                Vertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] },
                Vertex { pos: [-1.0, -1.0,  1.0, 1.0], normal: [-1.0,  0.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [-1.0,  1.0,  1.0, 1.0], normal: [-1.0,  0.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0] }, //. LEFT (FRONT/DOWN)
                Vertex { pos: [-1.0, -1.0, -1.0, 1.0], normal: [-1.0,  0.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [-1.0,  1.0,  1.0, 1.0], normal: [-1.0,  0.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [-1.0,  0.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0] }, //. LEFT (BACK/UP)
                Vertex { pos: [-1.0, -1.0, -1.0, 1.0], normal: [-1.0,  0.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [ 1.0,  1.0, -1.0, 1.0], normal: [ 1.0,  0.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0] },
                Vertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 1.0,  0.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0] }, //. RIGHT (FRONT/UP)
                Vertex { pos: [ 1.0, -1.0,  1.0, 1.0], normal: [ 1.0,  0.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0] },
                Vertex { pos: [ 1.0, -1.0, -1.0, 1.0], normal: [ 1.0,  0.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0] },
                Vertex { pos: [ 1.0,  1.0, -1.0, 1.0], normal: [ 1.0,  0.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0] }, //. RIGHT (BACK/DOWN)
                Vertex { pos: [ 1.0, -1.0,  1.0, 1.0], normal: [ 1.0,  0.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0] },
                Vertex { pos: [-1.0, -1.0, -1.0, 1.0], normal: [ 0.0,  0.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  0.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] }, //. BACK (LEFT/DOWN)
                Vertex { pos: [ 1.0, -1.0, -1.0, 1.0], normal: [ 0.0,  0.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  0.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [ 1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  0.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] }, //. BACK (RIGHT/UP)
                Vertex { pos: [ 1.0, -1.0, -1.0, 1.0], normal: [ 0.0,  0.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] },
                Vertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  0.0,  1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] },
                Vertex { pos: [-1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  0.0,  1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] }, //. FRONT (LEFT/UP)
                Vertex { pos: [-1.0, -1.0,  1.0, 1.0], normal: [ 0.0,  0.0,  1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] },
                Vertex { pos: [ 1.0, -1.0,  1.0, 1.0], normal: [ 0.0,  0.0,  1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] },
                Vertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  0.0,  1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] }, //. FRONT (RIGHT/DOWN)
                Vertex { pos: [-1.0, -1.0,  1.0, 1.0], normal: [ 0.0,  0.0,  1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] },
            ];
            let indices: Vec<u32> = (0..vertices.len() as u32).collect();
            (Mesh::<Vertex> {vertices: vertices.clone(), indices: indices.clone()}, Mesh::<Vertex> {vertices: vertices.into_iter().rev().map(|v| Vertex{ pos: v.pos , normal: [-v.normal[0], -v.normal[1], -v.normal[2], 1.0], color: v.color }).collect(), indices})
        };

        #[rustfmt::skip]
        let plane_mesh = {
            let vertices = vec![
                Vertex { pos: [-1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 0.0, 1.0] },
                Vertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] }, //. UP (LEFT/FRONT)
                Vertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] },
                Vertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] },
                Vertex { pos: [ 1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] }, //. UP (RIGHT/BACK)
                Vertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] },
            ];
            let indices = (0..vertices.len() as u32).collect();
            Mesh::<Vertex> {vertices, indices}
        };

        #[derive(Clone, Copy)]
        struct Object {
            transform: Transform<f32>,
            mesh: MeshIndex,
        }

        let mut position = Vec3 { x: 0.0, y: 0.0, z: -10.0 };
        let mut cam_yaw = 0.0;
        let mut cam_pitch = 0.0;

        let proj_mat = perspective_matrix(TAU / 4.0, 0.1, 10000.0);

        let mut event_loop = EventLoop::new().expect("Failed to create new event loop? How can this even fail???");

        let window = event_loop
            .create_window(Window::default_attributes().with_title("06-shadows").with_inner_size(winit::dpi::LogicalSize::new(window_width, window_height)))
            .unwrap();

        window.set_cursor_grab(winit::window::CursorGrabMode::Confined).unwrap();
        window.set_cursor_visible(false);

        let mut renderer = Renderer::<Vertex>::new(&window, window_width, window_height);

        let shadow_texture = renderer.register_image(RedHotImageCreateInfo {
            //. Create an image to store the shadow map
            size: ImageSize::Fixed(4096, 4096),
            format: vk::Format::D32_SFLOAT,                                                      //. That consists of depth? f32's
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, //. Which will be used as a depth stencil, and then as a sampled texture
            memory_flags: vk::MemoryPropertyFlags::DEVICE_LOCAL,                                 //. Which will only be on the GPU
            image_aspect_mask: vk::ImageAspectFlags::DEPTH,
        });
        let shadow_stage = renderer.register_stage(
            "Shadow".to_owned(),
            include_bytes!("./shader/shadow_vert.spv"),
            include_bytes!("./shader/shadow_frag.spv"),
            vk::PipelineRasterizationStateCreateInfo {
                cull_mode: vk::CullModeFlags::FRONT,
                front_face: vk::FrontFace::COUNTER_CLOCKWISE,
                line_width: 1.0,
                polygon_mode: vk::PolygonMode::FILL,
                ..Default::default()
            },
            vk::PipelineDepthStencilStateCreateInfo {
                depth_test_enable: 1,
                depth_write_enable: 1,
                depth_compare_op: vk::CompareOp::LESS,
                front: vk::StencilOpState { fail_op: vk::StencilOp::KEEP, pass_op: vk::StencilOp::KEEP, depth_fail_op: vk::StencilOp::KEEP, compare_op: vk::CompareOp::ALWAYS, ..Default::default() },
                back: vk::StencilOpState { fail_op: vk::StencilOp::KEEP, pass_op: vk::StencilOp::KEEP, depth_fail_op: vk::StencilOp::KEEP, compare_op: vk::CompareOp::ALWAYS, ..Default::default() },
                max_depth_bounds: 1.0,
                ..Default::default()
            },
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&[]),
            [
                vk::VertexInputAttributeDescription { location: 0, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 0 as u32 }, //. Position
                vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 4 * 32 / 8 as u32 }, //. Normal
                vk::VertexInputAttributeDescription { location: 2, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 8 * 32 / 8 as u32 }, //. Color
            ],
            vec![red_hot::renderer::RedHotStageImage::Image(shadow_texture)],
            &[vk::AttachmentDescription {
                format: vk::Format::D32_SFLOAT,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                ..Default::default()
            }],
            [vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } }],
            &[(vk::SubpassDescription::default()
                .depth_stencil_attachment(&vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL })
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS))],
            &[vk::SubpassDependency {
                src_subpass: 0,
                dst_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                src_access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
                dst_access_mask: vk::AccessFlags::SHADER_READ,
                ..Default::default()
            }],
            &[],
        );

        let shadow_texture2 = renderer.register_image(RedHotImageCreateInfo {
            //. Create an image to store the shadow map
            size: ImageSize::Fixed(4096, 4096),
            format: vk::Format::D32_SFLOAT,                                                      //. That consists of depth? f32's
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, //. Which will be used as a depth stencil, and then as a sampled texture
            memory_flags: vk::MemoryPropertyFlags::DEVICE_LOCAL,                                 //. Which will only be on the GPU
            image_aspect_mask: vk::ImageAspectFlags::DEPTH,
        });
        let shadow_stage2 = renderer.register_stage(
            "Shadow".to_owned(),
            include_bytes!("./shader/shadow_vert.spv"),
            include_bytes!("./shader/shadow_frag.spv"),
            vk::PipelineRasterizationStateCreateInfo {
                cull_mode: vk::CullModeFlags::FRONT,
                front_face: vk::FrontFace::COUNTER_CLOCKWISE,
                line_width: 1.0,
                polygon_mode: vk::PolygonMode::FILL,
                ..Default::default()
            },
            vk::PipelineDepthStencilStateCreateInfo {
                depth_test_enable: 1,
                depth_write_enable: 1,
                depth_compare_op: vk::CompareOp::LESS,
                front: vk::StencilOpState { fail_op: vk::StencilOp::KEEP, pass_op: vk::StencilOp::KEEP, depth_fail_op: vk::StencilOp::KEEP, compare_op: vk::CompareOp::ALWAYS, ..Default::default() },
                back: vk::StencilOpState { fail_op: vk::StencilOp::KEEP, pass_op: vk::StencilOp::KEEP, depth_fail_op: vk::StencilOp::KEEP, compare_op: vk::CompareOp::ALWAYS, ..Default::default() },
                max_depth_bounds: 1.0,
                ..Default::default()
            },
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&[]),
            [
                vk::VertexInputAttributeDescription { location: 0, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 0 as u32 }, //. Position
                vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 4 * 32 / 8 as u32 }, //. Normal
                vk::VertexInputAttributeDescription { location: 2, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 8 * 32 / 8 as u32 }, //. Color
            ],
            vec![red_hot::renderer::RedHotStageImage::Image(shadow_texture2)],
            &[vk::AttachmentDescription {
                format: vk::Format::D32_SFLOAT,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                ..Default::default()
            }],
            [vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } }],
            &[(vk::SubpassDescription::default()
                .depth_stencil_attachment(&vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL })
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS))],
            &[vk::SubpassDependency {
                src_subpass: 0,
                dst_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                src_access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
                dst_access_mask: vk::AccessFlags::SHADER_READ,
                ..Default::default()
            }],
            &[],
        );

        let depth_image = renderer.register_image(RedHotImageCreateInfo {
            size: ImageSize::SurfaceSize,
            format: vk::Format::D32_SFLOAT,
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            memory_flags: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            image_aspect_mask: vk::ImageAspectFlags::DEPTH,
        });
        let default_stage = renderer.register_stage(
            "Default".to_owned(),
            include_bytes!("./shader/vert.spv"),
            include_bytes!("./shader/frag.spv"),
            vk::PipelineRasterizationStateCreateInfo {
                cull_mode: vk::CullModeFlags::BACK,
                front_face: vk::FrontFace::COUNTER_CLOCKWISE,
                line_width: 1.0,
                polygon_mode: vk::PolygonMode::FILL,
                ..Default::default()
            },
            vk::PipelineDepthStencilStateCreateInfo {
                depth_test_enable: 1,
                depth_write_enable: 1,
                depth_compare_op: vk::CompareOp::LESS,
                front: vk::StencilOpState { fail_op: vk::StencilOp::KEEP, pass_op: vk::StencilOp::KEEP, depth_fail_op: vk::StencilOp::KEEP, compare_op: vk::CompareOp::ALWAYS, ..Default::default() },
                back: vk::StencilOpState { fail_op: vk::StencilOp::KEEP, pass_op: vk::StencilOp::KEEP, depth_fail_op: vk::StencilOp::KEEP, compare_op: vk::CompareOp::ALWAYS, ..Default::default() },
                max_depth_bounds: 1.0,
                ..Default::default()
            },
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
                vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 4 * 32 / 8 as u32 }, //. Normal
                vk::VertexInputAttributeDescription { location: 2, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 8 * 32 / 8 as u32 }, //. Color
            ],
            vec![red_hot::renderer::RedHotStageImage::SwapchainImage(), red_hot::renderer::RedHotStageImage::Image(depth_image)],
            &[
                vk::AttachmentDescription {
                    format: renderer.swapchain_image_format,
                    samples: vk::SampleCountFlags::TYPE_1,
                    load_op: vk::AttachmentLoadOp::CLEAR,
                    store_op: vk::AttachmentStoreOp::STORE,
                    final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    ..Default::default()
                },
                vk::AttachmentDescription {
                    format: vk::Format::D32_SFLOAT,
                    samples: vk::SampleCountFlags::TYPE_1,
                    load_op: vk::AttachmentLoadOp::CLEAR,
                    // initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, // TODO: non-undefined initial layout is currently unsupported, as I do not create the barriers to transition them into the correct state before the renderpass begins. In this case it is fine too, as we clear it anyways
                    final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                    ..Default::default()
                },
            ],
            [
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.09, 0.05, 0.14, 1.0] } },
                vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } },
            ],
            &[vk::SubpassDescription::default()
                .color_attachments(&[vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }])
                .depth_stencil_attachment(&vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL })
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)],
            &[vk::SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                ..Default::default()
            }],
            &[
                (
                    &red_hot::renderer::RedHotStageImage::Image(shadow_texture),
                    vk::SamplerCreateInfo::default()
                        .compare_enable(true)
                        .compare_op(vk::CompareOp::LESS_OR_EQUAL)
                        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                ),
                (
                    &red_hot::renderer::RedHotStageImage::Image(shadow_texture2),
                    vk::SamplerCreateInfo::default()
                        .compare_enable(true)
                        .compare_op(vk::CompareOp::LESS_OR_EQUAL)
                        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                ),
            ],
        );
        let debug_stage = renderer.register_stage(
            "Debug_Meshes".to_owned(),
            include_bytes!("./shader/vert.spv"),
            include_bytes!("./shader/green_frag.spv"),
            vk::PipelineRasterizationStateCreateInfo { cull_mode: vk::CullModeFlags::NONE, line_width: 1.0, polygon_mode: vk::PolygonMode::LINE, ..Default::default() },
            vk::PipelineDepthStencilStateCreateInfo {
                depth_test_enable: 1,
                depth_write_enable: 1,
                depth_compare_op: vk::CompareOp::LESS,
                front: vk::StencilOpState { fail_op: vk::StencilOp::KEEP, pass_op: vk::StencilOp::KEEP, depth_fail_op: vk::StencilOp::KEEP, compare_op: vk::CompareOp::ALWAYS, ..Default::default() },
                back: vk::StencilOpState { fail_op: vk::StencilOp::KEEP, pass_op: vk::StencilOp::KEEP, depth_fail_op: vk::StencilOp::KEEP, compare_op: vk::CompareOp::ALWAYS, ..Default::default() },
                max_depth_bounds: 1.0,
                ..Default::default()
            },
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
                vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 4 * 32 / 8 as u32 }, //. Normal
                vk::VertexInputAttributeDescription { location: 2, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 8 * 32 / 8 as u32 }, //. Color
            ],
            vec![red_hot::renderer::RedHotStageImage::SwapchainImage(), red_hot::renderer::RedHotStageImage::Image(depth_image)],
            &[
                vk::AttachmentDescription {
                    format: renderer.swapchain_image_format,
                    samples: vk::SampleCountFlags::TYPE_1,
                    load_op: vk::AttachmentLoadOp::LOAD,
                    initial_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    store_op: vk::AttachmentStoreOp::STORE,
                    final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    ..Default::default()
                },
                vk::AttachmentDescription {
                    format: vk::Format::D32_SFLOAT,
                    samples: vk::SampleCountFlags::TYPE_1,
                    load_op: vk::AttachmentLoadOp::CLEAR,
                    // initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, // TODO: non-undefined initial layout is currently unsupported, as I do not create the barriers to transition them into the correct state before the renderpass begins. In this case it is fine too, as we clear it anyways
                    final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                    ..Default::default()
                },
            ],
            [
                vk::ClearValue { color: vk::ClearColorValue { float32: [1.0, 0.0, 0.0, 0.0] } },
                vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } },
            ],
            &[vk::SubpassDescription::default()
                .color_attachments(&[vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }])
                .depth_stencil_attachment(&vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL })
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

        let texture_atlas_file: &mut [u8] = &mut read(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/text.bmp")).unwrap();
        let texture_atlas_bmp = parse_bmp(texture_atlas_file);

        let mut texture_atlas_rgba = Vec::with_capacity(texture_atlas_bmp.data.len() / 3 * 4);

        for i in (0..texture_atlas_bmp.data.len()).step_by(3) {
            texture_atlas_rgba.extend_from_slice(&[
                texture_atlas_bmp.data[i + 0],
                texture_atlas_bmp.data[i + 1],
                texture_atlas_bmp.data[i + 2],
                if texture_atlas_bmp.data[i + 0] == texture_atlas_bmp.data[i + 1] && texture_atlas_bmp.data[i + 1] == texture_atlas_bmp.data[i + 2] {
                    255 - texture_atlas_bmp.data[i + 0] //. Grey = transparent
                } else {
                    255
                },
            ]);
        }

        let texture_atlas = renderer.register_image(RedHotImageCreateInfo {
            size: ImageSize::Fixed(texture_atlas_bmp.width, texture_atlas_bmp.height),
            format: vk::Format::R8G8B8A8_UNORM,
            usage: vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            memory_flags: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            image_aspect_mask: vk::ImageAspectFlags::COLOR,
        });

        println!("red_hot: texture_atlas_bmp.data.len(): {}", texture_atlas_bmp.data.len());
        renderer.write_to_image(red_hot::renderer::RedHotStageImage::Image(texture_atlas), &texture_atlas_rgba);

        let ui_stage = renderer.register_stage(
            "UI".to_owned(),
            include_bytes!("./shader/ui_vert.spv"),
            include_bytes!("./shader/ui_frag.spv"),
            vk::PipelineRasterizationStateCreateInfo { cull_mode: vk::CullModeFlags::NONE, line_width: 1.0, polygon_mode: vk::PolygonMode::FILL, ..Default::default() },
            vk::PipelineDepthStencilStateCreateInfo {
                depth_test_enable: 0,
                depth_write_enable: 0,
                front: vk::StencilOpState { fail_op: vk::StencilOp::KEEP, pass_op: vk::StencilOp::KEEP, depth_fail_op: vk::StencilOp::KEEP, compare_op: vk::CompareOp::ALWAYS, ..Default::default() },
                back: vk::StencilOpState { fail_op: vk::StencilOp::KEEP, pass_op: vk::StencilOp::KEEP, depth_fail_op: vk::StencilOp::KEEP, compare_op: vk::CompareOp::ALWAYS, ..Default::default() },
                max_depth_bounds: 1.0,
                ..Default::default()
            },
            vk::PipelineColorBlendStateCreateInfo::default().logic_op(vk::LogicOp::CLEAR).attachments(&[vk::PipelineColorBlendAttachmentState {
                blend_enable: 1,
                src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
                dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::ONE,
                dst_alpha_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                alpha_blend_op: vk::BlendOp::ADD,
                color_write_mask: vk::ColorComponentFlags::RGBA,
            }]),
            [
                vk::VertexInputAttributeDescription { location: 0, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 0 as u32 }, //. Position
                vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 4 * 32 / 8 as u32 }, //. Normal
                vk::VertexInputAttributeDescription { location: 2, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 8 * 32 / 8 as u32 }, //. Color
            ],
            vec![red_hot::renderer::RedHotStageImage::SwapchainImage()],
            &[vk::AttachmentDescription {
                format: renderer.swapchain_image_format,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::LOAD,
                initial_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                store_op: vk::AttachmentStoreOp::STORE,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            }],
            [vk::ClearValue { color: vk::ClearColorValue { float32: [1.0, 0.0, 0.0, 0.0] } }],
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
            &[(
                &red_hot::renderer::RedHotStageImage::Image(texture_atlas),
                vk::SamplerCreateInfo::default()
                    .compare_enable(false)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
            )],
        );

        let meshes = vec![renderer.register_mesh(cube_mesh), renderer.register_mesh(pyramid_mesh)];
        let plane_meshi = renderer.register_mesh(plane_mesh);
        let inverse_cube_meshi = renderer.register_mesh(inverse_cube_mesh);
        let room_box = Object { transform: Transform { scale: [100.0, 100.0, 100.0].into(), ..Transform::default() }, mesh: inverse_cube_meshi };

        let mut rng = rand::rngs::StdRng::seed_from_u64(6969);
        let num_objs = 69;
        let mut objects: Vec<Object> = (0..num_objs)
            .map(|i| Object {
                transform: Transform {
                    position: Vec3::<f32> {
                        x: rng.random_range(-room_box.transform.scale.x..room_box.transform.scale.x),
                        y: rng.random_range(-room_box.transform.scale.y..room_box.transform.scale.y),
                        z: rng.random_range(-room_box.transform.scale.z..room_box.transform.scale.z),
                    },
                    rotation: Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 0.0 }.normalize(), i as f32 * TAU * 0.69),
                    scale: Vec3::<f32> { x: rng.random_range(0.5..10.0), y: rng.random_range(0.5..10.0), z: rng.random_range(0.5..10.0) },
                },
                mesh: meshes[i % meshes.len()],
            })
            .collect();

        let mut rotation_data: Vec<(Vec3<f32>, f32)> = (0..num_objs)
            .map(|i| {
                (
                    Vec3::<f32> {
                        x: rng.random_range(-room_box.transform.scale.x..room_box.transform.scale.x),
                        y: rng.random_range(-room_box.transform.scale.y..room_box.transform.scale.y),
                        z: rng.random_range(-room_box.transform.scale.z..room_box.transform.scale.z),
                    }
                    .normalize(),
                    rng.random_range(-5.0..5.0),
                )
            })
            .collect();

        let mut light_box = Object { transform: Transform::default(), mesh: meshes[0] };
        let mut light_box2 = Object { transform: Transform::default(), mesh: meshes[0] };

        let mut last_time;
        let mut current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let mut t = 0.0;
        let mut dt = 0.01;
        let mut control = 1.0;
        let mut should_close = false;
        let mut a = 1.0;
        let mut b = 1.0;
        let mut c = 0.05;
        let light_proj_matrix = perspective_matrix(TAU / 4.0, 0.1, 200.0);
        let mut typing = false;
        let mut text_input = String::new();
        let mut debug = false;

        let mut focused = false;

        const N_FRAMETIME: usize = 120;
        let mut frametime_circ_buffer = [0.0; N_FRAMETIME];
        let mut frametime_index = 0;

        while !should_close {
            //. Update the objects
            for (i, object) in &mut objects.iter_mut().enumerate() {
                object.transform.rotation = Quaternion::from_axis_rotation(rotation_data[i].0, rotation_data[i].1 * dt) * object.transform.rotation;
                object.transform.position = (rotation_data[i].0 * rotation_data[i].1 * b as f32 * dt) + object.transform.position;
                if object.transform.position.x >= (room_box.transform.scale.x - object.transform.scale.x) || object.transform.position.x <= -(room_box.transform.scale.x - object.transform.scale.x) {
                    rotation_data[i].0.x = -rotation_data[i].0.x;
                }
                if object.transform.position.y >= (room_box.transform.scale.y - object.transform.scale.y) || object.transform.position.y <= -(room_box.transform.scale.y - object.transform.scale.y) {
                    rotation_data[i].0.y = -rotation_data[i].0.y;
                }
                if object.transform.position.z >= (room_box.transform.scale.z - object.transform.scale.z) || object.transform.position.z <= -(room_box.transform.scale.z - object.transform.scale.z) {
                    rotation_data[i].0.z = -rotation_data[i].0.z;
                }
            }
            light_box.transform.position = Vec3 { x: 50.0 * f32::sin(t * 1.0), y: 50.0, z: 50.0 * f32::cos(t * 1.0) };
            light_box.transform.rotation = Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 1.0 }.normalize(), 0.01 * dt) * light_box.transform.rotation;
            light_box2.transform.position = Vec3 { x: 50.0 * f32::sin(PI + t * 1.0), y: 50.0 * f32::sin(PI + t * 1.0), z: 50.0 * f32::cos(PI + t * 1.0) };
            light_box2.transform.rotation = Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 1.0 }.normalize(), -0.1 * dt) * light_box2.transform.rotation;

            let light_dir = Vec3::forward().rotate(light_box.transform.rotation);
            let light_dir2 = Vec3::forward().rotate(light_box2.transform.rotation);

            cam_pitch = f32::clamp(cam_pitch % TAU, -TAU / 4.0, TAU / 4.0);
            cam_yaw = cam_yaw % TAU;
            let camera_transform = Transform {
                position,
                rotation: Quaternion::from_axis_rotation(Vec3 { x: 0.0, y: 1.0, z: 0.0 }.normalize(), cam_yaw)
                    * Quaternion::from_axis_rotation(Vec3 { x: -1.0, y: 0.0, z: 0.0 }.normalize(), cam_pitch),
                scale: Vec3::one(),
            };

            event_loop.pump_events(Some(std::time::Duration::ZERO), |event, _| {
                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested | WindowEvent::KeyboardInput { event: KeyEvent { state: ElementState::Pressed, physical_key: PhysicalKey::Code(KeyCode::Escape), .. }, .. },
                        ..
                    } => {
                        should_close = true; //. if we pressed close, close
                                             // *control_flow = ControlFlow::Exit;
                    },
                    Event::WindowEvent { event: WindowEvent::KeyboardInput { event: KeyEvent { state: ElementState::Pressed, physical_key: PhysicalKey::Code(keycode), text, .. }, .. }, .. } => {
                        if keycode == KeyCode::Enter {
                            //. Handle enter in either case
                            typing = !typing;
                        } else if typing {
                            if keycode == KeyCode::Backspace {
                                if !text_input.is_empty() {
                                    text_input.remove(text_input.len() - 1);
                                }
                            } else if let Some(text) = text {
                                text_input.extend(text.chars());
                            }
                        } else {
                            match keycode {
                                KeyCode::KeyW => position += Vec3::<f32>::forward().rotate(camera_transform.rotation),
                                KeyCode::KeyS => position += Vec3::<f32>::backwards().rotate(camera_transform.rotation),
                                KeyCode::KeyA => position += Vec3::<f32>::left().rotate(camera_transform.rotation),
                                KeyCode::KeyD => position += Vec3::<f32>::right().rotate(camera_transform.rotation),
                                KeyCode::Space => position += Vec3::<f32>::up(),
                                KeyCode::ControlLeft => position += Vec3::<f32>::down(),
                                KeyCode::KeyR => cam_pitch -= 0.3,
                                KeyCode::KeyF => cam_pitch += 0.3,
                                KeyCode::KeyQ => cam_yaw -= 0.3,
                                KeyCode::KeyE => cam_yaw += 0.3,
                                KeyCode::Digit1 => control -= 0.3,
                                KeyCode::Digit2 => control += 0.3,
                                KeyCode::KeyU => a -= 0.1,
                                KeyCode::KeyI => a += 0.1,
                                KeyCode::KeyJ => b -= 0.1,
                                KeyCode::KeyK => b += 0.1,
                                KeyCode::KeyN => c = 0.001 + (c - 0.01) % 1.001,
                                KeyCode::KeyM => c = 0.001 + (c + 0.01) % 1.001,
                                KeyCode::KeyZ => debug = !debug,
                                _ => (),
                            }
                        }
                    },
                    Event::DeviceEvent { event: MouseMotion { delta: (mouse_x, mouse_y) }, .. } => {
                        if focused {
                            cam_yaw += mouse_x as f32 / 40.0;
                            cam_pitch += mouse_y as f32 / 40.0;
                        }
                    },
                    Event::WindowEvent { event: WindowEvent::Resized(_physical_size), .. } => {
                        // TODO[Perf]: Wait until we've exited the pump_event to actually resize, and only register the size change here. This avoids doing a bunch of resize work repeadetly when someone is dragging.
                        let size = window.inner_size(); //. I think this is what we actually care about?? The other one did a weird "physical" resize that doesn't appear to do anything, but fucks my swapchain
                        if window_width != size.width || window_height != size.height {
                            window_width = size.width;
                            window_height = size.height;
                            // BUG: On GLaDOS (Debian KDE Wayland) windows get moved instead of resized, and the size is incomprehensible?!?!
                            renderer.resize_window(window_width, window_height);
                        }
                    },
                    Event::WindowEvent { event: WindowEvent::Focused(focus), .. } => {
                        focused = focus;
                        window.set_cursor_visible(!focus);
                    },
                    // Event::RedrawEventsCleared => *control_flow = ControlFlow::Exit, //. "return"
                    _ => (), //. ignore other events
                }
            });

            {
                //. Render
                renderer.render_begin(DrawUniform { _dummy: 69.0 });
                renderer.render_stage(
                    shadow_stage,
                    ShadowStageUniform { view_mat: light_box.transform.get_inverse_matrix(), proj_mat: light_proj_matrix },
                    objects.iter().map(|x| x.mesh).collect(),
                    objects.iter().map(|x| ObjectUniform { model_mat: x.transform.get_matrix(), is_light: 0 }).collect(),
                );
                renderer.render_stage(
                    shadow_stage2,
                    ShadowStageUniform { view_mat: light_box2.transform.get_inverse_matrix(), proj_mat: light_proj_matrix },
                    objects.iter().map(|x| x.mesh).collect(),
                    objects.iter().map(|x| ObjectUniform { model_mat: x.transform.get_matrix(), is_light: 0 }).collect(),
                );
                renderer.render_stage(
                    default_stage,
                    RenderStageUniform {
                        view_mat: camera_transform.get_inverse_matrix(),
                        proj_mat,
                        light_view_proj_mat: light_proj_matrix * light_box.transform.get_inverse_matrix(),
                        light_view_proj_mat2: light_proj_matrix * light_box2.transform.get_inverse_matrix(),
                        light_dir: [light_dir.x, light_dir.y, light_dir.z, 0.0],
                        light_dir2: [light_dir2.x, light_dir2.y, light_dir2.z, 0.0],
                    },
                    objects.iter().map(|x| x.mesh).chain([room_box.mesh, light_box.mesh, light_box2.mesh]).collect(),
                    objects
                        .iter()
                        .map(|x| ObjectUniform { model_mat: x.transform.get_matrix(), is_light: 0 })
                        .chain([
                            ObjectUniform { model_mat: room_box.transform.get_matrix(), is_light: 0 },
                            ObjectUniform { model_mat: light_box.transform.get_matrix(), is_light: 1 },
                            ObjectUniform { model_mat: light_box2.transform.get_matrix(), is_light: 1 },
                        ])
                        .collect(),
                );
                if debug {
                    renderer.render_stage(
                        debug_stage,
                        RenderStageUniform {
                            view_mat: camera_transform.get_inverse_matrix(),
                            proj_mat,
                            light_view_proj_mat: Mat4x4::<f32>::zero(),  //. Unused
                            light_view_proj_mat2: Mat4x4::<f32>::zero(), //. Unused
                            light_dir: [0.0, 0.0, 0.0, 0.0],             //. Unused
                            light_dir2: [0.0, 0.0, 0.0, 0.0],            //. Unused
                        },
                        objects.iter().map(|x| x.mesh).chain([room_box.mesh, light_box.mesh, light_box2.mesh]).collect(),
                        objects
                            .iter()
                            .map(|x| ObjectUniform { model_mat: x.transform.get_matrix(), is_light: 0 })
                            .chain([
                                ObjectUniform { model_mat: room_box.transform.get_matrix(), is_light: 0 },
                                ObjectUniform { model_mat: light_box.transform.get_matrix(), is_light: 1 },
                                ObjectUniform { model_mat: light_box2.transform.get_matrix(), is_light: 1 },
                            ])
                            .collect(),
                    );
                }

                let objects: Vec<UiObjectUniform> = if text_input.is_empty() { "*ENTER*" } else { &text_input }
                    .chars()
                    .enumerate()
                    .map(|(i, char)| {
                        let (atlas_x, atlas_y) = texture_atlas_letter_coords(char);
                        let scale = c;
                        let row_count = (1.0 / scale) as usize;
                        let ratio = window_width as f32 / window_height as f32;

                        UiObjectUniform {
                            model_mat: Transform {
                                position: Vec3 { x: -1.0 + scale + (i % row_count) as f32 * scale * 2.0, y: -1.0 + scale * ratio + ((i / row_count) as f32 * scale * ratio * 2.0), z: 0.0 },
                                rotation: Quaternion::from_axis_rotation(Vec3::right(), TAU / 4.0),
                                scale: Vec3 { x: scale, y: scale, z: scale * ratio },
                            }
                            .get_matrix(),

                            color: Vec3 { x: 0.3, y: 0.2, z: 0.9 },

                            use_color: if "abcdefghijklmnopqrstuvwxyz0123456789+-?=!.:,; ".contains(char.to_ascii_lowercase()) {
                                1
                            } else {
                                0
                            },

                            texture_offset: [
                                (1.0 + atlas_x as f32 * 99.0) / texture_atlas_bmp.width as f32,
                                (1.0 + atlas_y as f32 * 98.5) / texture_atlas_bmp.height as f32,
                            ],

                            texture_area: [98.0 / texture_atlas_bmp.width as f32, 98.0 / texture_atlas_bmp.height as f32],
                        }
                    })
                    .collect();

                renderer.render_stage(
                    ui_stage,
                    DrawUniform {
                        _dummy: if typing {
                            f32::from_be_bytes([0xFF, 0xFF, 0xFF, 0xFF])
                        } else {
                            f32::from_be_bytes([0x00, 0x00, 0x00, 0x00])
                        },
                    },
                    vec![plane_meshi; objects.len()],
                    objects,
                );

                renderer.render_commit();
            }
            last_time = current_time;
            current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            dt = (current_time - last_time).as_secs_f32();
            frametime_circ_buffer[frametime_index] = dt;
            frametime_index = (frametime_index + 1) % N_FRAMETIME;
            if frametime_circ_buffer[N_FRAMETIME - 1] != 0.0 {
                //. Wait untill buffer is filled
                let avg_frametime = frametime_circ_buffer.iter().sum::<f32>() / N_FRAMETIME as f32;
                println!("fps: {}, {a} {b} {c}", 1.0 / avg_frametime);
            }

            dt *= a; //. Modifyer
            t = t + dt;
        }

        renderer.destroy();
        println!("Goodbye");
    }
}
