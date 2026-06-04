//! 进程过滤逻辑。
//!
//! 规则：`process_names` 和 `pids` 两个 allow 列表都为空时全部放行；
//! 否则进程名（不区分大小写）或 PID 命中任一列表即放行。

use crate::config::FilterConfig;

/// 编译好的过滤器，进程名预先归一化为小写以便快速比较。
#[derive(Debug, Clone)]
pub struct Filter {
    names_lower: Vec<String>,
    pids: Vec<u32>,
}

impl Filter {
    pub fn new(cfg: &FilterConfig) -> Self {
        Self {
            names_lower: cfg
                .process_names
                .iter()
                .map(|n| n.trim().to_lowercase())
                .filter(|n| !n.is_empty())
                .collect(),
            pids: cfg.pids.clone(),
        }
    }

    /// 是否未配置任何过滤条件（全部放行）。
    pub fn is_pass_all(&self) -> bool {
        self.names_lower.is_empty() && self.pids.is_empty()
    }

    /// 判断给定 PID / 进程名的消息是否应被采集。
    pub fn allows(&self, pid: u32, process_name: Option<&str>) -> bool {
        if self.is_pass_all() {
            return true;
        }
        if self.pids.contains(&pid) {
            return true;
        }
        if let Some(name) = process_name {
            let name_lower = name.to_lowercase();
            if self.names_lower.iter().any(|n| n == &name_lower) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(names: &[&str], pids: &[u32]) -> FilterConfig {
        FilterConfig {
            process_names: names.iter().map(|s| s.to_string()).collect(),
            pids: pids.to_vec(),
        }
    }

    #[test]
    fn empty_filter_passes_all() {
        let f = Filter::new(&cfg(&[], &[]));
        assert!(f.is_pass_all());
        assert!(f.allows(123, Some("anything.exe")));
        assert!(f.allows(456, None));
    }

    #[test]
    fn matches_by_name_case_insensitive() {
        let f = Filter::new(&cfg(&["MyApp.exe"], &[]));
        assert!(f.allows(1, Some("myapp.exe")));
        assert!(f.allows(1, Some("MYAPP.EXE")));
        assert!(!f.allows(1, Some("other.exe")));
        assert!(!f.allows(1, None));
    }

    #[test]
    fn matches_by_pid() {
        let f = Filter::new(&cfg(&[], &[42]));
        assert!(f.allows(42, None));
        assert!(!f.allows(43, Some("x.exe")));
    }

    #[test]
    fn name_or_pid_either_matches() {
        let f = Filter::new(&cfg(&["a.exe"], &[42]));
        assert!(f.allows(42, Some("z.exe"))); // pid 命中
        assert!(f.allows(1, Some("a.exe"))); // 名字命中
        assert!(!f.allows(1, Some("z.exe")));
    }

    #[test]
    fn blank_names_are_ignored() {
        let f = Filter::new(&cfg(&["  "], &[]));
        // 仅有空白名字，等价于无过滤
        assert!(f.is_pass_all());
    }
}
