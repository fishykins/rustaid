use base64::{alphabet, engine, Engine};
use image::{DynamicImage, ImageFormat, RgbImage};
use scap::{
    capturer::{Capturer, Options},
    frame::Frame,
    get_all_targets, Target,
};
use std::io::Cursor;
use std::{
    sync::{Arc, Mutex},
    thread::sleep,
};
use tauri::State;
use tokio::{task, time::Duration};

#[derive(Default)]
struct AppState {
    pub window: Mutex<String>,
    pub last_frame: Mutex<Option<String>>, // Store Base64 frame
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() {
    let state = Arc::new(AppState::default());
    let state_clone = state.clone();

    task::spawn(async move {
        startup(state_clone).await;
    });

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_window, get_frame])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn startup(state: Arc<AppState>) {
    if !scap::is_supported() {
        println!("❌ Platform not supported");
        return;
    }

    if !scap::has_permission() {
        println!("❌ Permission not granted. Requesting permission...");
        if !scap::request_permission() {
            println!("❌ Permission denied");
            return;
        }
    }

    loop {
        if let Some(rust) = get_rust_target() {
            rust_capture(rust, state.clone());
        }
        sleep(Duration::from_secs(1));
    }
}

fn rust_capture(rust: Target, state: Arc<AppState>) {
    // Used to encode immages to strings.
    let encoder = engine::GeneralPurpose::new(&alphabet::STANDARD, engine::general_purpose::NO_PAD);

    let mut capturer = Capturer::build(Options {
        fps: 1,
        target: Some(rust),
        show_cursor: true,
        show_highlight: true,
        excluded_targets: None,
        output_type: scap::frame::FrameType::RGB,
        output_resolution: scap::capturer::Resolution::_1080p,
        crop_area: None,
    })
    .unwrap();

    let mut frame_count: u32 = 0;

    // Reusable buffer to prevent continuous reallocation
    let buffer: Vec<u8> = Vec::with_capacity(1920 * 1080 * 3);

    loop {
        frame_count += 1;

        capturer.start_capture();

        match capturer.get_next_frame() {
            Ok(frame) => {
                println!("{}: ✅ Frame captured successfully!", frame_count);

                if let Frame::BGRA(frame_data) = frame {
                    let encoder = encoder.clone();
                    let mut buffer = buffer.clone();
                    let state = state.clone();

                    task::spawn(async move {
                        let base64_image = frame_to_base64(
                            &encoder,
                            &mut buffer, // Reuse buffer
                            &frame_data.data,
                            frame_data.width as u32,
                            frame_data.height as u32,
                        );

                        // Store frame in state
                        let mut last_frame = state.last_frame.lock().unwrap();
                        *last_frame = Some(base64_image);
                    });
                }
            }
            Err(e) => {
                println!("⚠️ Failed to get frame: {}", e);
                capturer.stop_capture();
                break;
            }
        }

        // We need to replace this because it is silly. Starting and stopping is the ONLY way to prevent it running into a memory leak......
        capturer.stop_capture();
        std::thread::sleep(Duration::from_secs(1)); // Yield to other tasks
    }
}

/// Converts a frame to a Base64 encoded PNG
fn frame_to_base64<T: Engine>(
    encoder: &T,
    buffer: &mut Vec<u8>,
    bgra_data: &[u8],
    width: u32,
    height: u32,
) -> String {
    buffer.clear(); // Reuse memory, don't reallocate

    // Convert BGRA to RGB directly without using `flat_map`
    for chunk in bgra_data.chunks_exact(4) {
        buffer.push(chunk[0]); // R
        buffer.push(chunk[1]); // G
        buffer.push(chunk[2]); // B
    }

    let dynamic_image =
        DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, buffer.clone()).unwrap());

    // Convert the image to a byte vector in PNG format using ImageFormat
    let mut img_bytes = Cursor::new(Vec::new());
    dynamic_image
        .write_to(&mut img_bytes, ImageFormat::Png)
        .expect("Failed to write image to bytes");

    // Encode the byte vector (through Cursor) to a base64 string
    encoder.encode(img_bytes.get_ref())
    //"goon".to_string()
}

#[tauri::command]
fn get_window(state: State<Arc<AppState>>) -> Result<String, String> {
    let window = state.window.lock().unwrap();
    Ok(window.clone())
}

#[tauri::command]
fn get_frame(state: State<Arc<AppState>>) -> Result<String, String> {
    let frame = state.last_frame.lock().unwrap();
    match frame.clone() {
        Some(data) => Ok(data),
        None => Err("No frame captured yet".into()),
    }
}

fn get_rust_target() -> Option<Target> {
    get_all_targets()
        .into_iter()
        .find(|target| matches!(target, Target::Window(window) if window.title == "Rust"))
}
