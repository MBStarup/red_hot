use core::panic;
use std::{error::Error, fmt::Display, fs};

use red_hot_build::{self as build, ShaderStage};

fn main() {
    build::attempt! {
        macro_rules! shader_src { ($($path:literal)?) => { concat!("src/shader_src/", $($path)?) }; }
        macro_rules! shader_dst { ($($path:literal)?) => { concat!("src/shader/", $($path)?) }; }

        build::rerun_if_changed(shader_dst!());
        build::print_last_build_timestamp();

        fs::create_dir_all(shader_dst!())?;
        build::build_shader(ShaderStage::Vert, shader_src!("vert.glsl"), shader_dst!("vert.spv"), [])?;
        build::build_shader(ShaderStage::Vert, shader_src!("shadow_vert.glsl"), shader_dst!("shadow_vert.spv"), [])?;
        build::build_shader(ShaderStage::Frag, shader_src!("frag.glsl"), shader_dst!("frag.spv"), [])?;
        build::build_shader(ShaderStage::Frag, shader_src!("shadow_frag.glsl"), shader_dst!("shadow_frag.spv"), [])?;
        build::build_shader(ShaderStage::Frag, shader_src!("green_frag.glsl"), shader_dst!("green_frag.spv"), [])?;
        build::build_shader(ShaderStage::Vert, shader_src!("ui_vert.glsl"), shader_dst!("ui_vert.spv"), [])?;
        build::build_shader(ShaderStage::Frag, shader_src!("ui_frag.glsl"), shader_dst!("ui_frag.spv"), [])?;
    }
}
