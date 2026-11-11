use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io,
    os::unix::{
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use gpui::{App, AppContext, Global, QuitMode};
use ksni::TrayMethods;
use tokio::{net::UnixListener, sync::mpsc};

pub(crate) struct Instance {
    _lock: File,
    listener: Option<std::os::unix::net::UnixListener>,
    socket: PathBuf,
}

impl Instance {
    pub(crate) fn acquire(startup: bool) -> io::Result<Option<Self>> {
        let directory = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(format!("rdm-{}", rustix::process::getuid().as_raw()));
        Self::acquire_in(&directory, startup)
    }

    pub(crate) fn take_listener(&mut self) -> io::Result<std::os::unix::net::UnixListener> {
        self.listener
            .take()
            .ok_or_else(|| io::Error::other("El listener ya está iniciado"))
    }

    fn acquire_in(directory: &Path, startup: bool) -> io::Result<Option<Self>> {
        match fs::DirBuilder::new().mode(0o700).create(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir()
            || metadata.uid() != rustix::process::getuid().as_raw()
            || metadata.mode() & 0o077 != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "El directorio de instancia no es privado",
            ));
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(directory.join("instance.lock"))?;
        let socket = directory.join("activate.sock");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match lock.try_lock() {
                Ok(()) => {
                    match fs::remove_file(&socket) {
                        Ok(()) => {}
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error),
                    }
                    let listener = std::os::unix::net::UnixListener::bind(&socket)?;
                    listener.set_nonblocking(true)?;
                    return Ok(Some(Self {
                        _lock: lock,
                        listener: Some(listener),
                        socket,
                    }));
                }
                Err(TryLockError::WouldBlock) => {
                    if startup || UnixStream::connect(&socket).is_ok() {
                        return Ok(None);
                    }
                    if Instant::now() >= deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "No se pudo activar la instancia de rDM",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(TryLockError::Error(error)) => return Err(error),
            }
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket);
    }
}

enum DesktopEvent {
    Open,
    Quit,
    Ready,
    TrayLost,
}

struct Tray {
    events: mpsc::UnboundedSender<DesktopEvent>,
    available: Arc<AtomicBool>,
    icon: ksni::Icon,
    offline: AtomicBool,
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "rdm".into()
    }
    fn title(&self) -> String {
        "rDM".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }
    fn activate(&mut self, _: i32, _: i32) {
        let _ = self.events.send(DesktopEvent::Open);
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            ksni::menu::StandardItem {
                label: "Abrir rDM".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.events.send(DesktopEvent::Open);
                }),
                ..Default::default()
            }
            .into(),
            ksni::menu::StandardItem {
                label: "Salir".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.events.send(DesktopEvent::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
    fn watcher_offline(&self, _: ksni::OfflineReason) -> bool {
        self.offline.store(true, Ordering::Relaxed);
        self.available.store(false, Ordering::Relaxed);
        let _ = self.events.send(DesktopEvent::TrayLost);
        false
    }
}

struct Desktop {
    available: Arc<AtomicBool>,
}

impl Global for Desktop {}

pub(crate) fn available(cx: &App) -> bool {
    cx.try_global::<Desktop>()
        .is_some_and(|desktop| desktop.available.load(Ordering::Relaxed))
}

pub(crate) fn open_window(cx: &mut App) -> anyhow::Result<gpui::AnyWindowHandle> {
    if let Some(window) = cx.windows().into_iter().next() {
        window.update(cx, |_, window, _| window.activate_window())?;
        return Ok(window);
    }
    cx.set_global(crate::views::NewDownloadModalState::default());
    cx.set_global(crate::views::SettingsModalState::default());
    cx.set_global(crate::views::DeleteDownloadModalState::default());
    let options = gpui::WindowOptions {
        window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds::centered(
            None,
            gpui::size(gpui::px(1024.0), gpui::px(680.0)),
            cx,
        ))),
        app_id: Some("rdm".into()),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some("rDM".into()),
            appears_transparent: true,
            traffic_light_position: None,
        }),
        ..Default::default()
    };
    let (window, _) =
        gpui_kit::open_window(options, cx, |_, cx| cx.new(crate::router::RootRouter::new))?;
    Ok(window)
}

