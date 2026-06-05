//! 把 DBWIN 缓冲区里的 ANSI 字节解码为 UTF-8 字符串。
//!
//! `OutputDebugString` 写入缓冲区的始终是 ANSI（系统当前代码页，中文系统为 GBK），
//! 因此需要按 `CP_ACP` 解码，否则中文会乱码。

/// 解码一条原始消息：在 NUL 处截断 → 按系统 ANSI 代码页解码 → 去除尾部换行。
pub fn decode_message(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    let decoded = decode_ansi(&raw[..end]);
    decoded.trim_end_matches(['\r', '\n']).to_string()
}

/// 按系统 ANSI 代码页（CP_ACP）把字节解码为 UTF-8。
#[cfg(windows)]
fn decode_ansi(bytes: &[u8]) -> String {
    use windows::Win32::Globalization::{MULTI_BYTE_TO_WIDE_CHAR_FLAGS, MultiByteToWideChar};

    const CP_ACP: u32 = 0;
    if bytes.is_empty() {
        return String::new();
    }
    unsafe {
        // 先取所需的宽字符长度
        let needed = MultiByteToWideChar(CP_ACP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None);
        if needed <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut wide = vec![0u16; needed as usize];
        let written = MultiByteToWideChar(
            CP_ACP,
            MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
            bytes,
            Some(&mut wide),
        );
        if written <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        String::from_utf16_lossy(&wide[..written as usize])
    }
}

/// 非 Windows 平台的回退实现（仅用于跨平台编译与测试），按 UTF-8 宽松解码。
#[cfg(not(windows))]
fn decode_ansi(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_at_nul() {
        let raw = b"hello\0garbage";
        assert_eq!(decode_message(raw), "hello");
    }

    #[test]
    fn trims_trailing_newline() {
        assert_eq!(decode_message(b"line\r\n"), "line");
        assert_eq!(decode_message(b"line\n"), "line");
    }

    #[test]
    fn plain_ascii_roundtrips() {
        assert_eq!(decode_message(b"abc 123"), "abc 123");
    }

    #[test]
    fn empty_is_empty() {
        assert_eq!(decode_message(b""), "");
        assert_eq!(decode_message(b"\0"), "");
    }

    // GBK 中文解码依赖系统代码页，仅在 CP936（中文 GBK）Windows 上验证。
    #[cfg(windows)]
    #[test]
    fn decodes_gbk_chinese_on_windows() {
        use windows::Win32::Globalization::GetACP;
        if unsafe { GetACP() } != 936 {
            return; // 非中文代码页环境，跳过
        }
        let gbk = [0xD6u8, 0xD0, 0xCE, 0xC4]; // "中文" 的 GBK 编码
        assert_eq!(decode_message(&gbk), "中文");
    }
}
