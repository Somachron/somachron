use std::path::PathBuf;

// use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod media;
mod mq;
mod routes;
mod server;

// #[derive(Debug, Subcommand, Clone, Serialize, Deserialize)]
// #[clap(rename_all = "kebab_case")]
// enum MediaType {
//     Image {
//         path: PathBuf,
//     },
//     Video {
//         url: String,
//         tmp_path: PathBuf,
//     },
// }

// #[derive(Debug, Parser)]
// #[command(version, about)]
// struct Cli {
//     #[clap(subcommand)]
//     media: MediaType,

//     #[arg(short, long)]
//     rotation: Option<u64>,
// }

// // pub fn exit(self) {
// //     eprintln!("{} // [{}] - {}", self.message, self.at, self.err_msg);

// //     std::process::exit(1);
// // }

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn malloc_trim(__pad: libc::size_t) -> libc::c_int;
}

async fn run() {
    // initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env().add_directive(Level::INFO.into()))
        .with(tracing_subscriber::fmt::layer().with_thread_ids(true).json().flatten_event(true))
        .init();

    // load env
    dotenv::dotenv().ok();

    // serve app
    tokio::join!(server::serve(), async {
        tokio::runtime::Handle::current().spawn(async move {
            {
                #[cfg(target_os = "linux")]
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                    unsafe {
                        malloc_trim(0);
                    }
                }
            }
        });
    });

    tracing::info!("Server has stopped.");
}

fn main() {
    tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("Failed to build async rt").block_on(run())
}

// fn main() {
//     let cli = Cli::parse();

//     let result = match cli.media {
//         MediaType::Image {
//             path,
//         } => {
//             if !path.exists() {
//                 eprintln!("Provided path doesn't exist: {:?}", path);
//                 std::process::exit(1);
//             }
//             media::handle_image(path, cli.rotation)
//         }
//         MediaType::Video {
//             url,
//             tmp_path,
//         } => media::handle_video(url, tmp_path, cli.rotation),
//     };

//     match result {
//         Ok(data) => {
//             println!("{}", serde_json::to_string_pretty(&data).unwrap());
//         }
//         Err(err) => {
//             println!("{err:?}");
//         }
//     };
// }
