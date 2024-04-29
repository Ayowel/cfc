/// Check whether the current application is running
/// from within a container
/// 
/// # Examples
/// 
/// ```rust
/// use cfc::utils::is_docker_env;
/// if is_docker_env() {
///     println!("Running from a container");
/// }
/// ```
pub fn is_docker_env() -> bool {
    std::fs::metadata("/.dockerenv").is_ok()
}
