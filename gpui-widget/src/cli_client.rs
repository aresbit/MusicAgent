//! opencc CLI 客户端
//!
//! 使用 std::process::Command + smol::unblock 调用 opencc CLI 子进程，
//! 通过 -p/--print 管道模式发送一条消息并获取回复。
//!
//! CLI 路径: <project_root>/packages/opencc
//! 调用方式: bun run dist/cli.js -p "message"
//!
//! 注意: 不使用 tokio，因为 GPUI 运行在 smol 执行器上。
//!       改用 std::process::Command 在 smol::unblock 中同步执行。

use anyhow::{Context, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

// ─── Auto-load env vars from ~/.bashrc ─────────────────────────────────

/// Read ~/.bashrc and export all ANTHROPIC_* / DEEPSEEK_* variables.
/// Called once at module init so the CLI child process inherits them.
pub fn load_env_from_bashrc() {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());

    for filename in &[".bashrc", ".bash_profile", ".profile"] {
        let path = PathBuf::from(&home).join(filename);
        if !path.exists() {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                let line = line.trim();
                // match: export ANTHROPIC_FOO=value  or  export DEEPSEEK_FOO=value
                if let Some(rest) = line.strip_prefix("export ").or_else(|| {
                    // also support assignments without 'export' keyword
                    if line.contains('=') && !line.starts_with('#') { Some(line) } else { None }
                }) {
                    if let Some(eq) = rest.find('=') {
                        let key = rest[..eq].trim();
                        let val = rest[eq + 1..].trim().trim_matches('"').trim_matches('\'');
                        if (key.starts_with("ANTHROPIC_") || key.starts_with("DEEPSEEK_"))
                            && !val.is_empty()
                            && val.starts_with("sk-")
                        {
                            std::env::set_var(key, val);
                        }
                    }
                }
            }
        }
    }
}

/// opencc CLI 配置
#[derive(Debug, Clone)]
pub struct OpenCcConfig {
    /// opencc 项目根目录
    pub project_dir: PathBuf,
    /// CLI 入口文件（相对于 project_dir）
    pub cli_entry: PathBuf,
    /// 超时时间（秒）
    pub timeout_secs: u64,
}

impl Default for OpenCcConfig {
    fn default() -> Self {
        Self {
            project_dir: crate::paths::opencc_dir(),
            cli_entry: PathBuf::from(r"dist/cli.js"),
            timeout_secs: 120,
        }
    }
}

fn append_cli_log(line: &str) {
    let log_path = crate::paths::log_file("cli");
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let full = format!("[{}] {}\n", ts, line);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = f.write_all(full.as_bytes());
    }
}

/// 发送一条消息到 opencc CLI 并获取 AI 回复
///
/// 使用 `-p/--print` 管道模式：
///   bun run dist/cli.js --dangerously-skip-permissions -p "message"
///
/// 返回 AI 的文本回复，如果出错则返回错误信息。
pub async fn send_message(message: &str) -> String {
    // Auto-load env from shell config so ANTHROPIC_* vars are available
    load_env_from_bashrc();

    // Also try to derive ANTHROPIC_API_KEY from ANTHROPIC_AUTH_TOKEN if not set
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        if let Ok(token) = std::env::var("ANTHROPIC_AUTH_TOKEN") {
            std::env::set_var("ANTHROPIC_API_KEY", &token);
        }
    }

    send_message_with_config(message, &OpenCcConfig::default()).await
}

/// 带配置的发送消息（测试/自定义路径用）
pub async fn send_message_with_config(message: &str, config: &OpenCcConfig) -> String {
    let cli_path = config.project_dir.join(&config.cli_entry);

    // 检查 CLI 入口文件是否存在
    if !cli_path.exists() {
        return format!(
            "Error: CLI entry not found at {}",
            cli_path.display()
        );
    }

    // 检查 ANTHROPIC_API_KEY（这是 CLI 实际工作所需）
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        return "Error: ANTHROPIC_API_KEY environment variable is not set.\n\
                 The opencc CLI requires an Anthropic API key to function.\n\
                 Set it with: $env:ANTHROPIC_API_KEY='sk-ant-...'".into();
    }

    let result = run_cli_print_mode(message, config).await;

    match result {
        Ok(reply) => reply,
        Err(e) => format!("Error: {}", e),
    }
}

/// 通过一次一问模式调用 CLI
///
/// 使用 std::process::Command + smol::unblock 来避免 tokio runtime 依赖。
/// GPUI 运行在 smol 执行器上，tokio 的异步 API 会 panic。
async fn run_cli_print_mode(message: &str, config: &OpenCcConfig) -> Result<String> {
    let cli_path = config.project_dir.join(&config.cli_entry);
    let bat_path = config.project_dir.join("opencc.bat");
    let message = message.to_string();
    let project_dir = config.project_dir.clone();
    let timeout = Duration::from_secs(config.timeout_secs);
    let bat_exists = bat_path.exists();

    append_cli_log(&format!(
        "request start; cwd='{}' cli='{}' bat='{}' bat_exists={} msg_len={}",
        project_dir.display(),
        cli_path.display(),
        bat_path.display(),
        bat_exists,
        message.len()
    ));

    // 在 smol 线程池中同步执行阻塞的进程调用
    let output = smol::unblock(move || {
        let mut cmd = if bat_exists {
            // Use opencc.bat so Windows GUI launch does not depend on shell aliases.
            let mut c = Command::new("cmd");
            c.arg("/C")
                .arg(bat_path.to_str().context("Invalid opencc.bat path")?)
                .arg("-p")
                .arg(&message);
            #[cfg(windows)]
            { c.creation_flags(0x08000000); }
            c
        } else {
            let mut c = Command::new("bun");
            c.arg("run")
                .arg(cli_path.to_str().context("Invalid CLI path")?)
                .arg("--dangerously-skip-permissions")
                .arg("-p")
                .arg(&message);
            c
        };

        let mut child = cmd
            .current_dir(&project_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("CLAUDE_CODE_SIMPLE", "1")
            .spawn()
            .with_context(|| {
                format!(
                    "Failed to spawn CLI. cwd='{}', bat='{}', cli='{}'",
                    project_dir.display(),
                    bat_path.display(),
                    cli_path.display()
                )
            })?;

        let start = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    // 进程已退出，收集输出
                    return child
                        .wait_with_output()
                        .context("Failed to collect CLI output");
                }
                Ok(None) => {
                    if start.elapsed() > timeout {
                        let _ = child.kill();
                        anyhow::bail!("CLI timed out after {} seconds", timeout.as_secs());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    anyhow::bail!("CLI process error: {}", e);
                }
            }
        }
    })
    .await?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    append_cli_log(&format!(
        "request end; exit={:?} stdout_len={} stderr_len={} stderr_head='{}'",
        output.status.code(),
        stdout.len(),
        stderr.len(),
        stderr.chars().take(240).collect::<String>().replace('\n', "\\n")
    ));

    // 合并 stdout 和 stderr
    let combined = format!("{}\n{}", stdout, stderr);
    let trimmed = combined.trim();

    if trimmed.is_empty() {
        Ok("(no response from AI)".into())
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OpenCcConfig::default();
        assert!(config.cli_entry.to_str().unwrap().contains("cli.js"));
    }

    #[test]
    fn test_version_check() {
        let config = OpenCcConfig::default();
        let cli_path = config.project_dir.join(&config.cli_entry);
        assert!(cli_path.exists(), "CLI entry not found");
    }
}
