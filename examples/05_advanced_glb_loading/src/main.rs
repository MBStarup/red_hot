use std::{
    f32::consts::{PI, TAU},
    fs::read,
    iter,
    mem::size_of,
    println,
    time::{SystemTime, UNIX_EPOCH},
};

use ash::vk;
use glb::{
    AnimationTargetPath::{self},
    Gltf, SamplerInterpolation,
};
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
    interpolation: SamplerInterpolation,
}

struct Skin<'a> {
    name: String,
    animated_targets: Vec<AnimatedTarget<'a>>,
    inverse_bind_matricies: &'a [Mat4x4<f32>],
    joint_heirachy: Vec<(u32, Option<u32>)>,
}

struct Animation<'a> {
    name: String,
    animated_targets: Vec<AnimatedTarget<'a>>,
    inverse_bind_matricies: &'a [Mat4x4<f32>],
    joint_heirachy: Vec<(u32, Option<u32>, Option<u32>)>, //. Node ID/index, Parent node ID/index (None = root), Inverse binde matrix index (Some nodes are not nessecarely part of the skin's skeleton, but is part of the heirachy?)
}

struct AnimationHandler<'a> {
    animations: &'a Vec<Animation<'a>>,
    current_animation: u8, //. Assumes <= 255 animations
    current_anim_time: f32,
    gltf_header: &'a Gltf,
    gltf_buffer: &'a [u8],
}

