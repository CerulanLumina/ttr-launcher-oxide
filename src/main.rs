#![deny(unreachable_code, unreachable_patterns, unused_assignments, unused_must_use, unused_extern_crates)]
#![warn(unused_qualifications, unused_import_braces)]

use tokio::io::{AsyncBufReadExt, BufReader};

mod launch;
mod login;
mod opt;
mod update;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Update

    let opts = opt::get_options();
    if !opts.no_update {
        if let Err(err) = update::update(&opts).await {
            eprintln!("Failed to update!\n{}", err);
            return Ok(());
        }
    }

    // Get Username / pass
    let (username, password) = if opts.pass_stdin {
        // If the password is passed from stdin, read it
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
        // Retrieve password from keyring or tty
        let pass: Option<String> = if opts.keyring && !opts.reset_keyring {
            None
        } else {
            let pass = rpassword::read_password_from_tty(Some("Password: "))?;
            Some(pass)
        };

        (username.to_string(), pass)
    };

    // If user requested keyring reset, do that
    if opts.reset_keyring {
        login::reset_keyring(username.as_str());
    }

    // Login
    let save_password = password.is_some() && opts.keyring;
    match login::login(username, password, save_password).await {
        Some(login_cookie) => {
            println!("Logged in successfully! {}", &login_cookie.server);
            // Launch
            if !opts.manual {
                launch::launch(&opts, login_cookie).await;
            } else {
                println!(
                    "TTR_GAMESERVER={}\nTTR_PLAYCOOKIE={}",
                    login_cookie.server, login_cookie.cookie
                );
            }
        }
        None => {
            println!("Failed to log in.");
        }
    }

    Ok(())
}
