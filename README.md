# loghound

一个轻量级的 Windows 调试日志采集服务，用于捕获用户态 `OutputDebugString` 输出并持久化到本地文件。

> **DbgView** 是 Sysinternals 著名的调试输出查看工具，但它需要人工值守且日志难以长期归档。`loghound` 在此基础上提供了**无人值守、自动轮转、按需过滤、开机自启**等能力，适合生产环境持续采集特定进程的调试日志。

---

## 特性

- **轻量无依赖**：单文件可执行程序，开箱即用。
- **双模式运行**：支持**控制台前台运行**（方便桌面应用日志采集）和**Windows 服务模式**（适合后台服务，开机自启）。
- **自动日志轮转**：按天生成日志文件（如 `loghound.2026-06-04`），并自动清理超期的历史文件。
- **灵活过滤**：支持按**进程名**（不区分大小写）或 **PID** 精确采集，避免日志淹没。
- **中文不乱码**：针对 `OutputDebugString` 的 ANSI（系统代码页）编码，使用 Win32 API (`CP_ACP`) 正确解码，中文系统（GBK）不再乱码。
- **自定义格式**：日志单行格式可通过占位符自由配置（`{time}` `{pid}` `{process}` `{message}`）。
- **带界缓存**：进程名解析带 FIFO 缓存，降低高频输出场景的系统调用开销。
- **开箱即用安装包**：附带的 `.bat` 脚本可一键注册为登录启动任务（无需手动编写注册表或服务）。

---

## 快速开始

### 方式一：控制台模式（推荐，桌面应用日志采集）

```powershell
# 1. 编译（或下载 Release）
cargo build --release

# 2. 编辑配置文件 loghound.toml（可选）
# 默认过滤 ISVCommServer.exe，可修改为你要监控的进程

# 3. 运行
.\target\release\loghound.exe
# 或指定配置
.\target\release\loghound.exe --config C:\path\to\loghound.toml
```

按 `Ctrl+C` 即可优雅退出。

> **注意**：控制台模式只能在当前用户会话中采集进程输出，适合桌面交互式应用。

### 方式二：注册为 Windows 服务（后台服务，开机自启）

```powershell
# 以管理员身份运行
.\target\release\loghound.exe install
sc start loghound
```

卸载服务：
```powershell
.\target\release\loghound.exe uninstall
```

> **⚠️ 重要限制**：服务运行在 Session 0，而纯用户态 `DBWIN` 机制**无法跨会话**。因此服务模式下**只能采集到同样运行在 Session 0 的其他服务进程**的 `OutputDebugString`，**无法捕获桌面交互式应用的日志**。采集桌面应用请使用控制台模式。

### 方式三：登录启动计划任务（附赠脚本）

如果 Windows 服务模式的 Session 0 限制不符合你的场景，但又希望开机自启，可以使用仓库附带的脚本：

```powershell
# 在项目 dist/loghound/ 目录（或 scripts/）下：
右键 install.bat → 以管理员身份运行
```

这将注册一个**登录时启动的计划任务**，并在后台隐藏运行 `loghound`（使用 `run-hidden.vbs`），从而采集当前桌面会话中的调试输出。卸载时运行 `uninstall.bat`。

---

## 配置文件

配置文件为 TOML 格式，默认放在与 `loghound.exe` 同目录下。若不存在，首次启动会自动生成一份默认配置。

### 示例 `loghound.toml`

```toml
# loghound 配置文件
# 放在与 loghound.exe 同目录。修改后需重启服务/程序生效。

[log]
# 日志输出目录
dir = "C:\\ProgramData\\loghound\\logs"
# 日志文件名前缀，实际文件形如 loghound.2026-06-04
file_prefix = "loghound"
# 最多保留天数（按天滚动，超过的旧文件自动删除），必须 >= 1
max_days = 15
# 单行格式，占位符：{time} {pid} {process} {message}
line_format = "{time} [{pid} {process}] {message}"

[filter]
# 仅采集这些进程名的调试输出（不区分大小写）。留空 [] 表示采集全部。
process_names = ["ISVCommServer.exe"]
# 额外按 PID 过滤（一般留空，进程名更实用）
pids = []
```

### 配置项说明

