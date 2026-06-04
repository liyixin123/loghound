//! PID → 进程名解析，带一个有界缓存以避免对每条消息都做系统调用。

use std::collections::{HashMap, VecDeque};

/// 进程名查询的抽象，便于在非 Windows 上 / 测试中注入实现。
pub trait ProcessNameLookup {
    /// 查询给定 PID 的进程名（如 `myapp.exe`）。进程已退出或无权限时返回 `None`。
    fn lookup(&self, pid: u32) -> Option<String>;
}

/// 有界的进程名缓存（FIFO 淘汰）。
pub struct ProcessNameCache<L: ProcessNameLookup> {
    lookup: L,
    capacity: usize,
    map: HashMap<u32, Option<String>>,
    order: VecDeque<u32>,
}

impl<L: ProcessNameLookup> ProcessNameCache<L> {
    pub fn new(lookup: L, capacity: usize) -> Self {
        Self {
            lookup,
            capacity: capacity.max(1),
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// 取进程名，命中缓存则直接返回，否则查询并缓存结果。
    pub fn get(&mut self, pid: u32) -> Option<String> {
        if let Some(cached) = self.map.get(&pid) {
            return cached.clone();
        }
        let resolved = self.lookup.lookup(pid);
        self.insert(pid, resolved.clone());
        resolved
    }

    fn insert(&mut self, pid: u32, name: Option<String>) {
        if self.map.len() >= self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.map.remove(&oldest);
            }
        }
        self.map.insert(pid, name);
        self.order.push_back(pid);
    }
}

/// 基于 Win32 API 的实现。
#[cfg(windows)]
pub struct WinProcessLookup;

#[cfg(windows)]
impl ProcessNameLookup for WinProcessLookup {
    fn lookup(&self, pid: u32) -> Option<String> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        };
        use windows::core::PWSTR;

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut size = buf.len() as u32;
            let result = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut size,
            );
            let _ = CloseHandle(handle);
            result.ok()?;
            let full = String::from_utf16_lossy(&buf[..size as usize]);
            Some(file_name_of(&full))
        }
    }
}

/// 非 Windows 平台的占位实现，始终返回 `None`（仅用于跨平台编译）。
#[cfg(not(windows))]
pub struct NullLookup;

#[cfg(not(windows))]
impl ProcessNameLookup for NullLookup {
    fn lookup(&self, _pid: u32) -> Option<String> {
        None
    }
}

/// 当前平台的进程名查询实现类型。
#[cfg(windows)]
pub type PlatformLookup = WinProcessLookup;
#[cfg(not(windows))]
pub type PlatformLookup = NullLookup;

/// 构造当前平台的进程名查询实现。
pub fn platform_lookup() -> PlatformLookup {
    #[cfg(windows)]
    {
        WinProcessLookup
    }
    #[cfg(not(windows))]
    {
        NullLookup
    }
}

/// 从完整路径中取出文件名部分（兼容 `\` 与 `/` 分隔符）。
fn file_name_of(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// 记录每个 PID 被查询次数的 mock。
    struct MockLookup {
        names: HashMap<u32, String>,
        calls: RefCell<HashMap<u32, usize>>,
    }

    impl ProcessNameLookup for MockLookup {
        fn lookup(&self, pid: u32) -> Option<String> {
            *self.calls.borrow_mut().entry(pid).or_insert(0) += 1;
            self.names.get(&pid).cloned()
        }
    }

    #[test]
    fn file_name_extraction() {
        assert_eq!(file_name_of("C:\\dir\\app.exe"), "app.exe");
        assert_eq!(file_name_of("/usr/bin/app"), "app");
        assert_eq!(file_name_of("noslash"), "noslash");
    }

    #[test]
    fn caches_results_per_pid() {
        let mut names = HashMap::new();
        names.insert(1, "a.exe".to_string());
        let mock = MockLookup {
            names,
            calls: RefCell::new(HashMap::new()),
        };
        let calls_ref = mock.calls.borrow().clone();
        let _ = calls_ref;
        let mut cache = ProcessNameCache::new(mock, 16);

        assert_eq!(cache.get(1).as_deref(), Some("a.exe"));
        assert_eq!(cache.get(1).as_deref(), Some("a.exe"));
        // 未知 pid 缓存为 None，也不应重复查询
        assert_eq!(cache.get(2), None);
        assert_eq!(cache.get(2), None);

        let calls = cache.lookup.calls.borrow();
        assert_eq!(calls.get(&1), Some(&1));
        assert_eq!(calls.get(&2), Some(&1));
    }

    #[test]
    fn evicts_when_over_capacity() {
        let mock = MockLookup {
            names: HashMap::new(),
            calls: RefCell::new(HashMap::new()),
        };
        let mut cache = ProcessNameCache::new(mock, 2);
        cache.get(1);
        cache.get(2);
        cache.get(3); // 应淘汰 pid 1
        cache.get(1); // 重新查询 pid 1

        let calls = cache.lookup.calls.borrow();
        assert_eq!(calls.get(&1), Some(&2)); // 被查询了两次
    }
}
