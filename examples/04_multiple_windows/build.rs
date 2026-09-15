use std::env;
use std::fs;
use std::path::PathBuf;

use red_hot_build::{self as build, attempt};

fn main() {
    attempt! {
        let shader_src = PathBuf::from("src").join("shaders");
        let shader_dst = PathBuf::from(env::var("OUT_DIR")?).join("shaders");

        build::rerun_if_changed(&shader_src);
        build::print_last_build_timestamp();

        fs::create_dir_all(&shader_dst)?;
        build::build_shaders(&shader_src, &shader_dst)?;
    }
}
