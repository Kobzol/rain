use std::fs::File;
use std::sync::Arc;
use std::process::{Command, Stdio};
use tokio_process::CommandExt;
use futures::Future;
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::path::Path;
use std::io::Read;

use super::{TaskInstanceRef, TaskResult};
use worker::state::State;
use worker::data::{Data};
use errors::{Result};

fn read_stderr(path: &Path) -> Result<String> {
    // TODO: If the file is too big, truncate the beginning
    let mut file = File::open(path)?;
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    Ok(s)
}

pub fn task_run(instance_ref: TaskInstanceRef, state: &State) -> TaskResult {
    let (dir, future, stderr_path) = {
        let instance = instance_ref.get();
        let config = &instance.task.get().task_config;
        let mut cursor = ::std::io::Cursor::new(&config);

        let reader = ::capnp::serialize_packed::read_message(
            &mut cursor,
            ::capnp::message::ReaderOptions::new(),
        )?;

        let run_config = reader.get_root::<::tasks_capnp::run_task::Reader>()?;

        // Parse arguments
        let rargs: ::std::result::Result<Vec<_>, ::capnp::Error> =
            run_config.get_args()?.iter().collect();
        let args = rargs?;
        let name = args.get(0).ok_or_else(|| "Arguments are empty")?;
        let task = instance.task.get();

        let dir = state.work_dir().make_task_temp_dir(task.id)?;

        // Map inputs
        let mut in_io = Stdio::null();

        for (path, input) in run_config.get_input_paths()?.iter().zip(&task.inputs) {
            let path = path?;
            let obj = input.object.get();
            obj.data().map_to_path(&dir.path().join(path))?;
            if path == "+in" {
                let in_id = File::open(dir.path().join("+in"))?.into_raw_fd();
                in_io = unsafe { Stdio::from_raw_fd(in_id) };
            }
        }

        // Create files for stdout/stderr
        let out_id = File::create(dir.path().join("+out"))
            .expect("File for stdout cannot be opened")
            .into_raw_fd();
        let stderr_path = dir.path().join("+err");
        let err_id = File::create(&stderr_path)
            .expect("File for stderr cannot be opened")
            .into_raw_fd();

        let out_io = unsafe { Stdio::from_raw_fd(out_id) };
        let err_io = unsafe { Stdio::from_raw_fd(err_id) };

        debug!("Starting command: {}", name);

        let future = Command::new(&name)
            .args(&args[1..])
            .stdin(in_io)
            .stdout(out_io)
            .stderr(err_io)
            .current_dir(dir.path())
            .status_async2(state.handle())?;

        (dir, future, stderr_path)
    };

    Ok(Box::new(
        future.map_err(|e| e.into()).and_then(move |status| {
            if !status.success() {
                let stderr = match read_stderr(&stderr_path) {
                    Ok(s) => format!("Stderr: {}\n", s),
                    Err(e) => format!("Stderr could not be obtained: {}",
                                      ::std::error::Error::description(&e))
                };
                match status.code() {
                    Some(code) => bail!("Program exit with exit code {}\n{}", code, stderr),
                    None => bail!("Program terminated by signal\n{}", stderr),
                }
            }
            {
                let instance = instance_ref.get();
                let state = instance.state.get();
                let config = &instance.task.get().task_config;
                let task = instance.task.get();

                let mut cursor = ::std::io::Cursor::new(&config);
                let reader = ::capnp::serialize_packed::read_message(
                    &mut cursor,
                    ::capnp::message::ReaderOptions::new(),
                )?;
                let run_config = reader.get_root::<::tasks_capnp::run_task::Reader>()?;

                for (path, dataobj) in run_config.get_output_paths()?.iter().zip(&task.outputs) {
                    let path = dir.path().join(path?);
                    if !path.is_file() {
                        bail!("Output '{}' not found");
                    }
                    let target_path = state.work_dir().new_path_for_dataobject();
                    let data = Data::new_by_fs_move(&path, target_path)?;
                    let mut obj = dataobj.get_mut();
                    obj.set_data(Arc::new(data));
                }
            }
            Ok(())
        }),
    ))
}
