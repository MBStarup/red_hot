use std::fs;

use red_hot_build::{self as build, attempt, ShaderStage};

fn main() {
    attempt! {
        macro_rules! shader_src { ($($path:literal)?) => { concat!("src/shader_src/", $($path)?) }; }
        macro_rules! shader_dst { ($($path:literal)?) => { concat!("src/shader/", $($path)?) }; }

        build::rerun_if_changed(shader_dst!());
        build::print_last_build_timestamp();

        fs::create_dir_all(shader_dst!())?;
        build::build_shader(ShaderStage::Vert, shader_src!("vert.glsl"), shader_dst!("vert.spv"), [])?;
        build::build_shader(ShaderStage::Frag, shader_src!("frag.glsl"), shader_dst!("frag.spv"), [])?;
    }
}