| 配置项 | 说明 | 默认值 |
|--------|------|--------|
| `log.dir` | 日志文件存放目录 | `C:\ProgramData\loghound\logs` |
| `log.file_prefix` | 日志文件名前缀 | `loghound` |
| `log.max_days` | 日志保留天数（>=1） | `15` |
| `log.line_format` | 单行输出格式 | `{time} [{pid} {process}] {message}` |
| `filter.process_names` | 允许的进程名列表（不区分大小写） | `["ISVCommServer.exe"]` |
| `filter.pids` | 允许的 PID 列表 | `[]` |

> 将 `filter.process_names` 和 `filter.pids` 同时留空 `[]`，即可采集所有进程的调试输出。

---

## 日志输出示例

```
2026-06-04 10:23:45.123 [5628 ISVCommServer.exe] [INFO] Connection established
2026-06-04 10:23:45.234 [5628 ISVCommServer.exe] [DEBUG] Received heartbeat
2026-06-04 10:23:46.001 [5628 ISVCommServer.exe] [WARN] Latency threshold exceeded: 342ms
```

---

## 技术原理

`OutputDebugString` 是 Windows 提供的用户态调试输出 API。它的底层通过一组固定的内核对象（共享内存 `DBWIN_BUFFER` + 事件 `DBWIN_BUFFER_READY` / `DBWIN_DATA_READY`）进行进程间通信。`loghound` 扮演采集方，创建这些对象并循环读取缓冲区内容，从而在不依赖调试器的情况下无侵入地捕获目标进程的调试字符串。

关键技术点：
- **共享内存读取**：使用 Win32 `CreateFileMappingW` / `MapViewOfFile` 映射 `DBWIN_BUFFER`。
- **ANSI 解码**：`OutputDebugStringA` 写入的是系统当前 ANSI 代码页字节（中文系统通常为 GBK），通过 `MultiByteToWideChar(CP_ACP)` 正确转码为 UTF-8。
- **日志轮转**：复用 `tracing-appender` 的 `RollingFileAppender`，按天切分并自动清理过期文件。
- **进程名解析**：通过 `QueryFullProcessImageNameW` 查询 PID 对应的进程名，并使用有界 HashMap 缓存以避免高频系统调用。

---

## 项目结构

```
loghound/
├── Cargo.toml           # Rust 项目配置
├── loghound.toml        # 默认配置文件示例
├── scripts/
│   ├── install.bat      # 计划任务安装脚本（登录自启动）
│   ├── uninstall.bat    # 计划任务卸载脚本
│   └── run-hidden.vbs   # 后台隐藏运行辅助脚本
├── src/
│   ├── main.rs          # 程序入口：命令行解析与模式分发
│   ├── cli.rs           # 命令行参数定义（clap）
│   ├── config.rs        # TOML 配置加载、校验与默认值
│   ├── capture/
│   │   ├── mod.rs       # 捕获循环编排：读取 → 过滤 → 写入
│   │   ├── dbwin.rs     # DBWIN 共享内存 + 事件握手封装
│   │   └── decode.rs    # ANSI(GBK) 解码为 UTF-8
│   ├── filter.rs        # 进程名/PID 过滤逻辑
│   ├── logger.rs        # 日志文件写入（按天滚动+保留策略）
│   ├── model.rs         # 调试消息数据结构
│   ├── process.rs       # PID → 进程名解析 + 有界缓存
│   └── service.rs       # Windows 服务封装（install / uninstall / run）
└── ...
```

---

## 构建

### 环境要求

- [Rust](https://rustup.rs/) 1.85+ (Edition 2024)
- Windows SDK（用于 Win32 API 绑定）

### 编译

```bash
cargo build --release
```

编译产物位于 `target/release/loghound.exe`。

### 测试

```bash
cargo test
```

---

## 使用建议

1. **桌面应用日志采集**：直接在目标会话下用控制台模式运行 `loghound`，配合 `run-hidden.vbs` 隐藏窗口。
2. **纯后台服务日志采集**：若目标进程本身就是 Windows 服务（运行在 Session 0），可使用 `loghound.exe install` 注册系统服务。
3. **避免与 DbgView 同时运行**：`DBWIN` 机制只允许同一会话中**一个**采集方存在。如果 DbgView 或其它同类工具已运行，`loghound` 会报告冲突并退出。
4. **权限**：普通用户权限即可运行。安装 Windows 服务需要管理员权限。

---

## 许可证

[MIT](./LICENSE)

---

## 致谢

- 灵感来自 [Sysinternals DbgView](https://docs.microsoft.com/en-us/sysinternals/downloads/debugview)，致敬经典工具。
