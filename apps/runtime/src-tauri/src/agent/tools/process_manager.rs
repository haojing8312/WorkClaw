use crate::windows_process::hide_console_window;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// 后台进程的输出快照
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub stdout: String,
    pub stderr: String,
    pub exited: bool,
    pub exit_code: Option<i32>,
    pub output_file_path: PathBuf,
    pub output_file_size: u64,
}

/// 后台进程启动结果。
#[derive(Debug, Clone)]
pub struct ProcessHandle {
    pub id: String,
    pub output_file_path: PathBuf,
}

/// 后台进程完成通知。
#[derive(Debug, Clone)]
pub struct ProcessCompletion {
    pub process_id: String,
    pub command: String,
    pub exit_code: Option<i32>,
    pub output_file_path: PathBuf,
    pub output_file_size: u64,
}

pub type ProcessCompletionNotifier = Arc<dyn Fn(ProcessCompletion) + Send + Sync + 'static>;

/// 单个后台进程的内部状态
struct BackgroundProcess {
    /// 启动时的命令（用于 list 展示）
    command: String,
    /// 操作系统层面的进程 ID，用于 kill
    pid: u32,
    /// stdout 缓冲区（后台线程持续追加）
    stdout_buf: Arc<Mutex<Vec<String>>>,
    /// stderr 缓冲区（后台线程持续追加）
    stderr_buf: Arc<Mutex<Vec<String>>>,
    /// 进程退出状态（None = 仍在运行）
    exit_status: Arc<Mutex<Option<i32>>>,
    /// 持久化输出文件路径
    output_file_path: PathBuf,
}

/// 每个缓冲区最多保留的行数
const MAX_BUFFER_LINES: usize = 5000;

/// 最多保留的已完成进程数量
const MAX_COMPLETED_PROCESSES: usize = 30;

