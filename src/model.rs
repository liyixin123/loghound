//! 捕获到的一条调试消息的数据模型。

use chrono::{DateTime, Local};

/// 一条从 DBWIN 缓冲区读出的调试输出。
#[derive(Debug, Clone)]
pub struct DebugMessage {
    /// 产生该输出的进程 ID。
    pub pid: u32,
    /// 进程名（如 `myapp.exe`），无法解析时为 `None`。
    pub process_name: Option<String>,
    /// 已解码为 UTF-8、去除尾部换行后的文本。
    pub text: String,
    /// 捕获到该消息时的本地时间。
    pub timestamp: DateTime<Local>,
}

impl DebugMessage {
    /// 用当前本地时间构造一条消息。
    pub fn new(pid: u32, process_name: Option<String>, text: String) -> Self {
        Self {
            pid,
            process_name,
            text,
            timestamp: Local::now(),
        }
    }
}
