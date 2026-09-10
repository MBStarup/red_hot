use std::{
    f32::consts::{PI, TAU},
    fs::read,
    mem::size_of,
    time::{SystemTime, UNIX_EPOCH},
};

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

fn main() {
    unsafe {
        println!("Example 02: glb loading");
        let mut window_width: u32 = 2000;
        let mut window_height: u32 = 1200;
        let mut glb_file = read(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/default_monkey.glb")).unwrap();
        let gltf_data = glb::parse(&mut glb_file);

        // TODO: use "where" instead, or at least enforce the order
        let position_accessor = &gltf_data.0.accessors[gltf_data.0.meshes[0].primitives[0].attributes.position.expect("Expeted gltf file to have vertex attribute 'position'") as usize];
        let normal_accessor = &gltf_data.0.accessors[gltf_data.0.meshes[0].primitives[0].attributes.normal.expect("Expeted gltf file to have vertex attribute 'normal'") as usize];

        assert!(
            position_accessor.count == normal_accessor.count,
            "this currently assumes all indecies are shared between all dimensions, i.e. a mesh has one single index buffer, not one per vertex attribute. This does seem to be the case.",
        );
        let vert_count = position_accessor.count as usize;

        // TODO: abstract recast into function
        // TODO: handle cases with interleaved data (i.e. where they have a stride)
        let position_data_bytes = gltf_data.0.access_buffer(&gltf_data.1, position_accessor);
        let position_data_ptr = position_data_bytes.as_ptr() as *const [f32; 3];
        let position_data_len = position_data_bytes.len() / (size_of::<[f32; 3]>()); // TODO: if this is always vert_count, consider inlining it in the assert
        assert!(vert_count == position_data_len, "expected {vert_count} positions, found {position_data_len}");

        let normal_data_bytes = gltf_data.0.access_buffer(&gltf_data.1, normal_accessor);
        let normal_data_ptr = normal_data_bytes.as_ptr() as *const [f32; 3];
        let normal_data_len = normal_data_bytes.len() / (size_of::<[f32; 3]>()); // TODO: if this is always vert_count, consider inlining it in the assert
        assert!(vert_count == normal_data_len, "expected {vert_count} normals, found {normal_data_len}");

        #[allow(dead_code)]
        #[derive(Clone, Debug, Copy)]
        #[repr(C)]
        struct Vertex {
            pos: [f32; 4],
            normal: [f32; 4],
            color: [f32; 4],
        }

        let mut verts = Vec::<Vertex>::with_capacity(vert_count);
        for vert in 0..vert_count {
            //? Does this create the struct on stack and then copy, or is it able to be smart and emblace_back due to ownership semantics?
            let pos3 = position_data_ptr.offset(vert as isize).read_unaligned(); //. This performs a copy maybe?
            let norm3 = normal_data_ptr.offset(vert as isize).read_unaligned(); //. This performs a copy maybe?
            verts.push(Vertex {
                #[rustfmt::skip]
                // TODO: make [f32;3] -> [f32;4] function/macro
                pos: [pos3[0], pos3[1], pos3[2], 1.0],
                normal: [norm3[0], norm3[1], norm3[2], 1.0],
                color: [1.0, 0.0, 0.0, 1.0],
            });
        }

        let index_accessor = &gltf_data.0.accessors[gltf_data.0.meshes[0].primitives[0].indices as usize];

        let index_data_bytes = gltf_data.0.access_buffer(&gltf_data.1, index_accessor);
        let index_data_ptr = index_data_bytes.as_ptr() as *const u16;
        let index_data_len = index_data_bytes.len() / (size_of::<u16>() * 1);
        let index_data: &[u16] = std::slice::from_raw_parts(index_data_ptr, index_data_len);

        let imported_mesh = Mesh { vertices: verts, indices: index_data.iter().map(|&e| e as u32).collect() };

        //. Fake uniform not used by the shader, zero sized uniforms trips up the renderer atm
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
            .create_window(
                Window::default_attributes()
                    .with_title("02-glb-loading")
                    .with_inner_size(winit::dpi::LogicalSize::new(f64::from(window_width), f64::from(window_height))),
            )
            .unwrap();
        window.set_cursor_grab(winit::window::CursorGrabMode::Confined).unwrap();
        window.set_cursor_visible(false);

        let mut renderer = Renderer::<Vertex>::new(&window, window_width, window_height);

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
                    final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                    ..Default::default()
                },
                vk::AttachmentDescription {
                    format: vk::Format::D32_SFLOAT,
                    samples: vk::SampleCountFlags::TYPE_1,
                    load_op: vk::AttachmentLoadOp::CLEAR,
                    final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                    ..Default::default()
                },
            ],
            [
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.05, 0.00, 0.00, 1.0] } }, //. was renderer.clear_color
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

        let imported_meshi = renderer.register_mesh(imported_mesh);

        let mut objects = vec![Object {
            transform: Transform { position: Vec3::<f32> { x: 0.0, y: 0.0, z: 0.0 }, rotation: Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 0.0 }.normalize(), PI), scale: Vec3::one() },
            mesh: imported_meshi,
        }];

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
                object.transform.scale = Vec3::one() * 3.5 + Vec3::one() * f32::sin(t * 1.3) * 0.3;
                object.transform.rotation = Quaternion::from_axis_rotation(Vec3 { x: 0.0, y: 1.0, z: 0.0 }.normalize(), dt) * object.transform.rotation;
            }

            light_rotation = Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 0.0 }.normalize(), t * 1.0);
            light_dir = (Vec3::up()).normalize().rotate(light_rotation).normalize();

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
                    },
                    Event::WindowEvent { event: WindowEvent::KeyboardInput { event: KeyEvent { state: ElementState::Pressed, physical_key: PhysicalKey::Code(keycode), .. }, .. }, .. } => {
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
                            KeyCode::KeyU => a /= 1.1,
                            KeyCode::KeyI => a *= 1.1,
                            KeyCode::KeyJ => b /= 1.1,
                            KeyCode::KeyK => b *= 1.1,
                            KeyCode::KeyN => c /= 1.1,
                            KeyCode::KeyM => c *= 1.1,
                            _ => (),
                        }
                    },
                    Event::DeviceEvent { event: MouseMotion { delta: (mouse_x, mouse_y) }, .. } => {
                        if focused {
                            cam_yaw += mouse_x as f32 / 40.0;
                            cam_pitch += mouse_y as f32 / 40.0;
                        }
                    },
                    Event::WindowEvent { event: WindowEvent::Resized(_physical_size), .. } => {
                        let size = window.inner_size();
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
                    _ => (), //. ignore other events
                }
            });

            renderer.render_begin(DrawUniform { _dummy: 0.0 });
            renderer.render_stage(
                default_stage,
                RenderStageUniform { view_mat: camera_transform.get_inverse_matrix(), proj_mat, light_dir, ambient_light: 0.01 },
                objects.iter().map(|x| x.mesh).collect(),
                objects.iter().map(|x| ObjectUniform { model_mat: x.transform.get_matrix() }).collect(),
            );
            renderer.render_commit();

            last_time = current_time;
            current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            dt = (current_time - last_time).as_secs_f32();
            t = t + dt;
        }

        renderer.destroy();
        println!("Done!");
    }
}
