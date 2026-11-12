#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    hghubx_lib::web::run().await
}
