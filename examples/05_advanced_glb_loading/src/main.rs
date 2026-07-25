use std::{
    f32::consts::{PI, TAU},
    fs::read,
    mem::size_of,
    time::{SystemTime, UNIX_EPOCH},
};

use ash::vk;
use glb::Gltf;
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

const SKELETON_SIZE: usize = 24;
const BONES_PER_VERT: usize = 4;

// TODO: This is a stupid bandaid fix lmao
#[allow(dead_code)]
#[derive(Clone, Debug, Copy)]
enum Vec3or4Array<'a> {
    Vec3Array(&'a [[f32; 3]]),
    Vec4Array(&'a [[f32; 4]]),
}

impl Vec3or4Array<'_> {
    fn len(&self) -> usize {
        match self {
            Vec3or4Array::Vec3Array(items) => items.len(),
            Vec3or4Array::Vec4Array(items) => items.len(),
        }
    }
}

impl std::ops::Index<usize> for Vec3or4Array<'_> {
    type Output = [f32];

    fn index(&self, index: usize) -> &Self::Output {
        match self {
            Vec3or4Array::Vec3Array(items) => &items[index],
            Vec3or4Array::Vec4Array(items) => &items[index],
        }
    }
}

impl<'a> Into<&'a [[f32; 3]]> for Vec3or4Array<'a> {
    fn into(self) -> &'a [[f32; 3]] {
        match self {
            Vec3or4Array::Vec3Array(items) => items,
            Vec3or4Array::Vec4Array(_) => panic!("whoops this one was a Vec4"),
        }
    }
}

impl<'a> Into<&'a [[f32; 4]]> for Vec3or4Array<'a> {
    fn into(self) -> &'a [[f32; 4]] {
        match self {
            Vec3or4Array::Vec3Array(_) => panic!("whoops this one was a Vec3"),
            Vec3or4Array::Vec4Array(items) => items,
        }
    }
}

#[derive(Clone, Copy)]
struct Object {
    transform: Transform<f32>,
    mesh: MeshIndex,
}

//. Each animation for a given animation target is given as a function, A: (f32 -> Vec3 | Vec4)
//. This function is defined via timestamps[] and keyframes[] below
//. A(x), if x is exactly the i'th timestamp, timestamp[i], A(x) = keyframes[i]
//. Else, due to the monotonicity of timestamps, timestamps[i] < x < timestamps[i+1]
//. In this case you find the t, such that x = lerp(timestamps[i], timestamps[i+1], t), and A(x) = lerp(keyframes[i], keyframes[i+1], t)
//. Unless x < timestamps[0] or x > timestamps.last(), in which case, A(x) is keyframes[0] or keframes.last() respectively
struct AnimatedTarget<'a> {
    target: glb::Target,
    timestamps: &'a [f32],
    keyframes: Vec3or4Array<'a>,
}

struct Animation<'a> {
    name: String,
    animated_targets: Vec<AnimatedTarget<'a>>,
    inverse_bind_matricies: &'a [Mat4x4<f32>],
    joint_heirachy: Vec<(u32, Option<u32>)>,
}

struct AnimationHandler<'a> {
    animations: Vec<Animation<'a>>,
    current_animation: u8, //. Assumes <= 255 animations
    current_anim_time: f32,
}

