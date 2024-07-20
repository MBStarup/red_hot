use num::traits::real::Real;
use std::{
    fmt::Display,
    ops::{Add, AddAssign, Mul},
};

#[derive(Clone, Debug, Copy)]
pub struct Mat4x4<T>([T; 16])
where T: num::Num;

impl<T> Mat4x4<T>
where T: num::Num
{
    #[rustfmt::skip]
    pub fn zero() -> Mat4x4<T> {
        [
            T::zero(), T::zero(), T::zero(), T::zero(),
            T::zero(), T::zero(), T::zero(), T::zero(),
            T::zero(), T::zero(), T::zero(), T::zero(),
            T::zero(), T::zero(), T::zero(), T::zero(),
        ].into()
    }

    #[rustfmt::skip]
    pub fn unit() -> Mat4x4<T> {
        [
            T::one(),  T::zero(), T::zero(), T::zero(),
            T::zero(), T::one(),  T::zero(), T::zero(),
            T::zero(), T::zero(), T::one(),  T::zero(),
            T::zero(), T::zero(), T::zero(), T::one(),
        ].into()
    }
}

impl<T> core::ops::Deref for Mat4x4<T>
where T: num::Num
{
    type Target = [T; 16];
    fn deref(self: &'_ Self) -> &'_ Self::Target {
        &self.0
    }
}

impl<T> core::ops::DerefMut for Mat4x4<T>
where T: num::Num
{
    fn deref_mut(self: &'_ mut Self) -> &'_ mut Self::Target {
        &mut self.0
    }
}

impl<T> From<[T; 16]> for Mat4x4<T>
where T: num::Num
{
    fn from(value: [T; 16]) -> Self {
        Mat4x4(value)
    }
}

#[derive(Clone, Debug, Copy)]
#[repr(C)]
pub struct Vec3<T>
where T: num::Num
{
    pub x: T,
    pub y: T,
    pub z: T,
}

impl<T> Vec3<T>
where T: num::Num + Real
{
    pub fn zero() -> Vec3<T> {
        Vec3 { x: T::zero(), y: T::zero(), z: T::zero() }
    }

    pub fn one() -> Vec3<T> {
        Vec3 { x: T::one(), y: T::one(), z: T::one() }
    }

    pub fn up() -> Vec3<T> {
        Vec3 { x: T::zero(), y: -T::one(), z: T::zero() }
    }

    pub fn down() -> Vec3<T> {
        Vec3 { x: T::zero(), y: T::one(), z: T::zero() }
    }

    pub fn right() -> Vec3<T> {
        Vec3 { x: T::one(), y: T::zero(), z: T::zero() }
    }

    pub fn left() -> Vec3<T> {
        Vec3 { x: -T::one(), y: T::zero(), z: T::zero() }
    }

    pub fn forward() -> Vec3<T> {
        Vec3 { x: T::zero(), y: T::zero(), z: T::one() }
    }

    pub fn backwards() -> Vec3<T> {
        Vec3 { x: T::zero(), y: T::zero(), z: -T::one() }
    }

    pub fn normalize(&self) -> Vec3<T> {
        let s = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        Vec3 { x: self.x / s, y: self.y / s, z: self.z / s }
    }

    pub fn rotate(&self, r: Quaternion<T>) -> Vec3<T>
    where T: Display {
        let p = (r * Quaternion { x: self.x, y: self.y, z: self.z, w: T::zero() }) * Quaternion { x: -r.x, y: -r.y, z: -r.z, w: r.w };
        Vec3 { x: p.x, y: p.y, z: p.z }
    }
}

impl<T> Add for Vec3<T>
where T: num::Num
{
    type Output = Vec3<T>;

    fn add(self, rhs: Self) -> Self::Output {
        Vec3 { x: self.x + rhs.x, y: self.y + rhs.y, z: self.z + rhs.z }
    }
}

impl<T> std::ops::Mul<T> for Vec3<T>
where T: num::Num + Copy
{
    type Output = Vec3<T>;

    fn mul(self, rhs: T) -> Self::Output {
        Vec3 { x: self.x * rhs, y: self.y * rhs, z: self.z * rhs }
    }
}

impl<T> AddAssign for Vec3<T>
where T: num::Num + Copy
{
    fn add_assign(&mut self, rhs: Self) {
        self.x = self.x + rhs.x;
        self.y = self.y + rhs.y;
        self.z = self.z + rhs.z;
    }
}

impl<T> Display for Vec3<T>
where T: num::Num + Display
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{x: ")?;
        self.x.fmt(f)?;
        write!(f, ", y: ")?;
        self.y.fmt(f)?;
        write!(f, ", z: ")?;
        self.z.fmt(f)?;
        write!(f, "}}")
    }
}

#[derive(Clone, Debug, Copy)]
#[repr(C)]
pub struct Quaternion<T>
where T: num::Num
{
    pub x: T,
    pub y: T,
    pub z: T,
    pub w: T,
}

impl<T> Quaternion<T>
where T: num::Num + Real
{
    pub fn from_axis_rotation(axis: Vec3<T>, angle: T) -> Quaternion<T> {
        let two = T::one() + T::one();
        let scaled_vec = axis * (angle / two).sin();
        Quaternion { x: scaled_vec.x, y: scaled_vec.y, z: scaled_vec.z, w: (angle / two).cos() }
    }

    pub fn identity() -> Quaternion<T> {
        Quaternion { x: T::zero(), y: T::zero(), z: T::zero(), w: T::one() }
    }

    fn inverse(&self) -> Quaternion<T> {
        Quaternion { x: -self.x, y: -self.y, z: -self.z, w: self.w }
    }
}

impl<T> Mul for Quaternion<T>
where T: num::Num + Copy
{
    type Output = Quaternion<T>;

    fn mul(self, rhs: Self) -> Self::Output {
        Quaternion {
            x: self.w * rhs.x + self.x * rhs.w + self.y * rhs.z - self.z * rhs.y,
            y: self.w * rhs.y - self.x * rhs.z + self.y * rhs.w + self.z * rhs.x,
            z: self.w * rhs.z + self.x * rhs.y - self.y * rhs.x + self.z * rhs.w,
            w: self.w * rhs.w - self.x * rhs.x - self.y * rhs.y - self.z * rhs.z,
        }
    }
}

impl<T> Display for Quaternion<T>
where T: num::Num + Display
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{x: ")?;
        self.x.fmt(f)?;
        write!(f, ", y: ")?;
        self.y.fmt(f)?;
        write!(f, ", z: ")?;
        self.z.fmt(f)?;
        write!(f, ", w: ")?;
        self.w.fmt(f)?;
        write!(f, "}}")
    }
}

