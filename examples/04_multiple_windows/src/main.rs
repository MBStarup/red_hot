use std::{
    f32::consts::TAU,
    mem::MaybeUninit,
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

const SKELETON_SIZE: usize = 32; // !!! Important, this is also hardcoded in the shader, so any changes should be reflected there as well
const BONES_PER_VERT: usize = 3;

fn main() {
    unsafe {
        assert_eq!(BONES_PER_VERT, 3, "VertexInputAttributeDescription not generalized");
        println!("Example 04: multiple windows");
        let mut window_width: u32 = 2000;
        let mut window_height: u32 = 1200;

        // TODO: load assets
        #[allow(dead_code)]
        #[derive(Clone, Debug, Copy)]
        #[repr(C)]
        struct Vertex {
            pos: [f32; 4],
            normal: [f32; 4],
            color: [f32; 4],
        }

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
        struct StageUniform {
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

        #[derive(Clone, Copy)]
        struct Object {
            transform: Transform<f32>,
            mesh: MeshIndex,
        }

        #[allow(dead_code)]
        #[derive(Clone, Debug, Copy)]
        #[repr(C)]
        struct AnimatedVertex {
            pos: [f32; 4],
            normal: [f32; 4],
            color: [f32; 4],
            bones: [u32; 3],        //. Indexes of which bones affect this vertex
            bone_weights: [f32; 3], //. Weights of how much each of those bones affect
        }

        #[allow(dead_code)]
        #[derive(Clone, Debug, Copy)]
        #[repr(C)]
        struct AnimatedObjectUniform {
            model_mat: Mat4x4<f32>,
            skeleton: [Mat4x4<f32>; SKELETON_SIZE], //. Hard-coded max bone count for now
        }

        let mut position = Vec3 { x: 0.0, y: 0.0, z: -10.0 };
        let mut cam_yaw = 0.0;
        let mut cam_pitch = 0.0;

        let proj_mat = perspective_matrix(TAU / 4.0, 0.1, 10000.0);

        struct Bone {
            parent: i32,
            bind_transform: Transform<f32>,
            relative_transform: Transform<f32>,
        }

        let worm_length = 10.0; //. Worm lengthg in *either* direction from zero, so half the worm length, think of it as the line equivalent of a radius
        let mut worm_bones: [Bone; WORM_BONE_COUNT] = unsafe { MaybeUninit::uninit().assume_init() }; // TODO: fix, this is stupid
        for i in 0..WORM_BONE_COUNT {
            // * Bind pose of worm bones should be
            // *   0       1   ...   n-1      n, n = WORM_BONE_COUNT
            // *  []----->[]---...--->[]----->[]
            // *  |-------------2-------------|
            // * -worm_length   0   worm_length
            let transform = Transform { position: Vec3 { x: (i as f32 / ((WORM_BONE_COUNT as f32 - 1.0) * 0.5) - 1.0) * worm_length, y: 0.0, z: 0.0 }, ..Transform::default() }; //. The transform of the bone in model space, i.e. the bind transform
            let parent_transform = match i {
                0 => Transform::default(),                      //. As the root has no parent, we'll use the default transform, placed at 0,0,0, with no rotation and
                _ => worm_bones[i as usize - 1].bind_transform, //. Otherwise we'll use the bind transform of the parent
            };

            println!("Transform {i}: {transform:?}");

            worm_bones[i] = Bone {
                parent: i as i32 - 1, //. Since the worm is a single chain, every node, n, has exaclty one parent, n-1, and child, n+1, excluding the leaf node and the root node.
                relative_transform: Transform {
                    position: transform.position - parent_transform.position, //. The position of the node relative to the parent = it's position - the parents position
                    rotation: transform.rotation * parent_transform.rotation.conjugate(), //. The rotation of the node relative to the parent = it's model space rotation, without it's parents model space rotation
                    scale: Vec3::one(),                                                   // TODO: actually do somethign with scale of bones
                },
                // relative_transform: (transform.get_matrix() * parent_transform.get_inverse_matrix()).to_transform(), //. Could do this, if to_transform: Mat4x4<f32> -> Transform was implemented
                bind_transform: transform,
            };
        }
        let mut worm_bone_xs: [f32; WORM_BONE_COUNT] = unsafe { MaybeUninit::uninit().assume_init() };
        for i in 0..WORM_BONE_COUNT {
            worm_bone_xs[i] = worm_bones[i].bind_transform.position.x;
        }

        fn get_bone_mats<const N: usize>(bones: &[Bone; N]) -> [Mat4x4<f32>; N] {
            let mut parents: [i32; N] = unsafe { MaybeUninit::uninit().assume_init() }; // TODO: fix, this is stupid
            let mut result: [Mat4x4<f32>; N] = unsafe { MaybeUninit::uninit().assume_init() }; // TODO: fix, this is stupid
            let mut any_has_parent = false;
            for i in 0..N {
                parents[i] = bones[i].parent; // TODO: firgure out how to do fixed size array to fixed size array mapping...
                if parents[i] != -1 {
                    any_has_parent = true;
                }
            }

            //. Calculate the transformation for the bone, relative to it's parent
            for i in 0..N {
                result[i] = bones[i].relative_transform.get_matrix();
            }

            while any_has_parent {
                any_has_parent = false;
                for i in 0..N {
                    if parents[i] != -1 {
                        let parent = parents[i] as usize;
                        result[i] = result[parent] * result[i]; //. I'd think that it would be "first apply the parent transform, then the "rest"", but for some reason this order seems to work instead...
                        parents[i] = parents[parent]; //. If that parent was relative to some other parent, then the transfomration in result[i] is only the transformation relative to that new parent, not "global" yet
                        assert_eq!(parents[i], -1, "non ordered bone array"); //. As long as the bone array is ordered from the root, we should only hit this while loop once. This assert was just as a sanity check and can be removed to allow for non ordered bone arrays
                        if parents[i] != -1 {
                            any_has_parent = true;
                        }
                    }
                }
            }
            for i in 0..N {
                //. first undo the bind pose, then apply the current pose
                result[i] = result[i] * bones[i].bind_transform.get_inverse_matrix();
            }
            result //. Result is the final transforms needed to move from the bind pose, to the current pose for each bone
        }

        let worm_len_resolution = 30; //. Amount of "bands" the cylinder is made up of, min 1
        let worm_disc_resolution = 10; //. The amount of corners per bands. min 3 (kinda 2)
        let worm_radius = 1.0;
        const WORM_BONE_COUNT: usize = 30;
        const _: () = assert!(WORM_BONE_COUNT <= SKELETON_SIZE);
        let animated_worm_mesh = {
            let vertices = vec![
                (0..worm_disc_resolution) //. Start cap
                    .map(|t| {
                        let x = -worm_length;
                        let y = f32::sin(((t % worm_disc_resolution) as f32 * TAU) / worm_disc_resolution as f32);
                        let z = f32::cos(((t % worm_disc_resolution) as f32 * TAU) / worm_disc_resolution as f32);
                        assert!((y * y + z * z) - (1.0) < 0.01);
                        AnimatedVertex {
                            pos: [x, y, z, 1.0],
                            normal: [-1.0, 0.0, 0.0, 0.0],
                            // normal: [-1.0, y * 0.2, z * 0.2, 0.0], //. Lying about "normal" for lighting effect
                            color: [(x + worm_length) / (2.0 * worm_length), 0.0, 1.0, 1.0],
                            bones: {
                                let mut bones: [MaybeUninit<u32>; BONES_PER_VERT] = unsafe { MaybeUninit::uninit().assume_init() };
                                for i in 0..BONES_PER_VERT {
                                    // TODO: there gotta be a better way to init an array with a compile time known static range
                                    bones[i] = MaybeUninit::new(i as u32);
                                }
                                unsafe { std::mem::transmute::<_, [u32; BONES_PER_VERT]>(bones) }
                            },
                            bone_weights: {
                                let mut weights = (0..3).map(|b| {
                                    let b_x = worm_bone_xs[b];
                                    let d_x = b_x - x;
                                    if d_x == 0.0 {
                                        1000.0
                                    } else {
                                        1.0 / (d_x * d_x) //. Approximate distance to bone
                                    }
                                });
                                let ws = Vec3 {
                                    x: weights.next().unwrap(), //. this should always yield 3 results
                                    y: weights.next().unwrap(),
                                    z: weights.next().unwrap(),
                                }
                                .normalize()
                                .into();
                                let _ = if t == 0 { println!("S: {ws:?}") } else { () }; //. the rust formatter is very opinionated... So am I... This if is only allowed to be on a single line if it's "occurs in an expression context", like an assignment.
                                ws
                            },
                        }
                    })
                    .collect(),
                (0..worm_len_resolution + 1) //. Wall of cylinder
                    .map(|i| {
                        (0..worm_disc_resolution).map(move |t| {
                            let x = (i as f32 / (worm_len_resolution as f32 * 0.5) - 1.0) * worm_length;
                            let y = f32::sin(((t % worm_disc_resolution) as f32 * TAU) / worm_disc_resolution as f32);
                            let z = f32::cos(((t % worm_disc_resolution) as f32 * TAU) / worm_disc_resolution as f32);
                            assert!(((y * y + z * z) - 1.0) < 0.01);
                            AnimatedVertex {
                                pos: [x, y, z, 1.0],
                                normal: [0.0, y, z, 1.0],
                                color: [(x + worm_length) / (2.0 * worm_length), 0.0, 1.0, 1.0],
                                bones: {
                                    let mut offset = 0;
                                    assert!(WORM_BONE_COUNT >= BONES_PER_VERT);
                                    let sections = 1 + WORM_BONE_COUNT - BONES_PER_VERT;
                                    let mut val = -worm_length + ((2.0 * worm_length) / sections as f32) * (offset + 1) as f32;
                                    while x > val {
                                        //. Assumes bones are evenly spaced from -worm_length to worm_length
                                        offset += 1;
                                        val = -worm_length + ((2.0 * worm_length) / sections as f32) * (offset + 1) as f32;
                                    }
                                    let mut bones: [MaybeUninit<u32>; BONES_PER_VERT] = unsafe { MaybeUninit::uninit().assume_init() };
                                    for i in 0..BONES_PER_VERT {
                                        bones[i] = MaybeUninit::new(offset + (i as u32));
                                    }
                                    unsafe { std::mem::transmute::<_, [u32; BONES_PER_VERT]>(bones) }
                                },
                                bone_weights: if i <= (worm_len_resolution as f32 * 0.5) as usize {
                                    let mut weights = (0..BONES_PER_VERT).map(|b| {
                                        let b_x = &worm_bone_xs[b];
                                        let d_x = b_x - x;
                                        if d_x == 0.0 {
                                            1000.0 //. Can't divide by zero, but what we want is bones with a d_x = 0 to have a *very* large weight, as they're right on top of the vertex. However f32::MAX cooks us when we try to normalize, soooo
                                        } else {
                                            1.0 / (d_x * d_x) //. Approximate distance to bone
                                        }
                                    });
                                    let ws = Vec3 {
                                        x: weights.next().unwrap(), //. this should always yield 3 results
                                        y: weights.next().unwrap(),
                                        z: weights.next().unwrap(),
                                    }
                                    .normalize()
                                    .into();
                                    let _ = if t == 0 { println!("{i}: {ws:?}") } else { () }; //. the rust formatter is very opinionated... So am I... This if is only allowed to be on a single line if it's "occurs in an expression context", like an assignment.
                                    ws
                                } else {
                                    let mut weights = (1..4).map(|b| {
                                        let b_x = worm_bone_xs[b];
                                        let d_x = b_x - x;
                                        if d_x == 0.0 {
                                            1000.0
                                        } else {
                                            1.0 / (d_x * d_x) //. Approximate distance to bone
                                        }
                                    });
                                    let ws = Vec3 {
                                        x: weights.next().unwrap(), //. this should always yield 3 results
                                        y: weights.next().unwrap(),
                                        z: weights.next().unwrap(),
                                    }
                                    .normalize()
                                    .into();
                                    let _ = if t == 0 { println!("{i}: {ws:?}") } else { () }; //. the rust formatter is very opinionated... So am I... This if is only allowed to be on a single line if it's "occurs in an expression context", like an assignment.
                                    ws
                                },
                            }
                        })
                    })
                    .flatten()
                    .collect::<Vec<AnimatedVertex>>(),
                (0..worm_disc_resolution) //. End cap
                    .map(|t| {
                        let x = worm_length;
                        let y = f32::sin(((t % worm_disc_resolution) as f32 * TAU) / worm_disc_resolution as f32);
                        let z = f32::cos(((t % worm_disc_resolution) as f32 * TAU) / worm_disc_resolution as f32);
                        assert!((y * y + z * z) - (1 as f32) < 0.01);
                        AnimatedVertex {
                            pos: [x, y, z, 1.0],
                            normal: [1.0, 0.0, 0.0, 0.0],
                            // normal: [1.0, y * 0.2, z * 0.2, 0.0], //. Lying about "normal" for lighting effect
                            color: [(x + worm_length) / (2.0 * worm_length), 0.0, 1.0, 1.0],
                            bones: {
                                let mut bones: [MaybeUninit<u32>; BONES_PER_VERT] = unsafe { MaybeUninit::uninit().assume_init() };
                                for i in 0..BONES_PER_VERT {
                                    bones[i] = MaybeUninit::new((WORM_BONE_COUNT - BONES_PER_VERT + i) as u32);
                                }
                                unsafe { std::mem::transmute::<_, [u32; BONES_PER_VERT]>(bones) }
                            },
                            bone_weights: {
                                let mut weights = (1..4).map(|b| {
                                    let b_x = worm_bone_xs[b];
                                    let d_x = b_x - x;
                                    if d_x == 0.0 {
                                        1000.0
                                    } else {
                                        1.0 / (d_x * d_x) //. Approximate distance to bone
                                    }
                                });
                                let ws: [f32; 3] = Vec3 {
                                    x: weights.next().unwrap(), //. this shoudl always yield 3 results
                                    y: weights.next().unwrap(),
                                    z: weights.next().unwrap(),
                                }
                                .normalize()
                                .into();
                                let _ = if t == 0 { println!("E: {ws:?}") } else { () }; //. the rust formatter is very opinionated... So am I... This if is only allowed to be on a single line if it's "occurs in an expression context", like an assignment.
                                ws
                            },
                        }
                    })
                    .collect(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<AnimatedVertex>>();
            let indices = (vec![
                vec![(0..worm_disc_resolution - 2).map(|i| vec![0, 1 + i, 2 + i]).collect::<Vec<Vec<usize>>>()],
                (1..worm_len_resolution + 1)
                    .map(|band| {
                        (0..worm_disc_resolution)
                            .map(|tile| {
                                vec![
                                    band * worm_disc_resolution + tile,
                                    (band + 1) * worm_disc_resolution + tile,
                                    (band + 1) * worm_disc_resolution + ((tile + 1) % worm_disc_resolution),
                                    band * worm_disc_resolution + tile,
                                    (band + 1) * worm_disc_resolution + ((tile + 1) % worm_disc_resolution),
                                    band * worm_disc_resolution + ((tile + 1) % worm_disc_resolution),
                                ]
                            })
                            .collect::<Vec<Vec<usize>>>()
                    })
                    .collect::<Vec<Vec<Vec<usize>>>>(),
                vec![(0..worm_disc_resolution - 2)
                    .map(|i| {
                        vec![
                            worm_disc_resolution * (worm_len_resolution + 2),
                            worm_disc_resolution * (worm_len_resolution + 2) + 2 + i,
                            worm_disc_resolution * (worm_len_resolution + 2) + 1 + i,
                        ]
                    })
                    .collect::<Vec<Vec<usize>>>()],
            ])
            .into_iter()
            .flatten()
            .flatten()
            .flatten()
            .collect::<Vec<usize>>()
            .into_iter()
            .map(|x| x as u32)
            .collect();
            Mesh::<AnimatedVertex> { vertices, indices }
        };

        let mut event_loop = EventLoop::new().expect("Failed to create new event loop? How can this even fail???");

        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("04-multiple_windwows-01")
                    .with_inner_size(winit::dpi::LogicalSize::new(f64::from(window_width), f64::from(window_height))),
            )
            .unwrap();
        window.set_cursor_grab(winit::window::CursorGrabMode::Confined).unwrap();
        window.set_cursor_visible(false);

        let other_window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("04-multiple_windwows-02")
                    .with_inner_size(winit::dpi::LogicalSize::new(f64::from(window_width), f64::from(window_height))),
            )
            .unwrap();
        other_window.set_cursor_grab(winit::window::CursorGrabMode::Confined).unwrap();
        other_window.set_cursor_visible(false);

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
                vk::VertexInputAttributeDescription { location: 3, binding: 0, format: vk::Format::R32_UINT, offset: 12 * 32 / 8 as u32 },
                vk::VertexInputAttributeDescription { location: 4, binding: 0, format: vk::Format::R32_UINT, offset: 13 * 32 / 8 as u32 },
                vk::VertexInputAttributeDescription { location: 5, binding: 0, format: vk::Format::R32_UINT, offset: 14 * 32 / 8 as u32 },
                //
                vk::VertexInputAttributeDescription { location: 6, binding: 0, format: vk::Format::R32_SFLOAT, offset: 15 * 32 / 8 as u32 },
                vk::VertexInputAttributeDescription { location: 7, binding: 0, format: vk::Format::R32_SFLOAT, offset: 16 * 32 / 8 as u32 },
                vk::VertexInputAttributeDescription { location: 8, binding: 0, format: vk::Format::R32_SFLOAT, offset: 17 * 32 / 8 as u32 },
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
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.30, 0.75, 0.90, 1.0] } }, //. was renderer.clear_color
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

        let animated_worm_meshi = renderer.register_mesh(animated_worm_mesh);

        let mut other_renderer = Renderer::<Vertex>::new(&other_window, window_width, window_height);

        let other_depth_image = other_renderer.register_image(RedHotImageCreateInfo {
            size: ImageSize::SurfaceSize,
            format: vk::Format::D32_SFLOAT,
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            memory_flags: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            image_aspect_mask: vk::ImageAspectFlags::DEPTH,
        });

        let other_default_stage = other_renderer.register_stage(
            "Default".to_owned(),
            include_bytes!("./shader/vert_basic.spv"),
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
            ],
            vec![red_hot::renderer::RedHotStageImage::SwapchainImage(), red_hot::renderer::RedHotStageImage::Image(other_depth_image)],
            &[
                vk::AttachmentDescription {
                    format: other_renderer.swapchain_image_format,
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
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.30, 0.75, 0.90, 1.0] } }, //. was renderer.clear_color
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

        let cube_meshi = other_renderer.register_mesh(cube_mesh);

        let objects = vec![Object { transform: Transform::default(), mesh: animated_worm_meshi }];

        let mut last_time;
        let mut current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let mut t = 0.0;
        let mut dt;
        let mut control = 1.0;
        let mut should_close = false;
        let mut light_dir;
        let mut light_rotation;
        let mut a = 1.0;
        let mut b = 1.0;
        let mut c = 1.0;

        let mut focused = false;

        while !should_close {
            //. Animate the bones
            for i in 0..WORM_BONE_COUNT {
                // worm_bones[i].relative_transform.position = Vec3 { y: a * f32::sin(b * t + c * std::f32::consts::PI * i as f32 / (WORM_BONE_COUNT as f32 + 1.0) as f32), ..worm_bones[i].relative_transform.position };
                // worm_bones[i].relative_transform.rotation = Quaternion::from_axis_rotation(Vec3 { x: 0.0, y: 0.0, z: -1.0 }.normalize(), a * f32::sin(c + t * b) * std::f32::consts::PI / WORM_BONE_COUNT as f32);
                worm_bones[i].relative_transform.rotation = Quaternion::from_axis_rotation(Vec3 { x: 0.0, y: 0.0, z: -1.0 }.normalize(), f32::sin(i as f32 * c + t * b) * a);
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
                    Event::WindowEvent { window_id, event: WindowEvent::Resized(size), .. } => {
                        // BUG: On GLaDOS (Debian KDE Wayland) windows get moved instead of resized, and the size is incomprehensible?!?!
                        if window_id == window.id() {
                            renderer.resize_window(size.width, size.height);
                        } else if window_id == other_window.id() {
                            other_renderer.resize_window(size.width, size.height);
                        }
                    },
                    Event::WindowEvent { event: WindowEvent::Moved(winit::dpi::PhysicalPosition { x, y }), .. } => {
                        println!("Moved window to {x}x{y}");
                    },
                    Event::WindowEvent { event: WindowEvent::Focused(focus), .. } => {
                        focused = focus;
                        // window.set_cursor_visible(!focus);
                    },
                    _ => (), //. ignore other events
                }
            });

            //# Render
            other_renderer.render_begin(DrawUniform { _dummy: 0.0 });
            // * Render bones:
            // other_renderer.render_stage(
            //     other_default_stage,
            //     StageUniform { view_mat: camera_transform.get_inverse_matrix(), proj_mat, light_dir, ambient_light: 0.1 }, worm_bones.iter().map(|_| cube_meshi).collect(), {
            //     worm_bones
            //         .iter()
            //         .map(|b| {
            //             let mut mat = b.relative_transform.get_matrix();
            //             let mut parent = b.parent;
            //             while parent > -1 {
            //                 let p = parent as usize;
            //                 mat = worm_bones[p].relative_transform.get_matrix() * mat; //. first move by the parent, then by the rest, I think I accidentally made it right before left this project, oh well...
            //                 parent = worm_bones[p].parent;
            //             }
            //             ObjectUniform { model_mat: mat.scale(0.2) }
            //         })
            //         .collect()
            // });
            // * Render bones, other method:
            other_renderer.render_stage(
                other_default_stage,
                StageUniform { view_mat: camera_transform.get_inverse_matrix(), proj_mat, light_dir, ambient_light: 0.1 },
                (0..WORM_BONE_COUNT).map(|_| cube_meshi).collect(),
                {
                    get_bone_mats(&worm_bones)
                        .iter()
                        .zip(&worm_bones)
                        .map(|(bone_mat, bone)| ObjectUniform { model_mat: (*bone_mat * bone.bind_transform.get_matrix()).scale(0.1) })
                        .collect()
                },
            );
            other_renderer.render_commit();

            // * Render animated snake:
            renderer.render_begin(DrawUniform { _dummy: 0.0 });
            renderer.render_stage(
                default_stage,
                StageUniform { view_mat: camera_transform.get_inverse_matrix(), proj_mat, light_dir, ambient_light: 0.4 },
                objects.iter().map(|object| object.mesh).collect(),
                objects
                    .iter()
                    .map(|object| AnimatedObjectUniform {
                        model_mat: object.transform.get_matrix(),
                        skeleton: {
                            let mut bones = [Mat4x4::<f32>::unit(); SKELETON_SIZE];
                            for (i, mat) in get_bone_mats(&worm_bones).iter().enumerate() {
                                // TODO: for some FUCK ASS reason, the first bone transform is actually interpreted as the model transform
                                bones[i] = *mat;
                            }
                            bones
                        },
                    })
                    .collect(),
            );
            renderer.render_commit();

            last_time = current_time;
            current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            dt = (current_time - last_time).as_secs_f32();
            t = t + dt;
        }

        renderer.destroy();
        other_renderer.destroy();
        println!("Done!");
    }
}