fn animations_from_gltf_data<'a>(gltf_header: &Gltf, gltf_buffer: &[u8]) -> Vec<Animation<'a>> {
    iter::once(Animation { name: "Static (Default)".to_owned(), animated_targets: vec![], inverse_bind_matricies: &[], joint_heirachy: vec![] })
        .chain(gltf_header.animations.iter().map(|anim| {
            let animation: Vec<AnimatedTarget<'_>> = anim
                .channels
                .iter()
                .map(|channel| {
                    let sampler_index = channel.sampler as usize;
                    assert!(
                        anim.samplers[sampler_index].interpolation == SamplerInterpolation::LINEAR || anim.samplers[sampler_index].interpolation == SamplerInterpolation::STEP,
                        "Unsupported animation interpolation {:?} in animation {} for channel {:?} of node {} ({:?}); only LINEAR interpolation is supported",
                        anim.samplers[sampler_index].interpolation,
                        anim.name,
                        channel.target.path,
                        channel.target.node,
                        gltf_header.nodes[channel.target.node as usize].name,
                    );

                    let input_accessor = &gltf_header.accessors[anim.samplers[sampler_index].input as usize];
                    let output_accessor = &gltf_header.accessors[anim.samplers[sampler_index].output as usize];

                    assert!(input_accessor.count == output_accessor.count, "Sampler input/output accessor count mismatch",);

                    let input_data_bytes = {
                        let buffer_view = &gltf_header.buffer_views[input_accessor.buffer_view as usize];
                        assert!(
                            buffer_view.byte_stride.is_none(),
                            "Unsupported interleaved buffer view (byte_stride = {:?}); only tightly-packed animation data is supported",
                            buffer_view.byte_stride,
                        );
                        assert!(
                            input_accessor.component_type == glb::ComponentType::FLOAT,
                            "Unsupported accessor component_type {:?}; only FLOAT (5126) is supported for animation data",
                            input_accessor.component_type,
                        );
                        let buffer_start = buffer_view.byte_offset.unwrap_or(0) as usize + input_accessor.byte_offset.unwrap_or(0) as usize;
                        let len = input_accessor.count as usize * size_of::<f32>();
                        &gltf_buffer[buffer_start..buffer_start + len]
                    };
                    let input_data: &[f32] = unsafe { std::slice::from_raw_parts(input_data_bytes.as_ptr() as *const f32, input_data_bytes.len() / (size_of::<f32>())) };

                    let output_data_bytes = {
                        let buffer_view = &gltf_header.buffer_views[output_accessor.buffer_view as usize];
                        assert!(
                            buffer_view.byte_stride.is_none(),
                            "Unsupported interleaved buffer view (byte_stride = {:?}); only tightly-packed animation data is supported",
                            buffer_view.byte_stride,
                        );
                        assert!(
                            input_accessor.component_type == glb::ComponentType::FLOAT,
                            "Unsupported accessor component_type {:?}; only FLOAT (5126) is supported for animation data",
                            input_accessor.component_type,
                        );
                        let buffer_start = buffer_view.byte_offset.unwrap_or(0) as usize + output_accessor.byte_offset.unwrap_or(0) as usize;
                        let component_len = match channel.target.path {
                            AnimationTargetPath::ROTATION => size_of::<[f32; 4]>(),
                            AnimationTargetPath::TRANSLATION | AnimationTargetPath::SCALE => size_of::<[f32; 3]>(),
                            _ => size_of::<f32>(),
                        };
                        let len = output_accessor.count as usize * component_len;
                        &gltf_buffer[buffer_start..buffer_start + len]
                    };

                    // TODO: This is a bandaid fix
                    let output_data: Vec3or4Array;
                    match channel.target.path {
                        AnimationTargetPath::ROTATION => {
                            output_data =
                                Vec3or4Array::Vec4Array(unsafe { std::slice::from_raw_parts(output_data_bytes.as_ptr() as *const [f32; 4], output_data_bytes.len() / (size_of::<[f32; 4]>())) })
                        },
                        AnimationTargetPath::TRANSLATION | AnimationTargetPath::SCALE => {
                            output_data =
                                Vec3or4Array::Vec3Array(unsafe { std::slice::from_raw_parts(output_data_bytes.as_ptr() as *const [f32; 3], output_data_bytes.len() / (size_of::<[f32; 3]>())) })
                        },
                        x => panic!("Unexpected animation target path when parsing animation: {x:?}"),
                    }
                    println!(
                        "Animation: {}, node: {}, channel.target: {:?}, timestamps: {input_data:?}, data: {output_data:?}",
                        anim.name, channel.target.node, channel.target.path
                    );
                    AnimatedTarget {
                        target: glb::Target { node: channel.target.node, path: channel.target.path },
                        interpolation: anim.samplers[sampler_index].interpolation,
                        timestamps: input_data,
                        keyframes: output_data,
                    }
                })
                .collect();

            //# Inverse bind matrix
            let inverse_bind_matrix_accessor = &gltf_header.accessors[gltf_header.skins[0].inverse_bind_matrices.unwrap() as usize];
            let inverse_bind_matrix_data_bytes = gltf_header.access_buffer(&gltf_buffer, inverse_bind_matrix_accessor);
            let inverse_bind_matrix_data_ptr = inverse_bind_matrix_data_bytes.as_ptr() as *const Mat4x4<f32>;
            let inverse_bind_matrix_data_len = inverse_bind_matrix_data_bytes.len() / (size_of::<Mat4x4<f32>>()); // TODO: if this is always vert_count, consider inlining it in the assert
            let inverse_bind_matrix_data: &[Mat4x4<f32>] = unsafe { std::slice::from_raw_parts(inverse_bind_matrix_data_ptr, inverse_bind_matrix_data_len) };

            //# Create joint tree for the joints being animated
            let mut joints = animation.iter().map(|x| x.target.node).collect::<Vec<_>>(); //. Start with joints directly animated by this animation

            //. Compute the parent-closure of those joints, this is the full set of joinst whose transform is relevant for this animation
            //. Build the heirachy on the fly while computing the closure, as to avoid iterating twice
            let mut i = 0;
            let mut joint_heirachy = Vec::new();

            while i < joints.len() {
                let joint = joints[i];

                let parent = gltf_header.nodes.iter().position(|node| node.children.contains(&joint)); //. Get ID (index)) of first (and assumingly only) node which has this as child, or None if, well, None (root)
                let skin_joint = gltf_header.skins[0].joints.iter().position(|&x| x == joint);
                joint_heirachy.push((joint, parent.map(|x| x as u32), skin_joint.map(|x| x as u32)));

                if let Some(parent) = parent {
                    if !joints.contains(&(parent as u32)) {
                        joints.push(parent as u32); //. Push places it at the end, meaning we will visit this in a future iteration.
                    }
                }

                i += 1;
            }

            Animation { name: anim.name.clone(), animated_targets: animation, inverse_bind_matricies: inverse_bind_matrix_data, joint_heirachy: joint_heirachy }
        }))
        .collect()
}

