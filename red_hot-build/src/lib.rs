use std::{
    ffi::OsStr,
    fs::{self},
    path::Path,
    process::Command,
};

pub const RED: &str = "\x1b[91m";
pub const GREEN: &str = "\x1b[92m";
pub const YELLOW: &str = "\x1b[93m";
pub const BLUE: &str = "\x1b[94m";
pub const MAGENTA: &str = "\x1b[95m";
pub const CYAN: &str = "\x1b[96m";
pub const RESET: &str = "\x1b[0m";

pub const ERROR: &str = RED;
pub const WARNING: &str = YELLOW;
pub const INFO: &str = BLUE;

#[macro_export]
macro_rules! log {
    ($label:expr, $color:expr, $($arg:tt)*) => {{
        print!("cargo::warning={}[{}]{}: ", $color, $label, $crate::RESET);
        print!($($arg)*);
        print!("{}\n", $crate::RESET);
    }};
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => { $crate::log!("INFO", $crate::INFO, $($arg)*); };
}
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => { $crate::log!("WARNING", $crate::WARNING, $($arg)*); };
}
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => { $crate::log!("ERROR", $crate::ERROR, $($arg)*); };
}

pub fn is_leap_year(years_since_unix: u64) -> bool {
    let year = 1970 + years_since_unix;
    (year % 4 == 0) && (year % 100 != 0) || (year % 400 == 0)
}

pub fn convert_time(mut secs: u64) -> (u64, u64, u64, u64, u64) {
    let mut mins = secs / 60;
    secs = secs - (mins * 60);
    let mut hours = mins / 60;
    mins = mins - (hours * 60);
    let mut days = hours / 24;
    hours = hours - (days * 24);
    let mut years = 0;
    while days >= if is_leap_year(years) { 366 } else { 365 } {
        days = days - if is_leap_year(years) { 366 } else { 365 };
        years = years + 1;
    }
    (years, days, hours, mins, secs)
}

pub fn days_to_date(mut days: u64, leap_year: bool) -> Result<(&'static str, u64), String> {
    let jan_len = 31;
    let feb_len = if leap_year { 29 } else { 28 };
    let mar_len = 31;
    let apr_len = 30;
    let maj_len = 31;
    let jun_len = 30;
    let jul_len = 31;
    let aug_len = 31;
    let sep_len = 30;
    let oct_len = 31;
    let nov_len = 30;
    let dec_len = 31;

    days = days + 1;
    if days <= jan_len {
        return Ok(("Jan", days));
    }
    days = days - jan_len;
    if days <= feb_len {
        return Ok(("Feb", days));
    }
    days = days - feb_len;
    if days <= mar_len {
        return Ok(("Mar", days));
    }
    days = days - mar_len;
    if days <= apr_len {
        return Ok(("Apr", days));
    }
    days = days - apr_len;
    if days <= maj_len {
        return Ok(("Maj", days));
    }
    days = days - maj_len;
    if days <= jun_len {
        return Ok(("Jun", days));
    }
    days = days - jun_len;
    if days <= jul_len {
        return Ok(("Jul", days));
    }
    days = days - jul_len;
    if days <= aug_len {
        return Ok(("Aug", days));
    }
    days = days - aug_len;
    if days <= sep_len {
        return Ok(("Sep", days));
    }
    days = days - sep_len;
    if days <= oct_len {
        return Ok(("Oct", days));
    }
    days = days - oct_len;
    if days <= nov_len {
        return Ok(("Nov", days));
    }
    days = days - nov_len;
    if days <= dec_len {
        return Ok(("Dec", days));
    }
    days = days - dec_len;
    Result::Err(format!("{days} days too many for a year"))
}

pub fn print_last_build_timestamp() {
    let (years, days, hours, mins, secs) = convert_time(std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap().as_secs());
    let (month, date) = days_to_date(days, is_leap_year(years)).unwrap();
    let year = 1970 + years;
    info!("Last build.rs ran {GREEN}{year:04}/{month}/{date:02} {hours:02}:{mins:02}:{secs:02} UTC{RESET}");
}

pub fn rerun_if_changed<S: AsRef<Path>>(path: S) {
    println!("cargo::rerun-if-changed={}", path.as_ref().display()); // TODO: figure out how to detect of the files in dest_path!() are missing
}

#[derive(Clone, Debug)]
pub struct ExitStatusError {
    command: String,
    exit_code: i32,
    error_msg: String,
}

impl std::fmt::Display for ExitStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Command {WARNING}")?;
        self.command.fmt(f)?;
        write!(f, "{RESET} exited with status {WARNING}")?;
        self.exit_code.fmt(f)?;
        write!(f, "\n{ERROR}error:{RESET} ")?;
        self.error_msg.fmt(f)
    }
}

impl std::error::Error for ExitStatusError {}

#[macro_export]
macro_rules! build_shader {
    //. Wrapper macro to automatically fill out empty extra_args, because it needs the annoying type annotations
    ($src:expr, $dst:expr, $extra_args:expr) => {
        $crate::build_shader($src, $dst, $extra_args)
    };
    ($src:expr, $dst:expr) => {
        $crate::build_shader($src, $dst, [] as [&std::ffi::OsStr; 0])
    };
}

pub fn build_shader(src: impl AsRef<OsStr>, dst: impl AsRef<OsStr>, extra_args: impl IntoIterator<Item: AsRef<OsStr>>) -> Result<(), Box<dyn std::error::Error>> {
    let src = src.as_ref();

    let mut command = Command::new("glslc");
    command.args(extra_args).arg(src).arg("-o").arg(dst);

    let command_str = format!("{command:?}");
    let output = command.output()?;

    match output.status.success() {
        true => Ok(()),
        false => Err(Box::new(ExitStatusError {
            command: command_str,
            exit_code: output.status.code().unwrap_or(-1),
            error_msg: String::from_utf8_lossy(&output.stderr).into_owned(),
        })),
    }
}

pub fn build_shaders(shader_src: &Path, shader_dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    const SHADER_EXTENSIONS: [&str; 6] = [
        // NOTE: Supported extensions from the glslc man page
        "vert", "frag", "tesc", "tese", "geom", "comp",
    ];
    for entry in fs::read_dir(shader_src)? {
        let path = entry?.path();

        if path.is_file() {
            if !path.extension().and_then(OsStr::to_str).is_some_and(|ext| SHADER_EXTENSIONS.contains(&ext)) {
                // TODO[#pragma shader_stage]: We could support more extensions, like .glsl, and look for a '#pragma shader_stage()' to guess if it's a shader (or even just assume so and let glslc error if it can't find any), or even look for a valid (only preceded by preprocessor directives) '#pragma shader_stage()' in files with ANY extenion. Or maybe ask glslc to validate for us. But for now, forcing extension seems fine to me.
                warn!("Skipping compilation of non-shader {}", path.display());
                continue;
            }

            let output = shader_dst.join(path.file_name().unwrap());
            build_shader!(&path, output)?;
        }
    }

    Ok(())
}

//. Used so I can panic with my errors printing using Display, instead of Debug with the "?" operator
//. I'd prefer to just have main -> Result<(), Box<dyn Error>>, but alas, for some fuckass reason, even though Error REQUIRES Display, it choses to print using Debug...
//. The try {} is not much better, it wants type annotations, and that seemingly requires actually binding to an indetifier
#[macro_export]
macro_rules! attempt {
    ($($body:tt)*) => {
        if let Err(error) = (|| -> Result<(), Box<dyn std::error::Error>> {
            $($body)*
            Ok(())
        })() {
            panic!("{error}");
        }
    };
}