fn animations_from_gltf_data<'a>(gltf_header: &Gltf, gltf_buffer: &[u8]) -> Vec<Animation<'a>> {
    let animations = gltf_header.animations.iter().map(|anim| {
        let animation: Vec<AnimatedTarget<'_>> = anim
            .channels
            .iter()
            .map(|channel| {
                let sampler_index = channel.sampler as usize;
                let input_data_bytes = {
                    let buffer_view = &gltf_header.buffer_views[(gltf_header.accessors[anim.samplers[sampler_index].input as usize]).buffer_view as usize];
                    let buffer_start = buffer_view.byte_offset.unwrap_or(0) as usize;
                    &gltf_buffer[buffer_start..buffer_start + (buffer_view.byte_length as usize)]
                };
                let input_data: &[f32] = unsafe { std::slice::from_raw_parts(input_data_bytes.as_ptr() as *const f32, input_data_bytes.len() / (size_of::<f32>())) };

                let output_data_bytes = {
                    let buffer_view = &gltf_header.buffer_views[(gltf_header.accessors[anim.samplers[sampler_index].output as usize]).buffer_view as usize];
                    let buffer_start = buffer_view.byte_offset.unwrap_or(0) as usize;
                    &gltf_buffer[buffer_start..buffer_start + (buffer_view.byte_length as usize)]
                };

                // TODO: This is a bandaid fix
                let output_data: Vec3or4Array;
                match channel.target.path.as_str() {
                    "rotation" => {
                        output_data = Vec3or4Array::Vec4Array(unsafe { std::slice::from_raw_parts(output_data_bytes.as_ptr() as *const [f32; 4], output_data_bytes.len() / (size_of::<[f32; 4]>())) })
                    },
                    "translation" | "scale" => {
                        output_data = Vec3or4Array::Vec3Array(unsafe { std::slice::from_raw_parts(output_data_bytes.as_ptr() as *const [f32; 3], output_data_bytes.len() / (size_of::<[f32; 3]>())) })
                    },
                    x => panic!("Unexpected animation target path when parsing animation: {x:?}"),
                }
                //. Note we do a small clone here, this could be improved if we decide path can be an enum
                AnimatedTarget { target: glb::Target { node: channel.target.node, path: channel.target.path.clone() }, timestamps: input_data, keyframes: output_data }
            })
            .collect();

        //# Inverse bind matrix
        let inverse_bind_matrix_accessor = &gltf_header.accessors[gltf_header.skins[0].inverse_bind_matrices.unwrap() as usize];
        let inverse_bind_matrix_data_bytes = gltf_header.access_buffer(&gltf_buffer, inverse_bind_matrix_accessor);
        let inverse_bind_matrix_data_ptr = inverse_bind_matrix_data_bytes.as_ptr() as *const Mat4x4<f32>;
        let inverse_bind_matrix_data_len = inverse_bind_matrix_data_bytes.len() / (size_of::<Mat4x4<f32>>()); // TODO: if this is always vert_count, consider inlining it in the assert
        let inverse_bind_matrix_data: &[Mat4x4<f32>] = unsafe { std::slice::from_raw_parts(inverse_bind_matrix_data_ptr, inverse_bind_matrix_data_len) };

        //# Create joint tree for the joints being animated
        let mut joints = animation.iter().map(|x| x.target.node).collect::<Vec<_>>();
        joints.dedup();
        let joint_heirachy = joints
            .iter()
            .map(|j| {
                for joint in joints.iter() {
                    if gltf_header.nodes[*joint as usize].children.contains(j) {
                        return (*j, Some(*joint));
                    }
                }
                return (*j, None);
            })
            .collect::<Vec<_>>();

        Animation { name: anim.name.clone(), animated_targets: animation, inverse_bind_matricies: inverse_bind_matrix_data, joint_heirachy: joint_heirachy }
    });
    animations.collect()
}