// NOTE[Animation]: joint_heirachy should include the "parent-closure" of all animated joints
fn animated_skeleton(gltf_header: &Gltf, gltf_buffer: &[u8], Animation { name, animated_targets, inverse_bind_matricies, joint_heirachy }: &Animation, time: f32) -> [Mat4x4<f32>; SKELETON_SIZE] {
    let mut skeleton = vec![Mat4x4::<f32>::unit(); SKELETON_SIZE];
    {
        // TODO[Animation]: JOINTS_0 indexes skin.joints[], inverse_bind_matricies[], and skeleton[] (since the weights use this indexing)
        // TODO[Animation]: skin.joints[i] then indexes nodes[]
        //
        // TODO[Animation]: Instead of Option<skin_joint> in the hierarchy, consider separate vecs for skin joints and the hierarchy
        // TODO[Animation]: Not sold on introducing a separate Skin type, however, removes duplication of inverse_bind_matrices for all animations on a mesh, and conceptually skins and animations are separate and and animation could animate nodes referenced by multiple skins, and a skin be affected by multiple animations
        // TODO[Animation]: Therefore, We coudl have the simple Skin { animations: ... }, or the technically correct skins and animations separate for and both iterated for each instation of some node heirachy
        //
        // TODO[Animation]: Animation evaluation currently builds animated nodes + their ancestors
        // TODO[Animation]: Forward-propagating the entire node hierarchy is an alternative, but may be wasteful

        let mut joint_transforms: Vec<Mat4x4<f32>> = Vec::with_capacity(joint_heirachy.len());
        //. Bones
        {
            let mut parents: Vec<Option<u32>> = Vec::with_capacity(joint_heirachy.len());
            let mut any_has_parent = false;
            for i in 0..joint_heirachy.len() {
                parents.push(joint_heirachy.iter().position(|x| Some(x.0) == joint_heirachy[i].1).map(|x| x as u32));
                if parents[i] != None {
                    any_has_parent = true;
                }
            }

            //. Calculate the transformation for the bone, relative to it's parent
            for i in 0..joint_heirachy.len() {
                let anim_time = (time / 2.00) % animated_targets.iter().map(|t| *t.timestamps.last().unwrap()).reduce(f32::max).unwrap();
                let node = &gltf_header.nodes[joint_heirachy[i].0 as usize]; //. Non-animated targets or even bones, should default to their state in the heireachy, not a default transform
                let mut bone_transform = Transform::<f32> {
                    position: node.translation.map(Vec3::from).unwrap_or(Vec3::zero()),
                    rotation: node.rotation.map(Quaternion::from).unwrap_or(Quaternion::identity()),
                    scale: node.scale.map(Vec3::from).unwrap_or(Vec3::one()),
                };
                for AnimatedTarget { target, interpolation, timestamps, keyframes } in animated_targets.iter().filter(|AnimatedTarget { target, .. }| target.node == joint_heirachy[i].0) {
                    // TODO: there's 100% a more concise, readable, and "correct" way to align these inputs with outputs
                    assert!(timestamps.len() >= 2);
                    assert!(keyframes.len() >= 1);
                    let mut anim_from = 0;
                    let mut anim_to = 0;
                    let mut lerp_t = 0.0;

                    if keyframes.len() == 1 {
                    } else {
                        while anim_to < timestamps.len() && anim_time > timestamps[anim_to] {
                            //. If there are more time-"slices" after the current, AND the requested time is after the current slice, move the slice
                            anim_from = anim_to;
                            anim_to += 1;
                        }
                        if anim_time > timestamps[anim_to] {
                            //. If it's past the last slice, clamp to the end
                            anim_from = anim_to;
                        }

                        if anim_from == anim_to || *interpolation == SamplerInterpolation::STEP {
                            lerp_t = 0.0;
                        } else {
                            lerp_t = (anim_time - timestamps[anim_from]) / (timestamps[anim_to] - timestamps[anim_from]);
                        }
                    }
                    match target.path {
                        AnimationTargetPath::TRANSLATION => {
                            bone_transform.position = Vec3::lerp(Into::<&[[f32; 3]]>::into(*keyframes)[anim_from].into(), Into::<&[[f32; 3]]>::into(*keyframes)[anim_to].into(), lerp_t)
                        },
                        AnimationTargetPath::ROTATION => {
                            bone_transform.rotation = Quaternion::nlerp(Into::<&[[f32; 4]]>::into(*keyframes)[anim_from].into(), Into::<&[[f32; 4]]>::into(*keyframes)[anim_to].into(), lerp_t)
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
                        joint_transforms[i] = joint_transforms[parent] * joint_transforms[i]; //. Reverse order because of how we store the matrices contra how shaders interpret them
                        parents[i] = parents[parent]; //. If that parent was relative to some other parent, then the transfomration in result[i] is only the transformation relative to that new parent, not "global" yet
                                                      // assert_eq!(parents[i], None, "non ordered bone array"); //. As long as the bone array is ordered from the root, we should only hit this while loop once. This assert was just as a sanity check and can be removed to allow for non ordered bone arrays
                        if parents[i] != None {
                            any_has_parent = true;
                        }
                    }
                }
            }

            for i in 0..joint_heirachy.len() {
                //. First undo the bind pose, then apply the current pose
                if let Some(skin_joint) = joint_heirachy[i].2 {
                    //. Only joints that affect the skin has an inverse bind matrix
                    joint_transforms[i] = joint_transforms[i] * inverse_bind_matricies[skin_joint as usize].transpose();
                }
            }
        }
        for i in 0..joint_transforms.len() {
            if let Some(skin_joint) = joint_heirachy[i].2 {
                //. Only joints that affect the skin are part of the skeleton send to the GPU
                skeleton[skin_joint as usize] = joint_transforms[i];
            }
        }
    }
    return skeleton.try_into().unwrap();
}

fn main() {
    unsafe {
        assert_eq!(BONES_PER_VERT, 4, "VertexInputAttributeDescription not generalized");
        println!("Example 05: glb loading with animation");
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
        struct StageUniform {
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
            let mesh = gltf_header.meshes.first().expect("Expected at least one mesh");
            // TODO: use "where" instead, or at least enforce the order
            let position_accessor = &gltf_header.accessors[mesh.primitives[0].attributes.position.expect("Expected a position accessor on model") as usize];
            let normal_accessor = &gltf_header.accessors[mesh.primitives[0].attributes.normal.expect("Expected a normal accessor on model") as usize];

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

            let mut verts = Vec::<AnimatedVertex>::with_capacity(vert_count);
            match mesh.primitives[0].attributes.joints_0 {
                Some(acessor_index) => {
                    let joints_data_ptr = {
                        let joints_accessor = &gltf_header.accessors[acessor_index as usize];
                        println!("{:?}", joints_accessor.component_type);
                        let joints_data_bytes = gltf_header.access_buffer(&gltf_buffer, joints_accessor);
                        let joints_data_ptr = joints_data_bytes.as_ptr() as *const [u8; 4];
                        let joints_data_len = joints_data_bytes.len() / (size_of::<[u8; 4]>()); // TODO: if this is always vert_count, consider inlining it in the assert
                        assert!(vert_count == joints_data_len, "expected {vert_count} joints, found {joints_data_len}");
                        joints_data_ptr
                    };

                    let weights_data_ptr = {
                        let weights_accessor = &gltf_header.accessors[mesh.primitives[0].attributes.weights_0.expect("Expected a weigths accessor on model, since it had a joints accessor") as usize];
                        println!("weights_accessor: {weights_accessor:?}");
                        let weights_data_bytes = gltf_header.access_buffer(&gltf_buffer, weights_accessor);
                        let weights_data_ptr = weights_data_bytes.as_ptr() as *const [f32; 4];
                        let weights_data_len = weights_data_bytes.len() / (size_of::<[f32; 4]>()); // TODO: if this is always vert_count, consider inlining it in the assert
                        assert!(vert_count == weights_data_len, "expected {vert_count} weights, found {weights_data_len}");
                        weights_data_ptr
                    };

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
                            normal: [norm3[0], norm3[1], norm3[2], 0.0],
                            color: [1.0, 0.0, 0.0, 1.0],
                            bones: joint4,
                            bone_weights: weight4,
                        });
                    }
                },
                None => {
                    //. Not animated
                    for vert in 0..vert_count {
                        //? Does this create the struct on stack and then copy, or is it able to be smart and emblace_back due to ownership semantics?
                        let pos3 = unsafe { position_data_ptr.offset(vert as isize).read_unaligned() }; //. This performs a copy maybe?
                        let norm3 = unsafe { normal_data_ptr.offset(vert as isize).read_unaligned() }; //. This performs a copy maybe?
                        assert!(BONES_PER_VERT == 4, "only support exactly 4 bones per vert at the moment");
                        verts.push(AnimatedVertex {
                            #[rustfmt::skip]
                            // TODO: make [f32;3] -> [f32;4] function/macro
                            pos: [pos3[0], pos3[1], pos3[2], 1.0],
                            normal: [norm3[0], norm3[1], norm3[2], 1.0],
                            color: [1.0, 0.0, 0.0, 1.0],
                            bones: [0, 0, 0, 0],
                            bone_weights: [1.0, 0.0, 0.0, 0.0],
                        });
                    }
                },
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

        let mut event_loop = EventLoop::new().expect("Failed to create new event loop? How can this even fail???");

        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("03-animation")
                    .with_inner_size(winit::dpi::LogicalSize::new(f64::from(window_width), f64::from(window_height))),
            )
            .unwrap();
        window.set_cursor_grab(winit::window::CursorGrabMode::Confined).unwrap();
        window.set_cursor_visible(false);

        let mut renderer = Renderer::<AnimatedVertex>::new(&window, window_width, window_height);

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
                vk::VertexInputAttributeDescription { location: 7, binding: 0, format: vk::Format::R32_SFLOAT, offset: (12 * 32 + 4 * 8 + 0 * 32) / 8 as u32 },
                vk::VertexInputAttributeDescription { location: 8, binding: 0, format: vk::Format::R32_SFLOAT, offset: (12 * 32 + 4 * 8 + 1 * 32) / 8 as u32 },
                vk::VertexInputAttributeDescription { location: 9, binding: 0, format: vk::Format::R32_SFLOAT, offset: (12 * 32 + 4 * 8 + 2 * 32) / 8 as u32 },
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
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.80, 0.75, 0.90, 1.0] } }, //. was renderer.clear_color
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

        //# Load assets from files
        let glb_file_paths = [
            // concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Glorp.glb"),
            // concat!(env!("CARGO_MANIFEST_DIR"), "/assets/test.glb"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/013_Octogecko_Art.glb"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/021_Rhomgon_Art.glb"),
            // concat!(env!("CARGO_MANIFEST_DIR"), "/assets/test3.glb"),
        ];
        let mut file_data = glb_file_paths.map(|path| (path, read(path).unwrap()));
        let loaded_meshes: Vec<_> = file_data
            .iter_mut()
            .map(|(file_path, gltf_file_data)| {
                let (gltf_header, gltf_buffer) = glb::parse(gltf_file_data);
                println!("Registering mesh and animations from {}", file_path);
                (
                    renderer.register_mesh(animated_vertex_mesh_from_gltf_data(&gltf_header, gltf_buffer)),
                    animations_from_gltf_data(&gltf_header, gltf_buffer),
                    gltf_header,
                    gltf_buffer,
                )
            })
            .collect();

        let obj_amount: i32 = glb_file_paths.len() as i32;
        let mut animated_objects: Vec<(Object, AnimationHandler)> = (0..obj_amount)
            .map(|i| {
                let (mesh, animations, gltf_header, gltf_buffer) = &loaded_meshes[(i as usize) % loaded_meshes.len()];
                (
                    Object {
                        transform: Transform {
                            position: Vec3::<f32> { x: (-(obj_amount / 2) + i) as f32 * 10.0, y: 0.0, z: -5.0 },
                            rotation: Quaternion::from_axis_rotation(Vec3 { x: 1.0, y: 0.0, z: 0.0 }.normalize(), PI),
                            scale: Vec3::one(),
                        },
                        mesh: *mesh,
                    },
                    AnimationHandler { animations: animations, current_animation: animations.len() as u8 - 1, current_anim_time: (i as f32) / (obj_amount as f32), gltf_header, gltf_buffer },
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
                // object.transform.rotation = Quaternion::from_axis_rotation(Vec3 { x: 0.0, y: 1.0, z: 0.0 }.normalize(), dt) * object.transform.rotation;

                animation.current_anim_time += dt * control;
            }

            light_rotation = Quaternion::from_axis_rotation(Vec3 { x: 0.0, y: 1.0, z: 0.0 }.normalize(), t * 0.0);
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
                            KeyCode::KeyH => {
                                selected_obj = selected_obj.wrapping_sub(1) % animated_objects.len();
                                println!("selecting obj: {selected_obj}");
                            },
                            KeyCode::KeyL => {
                                selected_obj = selected_obj.wrapping_add(1) % animated_objects.len();
                                println!("selecting obj: {selected_obj}");
                            },
                            KeyCode::KeyJ => {
                                let current_anim = animated_objects[selected_obj].1.current_animation;
                                let anims_count = animated_objects[selected_obj].1.animations.len() as u8;
                                let next_anim = current_anim.wrapping_add(1) % anims_count;
                                animated_objects[selected_obj].1.current_animation = next_anim;
                                let anim_name = &animated_objects[selected_obj].1.animations[next_anim as usize].name;
                                println!("Setting animation for obj: {selected_obj} to ({next_anim}), {anim_name}");
                            },
                            KeyCode::KeyK => {
                                let current_anim = animated_objects[selected_obj].1.current_animation;
                                let anims_count = animated_objects[selected_obj].1.animations.len() as u8;
                                let next_anim = current_anim.wrapping_sub(1) % anims_count;
                                animated_objects[selected_obj].1.current_animation = next_anim;
                                let anim_name = &animated_objects[selected_obj].1.animations[next_anim as usize].name;
                                println!("Setting animation for obj: {selected_obj} to ({next_anim}), {anim_name}");
                            },
                            _ => (),
                        }
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
                    _ => (), //. ignore other events
                }
            });

            //# Render
            //# (Re-)Animate the bones (Necromancers be like)
            let skeletons: Vec<_> = animated_objects
                .iter()
                .map(|(_, animation_handler)| {
                    let current_anim = animation_handler.current_animation as usize;
                    let current_time = animation_handler.current_anim_time;
                    animated_skeleton(animation_handler.gltf_header, animation_handler.gltf_buffer, &animation_handler.animations[current_anim], current_time)
                })
                .collect();

            //# Render the objects
            renderer.render_begin(DrawUniform { _dummy: 0.0 });
            renderer.render_stage(
                default_stage,
                StageUniform { view_mat: camera_transform.get_inverse_matrix(), proj_mat, light_dir: light_dir, ambient_light: 0.5 },
                animated_objects.iter().map(|(object, _)| object.mesh).collect(),
                animated_objects
                    .iter()
                    .enumerate()
                    .map(|(i, (object, _))| AnimatedObjectUniform { selected: if i == selected_obj { 1 } else { 0 }, model_mat: object.transform.get_matrix(), skeleton: skeletons[i] })
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
