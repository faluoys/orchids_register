use std::process::ExitCode;

use orchids_core::workflow;

fn main() -> ExitCode {
    match workflow::run() {
        Ok(code) => ExitCode::from(code as u8),
        Err(err) => {
            let code = err.exit_code() as u8;
            if code == 1 {
                eprintln!("执行失败: {}", err);
            } else {
                eprintln!("{}", err);
            }
            ExitCode::from(code)
        }
    }
}
