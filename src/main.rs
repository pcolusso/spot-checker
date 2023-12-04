use std::fmt::Debug;
use std::time::{Duration, Instant};
use tokio::net::ToSocketAddrs;

use anyhow::Result;
use fantoccini::ClientBuilder;
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;

async fn check_timeout<A: ToSocketAddrs + Debug>(addr: &A) -> bool {
    let start_time = Instant::now();
    let timeout = Duration::from_secs(30); // 30 seconds timeout
    let mut is_connected = false;

    while Instant::now().duration_since(start_time) < timeout {
        match TcpStream::connect(addr).await {
            Ok(_) => {
                is_connected = true;
                break;
            }
            Err(_) => {
                // Sleep for a bit before trying again
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
    is_connected
}

fn compare_image(a: &[u8], b: &[u8], threshold: usize) -> Result<bool> {
    // Load the images
    let img1 = image::load_from_memory_with_format(a, image::ImageFormat::Png)?
        .to_rgba8();
    let img2 = image::load_from_memory_with_format(b, image::ImageFormat::Png)?
        .to_rgba8();

    // Compare pixels
    let mut diff_count = 0;
    for (x, y, pixel1) in img1.enumerate_pixels() {
        let pixel2 = img2.get_pixel(x, y);
        if pixel1 != pixel2 {
            diff_count += 1;
        }

        if diff_count > threshold {
            return Ok(false);
        }
    }

    Ok(true)
}

async fn check(endpoint: &str) -> Result<()> {
    // Find a free port, and use that
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    drop(listener); // Free the port we got.
                    // Start our own Chromedriver process.
    let mut child = Command::new("geckodriver")
        .arg("--port")
        .arg(addr.port().to_string())
        .spawn()?;
    // Load the specified page.
    let chromedriver_url = format!("http://{}", addr.to_string());
    if check_timeout(&addr).await == false {
        anyhow::bail!("Chromedriver not up!");
    }
    let client = ClientBuilder::native().connect(&chromedriver_url).await?;
    client.goto(endpoint).await?;
    let url = client.current_url().await?;
    assert_eq!(url.as_ref(), endpoint);
    // Compare the image.
    let _image = client.screenshot().await?;

    // Terminate our process.
    child.kill().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut set = tokio::task::JoinSet::new();

    for _ in 0..10 {
        let t = tokio::task::spawn(async move { check("http://localhost:3000").await });
        set.spawn(t);
    }

    while let Some(res) = set.join_next().await {
        let out = res?;
        println!("{:?}", out);
    }
    Ok(())
}
