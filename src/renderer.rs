use ash::{
    extensions::khr::{Surface, Swapchain},
    util::read_spv,
    vk::{
        self, AttachmentReference, Buffer, CommandBuffer, DescriptorType, DeviceMemory, Fence, Framebuffer, PhysicalDevice, PhysicalDeviceMemoryProperties, Queue, Rect2D, Semaphore, ShaderModule,
        ShaderStageFlags, SubpassDependency, SurfaceFormatKHR, SurfaceKHR, SwapchainKHR,
    },
    Entry,
};
pub use ash::{Device, Instance};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use std::{default::Default, ffi::CStr, io::Cursor, mem, os::raw::c_char};

use winit::window::Window;

#[derive(Debug, Clone, Copy)]
pub struct MeshIndex(usize);

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

pub struct Renderer<Vertex>
where Vertex: Copy
{
    _phantom: Option<Vertex>,
    entry: Entry,
    window_width: u32,
    window_height: u32,
    instance: Instance,
    surface: SurfaceKHR,
    pdevice: PhysicalDevice,
    device: Device,
    swapchain: SwapchainKHR,
    swapchain_loader: Swapchain, //? ???
    device_memory_properties: PhysicalDeviceMemoryProperties,
    present_complete_semaphore: Semaphore,
    rendering_complete_semaphore: Semaphore,
    surface_format: SurfaceFormatKHR,
    present_queue: Queue,
    color_attachment_refs: [AttachmentReference; 1],
    depth_attachment_ref: AttachmentReference,
    draw_commands_reuse_fence: Fence,
    draw_command_buffer: CommandBuffer,
    dependencies: [SubpassDependency; 1],

    //? Things we need to clean?
    framebuffers: Vec<Framebuffer>,
    vertex_shader_module: Option<ShaderModule>,
    fragment_shader_module: Option<ShaderModule>,
    index_buffer_memory: Option<DeviceMemory>,
    index_buffer: Option<Buffer>,
    should_regenerate_index_buffer: bool,
    vertex_buffer_memory: Option<DeviceMemory>,
    vertex_buffer: Option<Buffer>,
    should_regenerate_vertex_buffer: bool,
    registered_meshes: Vec<(Mesh<Vertex>, u32, i32)>,
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
                                .api_version(vk::make_api_version(0, 1, 2, 0)),
                        )
                        .enabled_layer_names(&layers_names_raw)
                        .enabled_extension_names(&extension_names)
                        .flags(vk::InstanceCreateFlags::default()),
                    None,
                )
                .expect("Instance creation error");

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

            //# Creating device
            let device: Device = instance
                .create_device(
                    pdevice,
                    &vk::DeviceCreateInfo::builder()
                        .queue_create_infos(std::slice::from_ref(
                            &vk::DeviceQueueCreateInfo::builder().queue_family_index(queue_family_index).queue_priorities(&[1.0]),
                        ))
                        .enabled_extension_names(&[Swapchain::name().as_ptr()])
                        .enabled_features(&(vk::PhysicalDeviceFeatures { shader_clip_distance: 1, ..Default::default() })),
                    None,
                )
                .unwrap();

            //# Creating swapchain
            let present_queue = device.get_device_queue(queue_family_index, 0);

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
                                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                                .unwrap_or(vk::PresentModeKHR::FIFO),
                        )
                        .clipped(true)
                        .image_array_layers(1),
                    None,
                )
                .unwrap();

            //# Command Buffers with Sync
            let command_buffers = device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::builder()
                        .command_buffer_count(2) //. We want 2 command buffers
                        .command_pool(
                            device
                                .create_command_pool(
                                    &vk::CommandPoolCreateInfo::builder().flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER).queue_family_index(queue_family_index),
                                    None,
                                )
                                .unwrap(),
                        )
                        .level(vk::CommandBufferLevel::PRIMARY), //. Both our command buffers are PRIMARY command buffers
                )
                .unwrap();
            let setup_command_buffer = command_buffers[0];
            let draw_command_buffer = command_buffers[1];

            //# Framebuffers
            let images = swapchain_loader.get_swapchain_images(swapchain).unwrap();

            let image_views: Vec<vk::ImageView> = images
                .iter()
                .map(|&image| {
                    let create_view_info = vk::ImageViewCreateInfo::builder()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
                        .components(vk::ComponentMapping { r: vk::ComponentSwizzle::R, g: vk::ComponentSwizzle::G, b: vk::ComponentSwizzle::B, a: vk::ComponentSwizzle::A })
                        .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 })
                        .image(image);
                    device.create_image_view(&create_view_info, None).unwrap()
                })
                .collect();

            let dependencies = [vk::SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                ..Default::default()
            }];

            let color_attachment_refs = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
            let depth_attachment_ref = vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL };

            let depth_image_format = vk::Format::D16_UNORM;

            let depth_image = device
                .create_image(
                    &vk::ImageCreateInfo::builder()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(depth_image_format)
                        .extent(*vk::Extent3D::builder().width(window_width).height(window_height).depth(1))
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                    None,
                )
                .unwrap();

            let depth_image_memory_req = device.get_image_memory_requirements(depth_image);
            let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);

            device
                .bind_image_memory(
                    depth_image,
                    device
                        .allocate_memory(
                            &vk::MemoryAllocateInfo::builder().allocation_size(depth_image_memory_req.size).memory_type_index(
                                device_memory_properties.memory_types[..device_memory_properties.memory_type_count as _]
                                    .iter()
                                    .enumerate()
                                    .find(|(index, memory_type)| {
                                        (1 << index) & depth_image_memory_req.memory_type_bits != 0
                                            && memory_type.property_flags & vk::MemoryPropertyFlags::DEVICE_LOCAL == vk::MemoryPropertyFlags::DEVICE_LOCAL
                                    })
                                    .map(|(index, _memory_type)| index as _)
                                    .expect("Unable to find suitable memory index for depth image."),
                            ),
                            None,
                        )
                        .unwrap(),
                    0,
                )
                .expect("Unable to bind depth image memory");

            let framebuffers: Vec<vk::Framebuffer> = image_views
                .iter()
                .map(|&image_view| {
                    let framebuffer_attachments = [
                        image_view,
                        device
                            .create_image_view(
                                &vk::ImageViewCreateInfo::builder()
                                    .subresource_range(vk::ImageSubresourceRange::builder().aspect_mask(vk::ImageAspectFlags::DEPTH).level_count(1).layer_count(1).build())
                                    .image(depth_image)
                                    .format(depth_image_format)
                                    .view_type(vk::ImageViewType::TYPE_2D),
                                None,
                            )
                            .unwrap(),
                    ];
                    let frame_buffer_create_info = vk::FramebufferCreateInfo::builder()
                        .render_pass(
                            device
                                .create_render_pass(
                                    &vk::RenderPassCreateInfo::builder()
                                        .attachments(&[
                                            vk::AttachmentDescription {
                                                format: surface_format.format,
                                                samples: vk::SampleCountFlags::TYPE_1,
                                                load_op: vk::AttachmentLoadOp::CLEAR,
                                                store_op: vk::AttachmentStoreOp::STORE,
                                                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                                                ..Default::default()
                                            },
                                            vk::AttachmentDescription {
                                                format: vk::Format::D16_UNORM,
                                                samples: vk::SampleCountFlags::TYPE_1,
                                                load_op: vk::AttachmentLoadOp::CLEAR,
                                                initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                                                final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                                                ..Default::default()
                                            },
                                        ])
                                        .subpasses(std::slice::from_ref(
                                            &vk::SubpassDescription::builder()
                                                .color_attachments(&color_attachment_refs)
                                                .depth_stencil_attachment(&depth_attachment_ref)
                                                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS),
                                        ))
                                        .dependencies(&dependencies),
                                    None,
                                )
                                .unwrap(),
                        )
                        .attachments(&framebuffer_attachments)
                        .width(window_width)
                        .height(window_height)
                        .layers(1);

                    device.create_framebuffer(&frame_buffer_create_info, None).unwrap()
                })
                .collect();

            //# Semaphores and Fences
            let present_complete_semaphore = device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None).unwrap();
            let rendering_complete_semaphore = device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None).unwrap();

            let draw_commands_reuse_fence = device.create_fence(&vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED), None).expect("Create fence failed.");
            let setup_commands_reuse_fence = device.create_fence(&vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED), None).expect("Create fence failed.");

            device.wait_for_fences(&[setup_commands_reuse_fence], true, std::u64::MAX).expect("Wait for fence failed.");

            device.reset_fences(&[setup_commands_reuse_fence]).expect("Reset fences failed.");

            device.reset_command_buffer(setup_command_buffer, vk::CommandBufferResetFlags::RELEASE_RESOURCES).expect("Reset command buffer failed.");

            device
                .begin_command_buffer(setup_command_buffer, &vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))
                .expect("Begin commandbuffer");

            device.cmd_pipeline_barrier(
                setup_command_buffer,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier::builder()
                    .image(depth_image)
                    .dst_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
                    .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .subresource_range(vk::ImageSubresourceRange::builder().aspect_mask(vk::ImageAspectFlags::DEPTH).layer_count(1).level_count(1).build())
                    .build()],
            );

            device.end_command_buffer(setup_command_buffer).expect("End commandbuffer");

            let command_buffers = vec![setup_command_buffer];

            let submit_info = vk::SubmitInfo::builder().wait_semaphores(&[]).wait_dst_stage_mask(&[]).command_buffers(&command_buffers).signal_semaphores(&[]);

            device.queue_submit(present_queue, &[submit_info.build()], setup_commands_reuse_fence).expect("queue submit failed.");

            Renderer::<Vertex> {
                _phantom: None,
                entry,
                window_width,
                window_height,
                instance,
                surface,
                pdevice,
                device,
                swapchain,
                swapchain_loader,

                framebuffers,
                device_memory_properties,
                present_complete_semaphore,
                rendering_complete_semaphore,
                surface_format,
                present_queue,
                color_attachment_refs,
                depth_attachment_ref,
                draw_commands_reuse_fence,
                draw_command_buffer,
                dependencies,
                vertex_shader_module: None,   //. No shaders by default, use other function to add these for now
                fragment_shader_module: None, //. No shaders by default, use other function to add these for now
                index_buffer_memory: None,
                index_buffer: None,
                should_regenerate_index_buffer: true,
                vertex_buffer_memory: None,
                vertex_buffer: None,
                should_regenerate_vertex_buffer: true,
                registered_meshes: vec![],
            }
        }
    }

    pub fn set_vertex_shader(&mut self, shader_bytes: &[u8]) {
        // TODO: Clean up old shaders if not None
        unsafe {
            self.vertex_shader_module = Some(
                self.device
                    .create_shader_module(
                        &vk::ShaderModuleCreateInfo::builder().code(&read_spv(&mut Cursor::new(shader_bytes)).expect("Failed to read vertex shader spv file")), // TODO: fix
                        None,
                    )
                    .expect("Vertex shader module error"),
            );
        }
    }

    pub fn set_fragment_shader(&mut self, shader_bytes: &[u8]) {
        // TODO: Clean up old shaders if not None
        unsafe {
            self.fragment_shader_module = Some(
                self.device
                    .create_shader_module(
                        &vk::ShaderModuleCreateInfo::builder().code(&read_spv(&mut Cursor::new(shader_bytes)).expect("Failed to read fragment shader spv file")), // TODO: fix
                        None,
                    )
                    .expect("Fragment shader module error"),
            );
        }
    }

    pub fn register_mesh(&mut self, mesh: Mesh<Vertex>) -> MeshIndex {
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

    pub fn render_once<U>(&mut self, meshi: MeshIndex, uniform: U)
    where U: Copy {
        unsafe {
            let (present_index, _) = self.swapchain_loader.acquire_next_image(self.swapchain, std::u64::MAX, self.present_complete_semaphore, vk::Fence::null()).unwrap();
            let clear_values = [
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 0.0] } },
                vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } },
            ];

            let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                .render_pass({
                    self.device
                        .create_render_pass(
                            &vk::RenderPassCreateInfo::builder()
                                .attachments(&[
                                    vk::AttachmentDescription {
                                        format: self.surface_format.format,
                                        samples: vk::SampleCountFlags::TYPE_1,
                                        load_op: vk::AttachmentLoadOp::CLEAR,
                                        store_op: vk::AttachmentStoreOp::STORE,
                                        final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                                        ..Default::default()
                                    },
                                    vk::AttachmentDescription {
                                        format: vk::Format::D16_UNORM,
                                        samples: vk::SampleCountFlags::TYPE_1,
                                        load_op: vk::AttachmentLoadOp::CLEAR,
                                        initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                                        final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                                        ..Default::default()
                                    },
                                ])
                                .subpasses(std::slice::from_ref(
                                    &vk::SubpassDescription::builder()
                                        .color_attachments(&self.color_attachment_refs)
                                        .depth_stencil_attachment(&self.depth_attachment_ref)
                                        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS),
                                ))
                                .dependencies(&self.dependencies),
                            None,
                        )
                        .unwrap()
                })
                .framebuffer(self.framebuffers[present_index as usize])
                .render_area(*Rect2D::builder().extent(*vk::Extent2D::builder().width(self.window_width).height(self.window_height)))
                .clear_values(&clear_values);

            self.device.wait_for_fences(&[self.draw_commands_reuse_fence], true, std::u64::MAX).expect("Wait for fence failed.");

            self.device.reset_fences(&[self.draw_commands_reuse_fence]).expect("Reset fences failed.");

            self.device.reset_command_buffer(self.draw_command_buffer, vk::CommandBufferResetFlags::RELEASE_RESOURCES).expect("Reset command buffer failed.");

            let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

            self.device.begin_command_buffer(self.draw_command_buffer, &command_buffer_begin_info).expect("Begin commandbuffer");

            //# Pipeline Layout
            let scissors = [*Rect2D::builder().extent(*vk::Extent2D::builder().width(self.window_width).height(self.window_height))];
            let viewports = [vk::Viewport { x: 0.0, y: 0.0, width: self.window_width as f32, height: self.window_height as f32, min_depth: 0.0, max_depth: 1.0 }];

            if self.should_regenerate_vertex_buffer {
                //# Clean up old buffer, if any
                match self.vertex_buffer_memory {
                    Some(buffer_memory) => self.device.free_memory(buffer_memory, None),
                    None => (),
                }
                match self.vertex_buffer {
                    Some(buffer) => self.device.destroy_buffer(buffer, None),
                    None => (),
                }
                //# Create new buffer
                (self.vertex_buffer_memory, self.vertex_buffer) = Some(self.bind_vertex_buffer()).unzip();
                self.should_regenerate_vertex_buffer = false;
            }

            if self.should_regenerate_index_buffer {
                //# Clean up old buffer, if any
                match self.index_buffer_memory {
                    Some(buffer_memory) => self.device.free_memory(buffer_memory, None),
                    None => (),
                }
                match self.index_buffer {
                    Some(buffer) => self.device.destroy_buffer(buffer, None),
                    None => (),
                }
                //# Create new buffer
                (self.index_buffer_memory, self.index_buffer) = Some(self.bind_index_buffer()).unzip();
                self.should_regenerate_index_buffer = false;
            }

            //# UNIFORM BUFFER EXPERIMENTATION
            let uniform_buffer = self
                .device
                .create_buffer(
                    &vk::BufferCreateInfo { size: mem::size_of::<U>() as u64, usage: vk::BufferUsageFlags::UNIFORM_BUFFER, sharing_mode: vk::SharingMode::EXCLUSIVE, ..Default::default() },
                    None,
                )
                .unwrap();
            let uniform_buffer_memory_req = self.device.get_buffer_memory_requirements(uniform_buffer);
            let uniform_buffer_memory = self
                .device
                .allocate_memory(
                    &vk::MemoryAllocateInfo {
                        allocation_size: uniform_buffer_memory_req.size,
                        memory_type_index: self.device_memory_properties.memory_types[..self.device_memory_properties.memory_type_count as _]
                            .iter()
                            .enumerate()
                            .find(|(index, memory_type)| {
                                (1 << index) & uniform_buffer_memory_req.memory_type_bits != 0
                                    && memory_type.property_flags & (vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                                        == (vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                            })
                            .map(|(index, _memory_type)| index as _)
                            .expect("Unable to find suitable memorytype for the uniform buffer."),
                        ..Default::default()
                    },
                    None,
                )
                .unwrap();

            let uniform_ptr = self //. Maps the memory on the cpu to the memory of the gpu
                .device
                .map_memory(uniform_buffer_memory, 0, uniform_buffer_memory_req.size, vk::MemoryMapFlags::empty())
                .unwrap();

            ash::util::Align::new(uniform_ptr, mem::align_of::<U>() as u64, uniform_buffer_memory_req.size).copy_from_slice(&[uniform]); //. This copy pastas the data from the stack/heap/whatever to the previously mapped memory, effectively moving it to the gpu
            self.device.unmap_memory(uniform_buffer_memory); //. Unmaps the memory, as the data is now on the gpu, can leave it mapped if we want to update this in real time to save the remapping every frame/object
            self.device.bind_buffer_memory(uniform_buffer, uniform_buffer_memory, 0).unwrap();

            let descriptor_set_layouts = [self
                .device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::builder().bindings(&[*vk::DescriptorSetLayoutBinding::builder()
                        .binding(0)
                        .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                        .descriptor_count(1)
                        .stage_flags(ShaderStageFlags::VERTEX)]),
                    None,
                )
                .unwrap()];

            let descriptor_set = self
                .device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::builder()
                        .descriptor_pool(
                            self.device
                                .create_descriptor_pool(
                                    &vk::DescriptorPoolCreateInfo::builder()
                                        .pool_sizes(&[*vk::DescriptorPoolSize::builder().ty(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(1 as u32)])
                                        .max_sets(1 as u32),
                                    None,
                                )
                                .unwrap(),
                        )
                        .set_layouts(&descriptor_set_layouts),
                )
                .unwrap()[0];

            self.device.update_descriptor_sets(
                &[*vk::WriteDescriptorSet::builder()
                    .dst_set(descriptor_set)
                    .dst_binding(0) //. We placed the uniform at layout binding = 0
                    .dst_array_element(0) //. Our uniform is just a single element, not an array, so it's at index 0
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(&[*vk::DescriptorBufferInfo::builder().buffer(uniform_buffer).offset(0).range(mem::size_of::<U>() as u64)])],
                &[],
            );

            let pipeline_layout = self.device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::builder().set_layouts(&descriptor_set_layouts), None).unwrap();

            self.device.cmd_bind_descriptor_sets(self.draw_command_buffer, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[descriptor_set], &[]);
            //# END OF UNIFORM BUFFER EXPERIMENTATION

            self.device.cmd_begin_render_pass(self.draw_command_buffer, &render_pass_begin_info, vk::SubpassContents::INLINE);
            self.device.cmd_bind_pipeline(
                self.draw_command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                *(self.device)
                    .create_graphics_pipelines(
                        vk::PipelineCache::null(),
                        &[vk::GraphicsPipelineCreateInfo::builder()
                            .stages(&[
                                vk::PipelineShaderStageCreateInfo {
                                    module: self.vertex_shader_module.expect("Vertex shader should be set, before rendering begins"),
                                    p_name: CStr::from_bytes_with_nul_unchecked(b"main\0").as_ptr(),
                                    stage: vk::ShaderStageFlags::VERTEX,
                                    ..Default::default()
                                },
                                vk::PipelineShaderStageCreateInfo {
                                    s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                                    module: self.fragment_shader_module.expect("Fragment shader should be set, before rendering begins"),
                                    p_name: CStr::from_bytes_with_nul_unchecked(b"main\0").as_ptr(),
                                    stage: vk::ShaderStageFlags::FRAGMENT,
                                    ..Default::default()
                                },
                            ])
                            .vertex_input_state(
                                &vk::PipelineVertexInputStateCreateInfo::builder()
                                    .vertex_attribute_descriptions(&[
                                        vk::VertexInputAttributeDescription { location: 0, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 0 as u32 },
                                        vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 4 * 32 / 8 as u32 },
                                    ])
                                    .vertex_binding_descriptions(&[vk::VertexInputBindingDescription { binding: 0, stride: mem::size_of::<Vertex>() as u32, input_rate: vk::VertexInputRate::VERTEX }]), //? Shouldn't this use the padded size? or am I misunderstanding that? If I am, fix the functions that make the vertex/index buffer, right now it doesn't matter though, as they seem to always be the same
                            )
                            .input_assembly_state(&vk::PipelineInputAssemblyStateCreateInfo { topology: vk::PrimitiveTopology::TRIANGLE_LIST, ..Default::default() })
                            .viewport_state(&vk::PipelineViewportStateCreateInfo::builder().scissors(&scissors).viewports(&viewports))
                            .rasterization_state(&vk::PipelineRasterizationStateCreateInfo {
                                front_face: vk::FrontFace::COUNTER_CLOCKWISE,
                                line_width: 1.0,
                                polygon_mode: vk::PolygonMode::FILL,
                                ..Default::default()
                            })
                            .multisample_state(&vk::PipelineMultisampleStateCreateInfo { rasterization_samples: vk::SampleCountFlags::TYPE_1, ..Default::default() })
                            .depth_stencil_state(&vk::PipelineDepthStencilStateCreateInfo {
                                depth_test_enable: 1,
                                depth_write_enable: 1,
                                depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
                                front: vk::StencilOpState {
                                    fail_op: vk::StencilOp::KEEP,
                                    pass_op: vk::StencilOp::KEEP,
                                    depth_fail_op: vk::StencilOp::KEEP,
                                    compare_op: vk::CompareOp::ALWAYS,
                                    ..Default::default()
                                },
                                back: vk::StencilOpState {
                                    fail_op: vk::StencilOp::KEEP,
                                    pass_op: vk::StencilOp::KEEP,
                                    depth_fail_op: vk::StencilOp::KEEP,
                                    compare_op: vk::CompareOp::ALWAYS,
                                    ..Default::default()
                                },
                                max_depth_bounds: 1.0,
                                ..Default::default()
                            })
                            .color_blend_state(
                                &vk::PipelineColorBlendStateCreateInfo::builder().logic_op(vk::LogicOp::CLEAR).attachments(&[vk::PipelineColorBlendAttachmentState {
                                    blend_enable: 0,
                                    src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
                                    dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,
                                    color_blend_op: vk::BlendOp::ADD,
                                    src_alpha_blend_factor: vk::BlendFactor::ZERO,
                                    dst_alpha_blend_factor: vk::BlendFactor::ZERO,
                                    alpha_blend_op: vk::BlendOp::ADD,
                                    color_write_mask: vk::ColorComponentFlags::RGBA,
                                }]),
                            )
                            .dynamic_state(&vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]))
                            .layout(pipeline_layout)
                            .render_pass(
                                self.device
                                    .create_render_pass(
                                        &vk::RenderPassCreateInfo::builder()
                                            .attachments(&[
                                                vk::AttachmentDescription {
                                                    format: self.surface_format.format,
                                                    samples: vk::SampleCountFlags::TYPE_1,
                                                    load_op: vk::AttachmentLoadOp::CLEAR,
                                                    store_op: vk::AttachmentStoreOp::STORE,
                                                    final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                                                    ..Default::default()
                                                },
                                                vk::AttachmentDescription {
                                                    format: vk::Format::D16_UNORM,
                                                    samples: vk::SampleCountFlags::TYPE_1,
                                                    load_op: vk::AttachmentLoadOp::CLEAR,
                                                    initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                                                    final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                                                    ..Default::default()
                                                },
                                            ])
                                            .subpasses(std::slice::from_ref(
                                                &vk::SubpassDescription::builder()
                                                    .color_attachments(&self.color_attachment_refs)
                                                    .depth_stencil_attachment(&self.depth_attachment_ref)
                                                    .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS),
                                            ))
                                            .dependencies(&self.dependencies),
                                        None,
                                    )
                                    .unwrap(),
                            )
                            .build()],
                        None,
                    )
                    .expect("Unable to create graphics pipeline")
                    .first()
                    .unwrap(),
            );

            self.device.cmd_set_viewport(self.draw_command_buffer, 0, &viewports);
            self.device.cmd_set_scissor(self.draw_command_buffer, 0, &scissors); //. This is downright silly
            self.device.cmd_bind_vertex_buffers(self.draw_command_buffer, 0, &[self.vertex_buffer.expect("No Vertex buffer")], &[0]);
            self.device.cmd_bind_index_buffer(self.draw_command_buffer, self.index_buffer.expect("No index buffer"), 0, vk::IndexType::UINT32);
            let (mesh, index_offset, vertex_offset) = self.registered_meshes.get(meshi.0).unwrap();
            self.device.cmd_draw_indexed(self.draw_command_buffer, mesh.indices.len() as u32, 1, *index_offset, *vertex_offset, 0);
            //. Or draw without the index buffer
            // self.device.cmd_draw(self.draw_command_buffer, 3, 1, 0, 0);
            self.device.cmd_end_render_pass(self.draw_command_buffer);

            self.device.end_command_buffer(self.draw_command_buffer).expect("End commandbuffer");

            let command_buffers = vec![self.draw_command_buffer];

            let wait_semaphores = [self.present_complete_semaphore];
            let signal_semaphores = [self.rendering_complete_semaphore];
            let submit_info = vk::SubmitInfo::builder()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                .command_buffers(&command_buffers)
                .signal_semaphores(&signal_semaphores);

            self.device.queue_submit(self.present_queue, &[submit_info.build()], self.draw_commands_reuse_fence).expect("queue submit failed.");

            let wait_semaphors = [self.rendering_complete_semaphore];
            let swapchains = [self.swapchain];
            let image_indices = [present_index];
            self.swapchain_loader
                .queue_present(
                    self.present_queue,
                    &vk::PresentInfoKHR::builder()
                        .wait_semaphores(&wait_semaphors) // &base.rendering_complete_semaphore)
                        .swapchains(&swapchains)
                        .image_indices(&image_indices),
                )
                .unwrap();

            //# Clean 'per render' items
            self.device.wait_for_fences(&[self.draw_commands_reuse_fence], true, std::u64::MAX).expect("Wait for fence failed.");

            self.device //. Vulkan complained that I could not free memory currently in use by a command buffer, so I added this reset_command_buffer call, pretty sure it also gets reset right before use, so this might lead to a double reset meme
                .reset_command_buffer(self.draw_command_buffer, vk::CommandBufferResetFlags::RELEASE_RESOURCES)
                .expect("Reset command buffer failed.");

            self.device.free_memory(uniform_buffer_memory, None);
            self.device.destroy_buffer(uniform_buffer, None);
        }
    }

    pub fn destroy(&self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            for framebuffer in &self.framebuffers {
                self.device.destroy_framebuffer(*framebuffer, None);
            }
            match self.vertex_shader_module {
                Some(vertex_shader_module) => self.device.destroy_shader_module(vertex_shader_module, None),
                None => {},
            }
            match self.fragment_shader_module {
                Some(fragment_shader_module) => self.device.destroy_shader_module(fragment_shader_module, None),
                None => {},
            }
        }
    }
}
