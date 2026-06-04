//! 命令行参数定义。

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// 采集 DbgView 用户态 OutputDebugString 调试输出到本地文件。
#[derive(Debug, Parser)]
#[command(name = "loghound", version, about, long_about = None)]
pub struct Cli {
    /// 配置文件路径（默认：可执行文件同目录下的 loghound.toml）。
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 控制台前台运行（默认）。采集当前会话的调试输出。
    Run,
    /// 注册为 Windows 服务（开机自启，需管理员权限）。
    Install,
    /// 卸载 Windows 服务（需管理员权限）。
    Uninstall,
    /// 由服务控制管理器（SCM）调起，不应手动运行。
    #[command(hide = true)]
    RunService,
}
