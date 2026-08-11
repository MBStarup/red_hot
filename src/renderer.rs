use ash::vk::Framebuffer;
use ash::{
    extensions::{
        ext::DebugUtils,
        khr::{Surface, Swapchain},
    },
    util::read_spv,
    vk::{
        self, AttachmentDescription, Buffer, ClearValue, CommandBuffer, DescriptorType, DeviceMemory, Fence, ImageView, PhysicalDevice, PhysicalDeviceMemoryProperties, Pipeline,
        PipelineColorBlendStateCreateInfo, PipelineDepthStencilStateCreateInfo, PipelineLayout, Queue, Rect2D, RenderPass, SamplerCreateInfo, Semaphore, ShaderModule, ShaderStageFlags,
        SubpassDependency, SubpassDescription, SurfaceKHR, SwapchainKHR, VertexInputAttributeDescription,
    },
    Entry,
};
pub use ash::{Device, Instance};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use core::panic;
use std::{
    borrow::Cow,
    default::Default,
    ffi::CStr,
    io::Cursor,
    mem::{self},
    os::raw::c_char,
    todo,
};
use std::{matches, println};

use winit::window::Window;

#[derive(Debug, Clone, Copy)]
pub struct MeshIndex(usize);
#[derive(Debug, Clone, Copy)]
pub struct StageIndex(usize);

impl std::fmt::Display for MeshIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MeshIndex({})", self.0)
    }
}

impl Into<usize> for MeshIndex {
    fn into(self) -> usize {
        self.0
    }
}

pub type Index = u32;

pub struct Mesh<Vertex>
where Vertex: Copy
{
    pub vertices: Vec<Vertex>,
    pub indices: Vec<Index>,
}

struct RenderStage {
    name: String,
    pipeline: RedHotPipeline, // TODO: Support multiple pipelines
    stage_descriptor_set: vk::DescriptorSet,
    object_descriptor_set: vk::DescriptorSet,
    vertex_attribute_descriptions: Box<[VertexInputAttributeDescription]>, // NOTE[perf]: Written VERY rarely (only during setup), read rarely, only during pipeline creation/re-creation
    render_pass: RenderPass,                                               // TODO: Convert to buffer backed index system , to allow reuse
    framebuffers: Vec<RedHotFramebuffer>,
    clear_values: Box<[vk::ClearValue]>,
}

#[derive(Clone, Copy)]
pub struct RedHotFramebuffer {
    framebuffer: Framebuffer,
    width: u32,
    height: u32,
}

pub enum RedHotStageImage {
    Image(RedHotImageInfo),
    SwapchainImage(), // TODO[multi-surface-support]: This should take a surface, for which the associated swapchain should be used
}

pub struct RedHotImageInfo {
    // image: vk::Image,
    // memory_req: MemoryRequirements,
    // memory: DeviceMemory,
    view: vk::ImageView,
    width: u32,
    height: u32,
    format: vk::Format,
}

pub struct RedHotPipelineCreateInfo {
    vertex_shader_module: ShaderModule,
    fragment_shader_module: ShaderModule,
    pipeline_layout: PipelineLayout,
    rasterization_state: vk::PipelineRasterizationStateCreateInfo,
    depth_stencil_state: PipelineDepthStencilStateCreateInfo,
    color_blend_state: PipelineColorBlendStateCreateInfo,
}
struct RedHotPipeline {
    graphics_pipeline: Option<Pipeline>,
    create_info: RedHotPipelineCreateInfo,
}

impl RenderStage {
    // pub fn destroy_graphics_pipeline(&mut self, device: &Device) {
    //     match self.graphics_pipeline {
    //         None => return,
    //         Some(pipeline) => unsafe {
    //             device.destroy_pipeline(pipeline, None);
    //         },
    //     }
    // }

    fn get_pipeline<Vertex>(&mut self, device: &Device, present_index: u32) -> Pipeline {
        match self.pipeline.graphics_pipeline {
            Some(pipeline) => pipeline,
            None => unsafe {
                let fb = self.get_framebuffer(present_index);
                self.pipeline.graphics_pipeline = Some(self.create_graphics_pipeline::<Vertex>(device, fb.width, fb.height));
                self.pipeline.graphics_pipeline.unwrap_unchecked()
            },
        }
    }

    pub unsafe fn create_graphics_pipeline<Vertex>(&mut self, device: &Device, width: u32, height: u32) -> Pipeline {
        // TODO: Recreate graphics pipeline on window resize
        // TODO: Check for (and remove) old graphics pipeline, in case this gets called "badly" (i.e. someone hasn't cleaned up the old first)
        println!("Creating graphics pipeline");
        let scissors = [*Rect2D::builder().extent(*vk::Extent2D::builder().width(width).height(height))];
        let viewports = [vk::Viewport { x: 0.0, y: 0.0, width: width as f32, height: height as f32, min_depth: 0.0, max_depth: 1.0 }];
        *(device)
            .create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[vk::GraphicsPipelineCreateInfo::builder()
                    .stages(&[
                        vk::PipelineShaderStageCreateInfo {
                            s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                            module: self.pipeline.create_info.vertex_shader_module,
                            p_name: CStr::from_bytes_with_nul_unchecked(b"main\0").as_ptr(),
                            stage: vk::ShaderStageFlags::VERTEX,
                            ..Default::default()
                        },
                        vk::PipelineShaderStageCreateInfo {
                            s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                            module: self.pipeline.create_info.fragment_shader_module,
                            p_name: CStr::from_bytes_with_nul_unchecked(b"main\0").as_ptr(),
                            stage: vk::ShaderStageFlags::FRAGMENT,
                            ..Default::default()
                        },
                    ])
                    .vertex_input_state(
                        &vk::PipelineVertexInputStateCreateInfo::builder()
                            .vertex_attribute_descriptions(&self.vertex_attribute_descriptions)
                            .vertex_binding_descriptions(&[vk::VertexInputBindingDescription { binding: 0, stride: mem::size_of::<Vertex>() as u32, input_rate: vk::VertexInputRate::VERTEX }]), //? Shouldn't this use the padded size? or am I misunderstanding that? If I am, fix the functions that make the vertex/index buffer, right now it doesn't matter though, as they seem to always be the same
                    )
                    .input_assembly_state(&vk::PipelineInputAssemblyStateCreateInfo { topology: vk::PrimitiveTopology::TRIANGLE_LIST, ..Default::default() })
                    .viewport_state(&vk::PipelineViewportStateCreateInfo::builder().scissors(&scissors).viewports(&viewports))
                    .rasterization_state(&self.pipeline.create_info.rasterization_state)
                    .multisample_state(&vk::PipelineMultisampleStateCreateInfo { rasterization_samples: vk::SampleCountFlags::TYPE_1, ..Default::default() })
                    .depth_stencil_state(&self.pipeline.create_info.depth_stencil_state)
                    .color_blend_state(&self.pipeline.create_info.color_blend_state)
                    .dynamic_state(&vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]))
                    .layout(self.pipeline.create_info.pipeline_layout)
                    .render_pass(self.render_pass)
                    .build()],
                None,
            )
            .expect("Unable to create graphics pipeline")
            .first()
            .unwrap()
    }

    fn get_framebuffer(&self, present_index: u32) -> &RedHotFramebuffer {
        if self.framebuffers.len() > 1 {
            // NOTE: Currently the only way yo thave more than one framebuffer, is if you are a "presentable stage", i.e. target the swapchain images
            &self.framebuffers[present_index as usize]
        } else {
            self.framebuffers.first().expect("No framebuffers???")
        }
    }
}

