//! Windows 服务封装：注册（install）、卸载（uninstall）、以及由 SCM 调起后的运行。
//!
//! ⚠️ 重要限制：服务以 LocalSystem 运行在 session 0，纯用户态 DBWIN 机制
//! 只能捕获 session 0 内进程（其它服务）的 `OutputDebugString`，**无法跨会话**
//! 捕获交互式桌面应用的调试输出。要采集桌面应用日志，请用控制台模式
//! （在该桌面会话里运行 `loghound run`）。

/// 服务模式启动时写入日志的限制说明。
pub const SESSION_LIMIT_BANNER: &str = "以 Windows 服务模式启动（session 0）。\
注意：纯用户态捕获无法跨会话，只能采集到 session 0 内进程（其它服务）的 OutputDebugString；\
桌面交互式应用的调试输出请改用控制台模式 `loghound run` 采集。";

#[cfg(windows)]
pub use windows_impl::{install, run_as_service, uninstall};

#[cfg(not(windows))]
mod stub {
    pub fn run_as_service() -> anyhow::Result<()> {
        anyhow::bail!("Windows 服务模式仅在 Windows 上可用")
    }
    pub fn install() -> anyhow::Result<()> {
        anyhow::bail!("Windows 服务模式仅在 Windows 上可用")
    }
    pub fn uninstall() -> anyhow::Result<()> {
        anyhow::bail!("Windows 服务模式仅在 Windows 上可用")
    }
}

#[cfg(not(windows))]
pub use stub::{install, run_as_service, uninstall};

#[cfg(windows)]
mod windows_impl {
    use std::ffi::OsString;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use windows_service::service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
    use windows_service::{define_windows_service, service_dispatcher};

    use crate::config::{self, Config};

    pub const SERVICE_NAME: &str = "loghound";
    const SERVICE_DISPLAY: &str = "loghound 调试日志采集服务";
    const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

    define_windows_service!(ffi_service_main, service_main);

    /// 进入服务调度循环（由 main 在收到 `run-service` 子命令时调用）。
    pub fn run_as_service() -> anyhow::Result<()> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
        Ok(())
    }

    fn service_main(_args: Vec<OsString>) {
        if let Err(e) = run_service() {
            eprintln!("loghound 服务运行失败: {e}");
        }
    }

    fn run_service() -> anyhow::Result<()> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_handler = stop.clone();

        let event_handler = move |control| -> ServiceControlHandlerResult {
            match control {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    stop_for_handler.store(true, Ordering::Relaxed);
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

        let report = |state: ServiceState, accept: ServiceControlAccept| {
            let _ = status_handle.set_service_status(ServiceStatus {
                service_type: SERVICE_TYPE,
                current_state: state,
                controls_accepted: accept,
                exit_code: ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: Duration::default(),
                process_id: None,
            });
        };

        report(
            ServiceState::Running,
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        );

        let config_path = config::default_config_path();
        let result = Config::load_or_create(&config_path).and_then(|config| {
            crate::capture::run(
                &config,
                stop.clone(),
                Some(super::SESSION_LIMIT_BANNER.to_string()),
            )
            .map_err(|e| config::ConfigError::Invalid(e.to_string()))
        });

        report(ServiceState::Stopped, ServiceControlAccept::empty());
        result.map_err(Into::into)
    }

    /// 注册为开机自启的 Windows 服务。
    pub fn install() -> anyhow::Result<()> {
        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
        )?;
        let exe = std::env::current_exe()?;
        let service_info = ServiceInfo {
            name: OsString::from(SERVICE_NAME),
            display_name: OsString::from(SERVICE_DISPLAY),
            service_type: SERVICE_TYPE,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: exe,
            launch_arguments: vec![OsString::from("run-service")],
            dependencies: vec![],
            account_name: None, // LocalSystem
            account_password: None,
        };
        let service =
            manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
        service.set_description(SERVICE_DISPLAY)?;
        println!("已注册服务 \"{SERVICE_NAME}\"。可用 `sc start {SERVICE_NAME}` 启动。");
        Ok(())
    }

    /// 停止并删除服务。
    pub fn uninstall() -> anyhow::Result<()> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service(
            SERVICE_NAME,
            ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
        )?;
        let _ = service.stop();
        service.delete()?;
        println!("已卸载服务 \"{SERVICE_NAME}\"。");
        Ok(())
    }
}
