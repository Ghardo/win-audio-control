#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::{engine::general_purpose, Engine as _};
use image::{ImageOutputFormat, RgbaImage};
use serde::Serialize;
use std::mem::size_of;
use std::ptr; 
use tauri::Manager;
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::Media::Audio::*,
    Win32::System::Com::*,
    Win32::System::Threading::*,
    Win32::UI::Shell::*,
    Win32::UI::WindowsAndMessaging::*,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            if cfg!(debug_assertions) {
                 let _ = app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                );
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_audio_sessions,
            set_app_volume,
            set_app_mute
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Serialize)]
struct AudioSession {
    pid: u32,
    name: String,
    volume: f32,
    muted: bool,
    icon_base64: Option<String>,
}

#[tauri::command]
fn get_audio_sessions() -> Vec<AudioSession> {
    let mut sessions = Vec::new();

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let device_enumerator: IMMDeviceEnumerator = match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
            Ok(de) => de,
            Err(_) => return sessions,
        };

        if let Ok(device) = device_enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) {
            if let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) {
                if let Ok(enumerator) = manager.GetSessionEnumerator() {
                    let count = enumerator.GetCount().unwrap_or(0);

                    for i in 0..count {
                        if let Ok(control) = enumerator.GetSession(i) {
                            let control2: IAudioSessionControl2 = match control.cast() {
                                Ok(c) => c,
                                Err(_) => continue,
                            };
                            
                            let simple_volume: ISimpleAudioVolume = match control.cast() {
                                Ok(v) => v,
                                Err(_) => continue,
                            };

                            let pid = control2.GetProcessId().unwrap_or(0);
                            let vol = simple_volume.GetMasterVolume().unwrap_or(0.0);
                            let muted = simple_volume.GetMute().map(|b| b.as_bool()).unwrap_or(false);

                            let mut name = "System".to_string();
                            let mut icon_base64 = None;

                            if pid > 0 {
                                if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                                    let mut buffer = [0u16; 1024];
                                    let mut size = buffer.len() as u32;

                                    if QueryFullProcessImageNameW(
                                        handle,
                                        PROCESS_NAME_FORMAT(0),
                                        PWSTR(buffer.as_mut_ptr()), 
                                        &mut size
                                    ).is_ok() {
                                        let full_path = String::from_utf16_lossy(&buffer[..size as usize]);
                                        name = full_path.split('\\').last().unwrap_or("Unknown").to_string();

                                        let mut shfi = SHFILEINFOW::default();
                                        
                                        let result = SHGetFileInfoW(
                                            PCWSTR(buffer.as_ptr()),
                                            windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0), 
                                            Some(&mut shfi),
                                            size_of::<SHFILEINFOW>() as u32,
                                            SHGFI_ICON | SHGFI_LARGEICON
                                        );

                                        if result != 0 {
                                            icon_base64 = icon_to_base64(shfi.hIcon);
                                            let _ = DestroyIcon(shfi.hIcon);
                                        }
                                    }
                                    let _ = CloseHandle(handle);
                                }
                            }

                            sessions.push(AudioSession {
                                pid,
                                name,
                                volume: vol,
                                muted,
                                icon_base64,
                            });
                        }
                    }
                }
            }
        }
    }
    sessions
}

#[tauri::command]
fn set_app_volume(pid: u32, volume: f32) -> std::result::Result<(), String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let device_enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let device = device_enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia).map_err(|e| e.to_string())?;
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None).map_err(|e| e.to_string())?;
        let enumerator = manager.GetSessionEnumerator().map_err(|e| e.to_string())?;
        let count = enumerator.GetCount().unwrap_or(0);

        for i in 0..count {
            if let Ok(control) = enumerator.GetSession(i) {
                let control2: IAudioSessionControl2 = control.cast().map_err(|e| e.to_string())?;
                if control2.GetProcessId().unwrap_or(0) == pid {
                    let simple_volume: ISimpleAudioVolume = control.cast().map_err(|e| e.to_string())?;
                    let safe_vol = volume.clamp(0.0, 1.0);
                    simple_volume.SetMasterVolume(safe_vol, ptr::null()).map_err(|e| e.to_string())?;
                    return Ok(());
                }
            }
        }
    }
    Err("Session nicht gefunden".to_string())
}

#[tauri::command]
fn set_app_mute(pid: u32, muted: bool) -> std::result::Result<(), String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let device_enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let device = device_enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia).map_err(|e| e.to_string())?;
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None).map_err(|e| e.to_string())?;
        let enumerator = manager.GetSessionEnumerator().map_err(|e| e.to_string())?;
        let count = enumerator.GetCount().unwrap_or(0);

        for i in 0..count {
            if let Ok(control) = enumerator.GetSession(i) {
                let control2: IAudioSessionControl2 = control.cast().map_err(|e| e.to_string())?;
                if control2.GetProcessId().unwrap_or(0) == pid {
                    let simple_volume: ISimpleAudioVolume = control.cast().map_err(|e| e.to_string())?;
                    
                    simple_volume.SetMute(muted, ptr::null()).map_err(|e| e.to_string())?;
                    return Ok(());
                }
            }
        }
    }
    Err("Session nicht gefunden".to_string())
}

unsafe fn icon_to_base64(hicon: HICON) -> Option<String> {
    if hicon.is_invalid() { return None; }

    let mut icon_info = ICONINFO::default();
    if GetIconInfo(hicon, &mut icon_info).is_err() { return None; };

    let _cleanup = defer(move || {
        if !icon_info.hbmColor.is_invalid() { let _ = DeleteObject(icon_info.hbmColor); }
        if !icon_info.hbmMask.is_invalid() { let _ = DeleteObject(icon_info.hbmMask); }
    });

    let mut bitmap_info = BITMAP::default();
    GetObjectW(
        icon_info.hbmColor,
        size_of::<BITMAP>() as i32,
        Some(&mut bitmap_info as *mut _ as *mut _),
    );
    
    let width = bitmap_info.bmWidth;
    let height = bitmap_info.bmHeight;

    if width == 0 || height == 0 { return None; }

    let dc = GetDC(None);
    let mem_dc = CreateCompatibleDC(dc);
    let _dc_cleanup = defer(move || {
        let _ = DeleteDC(mem_dc);
        ReleaseDC(None, dc);
    });
    
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0, 
            ..Default::default()
        },
        ..Default::default()
    };

    let mut pixels: Vec<u8> = vec![0; (width * height * 4) as usize];

    let res = GetDIBits(
        mem_dc,
        icon_info.hbmColor,
        0,
        height as u32,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );

    if res == 0 { return None; }

    for chunk in pixels.chunks_mut(4) {
        let b = chunk[0];
        let r = chunk[2];
        chunk[0] = r;
        chunk[2] = b;
    }

    let img_buffer = RgbaImage::from_raw(width as u32, height as u32, pixels)?;
    let mut cursor = std::io::Cursor::new(Vec::new());
    img_buffer.write_to(&mut cursor, ImageOutputFormat::Png).ok()?;
    Some(general_purpose::STANDARD.encode(cursor.into_inner()))
}

struct Defer<F: FnOnce()>(Option<F>);
impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) { if let Some(f) = self.0.take() { f(); } }
}
fn defer<F: FnOnce()>(f: F) -> Defer<F> { Defer(Some(f)) }