#[derive(Clone, Debug, Copy)]
pub struct Transform<T>
where T: num::Num
{
    pub position: Vec3<T>,
    pub rotation: Quaternion<T>,
    pub scale: Vec3<T>,
}

impl<T> Transform<T>
where T: num::Num + Real
{
    #[rustfmt::skip]
    pub fn get_matrix(&self) -> Mat4x4<T> {
        let two = T::one() + T::one();
        let q = self.rotation;
        [
            self.scale.x * (two * (q.w*q.w + q.x*q.x) - T::one()), self.scale.y *  two * (q.x*q.y - q.w*q.z),             self.scale.z *  two*(q.x*q.z + q.w*q.y),               self.position.x,
            self.scale.x *  two * (q.x*q.y + q.w*q.z),             self.scale.y * (two * (q.w*q.w + q.y*q.y) - T::one()), self.scale.z *  two*(q.y*q.z - q.w*q.x),               self.position.y,
            self.scale.x *  two * (q.x*q.z - q.w*q.y),             self.scale.y *  two * (q.y*q.z + q.w*q.x),             self.scale.z * (two * (q.w*q.w + q.z*q.z) - T::one()), self.position.z,
            T::zero(),                                             T::zero(),                                             T::zero(),                                             T::one(),
        ].into()
    }

    #[rustfmt::skip]
    pub fn get_inverse_matrix(&self) -> Mat4x4<T> {
        let two = T::one() + T::one();
        let q = self.rotation.inverse();
        [
            (two * (q.w*q.w + q.x*q.x) - T::one()) / self.scale.x,  two * (q.x*q.y - q.w*q.z)             / self.scale.x,  two*(q.x*q.z + q.w*q.y)               / self.scale.x, (-self.position.x * (two * (q.w*q.w + q.x*q.x) - T::one()) + -self.position.y *  two * (q.x*q.y - q.w*q.z)             + -self.position.z *  two*(q.x*q.z + q.w*q.y)              ) / self.scale.x,
             two * (q.x*q.y + q.w*q.z)             / self.scale.y, (two * (q.w*q.w + q.y*q.y) - T::one()) / self.scale.y,  two*(q.y*q.z - q.w*q.x)               / self.scale.y, (-self.position.x *  two * (q.x*q.y + q.w*q.z)             + -self.position.y * (two * (q.w*q.w + q.y*q.y) - T::one()) + -self.position.z *  two*(q.y*q.z - q.w*q.x)              ) / self.scale.y,
             two * (q.x*q.z - q.w*q.y)             / self.scale.z,  two * (q.y*q.z + q.w*q.x)             / self.scale.z, (two * (q.w*q.w + q.z*q.z) - T::one()) / self.scale.z, (-self.position.x *  two * (q.x*q.z - q.w*q.y)             + -self.position.y *  two * (q.y*q.z + q.w*q.x)             + -self.position.z * (two * (q.w*q.w + q.z*q.z) - T::one())) / self.scale.z,
            T::zero(),                                             T::zero(),                                             T::zero(),                                             T::one(),
        ].into()
    }
}

#[rustfmt::skip]
// https://www.youtube.com/watch?v=EqNcqBdrNyI
pub fn perspective_matrix<T>(fov_angle: T, near_z: T, far_z: T) -> Mat4x4<T> where T: num::Num + Real {
    let half = T::one() / (T::one() + T::one());
    let scale = T::one() / (fov_angle * half).tan();
    [
        scale,     T::zero(), T::zero(),                T::zero(),
        T::zero(), scale,     T::zero(),                T::zero(),
        T::zero(), T::zero(), far_z / (far_z - near_z), -far_z * near_z / (far_z - near_z),
        T::zero(), T::zero(), T::one(),                 T::zero(),
    ].into()
}
