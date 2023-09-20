use entrusted_l10n as l10n;
use std::collections::HashMap;
use std::env;
use once_cell::sync::OnceCell;

mod config;
mod model;
mod process;
mod server;
mod uil10n;

macro_rules! incl_gettext_files {
    ( $( $x:expr ),* ) => {
        {
            let mut ret = HashMap::with_capacity(2);
            $(
                let data = include_bytes!(concat!("../translations/", $x, "/LC_MESSAGES/messages.mo")).as_slice();
                ret.insert($x, data);

            )*

                ret
        }
    };
}

static INSTANCE_DEFAULT_HOST: OnceCell<String>  = OnceCell::new();
static INSTANCE_DEFAULT_PORT: OnceCell<String>  = OnceCell::new();
static INSTANCE_DEFAULT_IMAGE: OnceCell<String> = OnceCell::new();

fn default_host_to_str() -> &'static str {
    INSTANCE_DEFAULT_HOST.get().expect("Host value not set!")
}

fn default_port_to_str() -> &'static str {
    INSTANCE_DEFAULT_PORT.get().expect("Port value not set!")
}

fn default_container_image_to_str() -> &'static str {
    INSTANCE_DEFAULT_IMAGE.get().expect("Image value not set!")
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    l10n::load_translations(incl_gettext_files!("en", "fr"));

    let locale = match env::var(l10n::ENV_VAR_ENTRUSTED_LANGID) {
        Ok(selected_locale) => selected_locale,
        Err(_)              => l10n::sys_locale(),
    };
    let l10n = l10n::new_translations(locale);

    let help_host = l10n.gettext("Server host");
    let help_port = l10n.gettext("Server port");
    let help_container_image_name = l10n.gettext("Container image name");

    let appconfig: config::AppConfig = config::load_config()?;
    let port_number_text = format!("{}", appconfig.port);
    let app_version = option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown");
    
    INSTANCE_DEFAULT_HOST.set(appconfig.host.to_owned())?;
    INSTANCE_DEFAULT_PORT.set(port_number_text.to_owned())?;
    INSTANCE_DEFAULT_IMAGE.set(appconfig.container_image_name.to_owned())?;
    
    let cmd_help_template = l10n.gettext(&format!("{}\n{}\n{}\n\n{}\n\n{}\n{}",
                                                  "{bin} {version}",
                                                  "{author}",
                                                  "{about}",
                                                  "Usage: {usage}",
                                                  "Options:",
                                                  "{options}"));

    let app = clap::Command::new(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"))
        .version(app_version)
        .help_template(&cmd_help_template)
        .author(option_env!("CARGO_PKG_AUTHORS").unwrap_or("Unknown"))
        .about(option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"))
        .arg(
            clap::Arg::new("host")
                .long("host")
                .help(&help_host)
                .required(false)
                .default_value(default_host_to_str())
        )
        .arg(
            clap::Arg::new("port")
                .long("port")
                .help(&help_port)
                .required(false)
                .default_value(default_port_to_str())
        )
        .arg(
            clap::Arg::new("container-image-name")
                .long("container-image-name")
                .help(&help_container_image_name)
                .required(false)
                .default_value(default_container_image_to_str())
        );

    let run_matches = app.to_owned().get_matches();

    let ci_image_name = match run_matches.get_one::<String>("container-image-name") {
        Some(img_name) => img_name.to_owned(),
        _              => appconfig.container_image_name.clone(),
    };

    if let (Some(host), Some(port)) = (run_matches.get_one::<String>("host"), run_matches.get_one::<String>("port")) {
        if let Err(ex) = port.parse::<u16>() {
            return Err(format!(
                "{}: {}. {}",
                l10n.gettext("Invalid port number"),
                port,
                ex
            )
            .into());
        }

        if let Err(ex) = server::serve(host, port, ci_image_name, l10n.clone()).await {
            Err(ex)
        } else {
            Ok(())
        }
    } else {
        Err(l10n.gettext("Runtime error, missing parameters!").into())
    }
}
