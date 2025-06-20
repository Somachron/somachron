async fn run() {
    dotenv::dotenv().ok();
    println!("Hello, world!");
}

fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build async rt")
        .block_on(run())
}
