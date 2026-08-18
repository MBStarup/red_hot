use std::{
    f32::consts::TAU,
    time::{SystemTime, UNIX_EPOCH},
};

use ash::vk;
use red_hot::{
    math::{perspective_matrix, Mat4x4, Quaternion, Transform, Vec3},
    renderer::{Mesh, MeshIndex, Renderer},
};

use winit::{
    event::{DeviceEvent::MouseMotion, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

fn main() {
    println!("Example 01: shapes");
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
        light_dir: Vec3<f32>,
        ambient_light: f32,
    }

    #[allow(dead_code)]
    #[derive(Clone, Debug, Copy)]
    #[repr(C)]
    struct ObjectUniform {
        model_mat: Mat4x4<f32>,
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
    let cube_mesh = {
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
        let indices = (0..vertices.len() as u32).collect();
        Mesh::<Vertex> {vertices, indices}
    };

    #[rustfmt::skip]
    let plane_mesh = {
        let vertices = vec![
            Vertex { pos: [-1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] }, //. UP (LEFT/FRONT)
            Vertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] },
            Vertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] },
            Vertex { pos: [ 1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] }, //. UP (RIGHT/BACK)
            Vertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] },
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

    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("01-shapes")
        .with_inner_size(winit::dpi::LogicalSize::new(f64::from(window_width), f64::from(window_height)))
        .build(&event_loop)
        .unwrap();
    window.set_cursor_grab(winit::window::CursorGrabMode::Confined).unwrap();
    window.set_cursor_visible(false);

    let mut renderer = Renderer::<Vertex>::new(&window, window_width, window_height);

    let depth_image = unsafe {
        renderer.create_image(
            window_width,
            window_height,
            vk::Format::D32_SFLOAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
    };
    let default_stage = unsafe {
        renderer.register_stage(
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
            *vk::PipelineColorBlendStateCreateInfo::builder().logic_op(vk::LogicOp::CLEAR).attachments(&[vk::PipelineColorBlendAttachmentState {
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
            &[&red_hot::renderer::RedHotStageImage::SwapchainImage(), &depth_image],
            &[
                vk::AttachmentDescription {
                    format: renderer.swapchain_image_format,
                    samples: vk::SampleCountFlags::TYPE_1,
                    load_op: vk::AttachmentLoadOp::CLEAR,
                    store_op: vk::AttachmentStoreOp::STORE,
                    final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
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
            &[*vk::SubpassDescription::builder()
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
        )
    };

    let meshes = vec![renderer.register_mesh(cube_mesh), renderer.register_mesh(pyramid_mesh), renderer.register_mesh(plane_mesh)];

    let mut objects: Vec<Object> = (0..200)
        .map(|i| Object {
            transform: Transform {
                position: Vec3 { x: 3.0 * i as f32, y: 0.0, z: 0.0 },
                rotation: Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 0.0 }.normalize(), i as f32 * TAU * 0.69),
                scale: Vec3::<f32>::one(),
            },
            mesh: meshes[i % meshes.len()],
        })
        .collect();

    let mut last_time;
    let mut current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let mut t = 0.0;
    let mut dt = 0.01;
    let mut control = 1.0;
    let mut should_close = false;
    let mut light_dir;
    let mut light_rotation;
    let mut a = 1.0;
    let mut b = 1.0;
    let mut c = 1.0;

    let mut focused = false;

    while !should_close {
        //. Update the objects
        for object in &mut objects {
            object.transform.rotation = Quaternion::from_axis_rotation(Vec3 { x: 0.0, y: 1.0, z: 0.0 }.normalize(), dt) * object.transform.rotation;
        }

        light_rotation = Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 0.0 }.normalize(), 1.0 + t * 1.0);
        light_dir = (Vec3::up()).normalize().rotate(light_rotation).normalize();

        cam_pitch = f32::clamp(cam_pitch % TAU, -TAU / 4.0, TAU / 4.0);
        cam_yaw = cam_yaw % TAU;
        let camera_transform = Transform {
            position,
            rotation: Quaternion::from_axis_rotation(Vec3 { x: 0.0, y: 1.0, z: 0.0 }.normalize(), cam_yaw) * Quaternion::from_axis_rotation(Vec3 { x: -1.0, y: 0.0, z: 0.0 }.normalize(), cam_pitch),
            scale: Vec3::one(),
        };

        event_loop.run_return(|event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested | WindowEvent::KeyboardInput { input: KeyboardInput { state: ElementState::Pressed, virtual_keycode: Some(VirtualKeyCode::Escape), .. }, .. },
                    ..
                } => {
                    should_close = true; //. if we pressed close, close
                    *control_flow = ControlFlow::Exit;
                },
                Event::WindowEvent { event: WindowEvent::KeyboardInput { input: KeyboardInput { state: ElementState::Pressed, virtual_keycode: Some(keycode), .. }, .. }, .. } => match keycode {
                    VirtualKeyCode::W => position += Vec3::<f32>::forward().rotate(camera_transform.rotation),
                    VirtualKeyCode::S => position += Vec3::<f32>::backwards().rotate(camera_transform.rotation),
                    VirtualKeyCode::A => position += Vec3::<f32>::left().rotate(camera_transform.rotation),
                    VirtualKeyCode::D => position += Vec3::<f32>::right().rotate(camera_transform.rotation),
                    VirtualKeyCode::Space => position += Vec3::<f32>::up(),
                    VirtualKeyCode::LControl => position += Vec3::<f32>::down(),
                    VirtualKeyCode::R => cam_pitch -= 0.3,
                    VirtualKeyCode::F => cam_pitch += 0.3,
                    VirtualKeyCode::Q => cam_yaw -= 0.3,
                    VirtualKeyCode::E => cam_yaw += 0.3,
                    VirtualKeyCode::Key1 => control -= 0.3,
                    VirtualKeyCode::Key2 => control += 0.3,
                    VirtualKeyCode::U => a /= 1.1,
                    VirtualKeyCode::I => a *= 1.1,
                    VirtualKeyCode::J => b /= 1.1,
                    VirtualKeyCode::K => b *= 1.1,
                    VirtualKeyCode::N => c /= 1.1,
                    VirtualKeyCode::M => c *= 1.1,
                    _ => (),
                },
                Event::DeviceEvent { event: MouseMotion { delta: (mouse_x, mouse_y) }, .. } => {
                    if focused {
                        cam_yaw += mouse_x as f32 / 40.0;
                        cam_pitch += mouse_y as f32 / 40.0;
                    }
                },
                Event::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                    window_width = size.width;
                    window_height = size.height;
                    // BUG: On GLaDOS (Debian KDE Wayland) windows get moved instead of resized, and the size is incomprehensible?!?!
                    // TODO: update swapchain and stuff in renderer
                },
                Event::WindowEvent { event: WindowEvent::Focused(focus), .. } => {
                    focused = focus;
                    window.set_cursor_visible(!focus);
                },
                Event::MainEventsCleared => {
                    renderer.render_begin(DrawUniform { _dummy: 69.0 });
                    renderer.render_stage(
                        default_stage,
                        RenderStageUniform { view_mat: camera_transform.get_inverse_matrix(), proj_mat, light_dir, ambient_light: 0.2 },
                        objects.iter().map(|x| x.mesh).collect(),
                        objects.iter().map(|x| ObjectUniform { model_mat: x.transform.get_matrix() }).collect(),
                    );
                    renderer.render_commit();
                },
                Event::RedrawEventsCleared => *control_flow = ControlFlow::Exit, //. "return"
                _ => (),                                                         //. ignore other events
            }
        });

        last_time = current_time;
        current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        dt = (current_time - last_time).as_secs_f32();
        t = t + dt;
    }

    renderer.destroy();
    println!("Goodbye");
}
