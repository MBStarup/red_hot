use num::traits::real::Real;
use std::{
    fmt::Display,
    ops::{Add, AddAssign, Mul, Sub},
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

    #[rustfmt::skip]
    pub fn scale(&self, scale :T) -> Mat4x4<T> where T: Copy { // TODO: study the Copy trait
        [
            scale * self[00], scale * self[01], scale * self[02], self[03],
            scale * self[04], scale * self[05], scale * self[06], self[07],
            scale * self[08], scale * self[09], scale * self[10], self[11],
            scale * self[12], scale * self[13], scale * self[14], self[15], // TODO: is the bottom row always [0,0,0,1]? in that case we don't need to multiply by scale
        ].into()
    }
}

impl<T> Display for Mat4x4<T>
where T: num::Num + Display
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mat4x4<{}>([\n", std::any::type_name::<T>())?;
        for i in 0..4 {
            write!(f, " ")?;
            for j in 0..4 {
                self.0[i * 4 + j].fmt(f)?;
                write!(f, ", ")?;
            }
            write!(f, "\n")?;
        }
        write!(f, "])")
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

impl<T> std::ops::Mul<Mat4x4<T>> for Mat4x4<T>
where T: num::Num + Copy
{
    type Output = Mat4x4<T>;

    fn mul(self, Mat4x4(rhs): Mat4x4<T>) -> Self::Output {
        let lhs = self.0; // NOTE: It is SO CRINGE that you're not allowed to destructure the self parameter due to "backwards compatability"!!!
        [
            lhs[0 + 4 * 0] * rhs[0 + 4 * 0] + lhs[1 + 4 * 0] * rhs[0 + 4 * 1] + lhs[2 + 4 * 0] * rhs[0 + 4 * 2] + lhs[3 + 4 * 0] * rhs[0 + 4 * 3],
            lhs[0 + 4 * 0] * rhs[1 + 4 * 0] + lhs[1 + 4 * 0] * rhs[1 + 4 * 1] + lhs[2 + 4 * 0] * rhs[1 + 4 * 2] + lhs[3 + 4 * 0] * rhs[1 + 4 * 3],
            lhs[0 + 4 * 0] * rhs[2 + 4 * 0] + lhs[1 + 4 * 0] * rhs[2 + 4 * 1] + lhs[2 + 4 * 0] * rhs[2 + 4 * 2] + lhs[3 + 4 * 0] * rhs[2 + 4 * 3],
            lhs[0 + 4 * 0] * rhs[3 + 4 * 0] + lhs[1 + 4 * 0] * rhs[3 + 4 * 1] + lhs[2 + 4 * 0] * rhs[3 + 4 * 2] + lhs[3 + 4 * 0] * rhs[3 + 4 * 3],
            lhs[0 + 4 * 1] * rhs[0 + 4 * 0] + lhs[1 + 4 * 1] * rhs[0 + 4 * 1] + lhs[2 + 4 * 1] * rhs[0 + 4 * 2] + lhs[3 + 4 * 1] * rhs[0 + 4 * 3],
            lhs[0 + 4 * 1] * rhs[1 + 4 * 0] + lhs[1 + 4 * 1] * rhs[1 + 4 * 1] + lhs[2 + 4 * 1] * rhs[1 + 4 * 2] + lhs[3 + 4 * 1] * rhs[1 + 4 * 3],
            lhs[0 + 4 * 1] * rhs[2 + 4 * 0] + lhs[1 + 4 * 1] * rhs[2 + 4 * 1] + lhs[2 + 4 * 1] * rhs[2 + 4 * 2] + lhs[3 + 4 * 1] * rhs[2 + 4 * 3],
            lhs[0 + 4 * 1] * rhs[3 + 4 * 0] + lhs[1 + 4 * 1] * rhs[3 + 4 * 1] + lhs[2 + 4 * 1] * rhs[3 + 4 * 2] + lhs[3 + 4 * 1] * rhs[3 + 4 * 3],
            lhs[0 + 4 * 2] * rhs[0 + 4 * 0] + lhs[1 + 4 * 2] * rhs[0 + 4 * 1] + lhs[2 + 4 * 2] * rhs[0 + 4 * 2] + lhs[3 + 4 * 2] * rhs[0 + 4 * 3],
            lhs[0 + 4 * 2] * rhs[1 + 4 * 0] + lhs[1 + 4 * 2] * rhs[1 + 4 * 1] + lhs[2 + 4 * 2] * rhs[1 + 4 * 2] + lhs[3 + 4 * 2] * rhs[1 + 4 * 3],
            lhs[0 + 4 * 2] * rhs[2 + 4 * 0] + lhs[1 + 4 * 2] * rhs[2 + 4 * 1] + lhs[2 + 4 * 2] * rhs[2 + 4 * 2] + lhs[3 + 4 * 2] * rhs[2 + 4 * 3],
            lhs[0 + 4 * 2] * rhs[3 + 4 * 0] + lhs[1 + 4 * 2] * rhs[3 + 4 * 1] + lhs[2 + 4 * 2] * rhs[3 + 4 * 2] + lhs[3 + 4 * 2] * rhs[3 + 4 * 3],
            lhs[0 + 4 * 3] * rhs[0 + 4 * 0] + lhs[1 + 4 * 3] * rhs[0 + 4 * 1] + lhs[2 + 4 * 3] * rhs[0 + 4 * 2] + lhs[3 + 4 * 3] * rhs[0 + 4 * 3],
            lhs[0 + 4 * 3] * rhs[1 + 4 * 0] + lhs[1 + 4 * 3] * rhs[1 + 4 * 1] + lhs[2 + 4 * 3] * rhs[1 + 4 * 2] + lhs[3 + 4 * 3] * rhs[1 + 4 * 3],
            lhs[0 + 4 * 3] * rhs[2 + 4 * 0] + lhs[1 + 4 * 3] * rhs[2 + 4 * 1] + lhs[2 + 4 * 3] * rhs[2 + 4 * 2] + lhs[3 + 4 * 3] * rhs[2 + 4 * 3],
            lhs[0 + 4 * 3] * rhs[3 + 4 * 0] + lhs[1 + 4 * 3] * rhs[3 + 4 * 1] + lhs[2 + 4 * 3] * rhs[3 + 4 * 2] + lhs[3 + 4 * 3] * rhs[3 + 4 * 3],
        ]
        .into()
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

impl<T> Into<[T; 3]> for Vec3<T>
where T: num::Num + Real
{
    fn into(self) -> [T; 3] {
        [self.x, self.y, self.z]
    }
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

    //. Matrix multiplication of T * V, assuming implicit 4th component of the vector to be 1, and resulting 4th component to be truncated
    pub fn transform(&self, transformation: &Mat4x4<T>) -> Vec3<T> {
        let w = self.x * transformation[12] + self.y * transformation[13] + self.z * transformation[14] + transformation[15];

        Vec3 {
            x: self.x * transformation[0] + self.y * transformation[1] + self.z * transformation[2] + transformation[3],
            y: self.x * transformation[4] + self.y * transformation[5] + self.z * transformation[6] + transformation[7],
            z: self.x * transformation[8] + self.y * transformation[9] + self.z * transformation[10] + transformation[11],
        } * (T::one() / w)
    }

    //. Assumes affine transformation (skips division by w)
    //. Matrix multiplication of T * V, assuming implicit 4th component of the vector to be 1, and resulting 4th component to be truncated
    pub fn transform_affine(&self, transformation: &Mat4x4<T>) -> Vec3<T> {
        Vec3 {
            x: self.x * transformation[0] + self.y * transformation[1] + self.z * transformation[2] + transformation[3],
            y: self.x * transformation[4] + self.y * transformation[5] + self.z * transformation[6] + transformation[7],
            z: self.x * transformation[8] + self.y * transformation[9] + self.z * transformation[10] + transformation[11],
        }
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

impl<T> std::ops::Neg for Vec3<T>
where T: num::Num + Copy
{
    type Output = Vec3<T>;

    fn neg(self) -> Self::Output {
        self * (T::zero() - T::one())
    }
}

impl<T> Sub for Vec3<T>
where T: num::Num
{
    type Output = Vec3<T>;

    fn sub(self, rhs: Self) -> Self::Output {
        //. self + (-rhs)
        Vec3 { x: self.x - rhs.x, y: self.y - rhs.y, z: self.z - rhs.z }
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

    pub fn conjugate(&self) -> Quaternion<T> {
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
        let q = self.rotation.conjugate();
        [
            (two * (q.w*q.w + q.x*q.x) - T::one()) / self.scale.x,  two * (q.x*q.y - q.w*q.z)             / self.scale.x,  two*(q.x*q.z + q.w*q.y)               / self.scale.x, (-self.position.x * (two * (q.w*q.w + q.x*q.x) - T::one()) + -self.position.y *  two * (q.x*q.y - q.w*q.z)             + -self.position.z *  two*(q.x*q.z + q.w*q.y)              ) / self.scale.x,
             two * (q.x*q.y + q.w*q.z)             / self.scale.y, (two * (q.w*q.w + q.y*q.y) - T::one()) / self.scale.y,  two*(q.y*q.z - q.w*q.x)               / self.scale.y, (-self.position.x *  two * (q.x*q.y + q.w*q.z)             + -self.position.y * (two * (q.w*q.w + q.y*q.y) - T::one()) + -self.position.z *  two*(q.y*q.z - q.w*q.x)              ) / self.scale.y,
             two * (q.x*q.z - q.w*q.y)             / self.scale.z,  two * (q.y*q.z + q.w*q.x)             / self.scale.z, (two * (q.w*q.w + q.z*q.z) - T::one()) / self.scale.z, (-self.position.x *  two * (q.x*q.z - q.w*q.y)             + -self.position.y *  two * (q.y*q.z + q.w*q.x)             + -self.position.z * (two * (q.w*q.w + q.z*q.z) - T::one())) / self.scale.z,
            T::zero(),                                             T::zero(),                                             T::zero(),                                             T::one(),
        ].into()
    }

    pub fn default() -> Transform<T> {
        Transform { position: Vec3::zero(), rotation: Quaternion::identity(), scale: Vec3::one() }
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
