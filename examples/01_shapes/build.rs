use std::{fs, process::Command};

#[rustfmt::skip]
macro_rules! dest_path { () => {"src/shader"}; }
#[rustfmt::skip]
macro_rules! src_path { () => {"src/shader_src"}; }

const GREEN: &str = "\x1b[92m";
const ORANGE: &str = "\x1b[93m";
const RED: &str = "\x1b[91m";
const RESET: &str = "\x1b[0m";
const ERROR: &str = RED;
const WARNING: &str = ORANGE;

macro_rules! warn {
    ($($arg:tt)*) => {
        print!("cargo::warning=");
        print!($($arg)*);
        print!("\n");
    };
}

fn is_leap_year(years_since_unix: u64) -> bool {
    let year = 1970 + years_since_unix;
    (year % 4 == 0) && (year % 100 != 0) || (year % 400 == 0)
}

fn convert_time(mut secs: u64) -> (u64, u64, u64, u64, u64) {
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

#[rustfmt::skip]
fn days_to_date(mut days: u64, leap_year: bool) -> Result<(&'static str, u64), String>{
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
    if days <= jan_len{return Ok(("Jan", days))}
    days = days - jan_len;
    if days <= feb_len{return Ok(("Feb", days))}
    days = days - feb_len;
    if days <= mar_len{return Ok(("Mar", days))}
    days = days - mar_len;
    if days <= apr_len{return Ok(("Apr", days))}
    days = days - apr_len;
    if days <= maj_len{return Ok(("Maj", days))}
    days = days - maj_len;
    if days <= jun_len{return Ok(("Jun", days))}
    days = days - jun_len;
    if days <= jul_len{return Ok(("Jul", days))}
    days = days - jul_len;
    if days <= aug_len{return Ok(("Aug", days))}
    days = days - aug_len;
    if days <= sep_len{return Ok(("Sep", days))}
    days = days - sep_len;
    if days <= oct_len{return Ok(("Oct", days))}
    days = days - oct_len;
    if days <= nov_len{return Ok(("Nov", days))}
    days = days - nov_len;
    if days <= dec_len{return Ok(("Dec", days))}
    days = days - dec_len;
    Result::Err(format!("{days} days too many for a year"))
}

#[rustfmt::skip]
fn main() {
    println!("cargo::rerun-if-changed={}", src_path!()); // TODO: figure out how to detect of the files in dest_path!() are missing

    let (years, days, hours, mins, secs) = convert_time(std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap().as_secs());
    let (month, date) = days_to_date(days, is_leap_year(years)).unwrap();
    let year = 1970 + years;
    warn!("Last recompiled shaders from \"{}\" {GREEN}{year:04}/{month}/{date:02} {hours:02}:{mins:02}:{secs:02} UTC{RESET}", src_path!());

    fs::create_dir_all(dest_path!()).expect(concat!("Failed to create dir: ", dest_path!()));
    if let Err(err) = build_shader(ShaderStage::Vert, concat!(src_path!(), "/vert.glsl"), concat!(dest_path!(), "/vert.spv")) {panic!("{}", err)};
    if let Err(err) = build_shader(ShaderStage::Vert, concat!(src_path!(), "/shadow_vert.glsl"), concat!(dest_path!(), "/shadow_vert.spv")) {panic!("{}", err)};
    if let Err(err) = build_shader(ShaderStage::Frag, concat!(src_path!(), "/frag.glsl"), concat!(dest_path!(), "/frag.spv")) {panic!("{}", err)};
}

enum ShaderStage {
    Vert,
    Frag,
}

#[derive(Debug)]
struct ExitStatusError {
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

fn build_shader(stage: ShaderStage, src: &str, dest: &str) -> Result<(), ExitStatusError> {
    let mut command = Command::new("glslc");
    command.args(&[
        &("-fshader-stage=".to_owned() //. Having to to_owned here is cringe... The compiler should be smart enough to realize there are only two options an essentially just create two static strings at compile time imo.
            + match stage {
            ShaderStage::Vert => "vert",
            ShaderStage::Frag => "frag",
        }),
        src,
        "-o",
        dest,
    ]);
    let command_str = format!("{:?}", command);
    let result = command.output().unwrap();
    match result.status.success() {
        true => Result::Ok(()),
        false => Result::Err(ExitStatusError { command: command_str, exit_code: result.status.code().unwrap(), error_msg: String::from_utf8(result.stderr).unwrap() }),
    }
}
