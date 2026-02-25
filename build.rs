use std::env;
use std::path::PathBuf;

fn sanitize_python_virtualenv() {
    let venv = env::var_os("VIRTUAL_ENV");

    if let Some(venv_path) = venv {
        let venv_bin = PathBuf::from(venv_path).join("bin");

        if let Some(path_value) = env::var_os("PATH") {
            let filtered_paths: Vec<PathBuf> = env::split_paths(&path_value)
                .filter(|entry| entry != &venv_bin)
                .collect();

            if let Ok(new_path) = env::join_paths(filtered_paths) {
                unsafe {
                    env::set_var("PATH", new_path);
                }
            }
        }
    }

    unsafe {
        env::remove_var("VIRTUAL_ENV");
        env::remove_var("CONDA_PREFIX");
    }
}

fn main() {
    sanitize_python_virtualenv();
    embuild::espidf::sysenv::output();
}
