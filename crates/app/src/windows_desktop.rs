use std::{
    cell::Cell,
    env,
    ffi::OsStr,
    io,
    os::windows::ffi::OsStrExt,
    ptr::{null, null_mut, without_provenance},
    thread,
    time::{Duration, Instant},
};

use gpui::{AnyWindowHandle, App, Global, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use windows_sys::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND,
            ERROR_SUCCESS, HANDLE, HWND, LPARAM, LRESULT, POINT, SetLastError, WPARAM,
        },
        System::{
            LibraryLoader::GetModuleHandleW,
            Registry::{
                HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW,
                RegSetKeyValueW,
            },
            Threading::{CreateMutexExW, SYNCHRONIZATION_SYNCHRONIZE},
        },
        UI::{
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETFOCUS,
                NIM_SETVERSION, NIN_SELECT, NOTIFYICON_VERSION_4, NOTIFYICONDATAW,
                Shell_NotifyIconW,
            },
            WindowsAndMessaging::*,
        },
    },
    w,
};

const INSTANCE_NAME: *const u16 = w!(r"Local\rDM.GuiInstance");
const TRAY_CLASS: *const u16 = w!("rDM.TrayWindow");
const RUN_KEY: *const u16 = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const RUN_VALUE: *const u16 = w!("rDM");
const TRAY_MESSAGE: u32 = WM_APP + 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
const OPEN_MENU_ITEM: u32 = 1;
const EXIT_MENU_ITEM: u32 = 2;

pub(crate) struct Instance(HANDLE);

impl Instance {
    pub(crate) fn acquire(startup: bool) -> io::Result<Option<Self>> {
        if let Some(instance) = Self::try_acquire()? {
            return Ok(Some(instance));
        }
        if startup {
            return Ok(None);
        }
        Self::activate_existing()
    }

    fn try_acquire() -> io::Result<Option<Self>> {
        let handle = unsafe {
            SetLastError(ERROR_SUCCESS);
            CreateMutexExW(null(), INSTANCE_NAME, 0, SYNCHRONIZATION_SYNCHRONIZE)
        };
        let status = io::Error::last_os_error();
        if handle.is_null() {
            return Err(status);
        }
        let instance = Self(handle);
        Ok((status.raw_os_error() != Some(ERROR_ALREADY_EXISTS as i32)).then_some(instance))
    }

    fn activate_existing() -> io::Result<Option<Self>> {
        let message = activation_message()?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let window = unsafe { FindWindowW(TRAY_CLASS, null()) };
            if !window.is_null() {
                let mut process_id = 0;
                unsafe {
                    GetWindowThreadProcessId(window, &mut process_id);
                    if process_id != 0 {
                        AllowSetForegroundWindow(process_id);
                    }
                    if PostMessageW(window, message, 0, 0) != 0 {
                        return Ok(None);
                    }
                }
            }
            if let Some(instance) = Self::try_acquire()? {
                return Ok(Some(instance));
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "No se pudo activar la instancia de rDM",
                ));
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

fn activation_message() -> io::Result<u32> {
    register_message(w!("rDM.ActivateWindow"))
}

fn register_message(name: *const u16) -> io::Result<u32> {
    let message = unsafe { RegisterWindowMessageW(name) };
    if message == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(message)
}

pub(crate) fn startup_registered() -> io::Result<bool> {
    let mut size = 0;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            RUN_VALUE,
            RRF_RT_REG_SZ,
            null_mut(),
            null_mut(),
            &mut size,
        )
    };
    match status {
        ERROR_SUCCESS => Ok(true),
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => Ok(false),
        error => Err(io::Error::from_raw_os_error(error as i32)),
    }
}

pub(crate) fn set_startup_registered(enabled: bool) -> io::Result<()> {
    if enabled {
        let executable = env::current_exe()?;
        let command: Vec<u16> = OsStr::new("\"")
            .encode_wide()
            .chain(executable.as_os_str().encode_wide())
            .chain(OsStr::new("\" --startup").encode_wide())
            .chain([0])
            .collect();
        if command.len() > 261 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "La ruta de rDM excede el límite del registro de inicio de Windows",
            ));
        }
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                RUN_VALUE,
                REG_SZ,
                command.as_ptr().cast(),
                size_of_val(command.as_slice()) as u32,
            )
        };
        return registry_result(status);
    }

    match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE) } {
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => Ok(()),
        status => registry_result(status),
    }
}

fn registry_result(status: u32) -> io::Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}

enum DesktopEvent {
    Open,
    Quit,
}

pub(crate) struct DesktopEvents(UnboundedReceiver<DesktopEvent>);

impl DesktopEvents {
    pub(crate) fn listen(mut self, main_window: AnyWindowHandle, cx: &App) {
        cx.spawn(async move |cx| {
            while let Some(event) = self.0.recv().await {
                match event {
                    DesktopEvent::Open => {
                        if main_window
                            .update(cx, |_, window, _| restore_window(window))
                            .is_err()
                        {
                            break;
                        }
                    }
                    DesktopEvent::Quit => {
                        cx.update(|cx| cx.quit());
                        break;
                    }
                }
            }
        })
        .detach();
    }
}

struct TrayState {
    events: UnboundedSender<DesktopEvent>,
    taskbar_created: u32,
    activate: u32,
    icon: HICON,
    registered: Cell<bool>,
}

