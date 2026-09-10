use std::{
    collections::HashMap,
    f32::consts::{PI, TAU},
    fs::read,
    mem::size_of,
    time::{SystemTime, UNIX_EPOCH},
};

use ash::vk;
use glb::{AnimationTargetPath, Gltf};
use rand::{RngExt, SeedableRng};

use red_hot::{
    math::{inverse_perspective_matrix, perspective_matrix, Mat4x4, Quaternion, Transform, Vec3},
    renderer::{ImageSize, Mesh, MeshIndex, RedHotImageCreateInfo, Renderer},
};

use winit::{
    event::{DeviceEvent, ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    platform::pump_events::EventLoopExtPumpEvents,
    window::Window,
};

const SKELETON_SIZE: usize = 24;
const BONES_PER_VERT: usize = 4;

//# Anim loading bandaid
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
    meshi: MeshIndex,
}

struct AnimatedObject<'a> {
    object: Object,
    animation_handler: AnimationHandler<'a>,
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
    animations: &'a Vec<Animation<'a>>,
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
                match channel.target.path {
                    AnimationTargetPath::ROTATION => {
                        output_data = Vec3or4Array::Vec4Array(unsafe { std::slice::from_raw_parts(output_data_bytes.as_ptr() as *const [f32; 4], output_data_bytes.len() / (size_of::<[f32; 4]>())) })
                    },
                    AnimationTargetPath::TRANSLATION | AnimationTargetPath::SCALE => {
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

fn animated_skeleton(Animation { name: _, animated_targets, inverse_bind_matricies, joint_heirachy }: &Animation, time: f32) -> [Mat4x4<f32>; SKELETON_SIZE] {
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
                    match target.path {
                        AnimationTargetPath::TRANSLATION => {
                            let from = Into::<&[[f32; 3]]>::into(*keyframes)[anim_from].into();
                            let to = Into::<&[[f32; 3]]>::into(*keyframes)[anim_to].into();
                            let result = Vec3::lerp(from, to, lerp_t);
                            bone_transform.position = result;
                        },
                        AnimationTargetPath::ROTATION => {
                            let from = Into::<&[[f32; 4]]>::into(*keyframes)[anim_from].into();
                            let to = Into::<&[[f32; 4]]>::into(*keyframes)[anim_to].into();
                            let result = Quaternion::lerp(from, to, lerp_t);
                            bone_transform.rotation = result;
                        },
                        AnimationTargetPath::SCALE => {
                            bone_transform.scale = Vec3::lerp(Into::<&[[f32; 3]]>::into(*keyframes)[anim_from].into(), Into::<&[[f32; 3]]>::into(*keyframes)[anim_to].into(), lerp_t)
                        },
                        x => println!("Not animating property: {x:?}"),
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
                //. first undo the bind pose, then apply the current pose
                joint_transforms[i] = joint_transforms[i] * inverse_bind_matricies[i].transpose();
            }
        }
        for i in 0..joint_transforms.len() {
            skeleton[i] = joint_transforms[i];
        }
    }
    return skeleton.try_into().unwrap();
}

//. Given normalized screen coordinates in range [-1.0, 1.-0], returns the corresponding vector from the camera position to the near-plane of the projection matrix
fn screen_to_nearplane_offset((x, y): (f32, f32), camera_transform: &Transform<f32>, inverse_proj_mat: &Mat4x4<f32>) -> Vec3<f32> {
    Vec3 { x, y, z: -1.0 }
        .transform(inverse_proj_mat)
        .transform_affine(&Transform { position: Vec3::zero(), rotation: camera_transform.rotation, scale: Vec3::one() }.get_matrix())
}

//. Given normalized screen coordinates in range [-1.0, 1.-0], returns the corresponding normalized vector from the camera into the perspective view
fn screen_to_direction((x, y): (f32, f32), camera_transform: &Transform<f32>, inverse_proj_mat: &Mat4x4<f32>) -> Vec3<f32> {
    let cursor_in_viewspace = Vec3 { x, y, z: -1.0 }
        .transform(inverse_proj_mat)
        .transform_affine(&Transform { position: Vec3::zero(), rotation: camera_transform.rotation, scale: Vec3::one() }.get_matrix());
    (cursor_in_viewspace).normalize()
}

