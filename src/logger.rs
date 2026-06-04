//! 日志文件写入：按天滚动 + 最多保留 N 天。
//!
//! 复用 `tracing-appender` 的 `RollingFileAppender`（实现了 `io::Write`），
//! 由它负责按天切分文件以及清理超过 `max_log_files` 的旧文件。

use std::io::Write;

use tracing_appender::rolling::{RollingFileAppender, Rotation};

use crate::config::LogConfig;
use crate::model::DebugMessage;

/// 时间戳格式。
const TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.3f";
/// 进程名缺失时的占位符。
const UNKNOWN_PROCESS: &str = "-";

/// 把一条消息按格式串渲染为单行（不含结尾换行）。
///
/// 支持占位符：`{time}` `{pid}` `{process}` `{message}`。
pub fn format_line(line_format: &str, msg: &DebugMessage) -> String {
    let time = msg.timestamp.format(TIME_FORMAT).to_string();
    let process = msg.process_name.as_deref().unwrap_or(UNKNOWN_PROCESS);
    line_format
        .replace("{time}", &time)
        .replace("{pid}", &msg.pid.to_string())
        .replace("{process}", process)
        .replace("{message}", &msg.text)
}

/// 日志写入器。
pub struct LogWriter {
    appender: RollingFileAppender,
    line_format: String,
}

impl LogWriter {
    /// 根据配置构建写入器（含日志目录创建与保留策略）。
    pub fn new(cfg: &LogConfig) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&cfg.dir)?;
        let appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix(&cfg.file_prefix)
            .max_log_files(cfg.max_days)
            .build(&cfg.dir)?;
        Ok(Self {
            appender,
            line_format: cfg.line_format.clone(),
        })
    }

    /// 写入一条消息（追加换行）。
    pub fn write_message(&mut self, msg: &DebugMessage) -> std::io::Result<()> {
        let line = format_line(&self.line_format, msg);
        self.appender.write_all(line.as_bytes())?;
        self.appender.write_all(b"\n")?;
        Ok(())
    }

    /// 刷新缓冲。
    pub fn flush(&mut self) {
        let _ = self.appender.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};

    fn sample() -> DebugMessage {
        let ts = Local.with_ymd_and_hms(2026, 6, 4, 12, 34, 56).unwrap();
        DebugMessage {
            pid: 4321,
            process_name: Some("myapp.exe".to_string()),
            text: "hello world".to_string(),
            timestamp: ts,
        }
    }

    #[test]
    fn formats_all_placeholders() {
        let line = format_line("{time} [{pid} {process}] {message}", &sample());
        assert_eq!(line, "2026-06-04 12:34:56.000 [4321 myapp.exe] hello world");
    }

    #[test]
    fn missing_process_uses_placeholder() {
        let mut msg = sample();
        msg.process_name = None;
        let line = format_line("[{process}] {message}", &msg);
        assert_eq!(line, "[-] hello world");
    }

    #[test]
    fn custom_format_order() {
        let line = format_line("{message} <{pid}>", &sample());
        assert_eq!(line, "hello world <4321>");
    }

    #[test]
    fn writer_creates_file_and_writes() {
        let dir = std::env::temp_dir().join(format!("loghound-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cfg = LogConfig {
            dir: dir.clone(),
            file_prefix: "test".to_string(),
            max_days: 3,
            line_format: "{message}".to_string(),
        };
        {
            let mut w = LogWriter::new(&cfg).unwrap();
            w.write_message(&sample()).unwrap();
            w.flush();
        }
        // 目录下应出现一个以 test 开头的文件，且包含写入内容
        let mut found = false;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name().to_string_lossy().starts_with("test") {
                let content = std::fs::read_to_string(entry.path()).unwrap();
                assert!(content.contains("hello world"));
                found = true;
            }
        }
        assert!(found, "未找到日志文件");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
