use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod app;
mod routes;
mod server;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

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
            #[cfg(target_os = "linux")]
            {
                while true {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                    unsafe {
                        libc::malloc_trim(0);
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
