use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt()]
pub struct Options {
    /// Sets the installation directory for Toontown Rewritten
    #[structopt(long, env, parse(from_os_str), default_value = unsafe { INSTALL_DIR.as_str() })]
    pub install_dir: PathBuf,

    /// Disables updating, will try to login without doing so.
    #[structopt(long, short = "d")]
    pub no_update: bool,

    /// Pass passwords via stdin. If this is set, you must use --username to specify a username.
    #[structopt(long, short = "s")]
    pub pass_stdin: bool,

    /// Specifies a username
    #[structopt(long, short, required_if("pass_stdin", "true"))]
    pub username: Option<String>,

    /// If enabled, the system keyring will be used to save and remember passwords.
    #[structopt(long, short)]
    pub keyring: bool,

    /// Forgets any password held in the keyring.
    #[structopt(long)]
    pub reset_keyring: bool,

    /// Dumps the cookie and game server to stdout for manual launching
    #[structopt(long)]
    pub manual: bool,
}

static mut INSTALL_DIR: String = String::new();

fn install_dir() -> String {
    match dirs::data_dir() {
        Some(data) => data.join("toontown-rewritten").to_str().unwrap().to_string(),
        None => {
            eprintln!("Unsupported OS. You will have to use the --install-dir option.");
            String::new()
        }
    }
}

fn setup() {
    unsafe { INSTALL_DIR = install_dir() }; // Safe b/c it is initialized right at the start and never again.
}

pub fn get_options() -> Options {
    setup();
    Options::from_args()
}
