//! Windows-only controls for Instant Play: hide/show a game's windows, run it
//! at minimum cost while it waits (idle priority, efficiency mode, memory
//! trimmed), and mute its audio. All operate on another process by PID, so
//! they work for any Minecraft version or loader without a mod.
//!
//! On other platforms these are no-ops that report "unsupported".

pub fn supported() -> bool {
    cfg!(windows)
}

#[cfg(windows)]
mod imp {
    use windows::core::{Interface, BOOL};
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::Media::Audio::{
        eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
    use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
    use windows::Win32::System::Threading::{
        OpenProcess, ProcessPowerThrottling, SetPriorityClass, SetProcessInformation, TerminateProcess, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, PROCESS_TERMINATE,
        PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
        PROCESS_QUERY_INFORMATION, PROCESS_SET_INFORMATION, PROCESS_SET_QUOTA,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, ShowWindow, GW_OWNER, SW_HIDE, SW_SHOW,
    };

    struct Find {
        pid: u32,
        out: Vec<isize>,
    }

    unsafe extern "system" fn collect(hwnd: HWND, lp: LPARAM) -> BOOL {
        let f = &mut *(lp.0 as *mut Find);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        // Top-level, unowned, currently visible windows only.
        if pid == f.pid && GetWindow(hwnd, GW_OWNER).is_err() && IsWindowVisible(hwnd).as_bool() {
            f.out.push(hwnd.0 as isize);
        }
        BOOL(1)
    }

    /// Hides every visible top-level window of `pid`; returns the ones hidden.
    pub fn hide_visible(pid: u32) -> Vec<isize> {
        let mut f = Find { pid, out: vec![] };
        unsafe {
            let _ = EnumWindows(Some(collect), LPARAM(&mut f as *mut Find as isize));
            for h in &f.out {
                let _ = ShowWindow(HWND(*h as *mut _), SW_HIDE);
            }
        }
        f.out
    }

    /// Shows exactly the windows we hid earlier and brings the game to the front.
    pub fn show(pid: u32, hidden: &[isize]) {
        unsafe {
            let _ = AllowSetForegroundWindow(pid);
            for h in hidden {
                let hwnd = HWND(*h as *mut _);
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }

    /// Idle priority + Windows efficiency mode + trimmed memory while waiting;
    /// normal priority and full speed when `on` is false.
    pub fn set_background(pid: u32, on: bool) -> anyhow::Result<()> {
        unsafe {
            let h = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, false, pid)?;
            let _ = SetPriorityClass(h, if on { IDLE_PRIORITY_CLASS } else { NORMAL_PRIORITY_CLASS });
            let state = PROCESS_POWER_THROTTLING_STATE {
                Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
                StateMask: if on { PROCESS_POWER_THROTTLING_EXECUTION_SPEED } else { 0 },
            };
            let _ = SetProcessInformation(h, ProcessPowerThrottling, &state as *const _ as *const core::ffi::c_void, std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32);
            if on {
                // Moves the game's memory out of RAM until it's needed again.
                let _ = EmptyWorkingSet(h);
            }
            let _ = CloseHandle(h);
        }
        Ok(())
    }

    /// Ends a process immediately (used for hidden pre-loads when Halo quits).
    pub fn terminate(pid: u32) {
        unsafe {
            if let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                let _ = TerminateProcess(h, 0);
                let _ = CloseHandle(h);
            }
        }
    }

    /// Mutes/unmutes every audio session owned by `pid` on all active outputs.
    /// Returns true if at least one session was found.
    pub fn set_mute(pid: u32, mute: bool) -> anyhow::Result<bool> {
        let mut found = false;
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let devices = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            for i in 0..devices.GetCount()? {
                let Ok(device) = devices.Item(i) else { continue };
                let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
                let Ok(sessions) = manager.GetSessionEnumerator() else { continue };
                for j in 0..sessions.GetCount().unwrap_or(0) {
                    let Ok(control) = sessions.GetSession(j) else { continue };
                    let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
                    if control2.GetProcessId().unwrap_or(0) != pid {
                        continue;
                    }
                    if let Ok(volume) = control.cast::<ISimpleAudioVolume>() {
                        let _ = volume.SetMute(mute, std::ptr::null());
                        found = true;
                    }
                }
            }
        }
        Ok(found)
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn hide_visible(_pid: u32) -> Vec<isize> {
        vec![]
    }
    pub fn show(_pid: u32, _hidden: &[isize]) {}
    pub fn set_background(_pid: u32, _on: bool) -> anyhow::Result<()> {
        Ok(())
    }
    pub fn set_mute(_pid: u32, _mute: bool) -> anyhow::Result<bool> {
        Ok(false)
    }
    pub fn terminate(_pid: u32) {}
}

pub use imp::{hide_visible, set_background, set_mute, show, terminate};