struct CurrentRenderInfo {
    // render_pass_begin_info: vk::RenderPassBeginInfo,
    present_index: u32,
    draw_uniform_buffer: Buffer,
    draw_uniform_buffer_memory: DeviceMemory,
    object_uniform_memory: Vec<(Buffer, DeviceMemory)>,
    stage_uniform_memory: Vec<(Buffer, DeviceMemory)>,
}

pub struct Renderer<Vertex>
where Vertex: Copy
{
    _phantom: Option<Vertex>,
    entry: Entry,
    window_width: u32,
    window_height: u32,
    pub clear_color: [f32; 4],
    instance: Instance,
    surface: SurfaceKHR,
    pdevice: PhysicalDevice,
    device: Device,

    // TODO[multi-surface-support]: Combine these, then do a Surface -> Swapchain map
    swapchain: SwapchainKHR,
    swapchain_loader: Swapchain,
    swapchain_image_views: Vec<ImageView>,
    pub swapchain_image_format: vk::Format,
    swapchain_images: Vec<vk::Image>,

    device_memory_properties: PhysicalDeviceMemoryProperties,
    present_complete_semaphore: Semaphore,
    rendering_complete_semaphores: [Semaphore; 3], // NOTE: One per swapchain image, see: https://docs.vulkan.org/guide/latest/swapchain_semaphore_reuse.html
    present_queue: Queue,

    command_buffer: CommandBuffer,
    command_buffer_reuse_fence: Fence,

    //. Things we need to clean?
    min_uniform_buffer_offset_alignment: usize,

    stages: Vec<RenderStage>,

    draw_descriptor_set: vk::DescriptorSet,
    draw_descriptor_set_layout: vk::DescriptorSetLayout,

    index_buffer_memory: Option<DeviceMemory>,
    index_buffer: Option<Buffer>,
    should_regenerate_index_buffer: bool,

    vertex_buffer_memory: Option<DeviceMemory>,
    vertex_buffer: Option<Buffer>,
    should_regenerate_vertex_buffer: bool,

    registered_meshes: Vec<(Mesh<Vertex>, u32, i32)>,
    current_render: Option<CurrentRenderInfo>,
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number: i32 = callback_data.message_id_number as i32;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    println!("{:?}: {:?} [{} ({})] : {}", message_severity, message_type, message_id_name, &message_id_number.to_string(), message,);

    vk::FALSE
}

unsafe fn swapchain_stuff(
    device: &Device,
    instance: &Instance,
    entry: &Entry,
    pdevice: PhysicalDevice,
    surface: SurfaceKHR,
    window_width: u32,
    window_height: u32,
) -> (Swapchain, SwapchainKHR, vk::Format, Vec<vk::ImageView>, Vec<vk::Image>) {
    let surface_loader = Surface::new(&entry, &instance); //? Why is the surface loader a surface, and what is "surface loader"??? and why is the surface a KHR_surface?? What are these types??

    let surface_format = surface_loader.get_physical_device_surface_formats(pdevice, surface).unwrap()[0];

    let surface_capabilities = surface_loader.get_physical_device_surface_capabilities(pdevice, surface).unwrap();
    let mut desired_image_count = surface_capabilities.min_image_count + 1;
    if surface_capabilities.max_image_count > 0 && desired_image_count > surface_capabilities.max_image_count {
        desired_image_count = surface_capabilities.max_image_count;
    }
    let swapchain_loader = Swapchain::new(&instance, &device);

    let swapchain = swapchain_loader
        .create_swapchain(
            &vk::SwapchainCreateInfoKHR::builder()
                .surface(surface)
                .min_image_count(desired_image_count)
                .image_color_space(surface_format.color_space)
                .image_format(surface_format.format)
                .image_extent(match surface_capabilities.current_extent.width {
                    std::u32::MAX => vk::Extent2D { width: window_width, height: window_height },
                    _ => surface_capabilities.current_extent,
                })
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(if surface_capabilities.supported_transforms.contains(vk::SurfaceTransformFlagsKHR::IDENTITY) {
                    vk::SurfaceTransformFlagsKHR::IDENTITY
                } else {
                    surface_capabilities.current_transform
                })
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(
                    surface_loader
                        .get_physical_device_surface_present_modes(pdevice, surface)
                        .unwrap()
                        .iter()
                        .cloned()
                        .find(|&mode| mode == vk::PresentModeKHR::IMMEDIATE)
                        // .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                        .unwrap_or(vk::PresentModeKHR::FIFO),
                )
                .clipped(true)
                .image_array_layers(1),
            None,
        )
        .unwrap();

    let images = swapchain_loader.get_swapchain_images(swapchain).unwrap();

    let image_views: Vec<vk::ImageView> = images
        .iter()
        .map(|&image| {
            let create_view_info = vk::ImageViewCreateInfo::builder()
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(surface_format.format)
                .components(vk::ComponentMapping { r: vk::ComponentSwizzle::R, g: vk::ComponentSwizzle::G, b: vk::ComponentSwizzle::B, a: vk::ComponentSwizzle::A }) // NOTE: I'm pretty sure omitting this would set it to default(), which I think is zero (derived from i32), which correseponds to VK_COMPONENT_SWIZZLE_IDENTITY, which is just saying that each component maps to themselves
                .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 })
                .image(image);
            device.create_image_view(&create_view_info, None).unwrap()
        })
        .collect();

    (swapchain_loader, swapchain, surface_format.format, image_views, images)
}

