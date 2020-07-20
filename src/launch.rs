use crate::login::LoginToken;
use tokio::process::Command;
use crate::opt::Options;

pub async fn launch(options: &Options, token: LoginToken) {
    if let Err(err) = std::env::set_current_dir(&options.install_dir) {
        eprintln!("Failed to set working directory.\n{}", err);
        return;
    }
    #[cfg(target_os = "linux")]
    let mut command = Command::new("./TTREngine");

    #[cfg(target_os = "windows")]
    let mut command = Command::new("./TTREngine.exe");

    #[cfg(target_os = "macos")]
        let mut command = Command::new("./Toontown Rewritten");

    command.env("TTR_GAMESERVER", token.server);
    command.env("TTR_PLAYCOOKIE", token.cookie);
    match command.spawn() {
        Ok(handle) => {
            match handle.await {
                Ok(status) => {
                    if status.success() {
                        println!("TTREngine exited normally");
                    } else {
                        eprintln!("TTREngine executed abnormally! Exit code: {:?}", status.code());
                    }
                },
                Err(err) => {
                    eprintln!("TTREngine executed really abnormally!\n{}", err);
                }
            }
        },
        Err(err) => eprintln!("Failed to launch TTREngine!\n{}", err),
    }
}