//. This file defines constants that should be synchronized across both the rust code and the shader
//. Shader constants must be defined exactly: pub const NAME: type = value;
//. Only supported types will passed to the shader, see build.rs

pub const BONES_PER_VERT: usize = 4;
pub const SKELETON_SIZE: usize = 64;