fn main() {
    unsafe {
        assert_eq!(BONES_PER_VERT, 4, "VertexInputAttributeDescription not generalized");
        println!("Example 99: game");
        let mut window_width: u32 = 1600;
        let mut window_height: u32 = 900;

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
        struct AnimatedObjectUniform {
            model_mat: Mat4x4<f32>,
            skeleton: [Mat4x4<f32>; SKELETON_SIZE], //. Hard-coded max bone count for now
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

        fn animated_vertex_mesh_from_gltf_data<'a>((gltf_header, gltf_buffer): (&Gltf, &'a [u8])) -> Mesh<AnimatedVertex> {
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

        #[rustfmt::skip]
        let cube_mesh = {
            let vertices = vec![
                    AnimatedVertex { pos: [-1.0, -1.0, -1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [ 1.0, -1.0, -1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] }, //. DOWN (LEFT/BACK)
                    AnimatedVertex { pos: [-1.0, -1.0,  1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [ 1.0, -1.0, -1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [ 1.0, -1.0,  1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] }, //. DOWN (RIGHT/FRONT)
                    AnimatedVertex { pos: [-1.0, -1.0,  1.0, 1.0], normal: [ 0.0, -1.0,  0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [-1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] }, //. UP (LEFT/FRONT)
                    AnimatedVertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [ 1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] }, //. UP (RIGHT/BACK)
                    AnimatedVertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  1.0,  0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [-1.0, -1.0,  1.0, 1.0], normal: [-1.0,  0.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [-1.0,  1.0,  1.0, 1.0], normal: [-1.0,  0.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] }, //. LEFT (FRONT/DOWN)
                    AnimatedVertex { pos: [-1.0, -1.0, -1.0, 1.0], normal: [-1.0,  0.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [-1.0,  1.0,  1.0, 1.0], normal: [-1.0,  0.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [-1.0,  0.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] }, //. LEFT (BACK/UP)
                    AnimatedVertex { pos: [-1.0, -1.0, -1.0, 1.0], normal: [-1.0,  0.0,  0.0, 1.0], color: [1.0, 1.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [ 1.0,  1.0, -1.0, 1.0], normal: [ 1.0,  0.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 1.0,  0.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] }, //. RIGHT (FRONT/UP)
                    AnimatedVertex { pos: [ 1.0, -1.0,  1.0, 1.0], normal: [ 1.0,  0.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [ 1.0, -1.0, -1.0, 1.0], normal: [ 1.0,  0.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [ 1.0,  1.0, -1.0, 1.0], normal: [ 1.0,  0.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] }, //. RIGHT (BACK/DOWN)
                    AnimatedVertex { pos: [ 1.0, -1.0,  1.0, 1.0], normal: [ 1.0,  0.0,  0.0, 1.0], color: [0.0, 1.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [-1.0, -1.0, -1.0, 1.0], normal: [ 0.0,  0.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  0.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] }, //. BACK (LEFT/DOWN)
                    AnimatedVertex { pos: [ 1.0, -1.0, -1.0, 1.0], normal: [ 0.0,  0.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [-1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  0.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [ 1.0,  1.0, -1.0, 1.0], normal: [ 0.0,  0.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] }, //. BACK (RIGHT/UP)
                    AnimatedVertex { pos: [ 1.0, -1.0, -1.0, 1.0], normal: [ 0.0,  0.0, -1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  0.0,  1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [-1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  0.0,  1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] }, //. FRONT (LEFT/UP)
                    AnimatedVertex { pos: [-1.0, -1.0,  1.0, 1.0], normal: [ 0.0,  0.0,  1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [ 1.0, -1.0,  1.0, 1.0], normal: [ 0.0,  0.0,  1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
                    AnimatedVertex { pos: [ 1.0,  1.0,  1.0, 1.0], normal: [ 0.0,  0.0,  1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] }, //. FRONT (RIGHT/DOWN)
                    AnimatedVertex { pos: [-1.0, -1.0,  1.0, 1.0], normal: [ 0.0,  0.0,  1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0], bones: [0, 0, 0, 0], bone_weights: [1.0, 0.0, 0.0, 0.0] },
            ];
            let indices = (0..vertices.len() as u32).collect();
            Mesh::<AnimatedVertex> {vertices, indices}
        };

        let mut position = Vec3 { x: 0.0, y: 0.0, z: -10.0 };
        let mut cam_yaw = 0.0;
        let mut cam_pitch = 0.0;

        let proj_mat = perspective_matrix(TAU / 4.0, 0.1, 10000.0);
        let inv_proj_mat = inverse_perspective_matrix(TAU / 4.0, 0.1, 10000.0);

        let mut event_loop = EventLoop::new().expect("Failed to create new event loop? How can this even fail???");

        let window = event_loop
            .create_window(Window::default_attributes().with_title("99-game").with_inner_size(winit::dpi::LogicalSize::new(f64::from(window_width), f64::from(window_height))))
            .unwrap();
        window.set_cursor_grab(winit::window::CursorGrabMode::Confined).unwrap();
        window.set_cursor_visible(false);

        //# Set up renderer
        let mut renderer = Renderer::<AnimatedVertex>::new(&window, window_width, window_height);

        let depth_image = renderer.register_image(RedHotImageCreateInfo {
            size: ImageSize::SurfaceSize,
            format: vk::Format::D32_SFLOAT,
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            memory_flags: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            image_aspect_mask: vk::ImageAspectFlags::DEPTH,
        });

        #[rustfmt::skip]
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
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.05, 0.01, 0.02, 1.0] } }, //. was renderer.clear_color
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

        let cube_meshi = renderer.register_mesh(cube_mesh);

        //# Load assets from files
        struct AssetManager<'a> {
            _backing_data: Vec<Vec<u8>>,
            parsed_views: HashMap<String, (Gltf, &'a [u8])>,
        }

        impl<'a> AssetManager<'a> {
            fn new(filepaths: &[&str]) -> AssetManager<'a> {
                let mut backing_data = Vec::with_capacity(filepaths.len());
                let mut parsed_views = HashMap::with_capacity(filepaths.len());

                for path in filepaths {
                    backing_data.push(read(path).unwrap());
                    //. Safety: Assumes we NEVER; A) modify the contents of the vec (might realloc), and B) never drop the vec, as this would invalidate the pointer
                    let raw_data = backing_data.last_mut().unwrap().as_mut_slice() as *mut [u8];
                    let (gltf_header, blob_slice) = glb::parse(unsafe { std::slice::from_raw_parts_mut(raw_data as *mut u8, raw_data.len()) });
                    parsed_views.insert(path.to_string(), (gltf_header, &*blob_slice));
                    //. Only return immutable slice
                }

                AssetManager { _backing_data: backing_data, parsed_views }
            }
        }

        let glb_file_paths = [concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Glorp.glb"), concat!(env!("CARGO_MANIFEST_DIR"), "/assets/test.glb")];
        let asset_man = AssetManager::new(&glb_file_paths);

        //# Game?
        let mut rng = rand::rngs::StdRng::seed_from_u64(69);
        const WORM_COUNT: u32 = 100;
        let mut animated_objects: Vec<AnimatedObject> = Vec::with_capacity(WORM_COUNT as usize + 1);

        let (glorp_gltf_header, glorp_gltf_buffer) = asset_man.parsed_views.get(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Glorp.glb")).unwrap();
        let glorp_meshi = renderer.register_mesh(animated_vertex_mesh_from_gltf_data((&glorp_gltf_header, glorp_gltf_buffer))); // TODO: Add these to content management system mayhaps
        let glorp_animations = animations_from_gltf_data(&glorp_gltf_header, glorp_gltf_buffer); // TODO: Add these to content management system mayhaps

        let player = AnimatedObject {
            object: Object {
                transform: Transform {
                    position: Vec3::<f32> { x: 0.0, y: 0.5, z: 30.0 },
                    rotation: Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 0.0 }.normalize(), PI),
                    scale: Vec3::one(),
                },
                meshi: glorp_meshi,
            },
            animation_handler: AnimationHandler { animations: &glorp_animations, current_animation: 0, current_anim_time: 0.0 },
        };
        animated_objects.push(player);

        let (worm_gltf_header, worm_gltf_buffer) = asset_man.parsed_views.get(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/test.glb")).unwrap();
        let worm_meshi = renderer.register_mesh(animated_vertex_mesh_from_gltf_data((&worm_gltf_header, worm_gltf_buffer)));
        let worm_animations = animations_from_gltf_data(&worm_gltf_header, worm_gltf_buffer);
        for _i in 0..WORM_COUNT {
            animated_objects.push(AnimatedObject {
                object: Object {
                    transform: Transform {
                        position: Vec3::<f32> { x: rng.random_range(-50.0..50.0), y: 0.0, z: 30.0 + rng.random_range(-50.0..50.0) },
                        rotation: Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 0.0 }.normalize(), PI),
                        scale: Vec3::one(),
                    },
                    meshi: worm_meshi,
                },
                animation_handler: AnimationHandler { animations: &worm_animations, current_animation: 0, current_anim_time: rng.random_range(0.0..1.0) },
            });
        }

        let mut last_time;
        let mut current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let mut t = 0.0;
        let mut dt = 0.01;
        let mut control = 1.0;
        let mut should_close = false;
        let mut light_dir;
        let mut light_rotation;
        let cursor_speed = 0.003;
        let mut cursor = (0.0, 0.0);
        let mut dragging = false;

        let mut focused = false;

        while !should_close {
            //. Update the objects
            for AnimatedObject { object, animation_handler } in &mut animated_objects {
                animation_handler.current_anim_time += dt * control;
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
                rotation: Quaternion::from_axis_rotation(Vec3 { x: 0.0, y: 1.0, z: 0.0 }.normalize(), cam_yaw)
                    * Quaternion::from_axis_rotation(Vec3 { x: -1.0, y: 0.0, z: 0.0 }.normalize(), cam_pitch),
                scale: Vec3::one(),
            };

            let ray_dir = screen_to_nearplane_offset(cursor, &camera_transform, &inv_proj_mat) * 2.1; //? 2.1 has is a magic number, related to the size, to make sure all verts are in front of the near-plane

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
                            KeyCode::KeyH => {},
                            KeyCode::KeyL => {},
                            KeyCode::KeyJ => {},
                            KeyCode::KeyK => {},
                            _ => (),
                        }
                    },
                    Event::WindowEvent { event: WindowEvent::MouseInput { button: MouseButton::Middle, state, .. }, .. } => match state {
                        ElementState::Pressed => dragging = true,
                        ElementState::Released => dragging = false,
                    },
                    Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta: (mouse_x, mouse_y) }, .. } => {
                        if focused && dragging {
                            cam_yaw += mouse_x as f32 / 40.0;
                            cam_pitch += mouse_y as f32 / 40.0;
                        }
                        let (x, y) = cursor;
                        cursor = (f32::clamp(x + (mouse_x as f32) * cursor_speed, -1.0, 1.0), f32::clamp(y + (mouse_y as f32) * cursor_speed, -1.0, 1.0));
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

            //# (Re-)Animate the bones (Necromancers be like)
            let skeletons: Vec<_> = animated_objects
                .iter()
                .map(|AnimatedObject { object: _, animation_handler }| {
                    let current_anim = animation_handler.current_animation as usize;
                    let current_time = animation_handler.current_anim_time;
                    animated_skeleton(&animation_handler.animations[current_anim], current_time)
                })
                .collect();

            //# Render the objects
            renderer.render_begin(DrawUniform { _dummy: 0.0 });
            renderer.render_stage(
                default_stage,
                RenderStageUniform { view_mat: camera_transform.get_inverse_matrix(), proj_mat, light_dir: light_dir, ambient_light: 0.5 },
                animated_objects.iter().map(|AnimatedObject { object, animation_handler: _ }| object.meshi).chain(vec![cube_meshi]).collect(),
                animated_objects
                    .iter()
                    .enumerate()
                    .map(|(i, AnimatedObject { object, animation_handler: _ })| AnimatedObjectUniform { model_mat: object.transform.get_matrix(), skeleton: skeletons[i] })
                    .chain(vec![AnimatedObjectUniform {
                        model_mat: Transform { position: (camera_transform.position + ray_dir), rotation: Quaternion::<f32>::identity(), scale: Vec3::one() * 0.001 }.get_matrix(),
                        skeleton: vec![Mat4x4::<f32>::unit(); SKELETON_SIZE].try_into().unwrap(),
                    }])
                    .collect(),
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