fn animated_skeleton(Animation { name, animated_targets, inverse_bind_matricies, joint_heirachy }: &Animation, time: f32) -> [Mat4x4<f32>; SKELETON_SIZE] {
    let mut skeleton = vec![Mat4x4::<f32>::unit(); SKELETON_SIZE];
    {
        let mut joint_transforms: Vec<Mat4x4<f32>> = Vec::with_capacity(joint_heirachy.len());
        //. Bones
        {
            let mut parents: Vec<Option<u32>> = Vec::with_capacity(joint_heirachy.len());
            let mut any_has_parent = false;
            for i in 0..joint_heirachy.len() {
                parents.push(joint_heirachy.iter().position(|x| Some(x.0) == joint_heirachy[i].1).map(|x| x as u32)); // TODO: assumes all parent ids are present, so no match means parent = None
                if parents[i] != None {
                    any_has_parent = true;
                }
            }

            //. Calculate the transformation for the bone, relative to it's parent
            for i in 0..joint_heirachy.len() {
                let (joint, _) = joint_heirachy[i];
                let anim_time = (time / 1.00) % animated_targets[0].timestamps.last().unwrap(); //. Assumes all animated properties in a animated_targets is animated for the same range (more specifically that they all end at the same time)
                let mut bone_transform = Transform::<f32>::default();
                for AnimatedTarget { target, timestamps, keyframes } in animated_targets.iter().filter(|AnimatedTarget { target, .. }| target.node == joint) {
                    // TODO: there's 100% a more concise, readable, and "correct" way to align these inputs with outputs
                    assert!(timestamps.len() >= 2);
                    assert!(keyframes.len() >= 1);
                    let mut anim_from = 0;
                    let mut anim_to = 0;
                    let mut lerp_t = 0.0;

                    if keyframes.len() == 1 {
                    } else {
                        while anim_to < keyframes.len() && anim_time > timestamps[anim_from + 1] {
                            anim_from = anim_to;
                            anim_to += 1;
                        }
                        if anim_to >= keyframes.len() {
                            anim_to = anim_from;
                        }

                        if anim_from == anim_to {
                            lerp_t = 0.0;
                        } else {
                            lerp_t = (anim_time - timestamps[anim_from]) / (timestamps[anim_to] - timestamps[anim_from]);
                        }
                    }
                    match target.path.as_str() {
                        "translation" => {
                            let from = Into::<&[[f32; 3]]>::into(*keyframes)[anim_from].into();
                            let to = Into::<&[[f32; 3]]>::into(*keyframes)[anim_to].into();
                            let result = Vec3::lerp(from, to, lerp_t);
                            bone_transform.position = result;
                        },
                        "rotation" => {
                            let from = Into::<&[[f32; 4]]>::into(*keyframes)[anim_from].into();
                            let to = Into::<&[[f32; 4]]>::into(*keyframes)[anim_to].into();
                            let result = Quaternion::lerp(from, to, lerp_t);
                            bone_transform.rotation = result;
                        },
                        "scale" => bone_transform.scale = Vec3::lerp(Into::<&[[f32; 3]]>::into(*keyframes)[anim_from].into(), Into::<&[[f32; 3]]>::into(*keyframes)[anim_to].into(), lerp_t),
                        x => println!("Not animating property: {x}"),
                    }
                }
                joint_transforms.push(bone_transform.get_matrix());
            }

            while any_has_parent {
                any_has_parent = false;
                for i in 0..joint_heirachy.len() {
                    if parents[i] != None {
                        let parent = parents[i].unwrap() as usize;
                        joint_transforms[i] = joint_transforms[parent] * joint_transforms[i]; //. I'd think that it would be "first apply the parent transform, then the "rest"", but for some reason this order seems to work instead...
                        parents[i] = parents[parent]; //. If that parent was relative to some other parent, then the transfomration in result[i] is only the transformation relative to that new parent, not "global" yet
                        assert_eq!(parents[i], None, "non ordered bone array"); //. As long as the bone array is ordered from the root, we should only hit this while loop once. This assert was just as a sanity check and can be removed to allow for non ordered bone arrays
                        if parents[i] != None {
                            any_has_parent = true;
                        }
                    }
                }
            }

            for i in 0..joint_heirachy.len() {
                //. First undo the bind pose, then apply the current pose
                joint_transforms[i] = joint_transforms[i] * inverse_bind_matricies[i].transpose();
            }
        }
        for i in 0..joint_transforms.len() {
            skeleton[i] = joint_transforms[i];
        }
    }
    return skeleton.try_into().unwrap();
}

