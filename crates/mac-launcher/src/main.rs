
#[cfg(target_os = "macos")]
mod launcher;

fn main() {
    #[cfg(target_os = "macos")]
    launcher::launch();
}
