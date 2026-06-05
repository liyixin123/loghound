//! DBWIN 共享内存 + 事件握手的封装。
//!
//! 采集方负责创建 `DBWIN_BUFFER` 共享内存与 `DBWIN_BUFFER_READY` /
//! `DBWIN_DATA_READY` 两个事件，然后循环：置位 BUFFER_READY → 等待 DATA_READY →
//! 从缓冲区读出 `{ pid, ansi_string }`。系统中同一会话只能有一个采集方。

/// DBWIN 共享内存大小（固定 4KB）。
#[cfg(windows)]
const BUFFER_SIZE: usize = 4096;

/// DBWIN 监视器。
#[cfg(windows)]
pub struct DbwinMonitor {
    buffer_ready: windows::Win32::Foundation::HANDLE,
    data_ready: windows::Win32::Foundation::HANDLE,
    mapping: windows::Win32::Foundation::HANDLE,
    view: windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS,
}

/// 一次轮询的结果。
pub enum Poll {
    /// 读到一条消息：(pid, 解码后的文本)。
    Message(u32, String),
    /// 在超时时间内没有新消息。
    Timeout,
}

#[cfg(windows)]
mod imp {
    use super::{BUFFER_SIZE, DbwinMonitor, Poll};
    use crate::capture::decode::decode_message;

    use windows::Win32::Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
    };
    use windows::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_READ, MapViewOfFile, PAGE_READWRITE, UnmapViewOfFile,
    };
    use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForSingleObject};
    use windows::core::PCWSTR;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    impl DbwinMonitor {
        /// 创建所有内核对象。若 DebugView 等其它采集方已占用，则返回错误。
        pub fn new() -> anyhow::Result<Self> {
            unsafe {
                let buffer_ready_name = wide("DBWIN_BUFFER_READY");
                let data_ready_name = wide("DBWIN_DATA_READY");
                let buffer_name = wide("DBWIN_BUFFER");

                // 自动重置、初始非置位的事件
                let buffer_ready =
                    CreateEventW(None, false, false, PCWSTR(buffer_ready_name.as_ptr()))?;
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    let _ = CloseHandle(buffer_ready);
                    anyhow::bail!(
                        "DBWIN_BUFFER_READY 已被占用，可能 DebugView 或其它采集程序正在运行，请先关闭它"
                    );
                }

                let data_ready =
                    match CreateEventW(None, false, false, PCWSTR(data_ready_name.as_ptr())) {
                        Ok(h) => h,
                        Err(e) => {
                            let _ = CloseHandle(buffer_ready);
                            return Err(e.into());
                        }
                    };
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    let _ = CloseHandle(buffer_ready);
                    let _ = CloseHandle(data_ready);
                    anyhow::bail!("DBWIN_DATA_READY 已被占用，请先关闭其它调试输出采集程序");
                }

                let mapping = match CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    None,
                    PAGE_READWRITE,
                    0,
                    BUFFER_SIZE as u32,
                    PCWSTR(buffer_name.as_ptr()),
                ) {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = CloseHandle(buffer_ready);
                        let _ = CloseHandle(data_ready);
                        return Err(e.into());
                    }
                };

                let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, BUFFER_SIZE);
                if view.Value.is_null() {
                    let _ = CloseHandle(buffer_ready);
                    let _ = CloseHandle(data_ready);
                    let _ = CloseHandle(mapping);
                    anyhow::bail!("MapViewOfFile 失败");
                }

                Ok(Self {
                    buffer_ready,
                    data_ready,
                    mapping,
                    view,
                })
            }
        }

        /// 置位 BUFFER_READY 并等待下一条消息，最多等待 `timeout_ms` 毫秒。
        pub fn poll(&self, timeout_ms: u32) -> anyhow::Result<Poll> {
            unsafe {
                // 告诉生产者：缓冲区可用
                SetEvent(self.buffer_ready)?;
                let wait = WaitForSingleObject(self.data_ready, timeout_ms);
                if wait != WAIT_OBJECT_0 {
                    return Ok(Poll::Timeout);
                }
                let base = self.view.Value as *const u8;
                let pid = std::ptr::read_unaligned(base as *const u32);
                let data = std::slice::from_raw_parts(base.add(4), BUFFER_SIZE - 4);
                let text = decode_message(data);
                Ok(Poll::Message(pid, text))
            }
        }
    }

    impl Drop for DbwinMonitor {
        fn drop(&mut self) {
            unsafe {
                let _ = UnmapViewOfFile(self.view);
                let _ = CloseHandle(self.mapping);
                let _ = CloseHandle(self.data_ready);
                let _ = CloseHandle(self.buffer_ready);
            }
        }
    }
}

/// 非 Windows 平台的占位实现，仅用于跨平台编译。
#[cfg(not(windows))]
pub struct DbwinMonitor;

#[cfg(not(windows))]
impl DbwinMonitor {
    pub fn new() -> anyhow::Result<Self> {
        anyhow::bail!("DBWIN 捕获仅在 Windows 上可用")
    }

    pub fn poll(&self, _timeout_ms: u32) -> anyhow::Result<Poll> {
        anyhow::bail!("DBWIN 捕获仅在 Windows 上可用")
    }
}
