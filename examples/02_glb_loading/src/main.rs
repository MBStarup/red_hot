use std::{
    f32::consts::{PI, TAU},
    fs::read,
    mem::size_of,
    time::{SystemTime, UNIX_EPOCH},
};

use ash::vk;
use red_hot::{
    math::{perspective_matrix, Mat4x4, Quaternion, Transform, Vec3},
    renderer::{Mesh, MeshIndex, Renderer},
    Vertex,
};

use winit::{
    event::{DeviceEvent::MouseMotion, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

fn main() {
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

    let mut verts = Vec::<Vertex>::with_capacity(vert_count);
    for vert in 0..vert_count {
        //? Does this create the struct on stack and then copy, or is it able to be smart and emblace_back due to ownership semantics?
        let pos3 = unsafe { position_data_ptr.offset(vert as isize).read_unaligned() }; //. This performs a copy maybe?
        let norm3 = unsafe { normal_data_ptr.offset(vert as isize).read_unaligned() }; //. This performs a copy maybe?
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
    let index_data: &[u16] = unsafe { std::slice::from_raw_parts(index_data_ptr, index_data_len) };

    let imported_mesh = Mesh { vertices: verts, indices: index_data.iter().map(|&e| e as u32).collect() };

    // TODO: load assets
    #[allow(dead_code)]
    #[derive(Clone, Debug, Copy, Vertex)]
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

    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("02-glb-loading")
        .with_inner_size(winit::dpi::LogicalSize::new(f64::from(window_width), f64::from(window_height)))
        .build(&event_loop)
        .unwrap();
    window.set_cursor_grab(winit::window::CursorGrabMode::Confined).unwrap();
    window.set_cursor_visible(false);

    let mut renderer = Renderer::<Vertex, _>::new(&window, window_width, window_height, Vertex::ATTRIBUTE_DESCRIPTIONS);
    renderer.set_vertex_shader(include_bytes!("./shader/vert.spv"));
    renderer.set_fragment_shader(include_bytes!("./shader/frag.spv"));
    let imported_meshi = renderer.register_mesh(imported_mesh);

    let mut objects = vec![Object {
        transform: Transform { position: Vec3::<f32> { x: 0.0, y: 0.0, z: 0.0 }, rotation: Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 0.0 }.normalize(), PI), scale: Vec3::one() },
        mesh: imported_meshi,
    }];

    renderer.clear_color = [0.05, 0.00, 0.00, 1.0];

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
                    renderer.render(
                        DrawUniform { view_mat: camera_transform.get_inverse_matrix(), proj_mat, light_dir: light_dir, ambient_light: 0.01 },
                        objects.iter().map(|x| x.mesh).collect(),
                        objects.iter().map(|x| ObjectUniform { model_mat: x.transform.get_matrix() }).collect(),
                    );
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
    println!("Done!");
}
