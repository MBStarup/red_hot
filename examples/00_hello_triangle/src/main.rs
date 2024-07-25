use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

use red_hot::renderer::{Mesh, Renderer};

fn main() {
    println!("Example 00: hello triangle");

    let window_width: u32 = 1200;
    let window_height: u32 = 800;

    #[derive(Clone, Copy)]
    #[repr(C)]
    struct Vertex {
        pos: [f32; 4],
        color: [f32; 4],
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    struct Uniform {
        fake: u8,
    }

    let triangle = Mesh::<Vertex> {
        vertices: vec![
            Vertex { pos: [0.0, -0.5, 0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] },
            Vertex { pos: [0.5, 0.5, 0.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] },
            Vertex { pos: [-0.5, 0.5, 0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] },
        ],
        indices: vec![0, 1, 2],
    };

    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Hello Triangle")
        .with_inner_size(winit::dpi::LogicalSize::new(window_width, window_height))
        .build(&event_loop)
        .unwrap();

    let mut renderer = Renderer::<Vertex>::new(&window, window_width, window_height);
    renderer.set_vertex_shader(&include_bytes!("./shader/vert.spv")[..]);
    renderer.set_fragment_shader(&include_bytes!("./shader/frag.spv")[..]);
    let triangle_meshi = renderer.register_mesh(triangle);

    let uniform = Uniform { fake: 0 }; //. Fake uniform not used by the shader, zero sized uniforms trips up the renderer atm

    let mut should_close = false;
    while !should_close {
        event_loop.run_return(|event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested | WindowEvent::KeyboardInput { input: KeyboardInput { state: ElementState::Pressed, virtual_keycode: Some(VirtualKeyCode::Escape), .. }, .. },
                    ..
                } => {
                    should_close = true; //. if we pressed close, close
                    *control_flow = ControlFlow::Exit;
                },
                Event::MainEventsCleared => {
                    renderer.render(uniform, vec![triangle_meshi], vec![uniform]);
                },
                Event::RedrawEventsCleared => *control_flow = ControlFlow::Exit, //. "return"
                _ => (),                                                         //. ignore other events
            }
        });
    }

    renderer.destroy();
    println!("Goodbye");
}