impl TrayState {
    fn icon_data(&self, window: HWND) -> NOTIFYICONDATAW {
        NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: window,
            uID: 1,
            ..Default::default()
        }
    }

    fn register(&self, window: HWND) -> bool {
        let mut data = self.icon_data(window);
        if self.registered.replace(false) {
            unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
        }
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        data.uCallbackMessage = TRAY_MESSAGE;
        data.hIcon = self.icon;
        data.szTip[..3].copy_from_slice(&[b'r' as u16, b'D' as u16, b'M' as u16]);
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;

        if !self.icon.is_null() && unsafe { Shell_NotifyIconW(NIM_ADD, &data) } != 0 {
            if unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data) } != 0 {
                self.registered.set(true);
                return true;
            }
            unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
        }
        false
    }

    fn handle_click(&self, window: HWND, wparam: WPARAM, lparam: LPARAM) {
        let event = (lparam & 0xffff) as u32;
        match event {
            WM_LBUTTONUP | NIN_SELECT | NIN_KEYSELECT => {
                let _ = self.events.send(DesktopEvent::Open);
            }
            WM_CONTEXTMENU => {
                let x = wparam as i16 as i32;
                let y = (wparam >> 16) as i16 as i32;
                if let Some(event) = tray_menu(window, x, y) {
                    let _ = self.events.send(event);
                }
                unsafe { Shell_NotifyIconW(NIM_SETFOCUS, &self.icon_data(window)) };
            }
            _ => {}
        }
    }
}

impl Drop for TrayState {
    fn drop(&mut self) {
        if !self.icon.is_null() {
            unsafe { DestroyIcon(self.icon) };
        }
    }
}

fn tray_menu(window: HWND, x: i32, y: i32) -> Option<DesktopEvent> {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return None;
        }
        let mut position = POINT { x, y };
        if (x, y) == (-1, -1) {
            GetCursorPos(&mut position);
        }
        let mut selected = 0;
        if AppendMenuW(menu, MF_STRING, OPEN_MENU_ITEM as usize, w!("Abrir rDM")) != 0
            && AppendMenuW(menu, MF_STRING, EXIT_MENU_ITEM as usize, w!("Salir")) != 0
        {
            SetForegroundWindow(window);
            selected = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                position.x,
                position.y,
                0,
                window,
                null(),
            ) as u32;
            PostMessageW(window, WM_NULL, 0, 0);
        }
        DestroyMenu(menu);
        match selected {
            OPEN_MENU_ITEM => Some(DesktopEvent::Open),
            EXIT_MENU_ITEM => Some(DesktopEvent::Quit),
            _ => None,
        }
    }
}

unsafe extern "system" fn tray_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let creation = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(window, GWLP_USERDATA, creation.lpCreateParams as isize);
        }
        let state = GetWindowLongPtrW(window, GWLP_USERDATA) as *const TrayState;
        if let Some(state) = state.as_ref() {
            if message == state.taskbar_created {
                if !state.register(window) {
                    let _ = state.events.send(DesktopEvent::Open);
                }
                return 0;
            }
            if message == state.activate {
                let _ = state.events.send(DesktopEvent::Open);
                return 0;
            }
            match message {
                TRAY_MESSAGE => {
                    state.handle_click(window, wparam, lparam);
                    return 0;
                }
                WM_CLOSE => {
                    let _ = state.events.send(DesktopEvent::Quit);
                    return 0;
                }
                WM_NCDESTROY => {
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                }
                _ => {}
            }
        }
        DefWindowProcW(window, message, wparam, lparam)
    }
}

pub(crate) struct Desktop {
    window: HWND,
    state: Box<TrayState>,
}

impl Global for Desktop {}

impl Desktop {
    pub(crate) fn install(cx: &mut App) -> io::Result<DesktopEvents> {
        let (events, receiver) = mpsc::unbounded_channel();
        cx.set_global(Self::new(events)?);
        cx.on_app_quit(|cx| {
            cx.remove_global::<Self>();
            async {}
        })
        .detach();
        Ok(DesktopEvents(receiver))
    }

    fn new(events: UnboundedSender<DesktopEvent>) -> io::Result<Self> {
        let taskbar_created = register_message(w!("TaskbarCreated"))?;
        let activate = activation_message()?;
        let module = unsafe { GetModuleHandleW(null()) };
        if module.is_null() {
            return Err(io::Error::last_os_error());
        }
        let icon = unsafe {
            LoadImageW(
                module,
                without_provenance(1),
                IMAGE_ICON,
                GetSystemMetrics(SM_CXSMICON),
                GetSystemMetrics(SM_CYSMICON),
                0,
            )
        };
        let state = Box::new(TrayState {
            events,
            taskbar_created,
            activate,
            icon,
            registered: Cell::new(false),
        });
        let class = WNDCLASSW {
            lpfnWndProc: Some(tray_window_proc),
            hInstance: module,
            lpszClassName: TRAY_CLASS,
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let window = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                TRAY_CLASS,
                null(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                module,
                (&*state as *const TrayState).cast(),
            )
        };
        if window.is_null() {
            return Err(io::Error::last_os_error());
        }
        state.register(window);
        Ok(Self { window, state })
    }

    pub(crate) fn available(cx: &App) -> bool {
        cx.try_global::<Self>()
            .is_some_and(|desktop| desktop.state.registered.get())
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &self.state.icon_data(self.window));
            DestroyWindow(self.window);
        }
    }
}

fn native_handle(window: &Window) -> Option<HWND> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return None;
    };
    Some(handle.hwnd.get() as HWND)
}

pub(crate) fn hide_window(window: &Window) -> bool {
    native_handle(window).is_some_and(|handle| unsafe { ShowWindowAsync(handle, SW_HIDE) != 0 })
}

pub(crate) fn restore_window(window: &Window) {
    if let Some(handle) = native_handle(window) {
        unsafe {
            if IsIconic(handle) != 0 {
                ShowWindowAsync(handle, SW_RESTORE);
            } else if IsWindowVisible(handle) == 0 {
                ShowWindowAsync(handle, SW_SHOW);
            }
        }
    }
    window.activate_window();
}
