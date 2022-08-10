use clap;
use std::collections::HashMap;
use std::env;
use entrusted_l10n as l10n;

mod uil10n;
mod model;
mod config;
mod server;
mod process;

macro_rules! incl_gettext_files {
    ( $( $x:expr ),* ) => {
        {
            let mut ret = HashMap::new();
            $(
                let data = include_bytes!(concat!("../translations/", $x, "/LC_MESSAGES/messages.mo")).as_slice();
                ret.insert($x, data);

            )*

                ret
        }
    };
}

#[actix_web::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    l10n::load_translations(incl_gettext_files!("en", "fr"));
    
    let locale = match env::var(l10n::ENV_VAR_ENTRUSTED_LANGID) {
        Ok(selected_locale) => selected_locale,
        Err(_) => l10n::sys_locale()
    };
    let l10n = l10n::new_translations(locale);

    let help_host = l10n.gettext("Server host");
    let help_port = l10n.gettext("Server port");
    let help_container_image_name = l10n.gettext("Container image name");

    let appconfig: config::AppConfig = config::load_config()?;
    let port_number_text = format!("{}", appconfig.port);
    let app_version = option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown");

    let app = clap::App::new(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"))
        .version(app_version)
        .author(option_env!("CARGO_PKG_AUTHORS").unwrap_or("Unknown"))
        .about(option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"))
        .arg(
            clap::Arg::with_name("host")
                .long("host")
                .help(&help_host)
                .required(true)
                .default_value(&appconfig.host)
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("port")
                .long("port")
                .help(&help_port)
                .required(true)
                .default_value(&port_number_text)
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("container-image-name")
                .long("container-image-name")
                .help(&help_container_image_name)
                .required(true)
                .default_value(&appconfig.container_image_name)
                .takes_value(true));

    let run_matches = app.to_owned().get_matches();

    let ci_image_name = match run_matches.value_of("container-image-name") {
        Some(img_name) => img_name.to_string(),
        _              => appconfig.container_image_name.clone()
    };

    if let (Some(host), Some(port)) = (run_matches.value_of("host"), run_matches.value_of("port")) {
        if let Err(ex) = port.parse::<u16>() {
            return Err(format!("{}: {}. {}", l10n.gettext("Invalid port number"), port, ex.to_string()).into());
        }

        match server::serve(host, port, ci_image_name, l10n.clone()).await {
            Ok(_)   => Ok(()),
            Err(ex) => Err(ex.into()),
        }
    } else {
        Err(l10n.gettext("Runtime error, missing parameters!").into())
    }
}