impl<Vertex> Renderer<Vertex>
where Vertex: Copy
{
    pub fn new(window: &Window, window_width: u32, window_height: u32) -> Renderer<Vertex> {
        unsafe {
            let entry = Entry::load().unwrap(); //. Loads the Vulkan library

            let app_name = CStr::from_bytes_with_nul_unchecked(b"VulkanTriangle\0"); //. Appname, and egine name, not sure what's used for, hardcoded for naw

            let layer_names = [CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0")]; //. Enables validation
            let layers_names_raw: Vec<*const c_char> = layer_names.iter().map(|raw_name| raw_name.as_ptr()).collect(); //. Just get the afformentioned layernames as pointers, since that's what the api wants

            let mut extension_names = ash_window::enumerate_required_extensions(window.raw_display_handle()).unwrap().to_vec(); //. Get the list of Vulkan extensions required to use the window we have gotten, not sure what those are tho...
            extension_names.push(DebugUtils::name().as_ptr());

            //# Creating instance
            let instance: Instance = entry
                .create_instance(
                    &vk::InstanceCreateInfo::builder()
                        .application_info(
                            &vk::ApplicationInfo::builder()
                                .application_name(app_name)
                                .application_version(0)
                                .engine_name(app_name)
                                .engine_version(0)
                                .api_version(vk::make_api_version(0, 1, 0, 0)),
                        )
                        .enabled_layer_names(&layers_names_raw)
                        .enabled_extension_names(&extension_names)
                        .flags(vk::InstanceCreateFlags::default()),
                    None,
                )
                .expect("Instance creation error");

            //# Debug messenger callback
            let _debug_call_back = DebugUtils::new(&entry, &instance)
                .create_debug_utils_messenger(
                    &vk::DebugUtilsMessengerCreateInfoEXT::builder()
                        .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING | vk::DebugUtilsMessageSeverityFlagsEXT::INFO)
                        .message_type(vk::DebugUtilsMessageTypeFlagsEXT::GENERAL | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE)
                        .pfn_user_callback(Some(vulkan_debug_callback)),
                    None,
                )
                .unwrap();

            //# Creating surface
            let surface = ash_window::create_surface(&entry, &instance, window.raw_display_handle(), window.raw_window_handle(), None).unwrap(); //. Creates the surface, i.e. the Vulkan Window, from the actual window

            let (pdevice, queue_family_index) = instance
                .enumerate_physical_devices()
                .expect("Physical device error")
                .iter()
                .find_map(|pdevice| {
                    instance.get_physical_device_queue_family_properties(*pdevice).iter().enumerate().find_map(|(index, info)| {
                        if info.queue_flags.contains(vk::QueueFlags::GRAPHICS) && Surface::new(&entry, &instance).get_physical_device_surface_support(*pdevice, index as u32, surface).unwrap() {
                            Some((*pdevice, index as u32))
                        } else {
                            None
                        }
                    })
                })
                .expect("Couldn't find suitable device.");

            let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);

            let min_uniform_buffer_offset_alignment = instance.get_physical_device_properties(pdevice).limits.min_uniform_buffer_offset_alignment;

            //# Creating device
            let device: Device = instance
                .create_device(
                    pdevice,
                    &vk::DeviceCreateInfo::builder()
                        .queue_create_infos(std::slice::from_ref(
                            &vk::DeviceQueueCreateInfo::builder().queue_family_index(queue_family_index).queue_priorities(&[1.0]),
                        ))
                        .enabled_extension_names(&[Swapchain::name().as_ptr()])
                        .enabled_features(&(vk::PhysicalDeviceFeatures { shader_clip_distance: 1, fill_mode_non_solid: 1, ..Default::default() })),
                    None,
                )
                .unwrap();

            //# Command Buffer with Sync
            let command_buffer = device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::builder()
                        .command_buffer_count(1) //. We want 1 command buffer
                        .command_pool(
                            device
                                .create_command_pool(
                                    &vk::CommandPoolCreateInfo::builder().flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER).queue_family_index(queue_family_index),
                                    None,
                                )
                                .unwrap(),
                        )
                        .level(vk::CommandBufferLevel::PRIMARY),
                )
                .unwrap()[0];

            //# Semaphores and Fences
            let present_complete_semaphore = device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None).unwrap();
            let rendering_complete_semaphores = [
                device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None).unwrap(),
                device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None).unwrap(),
                device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None).unwrap(),
            ];

            let command_buffer_reuse_fence = device.create_fence(&vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED), None).expect("Create fence failed.");
            device.reset_fences(&[command_buffer_reuse_fence]).expect("Reset fences failed.");

            device.reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::RELEASE_RESOURCES).expect("Reset command buffer failed.");
            device
                .begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))
                .expect("Begin commandbuffer");

            // device.cmd_pipeline_barrier( // TODO: we should have a barrier that transitions the attachments to the correct states, specified by the initial_state of the renderpass attachment descriptions, for all that are not UNDEFINED
            //     command_buffer,
            //     vk::PipelineStageFlags::TOP_OF_PIPE,
            //     vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            //     vk::DependencyFlags::empty(),
            //     &[],
            //     &[],
            //     &[
            //         vk::ImageMemoryBarrier::builder()
            //             .image(depth_image)
            //             .dst_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            //             .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            //             .old_layout(vk::ImageLayout::UNDEFINED)
            //             .subresource_range(vk::ImageSubresourceRange::builder().aspect_mask(vk::ImageAspectFlags::DEPTH).layer_count(1).level_count(1).build())
            //             .build(),
            //         vk::ImageMemoryBarrier::builder()
            //             .image(shadow_map_image)
            //             .dst_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            //             .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            //             .old_layout(vk::ImageLayout::UNDEFINED)
            //             .subresource_range(vk::ImageSubresourceRange::builder().aspect_mask(vk::ImageAspectFlags::DEPTH).layer_count(1).level_count(1).build())
            //             .build(),
            //     ],
            // );

            device.end_command_buffer(command_buffer).expect("End commandbuffer");

            let present_queue = device.get_device_queue(queue_family_index, 0);
            device
                .queue_submit(
                    present_queue,
                    &[vk::SubmitInfo::builder().wait_semaphores(&[]).wait_dst_stage_mask(&[]).command_buffers(&vec![command_buffer]).signal_semaphores(&[]).build()],
                    command_buffer_reuse_fence,
                )
                .expect("queue submit failed.");
            device.wait_for_fences(&[command_buffer_reuse_fence], true, std::u64::MAX).expect("Wait for fence failed."); //. Wait for the setup commands to finish

            //# Swapchian images
            let (swapchain_loader, swapchain, swapchain_image_format, swapchain_image_views, swapchain_images) =
                swapchain_stuff(&device, &instance, &entry, pdevice, surface, window_width, window_height);

            //# Descriptors set for per frame DrawUniform
            let draw_descriptor_set_layout = device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::builder().bindings(&[*vk::DescriptorSetLayoutBinding::builder()
                        .binding(0)
                        .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                        .descriptor_count(1)
                        .stage_flags(ShaderStageFlags::VERTEX)]),
                    None,
                )
                .unwrap();

            let draw_descriptor_set = device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::builder()
                        .descriptor_pool(
                            device
                                .create_descriptor_pool(
                                    &vk::DescriptorPoolCreateInfo::builder()
                                        .pool_sizes(&[*vk::DescriptorPoolSize::builder().ty(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(1 as u32)])
                                        .max_sets(1 as u32),
                                    None,
                                )
                                .unwrap(),
                        )
                        .set_layouts(&[draw_descriptor_set_layout]),
                )
                .unwrap()[0];

            Renderer::<Vertex> {
                _phantom: None,
                entry,
                window_width,
                window_height,
                clear_color: [1.0, 1.0, 1.0, 1.0],
                instance,
                surface,
                pdevice,
                device,
                swapchain,
                swapchain_image_format,
                swapchain_loader,
                swapchain_image_views,
                swapchain_images,

                device_memory_properties,
                present_complete_semaphore,
                rendering_complete_semaphores,
                present_queue,

                min_uniform_buffer_offset_alignment: min_uniform_buffer_offset_alignment as usize,

                command_buffer,
                command_buffer_reuse_fence,

                registered_meshes: vec![],

                draw_descriptor_set_layout,
                draw_descriptor_set,

                stages: vec![],

                index_buffer_memory: None,
                index_buffer: None,
                should_regenerate_index_buffer: true,

                vertex_buffer_memory: None,
                vertex_buffer: None,
                should_regenerate_vertex_buffer: true,

                current_render: None,
            }
        }
    }

    // TODO[multi-surface-support]: Use a Surface -> Swapchain map
    // pub fn get_swapchain_images(&self) -> (RedHotStageImage, impl Fn() -> usize) {
    //     (
    //         RedHotStageImage::Images([
    //             RedHotImageInfo { view: self.swapchain_image_views[0], width: self.window_width, height: self.window_height, format: self.swapchain_image_format },
    //             RedHotImageInfo { view: self.swapchain_image_views[1], width: self.window_width, height: self.window_height, format: self.swapchain_image_format },
    //             RedHotImageInfo { view: self.swapchain_image_views[2], width: self.window_width, height: self.window_height, format: self.swapchain_image_format },
    //         ]),
    //         unsafe {
    //             // TODO: Safety
    //             // NOTE: The swapchain stuff is probably instantly invalided on swapchain recreation lmao, it should probably not be a fn() -> usize, but a fn(&Self) -> usize. Or we just hardcode this for the two cases, swapchain images, and frames_in_flight
    //             {
    //                 let swapchain_loader = self.swapchain_loader.clone();
    //                 let swapchain = self.swapchain;
    //                 let semaphore = self.present_complete_semaphore;
    //                 move || swapchain_loader.acquire_next_image(swapchain, u64::MAX, semaphore, vk::Fence::null()).unwrap().0 as usize
    //                 // TODO: Right now we're kinda hard-coding acquire_next_image in begin/commit render, which makes us acquire twice per frame, this is very probably wrong
    //             }
    //         },
    //     )
    // }

    pub unsafe fn create_image(&self, width: u32, height: u32, format: vk::Format, usage: vk::ImageUsageFlags, memory_flags: vk::MemoryPropertyFlags) -> RedHotStageImage {
        let image = self
            .device
            .create_image(
                &vk::ImageCreateInfo::builder()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(format)
                    .extent(*vk::Extent3D::builder().width(width).height(height).depth(1))
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(usage)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
            .unwrap();

        let image_memory_req = self.device.get_image_memory_requirements(image);

        let image_memory = self
            .device
            .allocate_memory(
                &vk::MemoryAllocateInfo::builder().allocation_size(image_memory_req.size).memory_type_index(
                    self.device_memory_properties.memory_types[..self.device_memory_properties.memory_type_count as _]
                        .iter()
                        .enumerate()
                        .find(|(index, memory_type)| (1 << index) & image_memory_req.memory_type_bits != 0 && memory_type.property_flags & memory_flags == memory_flags)
                        .map(|(index, _memory_type)| index as _)
                        .expect("Unable to find suitable memory index for image."),
                ),
                None,
            )
            .unwrap();

        self.device.bind_image_memory(image, image_memory, 0).expect("Unable to bind image memory for image");

        let image_view = self
            .device
            .create_image_view(
                &vk::ImageViewCreateInfo::builder()
                    .subresource_range(vk::ImageSubresourceRange::builder().aspect_mask(vk::ImageAspectFlags::DEPTH).level_count(1).layer_count(1).build())
                    .image(image)
                    .format(format)
                    .view_type(vk::ImageViewType::TYPE_2D),
                None,
            )
            .unwrap();

        RedHotStageImage::Image(RedHotImageInfo { view: image_view, width, height, format })
    }

    pub unsafe fn register_stage<const N_VERTEX_ATTRIBUTE_DESCRIPTIONS: usize, const N_CLEAR_VALUES: usize>(
        &mut self,
        name: String,
        vertex_shader_bytes: &[u8],
        fragment_shader_bytes: &[u8],
        rasterization_state: vk::PipelineRasterizationStateCreateInfo,
        depth_stencil_state: PipelineDepthStencilStateCreateInfo,
        color_blend_state: PipelineColorBlendStateCreateInfo,
        vertex_attribute_descriptions: [VertexInputAttributeDescription; N_VERTEX_ATTRIBUTE_DESCRIPTIONS],
        images: &[&RedHotStageImage],
        attachments: &[AttachmentDescription],
        clear_values: [ClearValue; N_CLEAR_VALUES],
        subpass_description: &[SubpassDescription],
        depedencies: &[SubpassDependency],
        img_textures: &[(&RedHotStageImage, SamplerCreateInfo)],
    ) -> StageIndex {
        let render_pass = self
            .device
            .create_render_pass(
                &vk::RenderPassCreateInfo::builder().attachments(attachments).subpasses(subpass_description).dependencies(depedencies),
                None,
            )
            .unwrap();

        println!("Registering stage: {name}");

        let is_presentable_stage = images.iter().any(|x| matches!(x, RedHotStageImage::SwapchainImage()));
        let n_framebuffers = if is_presentable_stage { 3 } else { 1 };

        let framebuffers: Vec<_> = (0..n_framebuffers)
            .map(|i| {
                let mut width = u32::MAX;
                let mut height = u32::MAX;
                let mut views = vec![];

                for img in images.iter() {
                    let img_info = match img {
                        RedHotStageImage::Image(img_info) => img_info,
                        RedHotStageImage::SwapchainImage() => &RedHotImageInfo { view: self.swapchain_image_views[i], width, height, format: self.swapchain_image_format }, // TODO[multi-surface-support]: This should depend on surface
                    };
                    width = u32::min(width, img_info.width);
                    height = u32::min(width, img_info.height);
                    views.push(img_info.view);
                }

                RedHotFramebuffer {
                    framebuffer: self
                        .device
                        .create_framebuffer(
                            &vk::FramebufferCreateInfo::builder().render_pass(render_pass).attachments(&views).width(width).height(height).layers(1),
                            None,
                        )
                        .unwrap(),
                    width,
                    height,
                }
            })
            .collect();

        let object_descriptor_set_layout = self
            .device
            .create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::builder().bindings(&[*vk::DescriptorSetLayoutBinding::builder()
                    .binding(0)
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                    .descriptor_count(1)
                    .stage_flags(ShaderStageFlags::VERTEX)]),
                None,
            )
            .unwrap();
        let object_descriptor_set = self
            .device
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(
                        self.device
                            .create_descriptor_pool(
                                &vk::DescriptorPoolCreateInfo::builder()
                                    .pool_sizes(&[*vk::DescriptorPoolSize::builder().ty(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC).descriptor_count(1 as u32)])
                                    .max_sets(1 as u32),
                                None,
                            )
                            .unwrap(),
                    )
                    .set_layouts(&[object_descriptor_set_layout]),
            )
            .unwrap()[0];
        let stage_descriptor_set_layout = self
            .device
            .create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::builder().bindings(
                    &[*vk::DescriptorSetLayoutBinding::builder()
                        .binding(0)
                        .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                        .descriptor_count(1)
                        .stage_flags(ShaderStageFlags::VERTEX)]
                    .into_iter()
                    .chain((0..img_textures.len()).map(|i| {
                        *vk::DescriptorSetLayoutBinding::builder()
                            .binding(1 + i as u32)
                            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                            .descriptor_count(1) // TODO: maybe drop the loop in favor of just having a higher descriptor count? Would maybe make shaders less nice?
                            .stage_flags(vk::ShaderStageFlags::FRAGMENT) // TODO: For now, texture uniforms are hard-coded to the fragment shader
                    }))
                    .collect::<Vec<_>>(),
                ),
                None,
            )
            .unwrap();

        let mut pool_sizes = vec![*vk::DescriptorPoolSize::builder().ty(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(1)];
        if img_textures.len() > 0 {
            pool_sizes.push(*vk::DescriptorPoolSize::builder().ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(img_textures.len() as u32));
        }
        let stage_descriptor_set = self
            .device
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(self.device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::builder().pool_sizes(&pool_sizes).max_sets(1 as u32), None).unwrap())
                    .set_layouts(&[stage_descriptor_set_layout]),
            )
            .unwrap()[0];
        let pipeline_layout = self
            .device
            .create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::builder().set_layouts(&[self.draw_descriptor_set_layout, stage_descriptor_set_layout, object_descriptor_set_layout]),
                None,
            )
            .unwrap();
        let vertex_shader_module = self
            .device
            .create_shader_module(
                &vk::ShaderModuleCreateInfo::builder().code(&read_spv(&mut Cursor::new(vertex_shader_bytes)).expect("Failed to read vertex shader spv file")), //TODO: fix
                None,
            )
            .expect("Vertex shader module error");
        let fragment_shader_module = self
            .device
            .create_shader_module(
                &vk::ShaderModuleCreateInfo::builder().code(&read_spv(&mut Cursor::new(fragment_shader_bytes)).expect("Failed to read fragment shader spv file")), //TODO: fix
                None,
            )
            .expect("Fragment shader module error");

        let _: Vec<_> = img_textures
            .iter()
            .enumerate()
            .map(|(i, (img, sampler_info))| {
                // TODO: Store the sampler for cleanup later
                // TODO: Store the sampler for reuse on OTHER textures
                let sampler = self.device.create_sampler(sampler_info, None).unwrap();
                self.device.update_descriptor_sets(
                    &[*vk::WriteDescriptorSet::builder()
                        .dst_set(stage_descriptor_set)
                        .dst_binding(1 + i as u32)
                        .dst_array_element(0)
                        .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&[*vk::DescriptorImageInfo::builder()
                            .sampler(sampler)
                            .image_view(match img {
                                RedHotStageImage::Image(img_info) => img_info.view,
                                RedHotStageImage::SwapchainImage() => todo!(), // TODO[presentable-images]: This needs to know about the associated rederers swapchain image views, if such exist?
                            })
                            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)])],
                    &[],
                );
            })
            .collect();

        self.stages.push({
            RenderStage {
                name: name,
                vertex_attribute_descriptions: Box::new(vertex_attribute_descriptions),
                pipeline: RedHotPipeline {
                    graphics_pipeline: None,
                    create_info: RedHotPipelineCreateInfo {
                        vertex_shader_module,
                        fragment_shader_module,
                        pipeline_layout,
                        rasterization_state: rasterization_state,
                        depth_stencil_state: depth_stencil_state,
                        color_blend_state: color_blend_state,
                    },
                },
                stage_descriptor_set,
                object_descriptor_set,
                render_pass: render_pass,
                framebuffers: framebuffers,
                clear_values: Box::new(clear_values),
            }
        });
        StageIndex(self.stages.len() - 1)
    }

    pub fn resize_window(&mut self, window_width: u32, window_height: u32) {
        // self.window_width = window_width;
        // self.window_height = window_height;

        // // TODO: Probably need to also re-create other stuff, like swapchain
        // //# Re-create graphics pipeline
        // for stage in &mut self.stages {
        //     stage.destroy_graphics_pipeline(&self.device);
        //     unsafe { stage.create_graphics_pipeline::<Vertex>(&self.device, self.window_width, self.window_height) };
        // }
    }

    pub fn register_mesh(&mut self, mesh: Mesh<Vertex>) -> MeshIndex {
        println!("Registering mesh");
        match self.registered_meshes.last() {
            Some((last_mesh, last_index_offset, last_vertex_offset)) => {
                self.registered_meshes.push((mesh, last_index_offset + last_mesh.indices.len() as u32, last_vertex_offset + last_mesh.vertices.len() as i32))
            },
            None => self.registered_meshes.push((mesh, 0, 0)),
        }
        self.should_regenerate_vertex_buffer = true;
        self.should_regenerate_index_buffer = true;
        MeshIndex(self.registered_meshes.len() - 1)
    }

    pub fn bind_index_buffer(&mut self) -> (vk::DeviceMemory, vk::Buffer) {
        println!("Binding index buffer");
        unsafe {
            let index_buffer = self
                .device
                .create_buffer(
                    &vk::BufferCreateInfo::builder()
                        .size((std::mem::size_of::<Index>() * self.registered_meshes.iter().fold(0, |sum, (mesh, _, _)| sum + mesh.indices.len())) as u64)
                        .usage(vk::BufferUsageFlags::INDEX_BUFFER)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                    None,
                )
                .unwrap();

            let index_buffer_memory_req = self.device.get_buffer_memory_requirements(index_buffer);
            let index_buffer_memory = self
                .device
                .allocate_memory(
                    &vk::MemoryAllocateInfo {
                        allocation_size: index_buffer_memory_req.size,
                        memory_type_index: self.device_memory_properties.memory_types[..self.device_memory_properties.memory_type_count as _]
                            .iter()
                            .enumerate()
                            .find(|(index, memory_type)| {
                                (1 << index) & index_buffer_memory_req.memory_type_bits != 0
                                    && memory_type.property_flags & (vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                                        == (vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                            })
                            .map(|(index, _memory_type)| index as _)
                            .expect("Unable to find suitable memorytype for the index buffer."),
                        ..Default::default()
                    },
                    None,
                )
                .unwrap();

            let mut index_ptr = self.device.map_memory(index_buffer_memory, 0, index_buffer_memory_req.size, vk::MemoryMapFlags::empty()).unwrap() as *mut Index;

            for (mesh, _, _) in self.registered_meshes.iter() {
                std::ptr::copy_nonoverlapping::<Index>(mesh.indices.as_ptr(), index_ptr, mesh.indices.len());
                index_ptr = index_ptr.add(mesh.indices.len());
            }

            self.device.unmap_memory(index_buffer_memory);
            self.device.bind_buffer_memory(index_buffer, index_buffer_memory, 0).unwrap();
            (index_buffer_memory, index_buffer)
        }
    }

    pub fn bind_vertex_buffer(&mut self) -> (vk::DeviceMemory, vk::Buffer) {
        println!("Binding vertex buffer");
        unsafe {
            let vertex_buffer = self
                .device
                .create_buffer(
                    &vk::BufferCreateInfo {
                        size: (mem::size_of::<Vertex>() * self.registered_meshes.iter().fold(0, |sum, (mesh, _, _)| sum + mesh.vertices.len())) as u64,
                        usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                        sharing_mode: vk::SharingMode::EXCLUSIVE,
                        ..Default::default()
                    },
                    None,
                )
                .unwrap();

            let vertex_buffer_memory_req = self.device.get_buffer_memory_requirements(vertex_buffer);
            let v_size = mem::size_of::<Vertex>();
            let v_count = self.registered_meshes.iter().fold(0, |sum, (mesh, _, _)| sum + mesh.vertices.len());
            println!("v_count: {v_count}");
            println!("v_size : {v_size}");
            println!("Actual vertex data size: {}", v_size * v_count);
            println!("Vertex Buffer mem req: {vertex_buffer_memory_req:?}");
            let vertex_buffer_memory = self
                .device
                .allocate_memory(
                    &vk::MemoryAllocateInfo {
                        allocation_size: vertex_buffer_memory_req.size,
                        memory_type_index: self.device_memory_properties.memory_types[..self.device_memory_properties.memory_type_count as _]
                            .iter()
                            .enumerate()
                            .find(|(index, memory_type)| {
                                (1 << index) & vertex_buffer_memory_req.memory_type_bits != 0
                                    && memory_type.property_flags & (vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                                        == (vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                            })
                            .map(|(index, _memory_type)| index as _)
                            .expect("Unable to find suitable memorytype for the vertex buffer."),
                        ..Default::default()
                    },
                    None,
                )
                .unwrap();

            let mut vert_ptr = self.device.map_memory(vertex_buffer_memory, 0, vertex_buffer_memory_req.size, vk::MemoryMapFlags::empty()).unwrap() as *mut Vertex;

            for (mesh, _, _) in self.registered_meshes.iter() {
                std::ptr::copy_nonoverlapping::<Vertex>(mesh.vertices.as_ptr(), vert_ptr, mesh.vertices.len());
                vert_ptr = vert_ptr.add(mesh.vertices.len());
            }

            self.device.unmap_memory(vertex_buffer_memory); //? ???
            self.device.bind_buffer_memory(vertex_buffer, vertex_buffer_memory, 0).unwrap();
            (vertex_buffer_memory, vertex_buffer)
        }
    }

    pub fn get_mesh(&self, meshi: MeshIndex) -> &Mesh<Vertex> {
        &self.registered_meshes.get(Into::<usize>::into(meshi)).unwrap_or_else(|| panic!("Use of unregistered mesh: {meshi}")).0
    }

    pub fn render_begin<DU>(&mut self, draw_uniform: DU)
    where DU: Copy {
        // println!("Starting frame");

        unsafe {
            assert!(self.current_render.is_none(), "Call render_commit before begin_render");
            let (present_index, swapchain_suboptimal) = self.swapchain_loader.acquire_next_image(self.swapchain, std::u64::MAX, self.present_complete_semaphore, vk::Fence::null()).unwrap();
            if swapchain_suboptimal {
                println!("Swapchain suboptimal");
            }

            self.device.wait_for_fences(&[self.command_buffer_reuse_fence], true, std::u64::MAX).expect("Wait for fence failed.");
            self.device.reset_fences(&[self.command_buffer_reuse_fence]).expect("Reset fences failed.");
            self.device.reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::RELEASE_RESOURCES).expect("Reset command buffer failed.");

            let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            self.device.begin_command_buffer(self.command_buffer, &command_buffer_begin_info).expect("Begin commandbuffer");

            if self.should_regenerate_vertex_buffer {
                match self.vertex_buffer_memory {
                    Some(buffer_memory) => self.device.free_memory(buffer_memory, None),
                    None => (),
                }
                match self.vertex_buffer {
                    Some(buffer) => self.device.destroy_buffer(buffer, None),
                    None => (),
                }
                (self.vertex_buffer_memory, self.vertex_buffer) = Some(self.bind_vertex_buffer()).unzip();
                self.should_regenerate_vertex_buffer = false;
            }

            if self.should_regenerate_index_buffer {
                match self.index_buffer_memory {
                    Some(buffer_memory) => self.device.free_memory(buffer_memory, None),
                    None => (),
                }
                match self.index_buffer {
                    Some(buffer) => self.device.destroy_buffer(buffer, None),
                    None => (),
                }
                (self.index_buffer_memory, self.index_buffer) = Some(self.bind_index_buffer()).unzip();
                self.should_regenerate_index_buffer = false;
            }

            let draw_uniform_buffer = self
                .device
                .create_buffer(
                    &vk::BufferCreateInfo { size: mem::size_of::<DU>() as u64, usage: vk::BufferUsageFlags::UNIFORM_BUFFER, sharing_mode: vk::SharingMode::EXCLUSIVE, ..Default::default() },
                    None,
                )
                .unwrap();
            let draw_uniform_buffer_memory_req = self.device.get_buffer_memory_requirements(draw_uniform_buffer);
            let draw_uniform_buffer_memory = self
                .device
                .allocate_memory(
                    &vk::MemoryAllocateInfo {
                        allocation_size: draw_uniform_buffer_memory_req.size,
                        memory_type_index: self.device_memory_properties.memory_types[..self.device_memory_properties.memory_type_count as _]
                            .iter()
                            .enumerate()
                            .find(|(index, memory_type)| {
                                (1 << index) & draw_uniform_buffer_memory_req.memory_type_bits != 0
                                    && memory_type.property_flags & (vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                                        == (vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                            })
                            .map(|(index, _memory_type)| index as _)
                            .expect("Unable to find suitable memorytype for the draw uniform buffer."),
                        ..Default::default()
                    },
                    None,
                )
                .unwrap();

            let draw_uniform_ptr = self.device.map_memory(draw_uniform_buffer_memory, 0, draw_uniform_buffer_memory_req.size, vk::MemoryMapFlags::empty()).unwrap();

            ash::util::Align::new(draw_uniform_ptr, mem::align_of::<DU>() as u64, draw_uniform_buffer_memory_req.size).copy_from_slice(&[draw_uniform]);
            self.device.unmap_memory(draw_uniform_buffer_memory);
            self.device.bind_buffer_memory(draw_uniform_buffer, draw_uniform_buffer_memory, 0).unwrap();

            self.device.update_descriptor_sets(
                &[*vk::WriteDescriptorSet::builder()
                    .dst_set(self.draw_descriptor_set)
                    .dst_binding(0) //. We placed the draw uniform at layout binding = 0
                    .dst_array_element(0) //. Our uniform is just a single element, not an array, so it's at index 0
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(&[*vk::DescriptorBufferInfo::builder().buffer(draw_uniform_buffer).offset(0).range(mem::size_of::<DU>() as u64)])],
                &[],
            );

            self.current_render = Some(CurrentRenderInfo { present_index, draw_uniform_buffer, draw_uniform_buffer_memory, object_uniform_memory: vec![], stage_uniform_memory: vec![] })
        }
    }

    pub fn render_stage<SU, OU>(&mut self, stagei: StageIndex, stage_uniform: SU, meshis: Vec<MeshIndex>, object_uniforms: Vec<OU>)
    where
        SU: Copy,
        OU: Copy,
    {
        let stage = &mut self.stages[stagei.0];
        // println!("Render stage: {}", stage.name);
        let current_render = self.current_render.as_mut().expect("Call render_begin before calling render_stage");
        let stage_framebuffer = stage.get_framebuffer(current_render.present_index);
        let scissors = [*Rect2D::builder().extent(*vk::Extent2D::builder().width(stage_framebuffer.width).height(stage_framebuffer.height))];
        let viewports = [vk::Viewport { x: 0.0, y: 0.0, width: stage_framebuffer.width as f32, height: stage_framebuffer.height as f32, min_depth: 0.0, max_depth: 1.0 }];

        let clear_values = stage.clear_values.clone(); // TODO: figure out how to avoid this

        let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
            .render_pass(stage.render_pass)
            .framebuffer(stage_framebuffer.framebuffer)
            .render_area(*Rect2D::builder().extent(*vk::Extent2D::builder().width(stage_framebuffer.width).height(stage_framebuffer.height)))
            .clear_values(&clear_values);

        unsafe {
            let pipeline = stage.get_pipeline::<Vertex>(&self.device, current_render.present_index);

            let stage_uniform_buffer = self
                .device
                .create_buffer(
                    &vk::BufferCreateInfo { size: mem::size_of::<SU>() as u64, usage: vk::BufferUsageFlags::UNIFORM_BUFFER, sharing_mode: vk::SharingMode::EXCLUSIVE, ..Default::default() },
                    None,
                )
                .unwrap();
            let stage_uniform_buffer_memory_req = self.device.get_buffer_memory_requirements(stage_uniform_buffer);
            let stage_uniform_buffer_memory = self
                .device
                .allocate_memory(
                    &vk::MemoryAllocateInfo {
                        allocation_size: stage_uniform_buffer_memory_req.size,
                        memory_type_index: self.device_memory_properties.memory_types[..self.device_memory_properties.memory_type_count as _]
                            .iter()
                            .enumerate()
                            .find(|(index, memory_type)| {
                                (1 << index) & stage_uniform_buffer_memory_req.memory_type_bits != 0
                                    && memory_type.property_flags & (vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                                        == (vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                            })
                            .map(|(index, _memory_type)| index as _)
                            .expect("Unable to find suitable memorytype for the stage uniform buffer."),
                        ..Default::default()
                    },
                    None,
                )
                .unwrap();

            let stage_uniform_ptr = self.device.map_memory(stage_uniform_buffer_memory, 0, stage_uniform_buffer_memory_req.size, vk::MemoryMapFlags::empty()).unwrap();
            ash::util::Align::new(stage_uniform_ptr, mem::align_of::<SU>() as u64, stage_uniform_buffer_memory_req.size).copy_from_slice(&[stage_uniform]);
            self.device.unmap_memory(stage_uniform_buffer_memory);
            self.device.bind_buffer_memory(stage_uniform_buffer, stage_uniform_buffer_memory, 0).unwrap();

            self.device.update_descriptor_sets(
                &[*vk::WriteDescriptorSet::builder()
                    .dst_set(stage.stage_descriptor_set)
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(&[*vk::DescriptorBufferInfo::builder().buffer(stage_uniform_buffer).offset(0).range(mem::size_of::<SU>() as u64)])],
                &[],
            );

            current_render.stage_uniform_memory.push((stage_uniform_buffer, stage_uniform_buffer_memory));

            let object_uniform_stride = (((mem::size_of::<OU>() - 1) / self.min_uniform_buffer_offset_alignment) + 1) * self.min_uniform_buffer_offset_alignment; //. Always at least "align"
            let object_uniform_buffer = self
                .device
                .create_buffer(
                    &vk::BufferCreateInfo {
                        size: (object_uniform_stride * object_uniforms.len()) as u64,
                        usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
                        sharing_mode: vk::SharingMode::EXCLUSIVE,
                        ..Default::default()
                    },
                    None,
                )
                .unwrap();

            let object_uniform_buffer_memory_req = self.device.get_buffer_memory_requirements(object_uniform_buffer);
            let object_uniform_buffer_memory = self
                .device
                .allocate_memory(
                    &vk::MemoryAllocateInfo {
                        allocation_size: object_uniform_buffer_memory_req.size,
                        memory_type_index: self.device_memory_properties.memory_types[..self.device_memory_properties.memory_type_count as _]
                            .iter()
                            .enumerate()
                            .find(|(index, memory_type)| {
                                (1 << index) & object_uniform_buffer_memory_req.memory_type_bits != 0
                                    && memory_type.property_flags & (vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                                        == (vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                            })
                            .map(|(index, _memory_type)| index as _)
                            .expect("Unable to find suitable memorytype for the object uniform buffer."),
                        ..Default::default()
                    },
                    None,
                )
                .unwrap();

            let object_uniform_ptr = self.device.map_memory(object_uniform_buffer_memory, 0, object_uniform_buffer_memory_req.size, vk::MemoryMapFlags::empty()).unwrap();

            for (i, object_uniform) in object_uniforms.iter().enumerate() {
                *(object_uniform_ptr.cast::<u8>().add(i * object_uniform_stride).cast::<OU>()) = *object_uniform;
            }

            self.device.unmap_memory(object_uniform_buffer_memory);
            self.device.bind_buffer_memory(object_uniform_buffer, object_uniform_buffer_memory, 0).unwrap();

            self.device.update_descriptor_sets(
                &[*vk::WriteDescriptorSet::builder()
                    .dst_set(stage.object_descriptor_set)
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                    .buffer_info(&[*vk::DescriptorBufferInfo::builder().buffer(object_uniform_buffer).offset(0).range(mem::size_of::<OU>() as u64)])],
                &[],
            );

            {
                self.device.cmd_begin_render_pass(self.command_buffer, &render_pass_begin_info, vk::SubpassContents::INLINE);
                self.device.cmd_bind_pipeline(self.command_buffer, vk::PipelineBindPoint::GRAPHICS, pipeline);

                self.device.cmd_set_viewport(self.command_buffer, 0, &viewports);
                self.device.cmd_set_scissor(self.command_buffer, 0, &scissors);

                self.device.cmd_bind_vertex_buffers(self.command_buffer, 0, &[self.vertex_buffer.expect("No Vertex buffer")], &[0]);
                self.device.cmd_bind_index_buffer(self.command_buffer, self.index_buffer.expect("No index buffer"), 0, vk::IndexType::UINT32);

                for (i, meshi) in meshis.iter().enumerate() {
                    let (mesh, index_offset, vertex_offset) = self.registered_meshes.get(meshi.0).unwrap();
                    self.device.cmd_bind_descriptor_sets(
                        self.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        stage.pipeline.create_info.pipeline_layout,
                        0,
                        &[self.draw_descriptor_set, stage.stage_descriptor_set, stage.object_descriptor_set],
                        &[(i * object_uniform_stride as usize) as u32],
                    );

                    self.device.cmd_draw_indexed(self.command_buffer, mesh.indices.len() as u32, 1, *index_offset, *vertex_offset, 0);
                }

                self.device.cmd_end_render_pass(self.command_buffer);
            }

            current_render.object_uniform_memory.push((object_uniform_buffer, object_uniform_buffer_memory));
        }
    }

    pub fn render_commit(&mut self) {
        // println!("Committing frame");
        unsafe {
            let current_render = self.current_render.as_ref().expect("Call render_begin before calling render_end");
            self.device.end_command_buffer(self.command_buffer).expect("End commandbuffer");

            let command_buffers = vec![self.command_buffer];

            self.device
                .queue_submit(
                    self.present_queue,
                    &[vk::SubmitInfo::builder()
                        .wait_semaphores(&[self.present_complete_semaphore])
                        .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                        .command_buffers(&command_buffers)
                        .signal_semaphores(&[self.rendering_complete_semaphores[current_render.present_index as usize]])
                        .build()],
                    self.command_buffer_reuse_fence,
                )
                .expect("queue submit failed.");

            self.swapchain_loader
                .queue_present(
                    self.present_queue,
                    &vk::PresentInfoKHR::builder()
                        .wait_semaphores(&[self.rendering_complete_semaphores[current_render.present_index as usize]])
                        .swapchains(&[self.swapchain])
                        .image_indices(&[current_render.present_index]),
                )
                .unwrap();

            self.device.wait_for_fences(&[self.command_buffer_reuse_fence], true, std::u64::MAX).expect("Wait for fence failed.");

            self.device.reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::RELEASE_RESOURCES).expect("Reset command buffer failed.");

            self.device.free_memory(current_render.draw_uniform_buffer_memory, None);
            self.device.destroy_buffer(current_render.draw_uniform_buffer, None);
            for (object_uniform_buffer, object_uniform_buffer_memory) in &current_render.object_uniform_memory {
                self.device.free_memory(*object_uniform_buffer_memory, None);
                self.device.destroy_buffer(*object_uniform_buffer, None);
            }
            for (stage_uniform_buffer, stage_uniform_buffer_memory) in &current_render.stage_uniform_memory {
                self.device.free_memory(*stage_uniform_buffer_memory, None);
                self.device.destroy_buffer(*stage_uniform_buffer, None);
            }
            self.current_render = None;
        }
    }

    #[rustfmt::skip]
    pub fn destroy(&self) {
        todo!("Clean up");
        // unsafe {
        //     self.device.device_wait_idle().unwrap();
        //     for framebuffer in &self.framebuffers {
        //         self.device.destroy_framebuffer(*framebuffer, None);
        //     }
        //     for stage in &self.stages {
        //         self.device.destroy_shader_module(stage.vertex_shader_module, None);
        //         self.device.destroy_shader_module(stage.fragment_shader_module, None);
        //     }
        //     if let Some(buffer_memory) = self.vertex_buffer_memory {self.device.free_memory(buffer_memory, None);}
        //     if let Some(buffer) = self.vertex_buffer {self.device.destroy_buffer(buffer, None);}
        //     if let Some(buffer_memory) = self.index_buffer_memory {self.device.free_memory(buffer_memory, None);}
        //     if let Some(buffer) = self.index_buffer {self.device.destroy_buffer(buffer, None);}
        // }
    }
}
