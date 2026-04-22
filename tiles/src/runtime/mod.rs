#[allow(unused_imports)]
use crate::{core::storage::db::Dbconn, runtime::mlx::MLXRuntime};
use anyhow::Result;
pub mod mlx;

pub struct RunArgs {
    pub modelfile_path: Option<String>,
    pub relay_count: u32,
    pub memory: bool, // Future flags go here
    pub pi: bool,
}

pub enum Runtime {
    Mlx(MLXRuntime),
}

impl Runtime {
    pub async fn run(&self, run_args: RunArgs, db_conn: &Dbconn) -> Result<()> {
        match self {
            Runtime::Mlx(runtime) => runtime.run(run_args, db_conn).await,
        }
    }

    pub async fn start_server_daemon(&self) -> Result<()> {
        match self {
            Runtime::Mlx(runtime) => runtime.start_server_daemon().await,
        }
    }

    pub async fn stop_server_daemon(&self) -> Result<()> {
        match self {
            Runtime::Mlx(runtime) => runtime.stop_server_daemon().await,
        }
    }
}

#[cfg(target_os = "macos")]
pub fn build_runtime() -> Runtime {
    Runtime::Mlx(MLXRuntime::new())
}

// NOTE: We reuse MLXRuntime on Linux because the Rust runtime only
// manages the Python server process lifecycle.  The actual backend
// dispatch (MLX vs llama-cpp) happens in Python via main.py's
// get_backend(), which checks sys.platform at import time.
#[cfg(not(target_os = "macos"))]
pub fn build_runtime() -> Runtime {
    Runtime::Mlx(MLXRuntime::new())
}