pub(crate) fn install(
    listener: std::os::unix::net::UnixListener,
    runtime: &tokio::runtime::Handle,
    startup: bool,
    cx: &mut App,
) -> io::Result<()> {
    let _guard = runtime.enter();
    let listener = UnixListener::from_std(listener)?;
    let (events, mut receiver) = mpsc::unbounded_channel();
    let activation_events = events.clone();
    runtime.spawn(async move {
        loop {
            match listener.accept().await {
                Ok(_) => {
                    if activation_events.send(DesktopEvent::Open).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    eprintln!("Error de activación de rDM: {error}");
                    break;
                }
            }
        }
    });
    let tray_available = Arc::new(AtomicBool::new(false));
    cx.set_global(Desktop {
        available: Arc::clone(&tray_available),
    });
    cx.set_quit_mode(QuitMode::Explicit);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty()
            && !(available(cx) && crate::views::SettingsModal::close_to_tray(cx))
        {
            cx.quit();
        }
    })
    .detach();
    runtime.spawn(run_tray(events, tray_available));
    cx.spawn(async move |cx| {
        let mut initialized = false;
        let mut last_tray_available = false;
        while let Some(event) = receiver.recv().await {
            cx.update(|cx| {
                let tray_available = available(cx);
                let tray_changed = tray_available != last_tray_available;
                last_tray_available = tray_available;
                let show = match event {
                    DesktopEvent::Open => true,
                    DesktopEvent::Quit => {
                        cx.quit();
                        return;
                    }
                    DesktopEvent::Ready => {
                        let show = !initialized && (!startup || !tray_available);
                        initialized = true;
                        show
                    }
                    DesktopEvent::TrayLost => {
                        initialized && !tray_available && cx.windows().is_empty()
                    }
                };
                if show && let Err(error) = open_window(cx) {
                    eprintln!("No se pudo abrir rDM: {error}");
                    cx.quit();
                }
                if tray_changed {
                    cx.refresh_windows();
                }
            });
        }
    })
    .detach();
    Ok(())
}

async fn run_tray(events: mpsc::UnboundedSender<DesktopEvent>, tray_available: Arc<AtomicBool>) {
    let icon = match image::load_from_memory(include_bytes!("../../../icon.png")) {
        Ok(image) => {
            let image = image
                .resize_exact(32, 32, image::imageops::FilterType::Triangle)
                .into_rgba8();
            let data = image
                .pixels()
                .flat_map(|pixel| [pixel[3], pixel[0], pixel[1], pixel[2]])
                .collect();
            ksni::Icon {
                width: 32,
                height: 32,
                data,
            }
        }
        Err(error) => {
            eprintln!("No se pudo cargar el icono de bandeja: {error}");
            let _ = events.send(DesktopEvent::Ready);
            return;
        }
    };
    loop {
        let tray = Tray {
            events: events.clone(),
            available: Arc::clone(&tray_available),
            icon: icon.clone(),
            offline: AtomicBool::new(false),
        };
        let result = tokio::time::timeout(Duration::from_secs(5), tray.spawn()).await;
        let handle = match result {
            Ok(Ok(handle)) => {
                handle
                    .update(|tray| {
                        if !tray.offline.load(Ordering::Relaxed) {
                            tray.available.store(true, Ordering::Relaxed);
                        }
                    })
                    .await;
                Some(handle)
            }
            _ => None,
        };
        let _ = events.send(DesktopEvent::Ready);
        if let Some(handle) = handle {
            while !handle.is_closed() && !events.is_closed() {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            handle.shutdown().await;
        }
        tray_available.store(false, Ordering::Relaxed);
        if events.send(DesktopEvent::TrayLost).is_err() {
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn startup_path() -> io::Result<PathBuf> {
    dirs::config_dir()
        .map(|directory| directory.join("autostart/rdm.desktop"))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "No se encontró el directorio de configuración",
            )
        })
}

pub(crate) fn startup_registered() -> io::Result<bool> {
    match fs::read_to_string(startup_path()?) {
        Ok(content) => Ok(content.lines().any(|line| line == "X-rDM-Managed=true")
            && !content.lines().any(|line| line == "Hidden=true")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn startup_entry(executable: &Path) -> io::Result<String> {
    let executable = executable
        .to_str()
        .filter(|value| executable.is_absolute() && !value.chars().any(char::is_control))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "La ruta de inicio no es válida",
            )
        })?;
    let quoted = quote_desktop_argument(executable);
    let command = if executable.contains('%') {
        let script = quote_desktop_argument("exec \"$@\"");
        format!("/bin/sh -c {script} rdm {quoted} --startup")
    } else {
        format!("{quoted} --startup")
    };
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=rDM\nExec={command}\nIcon=rdm\nTerminal=false\nX-rDM-Managed=true\n"
    ))
}