/// 后台进程管理器，管理所有 spawn 出来的后台 shell 进程
pub struct ProcessManager {
    processes: Arc<Mutex<HashMap<String, BackgroundProcess>>>,
    completion_notifier: Option<ProcessCompletionNotifier>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
            completion_notifier: None,
        }
    }

    pub fn with_completion_notifier(notifier: ProcessCompletionNotifier) -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
            completion_notifier: Some(notifier),
        }
    }

    /// 获取平台对应的 shell 和参数标志
    #[cfg(target_os = "windows")]
    fn get_shell() -> (&'static str, &'static str) {
        ("cmd", "/C")
    }

    #[cfg(not(target_os = "windows"))]
    fn get_shell() -> (&'static str, &'static str) {
        ("bash", "-c")
    }

    /// 启动一个后台进程，返回 process_id（UUID 前 8 位）
    pub fn spawn(&self, command: &str, work_dir: Option<&Path>) -> Result<String> {
        Ok(self.spawn_handle(command, work_dir)?.id)
    }

    pub fn spawn_handle(&self, command: &str, work_dir: Option<&Path>) -> Result<ProcessHandle> {
        let (shell, flag) = Self::get_shell();
        self.spawn_with_shell_handle(command, work_dir, shell, &[flag])
    }

    pub fn spawn_with_shell(
        &self,
        command: &str,
        work_dir: Option<&Path>,
        shell: &str,
        shell_args: &[&str],
    ) -> Result<String> {
        Ok(self
            .spawn_with_shell_handle(command, work_dir, shell, shell_args)?
            .id)
    }

    pub fn spawn_with_shell_handle(
        &self,
        command: &str,
        work_dir: Option<&Path>,
        shell: &str,
        shell_args: &[&str],
    ) -> Result<ProcessHandle> {
        let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let output_file_path = background_output_file_path(&id)?;
        let output_file = Arc::new(Mutex::new(open_background_output_file(&output_file_path)?));
        let command_for_completion = command.to_string();
        let process_id_for_completion = id.clone();
        let output_file_path_for_completion = output_file_path.clone();
        let completion_notifier = self.completion_notifier.clone();

        let mut cmd = Command::new(shell);
        cmd.args(shell_args)
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(wd) = work_dir {
            cmd.current_dir(wd);
        }

        hide_console_window(&mut cmd);
        let mut child = cmd.spawn()?;
        let pid = child.id();

        let stdout_buf: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let stderr_buf: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let exit_status: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));

        // 在后台线程中读取 stdout
        let stdout_pipe = child.stdout.take();
        let stdout_buf_clone = Arc::clone(&stdout_buf);
        let stdout_file = Arc::clone(&output_file);
        thread::spawn(move || {
            if let Some(pipe) = stdout_pipe {
                let reader = BufReader::new(pipe);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            write_background_output_line(&stdout_file, "stdout", &l);
                            let mut buf = stdout_buf_clone.lock().unwrap();
                            buf.push(l);
                            // 超过上限时截断前面的行
                            if buf.len() > MAX_BUFFER_LINES {
                                let drain_count = buf.len() - MAX_BUFFER_LINES;
                                buf.drain(..drain_count);
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        });

        // 在后台线程中读取 stderr
        let stderr_pipe = child.stderr.take();
        let stderr_buf_clone = Arc::clone(&stderr_buf);
        let stderr_file = Arc::clone(&output_file);
        thread::spawn(move || {
            if let Some(pipe) = stderr_pipe {
                let reader = BufReader::new(pipe);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            write_background_output_line(&stderr_file, "stderr", &l);
                            let mut buf = stderr_buf_clone.lock().unwrap();
                            buf.push(l);
                            if buf.len() > MAX_BUFFER_LINES {
                                let drain_count = buf.len() - MAX_BUFFER_LINES;
                                buf.drain(..drain_count);
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        });

        // 在后台线程中等待子进程退出，更新退出状态
        // 注意：child 的所有权移入此线程，不需要 Mutex
        let exit_status_clone = Arc::clone(&exit_status);
        thread::spawn(move || match child.wait() {
            Ok(status) => {
                let exit_code = Some(status.code().unwrap_or(-1));
                *exit_status_clone.lock().unwrap() = exit_code;
                notify_process_completion(
                    completion_notifier.as_ref(),
                    ProcessCompletion {
                        process_id: process_id_for_completion,
                        command: command_for_completion,
                        exit_code,
                        output_file_size: fs::metadata(&output_file_path_for_completion)
                            .map(|metadata| metadata.len())
                            .unwrap_or(0),
                        output_file_path: output_file_path_for_completion,
                    },
                );
            }
            Err(_) => {
                let exit_code = Some(-1);
                *exit_status_clone.lock().unwrap() = exit_code;
                notify_process_completion(
                    completion_notifier.as_ref(),
                    ProcessCompletion {
                        process_id: process_id_for_completion,
                        command: command_for_completion,
                        exit_code,
                        output_file_size: fs::metadata(&output_file_path_for_completion)
                            .map(|metadata| metadata.len())
                            .unwrap_or(0),
                        output_file_path: output_file_path_for_completion,
                    },
                );
            }
        });

        let bg_process = BackgroundProcess {
            command: command.to_string(),
            pid,
            stdout_buf,
            stderr_buf,
            exit_status,
            output_file_path: output_file_path.clone(),
        };

        self.processes
            .lock()
            .unwrap()
            .insert(id.clone(), bg_process);
        Ok(ProcessHandle {
            id,
            output_file_path,
        })
    }

    /// 获取指定进程的输出
    ///
    /// - `block=true` 时轮询等待直到进程退出
    /// - `block=false` 时立即返回当前可用输出
    pub fn get_output(&self, id: &str, block: bool) -> Result<ProcessOutput> {
        // 先检查进程是否存在，并获取需要的 Arc 引用
        let exit_status_arc;
        let stdout_buf_arc;
        let stderr_buf_arc;
        let output_file_path;
        {
            let procs = self.processes.lock().unwrap();
            let proc = procs.get(id).ok_or_else(|| anyhow!("进程 {} 不存在", id))?;
            exit_status_arc = Arc::clone(&proc.exit_status);
            stdout_buf_arc = Arc::clone(&proc.stdout_buf);
            stderr_buf_arc = Arc::clone(&proc.stderr_buf);
            output_file_path = proc.output_file_path.clone();
        }

        if block {
            // 轮询等待进程退出（不持有 processes 锁）
            loop {
                let exited = exit_status_arc.lock().unwrap().is_some();
                if exited {
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        }

        let stdout = stdout_buf_arc.lock().unwrap().join("\n");
        let stderr = stderr_buf_arc.lock().unwrap().join("\n");
        let exit_status = *exit_status_arc.lock().unwrap();
        let exited = exit_status.is_some();
        let output_file_size = fs::metadata(&output_file_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        Ok(ProcessOutput {
            stdout,
            stderr,
            exited,
            exit_code: exit_status,
            output_file_path,
            output_file_size,
        })
    }

    /// 终止指定进程
    ///
    /// 通过操作系统 PID 终止。Windows 上使用 taskkill /T /F 终止进程树，
    /// Unix 上使用 kill 信号。
    pub fn kill(&self, id: &str) -> Result<()> {
        let pid;
        {
            let procs = self.processes.lock().unwrap();
            let proc = procs.get(id).ok_or_else(|| anyhow!("进程 {} 不存在", id))?;
            pid = proc.pid;
        }

        // 通过 PID 直接杀掉进程，无需持有任何锁
        Self::kill_process_by_pid(pid)?;
        Ok(())
    }

    /// 通过 PID 终止进程（跨平台实现）
    #[cfg(target_os = "windows")]
    fn kill_process_by_pid(pid: u32) -> Result<()> {
        // Windows 上使用 taskkill /T 终止整个进程树
        let mut command = Command::new("taskkill");
        command.args(["/T", "/F", "/PID", &pid.to_string()]);
        hide_console_window(&mut command);
        let output = command.output();
        match output {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!("终止进程 {} 失败: {}", pid, e)),
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn kill_process_by_pid(pid: u32) -> Result<()> {
        // Unix 上使用 kill -9 终止进程
        let output = Command::new("kill").args(["-9", &pid.to_string()]).output();
        match output {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!("终止进程 {} 失败: {}", pid, e)),
        }
    }

    /// 列出所有进程: (id, command, exited)
    pub fn list(&self) -> Vec<(String, String, bool)> {
        let procs = self.processes.lock().unwrap();
        procs
            .iter()
            .map(|(id, proc)| {
                let exited = proc.exit_status.lock().unwrap().is_some();
                (id.clone(), proc.command.clone(), exited)
            })
            .collect()
    }

    /// 清理已完成的旧进程，只保留最近 MAX_COMPLETED_PROCESSES 个
    pub fn cleanup(&self) {
        let mut procs = self.processes.lock().unwrap();
        let completed: Vec<String> = procs
            .iter()
            .filter(|(_, p)| p.exit_status.lock().unwrap().is_some())
            .map(|(id, _)| id.clone())
            .collect();

        if completed.len() > MAX_COMPLETED_PROCESSES {
            // 移除最早的（超出上限的部分）
            let to_remove = completed.len() - MAX_COMPLETED_PROCESSES;
            for id in completed.iter().take(to_remove) {
                procs.remove(id);
            }
        }
    }
}

fn background_output_file_path(id: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("workclaw-background-processes");
    fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{id}.log")))
}

fn open_background_output_file(path: &Path) -> Result<File> {
    Ok(OpenOptions::new().create(true).append(true).open(path)?)
}

fn write_background_output_line(output_file: &Arc<Mutex<File>>, stream: &str, line: &str) {
    if let Ok(mut file) = output_file.lock() {
        let _ = writeln!(file, "{stream}: {line}");
        let _ = file.flush();
    }
}

fn notify_process_completion(
    notifier: Option<&ProcessCompletionNotifier>,
    completion: ProcessCompletion,
) {
    if let Some(notifier) = notifier {
        notifier(completion);
    }
}
