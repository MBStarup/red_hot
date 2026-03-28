pub mod math;
pub mod renderer;
pub use red_hot_macros::*;

pub trait GlslType {
    const TYPE: ash::vk::Format;
}

pub trait Vertex {
    // TODO: Rewrite when/if #![feature(generic_const_exprs)] becomes staple
    //. We'd rather want:
    //. const N: usize;
    //. const ATTRIBUTE_DESCRIPTIONS: [ash::vk::VertexInputAttributeDescription; Self::N];
    //. Since that more clearly communicates (and enforces) that the type of the array is VertexAttributeDescription, which is has to be, and only the lenght is able to differ.
    type AttributeDescriptions;
    const ATTRIBUTE_DESCRIPTIONS: Self::AttributeDescriptions;
}

impl GlslType for [f32; 4] {
    // const SIZE: usize = size_of::<f32>() * 4;
    const TYPE: ash::vk::Format = ash::vk::Format::R32G32B32A32_SFLOAT;
}