fn quote_desktop_argument(value: &str) -> String {
    let mut escaped = String::from("\"");
    for character in value.chars() {
        match character {
            '%' => escaped.push_str("%%"),
            '\\' => escaped.push_str("\\\\\\\\"),
            '"' | '$' | '`' => {
                escaped.push_str("\\\\");
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

pub(crate) fn set_startup_registered(enabled: bool) -> io::Result<()> {
    let path = startup_path()?;
    let executable = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_exe)?;
    set_startup_at(&path, &executable, enabled)
}

fn set_startup_at(path: &Path, executable: &Path, enabled: bool) -> io::Result<()> {
    match fs::read_to_string(path) {
        Ok(content) if !content.lines().any(|line| line == "X-rDM-Managed=true") => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "La entrada de inicio existente no pertenece a rDM",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    if !enabled {
        return match fs::remove_file(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        };
    }
    let entry = startup_entry(executable)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        use std::io::Write;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(entry.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, Read, Write},
        process::{Child, Command, Stdio},
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> io::Result<Self> {
            let path = std::env::temp_dir().join(format!("rdm-test-{}", uuid::Uuid::new_v4()));
            fs::DirBuilder::new().mode(0o700).create(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct TestProcess(Child);

    impl Drop for TestProcess {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    #[ignore]
    fn instance_process() -> anyhow::Result<()> {
        let directory = PathBuf::from(
            std::env::var_os("RDM_TEST_INSTANCE")
                .ok_or_else(|| io::Error::other("missing directory"))?,
        );
        let _instance = Instance::acquire_in(&directory, false)?
            .ok_or_else(|| io::Error::other("duplicate instance"))?;
        println!("INSTANCE_READY");
        io::stdout().flush()?;
        let mut byte = [0];
        io::stdin().read_exact(&mut byte)?;
        Ok(())
    }

    #[test]
    fn instance_activation_and_crash_recovery() -> anyhow::Result<()> {
        let directory = TestDirectory::new()?;
        let primary = Instance::acquire_in(&directory.0, false)?
            .ok_or_else(|| io::Error::other("missing instance"))?;
        assert!(Instance::acquire_in(&directory.0, true)?.is_none());
        assert_eq!(
            primary
                .listener
                .as_ref()
                .ok_or_else(|| io::Error::other("missing listener"))?
                .accept()
                .err()
                .map(|error| error.kind()),
            Some(io::ErrorKind::WouldBlock)
        );
        assert!(Instance::acquire_in(&directory.0, false)?.is_none());
        assert!(
            primary
                .listener
                .as_ref()
                .ok_or_else(|| io::Error::other("missing listener"))?
                .accept()
                .is_ok()
        );
        drop(primary);

        let mut child = TestProcess(
            Command::new(std::env::current_exe()?)
                .args([
                    "--exact",
                    "linux_desktop::tests::instance_process",
                    "--ignored",
                    "--nocapture",
                ])
                .env("RDM_TEST_INSTANCE", &directory.0)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?,
        );
        let output = child
            .0
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing stdout"))?;
        let mut lines = io::BufReader::new(output).lines();
        assert!(
            lines
                .by_ref()
                .any(|line| line.is_ok_and(|line| line == "INSTANCE_READY"))
        );
        assert!(Instance::acquire_in(&directory.0, true)?.is_none());
        assert!(Instance::acquire_in(&directory.0, false)?.is_none());
        child.0.kill()?;
        child.0.wait()?;
        assert!(directory.0.join("activate.sock").exists());
        let recovered = Instance::acquire_in(&directory.0, false)?
            .ok_or_else(|| io::Error::other("recovery failed"))?;
        drop(recovered);
        assert!(!directory.0.join("activate.sock").exists());
        Ok(())
    }

    #[test]
    fn instance_rejects_shared_directory() -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let directory = TestDirectory::new()?;
        fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o755))?;
        assert!(
            matches!(Instance::acquire_in(&directory.0, false), Err(error) if error.kind() == io::ErrorKind::PermissionDenied)
        );
        Ok(())
    }

    #[test]
    fn autostart_registration_preserves_unmanaged_entries() -> io::Result<()> {
        let directory = TestDirectory::new()?;
        let path = directory.0.join("autostart/rdm.desktop");
        let executable = Path::new("/home/user/My Apps/rDM.AppImage");
        set_startup_at(&path, executable, true)?;
        let content = fs::read_to_string(&path)?;
        assert!(content.contains("Exec=\"/home/user/My Apps/rDM.AppImage\" --startup"));
        let status = Command::new("desktop-file-validate").arg(&path).status()?;
        assert!(status.success());
        set_startup_at(&path, executable, false)?;
        assert!(!path.exists());
        set_startup_at(&path, executable, false)?;
        fs::write(
            &path,
            "[Desktop Entry]\nType=Application\nName=Custom\nExec=/bin/true\n",
        )?;
        assert!(set_startup_at(&path, executable, true).is_err());
        assert!(set_startup_at(&path, executable, false).is_err());
        assert!(fs::read_to_string(&path)?.contains("Name=Custom"));
        Ok(())
    }

    #[test]
    fn autostart_quotes_reserved_characters() -> io::Result<()> {
        let entry = startup_entry(Path::new("/home/user/a \"$`\\%/rdm"))?;
        assert!(entry.contains("%%"));
        assert!(entry.contains("\\\\\""));
        assert!(entry.contains("\\\\$"));
        assert!(entry.contains("\\\\`"));
        assert!(entry.contains("\\\\\\\\"));
        assert!(startup_entry(Path::new("relative/rdm")).is_err());
        assert!(startup_entry(Path::new("/tmp/rdm\nExec=/bin/false")).is_err());
        let directory = TestDirectory::new()?;
        let executable = directory.0.join("a \"$`\\% rdm");
        std::os::unix::fs::symlink("/bin/echo", &executable)?;
        let desktop_file = directory.0.join("rdm.desktop");
        fs::write(&desktop_file, startup_entry(&executable)?)?;
        let output = Command::new("gio")
            .arg("launch")
            .arg(&desktop_file)
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("--startup"));
        Ok(())
    }

    struct Watcher {
        host: bool,
    }

    #[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
    impl Watcher {
        fn register_status_notifier_item(&self, _service: &str) {}
        #[zbus(property)]
        fn is_status_notifier_host_registered(&self) -> bool {
            self.host
        }
        #[zbus(property)]
        fn protocol_version(&self) -> i32 {
            0
        }
    }

    async fn mock_watcher(host: bool) -> zbus::Result<zbus::Connection> {
        zbus::connection::Builder::session()?
            .name("org.kde.StatusNotifierWatcher")?
            .serve_at("/StatusNotifierWatcher", Watcher { host })?
            .build()
            .await
    }

    async fn wait_ready(
        receiver: &mut mpsc::UnboundedReceiver<DesktopEvent>,
        available: &AtomicBool,
        expected: bool,
    ) -> anyhow::Result<()> {
        tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(event) = receiver.recv().await {
                if matches!(event, DesktopEvent::Ready)
                    && available.load(Ordering::Relaxed) == expected
                {
                    return Ok(());
                }
            }
            Err(io::Error::other("tray channel closed"))
        })
        .await??;
        Ok(())
    }

    #[test]
    fn tray_recovers_after_host_disappears() -> anyhow::Result<()> {
        if std::env::var_os("RDM_TEST_PRIVATE_BUS").is_none() {
            let mut daemon = TestProcess(
                Command::new("dbus-daemon")
                    .args(["--session", "--nofork", "--print-address=1"])
                    .stdout(Stdio::piped())
                    .spawn()?,
            );
            let output = daemon
                .0
                .stdout
                .take()
                .ok_or_else(|| io::Error::other("missing dbus stdout"))?;
            let mut address = String::new();
            io::BufReader::new(output).read_line(&mut address)?;
            let mut child = TestProcess(
                Command::new(std::env::current_exe()?)
                    .args([
                        "--exact",
                        "linux_desktop::tests::tray_recovers_after_host_disappears",
                        "--nocapture",
                    ])
                    .env("RDM_TEST_PRIVATE_BUS", "1")
                    .env("DBUS_SESSION_BUS_ADDRESS", address.trim())
                    .spawn()?,
            );
            let deadline = Instant::now() + Duration::from_secs(40);
            loop {
                if let Some(status) = child.0.try_wait()? {
                    assert!(status.success());
                    break;
                }
                if Instant::now() >= deadline {
                    return Err(io::Error::other("tray test timed out").into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            return Ok(());
        }
        tokio::runtime::Runtime::new()?.block_on(async {
            let available = Arc::new(AtomicBool::new(false));
            let (events, mut receiver) = mpsc::unbounded_channel();
            let task = tokio::spawn(run_tray(events, Arc::clone(&available)));
            wait_ready(&mut receiver, &available, false).await?;
            let hostless = mock_watcher(false).await?;
            wait_ready(&mut receiver, &available, false).await?;
            assert!(!available.load(Ordering::Relaxed));
            hostless.close().await?;
            let watcher = mock_watcher(true).await?;
            wait_ready(&mut receiver, &available, true).await?;
            watcher.close().await?;
            tokio::time::timeout(Duration::from_secs(5), async {
                while let Some(event) = receiver.recv().await {
                    if matches!(event, DesktopEvent::TrayLost) {
                        break;
                    }
                }
            })
            .await?;
            assert!(!available.load(Ordering::Relaxed));
            let watcher = mock_watcher(true).await?;
            wait_ready(&mut receiver, &available, true).await?;
            drop(receiver);
            tokio::time::timeout(Duration::from_secs(5), task).await??;
            watcher.close().await?;
            Ok(())
        })
    }
}
