//! loghound —— 采集 DbgView 用户态 OutputDebugString 调试输出到本地文件。

mod capture;
mod cli;
mod config;
mod filter;
mod logger;
mod model;
mod process;
mod service;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Parser;

use cli::{Cli, Command};
use config::Config;

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let command = args.command.unwrap_or(Command::Run);

    match command {
        Command::Run => {
            let path = args.config.unwrap_or_else(config::default_config_path);
            let cfg = Config::load_or_create(&path)?;
            console_run(cfg)
        }
        Command::Install => service::install(),
        Command::Uninstall => service::uninstall(),
        Command::RunService => service::run_as_service(),
    }
}

/// 控制台前台运行：采集当前会话的调试输出，Ctrl+C 优雅退出。
fn console_run(config: Config) -> anyhow::Result<()> {
    let stop = Arc::new(AtomicBool::new(false));

    #[cfg(windows)]
    {
        install_ctrl_handler()?;
        let stop_watch = stop.clone();
        std::thread::spawn(move || {
            while !CONSOLE_STOP.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            stop_watch.store(true, Ordering::Relaxed);
        });
    }

    println!(
        "loghound 控制台模式运行中（Ctrl+C 退出）。日志目录: {}",
        config.log.dir.display()
    );

    let banner = startup_banner(&config.filter);
    println!("{banner}");
    capture::run(&config, stop, Some(banner))
}

/// 程序启动时写入日志的第一条记录，标注本次运行采集哪些进程。
fn startup_banner(filter: &config::FilterConfig) -> String {
    if filter.process_names.is_empty() && filter.pids.is_empty() {
        "loghound 已启动，采集全部进程的 OutputDebugString".to_string()
    } else {
        format!(
            "loghound 已启动，采集 进程名={:?} PID={:?} 的 OutputDebugString",
            filter.process_names, filter.pids
        )
    }
}

/// 控制台 Ctrl+C / 关闭事件的全局停止标志。
#[cfg(windows)]
static CONSOLE_STOP: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
unsafe extern "system" fn console_ctrl_handler(
    _ctrl_type: u32,
) -> windows::Win32::Foundation::BOOL {
    CONSOLE_STOP.store(true, Ordering::Relaxed);
    windows::Win32::Foundation::TRUE
}

#[cfg(windows)]
fn install_ctrl_handler() -> anyhow::Result<()> {
    use windows::Win32::System::Console::SetConsoleCtrlHandler;
    unsafe {
        SetConsoleCtrlHandler(Some(console_ctrl_handler), true)?;
    }
    Ok(())
}
