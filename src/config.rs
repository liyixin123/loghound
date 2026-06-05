//! 配置加载、校验与默认值。
//!
//! 配置以 TOML 文件形式存放，默认与可执行文件同目录的 `loghound.toml`。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 配置相关错误。
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("读取配置文件失败 {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("写入默认配置失败 {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("解析配置文件失败 {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("配置校验失败: {0}")]
    Invalid(String),
}

/// 顶层配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default)]
    pub log: LogConfig,
    #[serde(default)]
    pub filter: FilterConfig,
}

/// 日志输出与轮转配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogConfig {
    /// 日志目录。
    pub dir: PathBuf,
    /// 日志文件名前缀（实际文件如 `loghound.2026-06-04`）。
    pub file_prefix: String,
    /// 最多保留天数（对应每日滚动的文件个数），必须 >= 1。
    pub max_days: usize,
    /// 单行格式，支持占位符 `{time}` `{pid}` `{process}` `{message}`。
    pub line_format: String,
}

/// 进程过滤配置。两个列表都为空表示全部采集。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FilterConfig {
    /// 允许的进程名列表（不区分大小写），如 `["myapp.exe"]`。
    #[serde(default)]
    pub process_names: Vec<String>,
    /// 允许的 PID 列表。
    #[serde(default)]
    pub pids: Vec<u32>,
}

impl Default for FilterConfig {
    fn default() -> Self {
        // 默认仅监控 ISVCommServer.exe（不区分大小写）。
        Self {
            process_names: vec!["ISVCommServer.exe".to_string()],
            pids: Vec::new(),
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            dir: default_log_dir(),
            file_prefix: "loghound".to_string(),
            max_days: 15,
            line_format: "{time} [{pid} {process}] {message}".to_string(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            log: LogConfig::default(),
            filter: FilterConfig::default(),
        }
    }
}

/// 默认日志目录。Windows 下用 ProgramData，其它平台用临时目录（便于在非 Windows 上测试）。
fn default_log_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let base = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
        base.join("loghound").join("logs")
    }
    #[cfg(not(windows))]
    {
        std::env::temp_dir().join("loghound").join("logs")
    }
}

impl Config {
    /// 从指定路径加载配置；文件不存在时写出一份默认配置并返回它。
    pub fn load_or_create(path: &Path) -> Result<Self, ConfigError> {
        if path.exists() {
            let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
                path: path.to_path_buf(),
                source,
            })?;
            let config: Config = toml::from_str(&text).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            })?;
            config.validate()?;
            Ok(config)
        } else {
            let config = Config::default();
            config.write_to(path)?;
            Ok(config)
        }
    }

    /// 将配置序列化写入指定路径（含父目录创建）。
    pub fn write_to(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
                path: path.to_path_buf(),
                source,
            })?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::Invalid(format!("序列化失败: {e}")))?;
        std::fs::write(path, text).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    /// 校验配置合法性，fail-fast。
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.log.max_days == 0 {
            return Err(ConfigError::Invalid("log.max_days 必须 >= 1".to_string()));
        }
        if self.log.file_prefix.trim().is_empty() {
            return Err(ConfigError::Invalid("log.file_prefix 不能为空".to_string()));
        }
        Ok(())
    }
}

/// 默认配置文件路径：可执行文件同目录下的 `loghound.toml`。
pub fn default_config_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("loghound.toml")))
        .unwrap_or_else(|| PathBuf::from("loghound.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        assert!(Config::default().validate().is_ok());
    }

    #[test]
    fn rejects_zero_max_days() {
        let mut c = Config::default();
        c.log.max_days = 0;
        assert!(c.validate().is_err());
    }

    #[test]
    fn rejects_empty_prefix() {
        let mut c = Config::default();
        c.log.file_prefix = "   ".to_string();
        assert!(c.validate().is_err());
    }

    #[test]
    fn parses_minimal_toml_with_defaults() {
        let toml = r#"
            [filter]
            process_names = ["a.exe"]
        "#;
        let c: Config = toml::from_str(toml).unwrap();
        assert_eq!(c.filter.process_names, vec!["a.exe".to_string()]);
        // log 段缺失时应使用默认值
        assert_eq!(c.log.max_days, 15);
    }

    #[test]
    fn roundtrip_serialize_parse() {
        let original = Config::default();
        let text = toml::to_string_pretty(&original).unwrap();
        let parsed: Config = toml::from_str(&text).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn load_or_create_writes_default_when_missing() {
        let dir = std::env::temp_dir().join(format!("loghound-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("loghound.toml");
        let c = Config::load_or_create(&path).unwrap();
        assert!(path.exists());
        assert_eq!(c, Config::default());
        // 第二次加载应解析刚写出的文件
        let c2 = Config::load_or_create(&path).unwrap();
        assert_eq!(c, c2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