fn main() {
    assert_eq!(BONES_PER_VERT, 4, "VertexInputAttributeDescription not generalized");
    println!("Example 05: glb loading with animation");
    let mut window_width: u32 = 1600;
    let mut window_height: u32 = 900;

    #[allow(dead_code)]
    #[derive(Clone, Debug, Copy)]
    #[repr(C)]
    struct DrawUniform {
        view_mat: Mat4x4<f32>,
        proj_mat: Mat4x4<f32>,
        light_dir: Vec3<f32>,
        ambient_light: f32,
    }

    // NOTE: Mat4x4 (and other vec4 like types) must, by std140, be 16 byte aligned. So moving `selected` to the beginning of the struct would break this
    // NOTE: We could make Mat4x4 #[repr(align(16))], however this would force it everywhere, which is a kinda big issue when reinterpret casting from buffers of arbritrary glTF files, which will not, generally, be aligned
    // TODO: All of this should probably be fixed along with the shader tools required for generating/verifying agreement between the shaders and the Rust code
    // NOTE: We could also either wrap the Mat4x4 type in another, 16 byte aligned, UniformMat4x4, or add a Writer interface which is responsible for this logic. I, however, don't like either of those solutions.
    #[allow(dead_code)]
    #[derive(Clone, Debug, Copy)]
    #[repr(C)]
    struct AnimatedObjectUniform {
        model_mat: Mat4x4<f32>,
        skeleton: [Mat4x4<f32>; SKELETON_SIZE], //. Hard-coded max bone count for now
        selected: u32,
    }

    #[allow(dead_code)]
    #[derive(Clone, Debug, Copy)]
    #[repr(C)]
    struct AnimatedVertex {
        pos: [f32; 4],
        normal: [f32; 4],
        color: [f32; 4],
        bones: [u8; BONES_PER_VERT],         //. Indexes of which bones affect this vertex
        bone_weights: [f32; BONES_PER_VERT], //. Weights of how much each of those bones affect
    }

    fn animated_vertex_mesh_from_gltf_data<'a>(gltf_header: &Gltf, gltf_buffer: &'a [u8]) -> Mesh<AnimatedVertex> {
        let mesh = gltf_header.meshes.first().expect("");
        // TODO: use "where" instead, or at least enforce the order
        let position_accessor = &gltf_header.accessors[mesh.primitives[0].attributes.position.unwrap() as usize];
        let normal_accessor = &gltf_header.accessors[mesh.primitives[0].attributes.normal.unwrap() as usize];

        assert!(
            position_accessor.count == normal_accessor.count,
            "this currently assumes all indecies are shared between all dimensions, i.e. a mesh has one single index buffer, not one per vertex attribute. This does seem to be the case.",
        );
        let vert_count = position_accessor.count as usize;

        // TODO: abstract recast into function
        // TODO: handle cases with interleaved data (i.e. where they have a stride)
        let position_data_bytes = gltf_header.access_buffer(&gltf_buffer, position_accessor);
        let position_data_ptr = position_data_bytes.as_ptr() as *const [f32; 3];
        let position_data_len = position_data_bytes.len() / (size_of::<[f32; 3]>()); // TODO: if this is always vert_count, consider inlining it in the assert
        assert!(vert_count == position_data_len, "expected {vert_count} positions, found {position_data_len}");

        let normal_data_bytes = gltf_header.access_buffer(&gltf_buffer, normal_accessor);
        let normal_data_ptr = normal_data_bytes.as_ptr() as *const [f32; 3];
        let normal_data_len = normal_data_bytes.len() / (size_of::<[f32; 3]>()); // TODO: if this is always vert_count, consider inlining it in the assert
        assert!(vert_count == normal_data_len, "expected {vert_count} normals, found {normal_data_len}");

        let joints_accessor = &gltf_header.accessors[mesh.primitives[0].attributes.joints_0.unwrap() as usize];
        let joints_data_bytes = gltf_header.access_buffer(&gltf_buffer, joints_accessor);
        let joints_data_ptr = joints_data_bytes.as_ptr() as *const [u8; 4];
        let joints_data_len = joints_data_bytes.len() / (size_of::<[u8; 4]>()); // TODO: if this is always vert_count, consider inlining it in the assert
        assert!(vert_count == joints_data_len, "expected {vert_count} joints, found {joints_data_len}");

        let weights_accessor = &gltf_header.accessors[mesh.primitives[0].attributes.weights_0.unwrap() as usize];
        let weights_data_bytes = gltf_header.access_buffer(&gltf_buffer, weights_accessor);
        let weights_data_ptr = weights_data_bytes.as_ptr() as *const [f32; 4];
        let weights_data_len = weights_data_bytes.len() / (size_of::<[f32; 4]>()); // TODO: if this is always vert_count, consider inlining it in the assert
        assert!(vert_count == weights_data_len, "expected {vert_count} weights, found {weights_data_len}");

        let mut verts = Vec::<AnimatedVertex>::with_capacity(vert_count);
        for vert in 0..vert_count {
            //? Does this create the struct on stack and then copy, or is it able to be smart and emblace_back due to ownership semantics?
            let pos3 = unsafe { position_data_ptr.offset(vert as isize).read_unaligned() }; //. This performs a copy maybe?
            let norm3 = unsafe { normal_data_ptr.offset(vert as isize).read_unaligned() }; //. This performs a copy maybe?
            let joint4 = unsafe { joints_data_ptr.offset(vert as isize).read_unaligned() }; //. This performs a copy maybe?
            let weight4 = unsafe { weights_data_ptr.offset(vert as isize).read_unaligned() }; //. This performs a copy maybe?
            assert!(BONES_PER_VERT == 4, "only support exactly 4 bones per vert at the moment");
            verts.push(AnimatedVertex {
                #[rustfmt::skip]
                // TODO: make [f32;3] -> [f32;4] function/macro
                pos: [pos3[0], pos3[1], pos3[2], 1.0],
                normal: [norm3[0], norm3[1], norm3[2], 1.0],
                color: [1.0, 0.0, 0.0, 1.0],
                bones: joint4,
                bone_weights: weight4,
            });
        }

        let index_accessor = &gltf_header.accessors[mesh.primitives[0].indices as usize];

        let index_data_bytes = gltf_header.access_buffer(&gltf_buffer, index_accessor);
        let index_data_ptr = index_data_bytes.as_ptr() as *const u16;
        let index_data_len = index_data_bytes.len() / (size_of::<u16>() * 1);
        let index_data: &[u16] = unsafe { std::slice::from_raw_parts(index_data_ptr, index_data_len) };

        Mesh { vertices: verts, indices: index_data.iter().map(|&e| e as u32).collect() }
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

    //# Set up renderer
    #[rustfmt::skip]
    let mut renderer = Renderer::<AnimatedVertex, _>::new(
        &window,
        window_width,
        window_height,
        [
            vk::VertexInputAttributeDescription { location: 0, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 0 as u32 },
            vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 4 * 32 / 8 as u32 },
            vk::VertexInputAttributeDescription { location: 2, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 8 * 32 / 8 as u32 },
            //
            // vk::VertexInputAttributeDescription { location: 3, binding: 0, format: vk::Format::R8G8B8A8_UINT, offset: 12 * 32 / 8 as u32 }, //. this doesn't work
            vk::VertexInputAttributeDescription { location: 3, binding: 0, format: vk::Format::R8_UINT, offset: (12 * 32 + 0 * 8) / 8 as u32 },
            vk::VertexInputAttributeDescription { location: 4, binding: 0, format: vk::Format::R8_UINT, offset: (12 * 32 + 1 * 8) / 8 as u32 },
            vk::VertexInputAttributeDescription { location: 5, binding: 0, format: vk::Format::R8_UINT, offset: (12 * 32 + 2 * 8) / 8 as u32 },
            vk::VertexInputAttributeDescription { location: 6, binding: 0, format: vk::Format::R8_UINT, offset: (12 * 32 + 3 * 8) / 8 as u32 },
            //
            // vk::VertexInputAttributeDescription { location:  7, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: (12 * 32 + 4 * 8) / 8 as u32 }, //. this only works for exactly 4, and then the shader has to specify vec4, instead of float[4]
            vk::VertexInputAttributeDescription { location:  7, binding: 0, format: vk::Format::R32_SFLOAT, offset: (12 * 32 + 4 * 8 + 0 * 32) / 8 as u32 },
            vk::VertexInputAttributeDescription { location:  8, binding: 0, format: vk::Format::R32_SFLOAT, offset: (12 * 32 + 4 * 8 + 1 * 32) / 8 as u32 },
            vk::VertexInputAttributeDescription { location:  9, binding: 0, format: vk::Format::R32_SFLOAT, offset: (12 * 32 + 4 * 8 + 2 * 32) / 8 as u32 },
            vk::VertexInputAttributeDescription { location: 10, binding: 0, format: vk::Format::R32_SFLOAT, offset: (12 * 32 + 4 * 8 + 3 * 32) / 8 as u32 },
        ],
    );
    renderer.set_vertex_shader(include_bytes!("./shader/vert.spv"));
    renderer.set_fragment_shader(include_bytes!("./shader/frag.spv"));
    renderer.clear_color = [0.05, 0.01, 0.02, 1.0];

    //# Load assets from files
    let glb_file_paths = [concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Glorp.glb"), concat!(env!("CARGO_MANIFEST_DIR"), "/assets/test.glb")];
    let mut file_data = glb_file_paths.map(|file| read(file).unwrap());
    let gltf_datas = file_data.iter_mut().map(|gltf_file_data| glb::parse(gltf_file_data)).collect::<Vec<_>>();

    let obj_amount: i32 = 6;
    let mut animated_objects: Vec<(Object, AnimationHandler)> = (0..obj_amount)
        .map(|i| {
            let (gltf_header, gltf_buffer) = &gltf_datas[(i as usize) % gltf_datas.len()];
            (
                Object {
                    transform: Transform {
                        position: Vec3::<f32> { x: (-(obj_amount / 2) + i) as f32 * 10.0, y: 0.0, z: 30.0 },
                        rotation: Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 0.0 }.normalize(), PI),
                        scale: Vec3::one(),
                    },
                    mesh: renderer.register_mesh(animated_vertex_mesh_from_gltf_data(gltf_header, gltf_buffer)),
                },
                AnimationHandler { animations: animations_from_gltf_data(gltf_header, gltf_buffer), current_animation: 0, current_anim_time: (i as f32) / (obj_amount as f32) },
            )
        })
        .into_iter()
        .collect();

    let mut last_time;
    let mut current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let mut t = 0.0;
    let mut dt = 0.01;
    let mut control = 1.0;
    let mut should_close = false;
    let mut light_dir;
    let mut light_rotation;
    let mut selected_obj: usize = 0;

    let mut focused = false;

    while !should_close {
        //. Update the objects
        for (object, animation) in &mut animated_objects {
            object.transform.rotation = Quaternion::from_axis_rotation(Vec3 { x: 0.0, y: 1.0, z: 0.0 }.normalize(), dt) * object.transform.rotation;

            animation.current_anim_time += dt * control;
        }

        light_rotation = Quaternion::from_axis_rotation(Vec3 { x: 0.0, y: 1.0, z: 0.0 }.normalize(), t * 1.0);
        light_dir = (Vec3::down())
            .normalize()
            .rotate(Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 0.0 }.normalize(), 0.3 * PI))
            .normalize()
            .rotate(light_rotation)
            .normalize();

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
                    VirtualKeyCode::H => {
                        selected_obj = selected_obj.wrapping_sub(1) % animated_objects.len();
                        println!("selecting obj: {selected_obj}");
                    },
                    VirtualKeyCode::L => {
                        selected_obj = selected_obj.wrapping_add(1) % animated_objects.len();
                        println!("selecting obj: {selected_obj}");
                    },
                    VirtualKeyCode::J => {
                        let current_anim = animated_objects[selected_obj].1.current_animation;
                        let anims_count = animated_objects[selected_obj].1.animations.len() as u8;
                        let next_anim = current_anim.wrapping_add(1) % anims_count;
                        animated_objects[selected_obj].1.current_animation = next_anim;
                        let anim_name = &animated_objects[selected_obj].1.animations[next_anim as usize].name;
                        println!("Setting animation for obj: {selected_obj} to ({next_anim}), {anim_name}");
                    },
                    VirtualKeyCode::K => {
                        let current_anim = animated_objects[selected_obj].1.current_animation;
                        let anims_count = animated_objects[selected_obj].1.animations.len() as u8;
                        let next_anim = current_anim.wrapping_sub(1) % anims_count;
                        animated_objects[selected_obj].1.current_animation = next_anim;
                        let anim_name = &animated_objects[selected_obj].1.animations[next_anim as usize].name;
                        println!("Setting animation for obj: {selected_obj} to ({next_anim}), {anim_name}");
                    },
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
                    //# (Re-)Animate the bones (Necromancers be like)
                    let skeletons: Vec<_> = animated_objects
                        .iter()
                        .map(|(_, animation_handler)| {
                            let current_anim = animation_handler.current_animation as usize;
                            let current_time = animation_handler.current_anim_time;
                            animated_skeleton(&animation_handler.animations[current_anim], current_time)
                        })
                        .collect();

                    //# Render the objects
                    renderer.render(
                        DrawUniform { view_mat: camera_transform.get_inverse_matrix(), proj_mat, light_dir: light_dir, ambient_light: 0.5 },
                        animated_objects.iter().map(|(object, _)| object.mesh).collect(),
                        animated_objects
                            .iter()
                            .enumerate()
                            .map(|(i, (object, _))| AnimatedObjectUniform { selected: if i == selected_obj { 1 } else { 0 }, model_mat: object.transform.get_matrix(), skeleton: skeletons[i] })
                            .collect(),
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
