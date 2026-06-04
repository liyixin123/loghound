//! 捕获循环编排：DBWIN 读取 → 进程名解析 → 过滤 → 写入日志。

pub mod dbwin;
pub mod decode;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::Config;
use crate::filter::Filter;
use crate::logger::LogWriter;
use crate::model::DebugMessage;
use crate::process::{self, ProcessNameCache};

use dbwin::{DbwinMonitor, Poll};

/// 单次等待消息的超时（毫秒）。超时即回到循环顶部检查停止标志。
const POLL_TIMEOUT_MS: u32 = 500;
/// 进程名缓存容量。
const NAME_CACHE_CAPACITY: usize = 512;

/// 运行捕获循环，直到 `stop` 被置位。阻塞调用方线程。
///
/// `banner` 若有值，会作为第一条记录写入日志（用于标注运行模式 / 限制说明）。
pub fn run(config: &Config, stop: Arc<AtomicBool>, banner: Option<String>) -> anyhow::Result<()> {
    let monitor = DbwinMonitor::new()?;
    let filter = Filter::new(&config.filter);
    let mut writer = LogWriter::new(&config.log)?;
    let mut names = ProcessNameCache::new(process::platform_lookup(), NAME_CACHE_CAPACITY);

    if let Some(text) = banner {
        let msg = DebugMessage::new(0, Some("loghound".to_string()), text);
        let _ = writer.write_message(&msg);
        writer.flush();
    }

    while !stop.load(Ordering::Relaxed) {
        match monitor.poll(POLL_TIMEOUT_MS)? {
            Poll::Timeout => continue,
            Poll::Message(pid, text) => {
                let name = names.get(pid);
                if filter.allows(pid, name.as_deref()) {
                    let msg = DebugMessage::new(pid, name, text);
                    if let Err(e) = writer.write_message(&msg) {
                        eprintln!("写入日志失败: {e}");
                    }
                }
            }
        }
    }

    writer.flush();
    Ok(())
}
