use tokio::io::{BufReader, AsyncBufReadExt};

mod opt;
mod update;
mod login;
mod launch;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = opt::get_options();
    if !opts.no_update {
        // TODO real error handling
        if let Err(err) = update::update(&opts).await {
            eprintln!("Failed to update!\n{}", err);
            return Ok(());
        }
    }

    let (username, password) = if opts.pass_stdin {
        let username = opts.username.as_ref().unwrap().as_str();
        let mut bufreader = BufReader::new(tokio::io::stdin());
        let mut password = String::new();
        bufreader.read_line(&mut password).await?;
        let pass = password.trim();
        (username.to_string(), Some(pass.to_string()))
    } else {
        let username = if opts.username.is_none() {
            let u = rprompt::prompt_reply_stdout("Username: ")?;
            u.trim().to_string()
        } else {
            opts.username.as_ref().unwrap().clone()
        };
        println!("Logging in on {}", &username);
        let pass: Option<String> = if opts.keyring && !opts.reset_keyring {
            None
        } else {
            let pass = rpassword::read_password_from_tty(Some("Password: "))?;
            Some(pass)
        };

        (username.to_string(), pass)
    };

    if opts.reset_keyring {
        login::reset_keyring(username.as_str());
    }
    let save_password = password.is_some() && opts.keyring;
    match login::login(username, password, save_password).await {
        Some(login_cookie) => {
            println!("Logged in successfully! {}", &login_cookie.server);
            launch::launch(&opts, login_cookie).await
        },
        None => {
            println!("Failed to log in.");
        }
    }
    Ok(())
}


