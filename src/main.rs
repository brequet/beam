mod input;
mod ui;

use std::net::{IpAddr, UdpSocket};
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use tokio::net::TcpListener;
use topcoat::asset::{AssetBundle, RouterBuilderAssetExt};
use topcoat::router::Router;
use topcoat::runtime::RouterBuilderProcedureExt;

use crate::input::{InputService, MockInput, OsInput};
use crate::ui::HostInfo;

/// beam — send text and special keys from any device on the local network
/// to the host machine's focused window.
#[derive(Parser, Debug)]
#[command(name = "beam", version, about)]
struct Args {
    /// Address to bind the web server to.
    #[arg(long, env = "BEAM_HOST", default_value = "0.0.0.0")]
    host: String,

    /// Port to bind the web server to.
    #[arg(long, env = "BEAM_PORT", default_value_t = 5000)]
    port: u16,

    /// Log input events instead of injecting real keystrokes (for development).
    #[arg(long)]
    mock: bool,
}

/// Best-effort LAN address detection: a UDP "connect" picks the outbound
/// interface without sending any packets.
fn detect_lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip())
}

/// Renders `text` as a scannable QR code built from half-block characters.
///
/// The colors are swapped (dark modules render as blanks) so the code shows
/// up dark-on-light on the usual dark terminal theme; scanners decode the
/// inverse just as well on light themes.
fn qr_text(text: &str) -> String {
    use qrcode::render::unicode;
    qrcode::QrCode::new(text.as_bytes())
        .expect("QR payload fits the code")
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .build()
        .to_string()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let input: Arc<dyn InputService> = if args.mock {
        println!("mock input backend active: events are logged, not injected");
        Arc::new(MockInput::default())
    } else {
        Arc::new(OsInput::new().context("initializing the OS input backend")?)
    };

    let lan_ip = detect_lan_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "localhost".to_owned());

    let url = format!("http://{lan_ip}:{}", args.port);

    let router = Router::builder()
        .page(ui::home)
        .procedure(ui::send_text)
        .procedure(ui::press_key)
        .app_context(input)
        .app_context(HostInfo { url: url.clone() })
        .assets(AssetBundle::load().context(
            "asset bundle not found next to the executable; run `topcoat asset bundle` (or use `topcoat dev`)",
        )?)
        .build();

    let listener = TcpListener::bind((args.host.as_str(), args.port))
        .await
        .with_context(|| format!("binding {}:{} failed", args.host, args.port))?;

    println!(
        "beam is up: {lan_ip}:{} (bound to {}:{})",
        args.port, args.host, args.port
    );
    println!("pair a phone by scanning:\n{}", qr_text(&url));

    topcoat::serve(listener, router)
        .await
        .context("serving beam")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_renders_a_multiline_block_graphic() {
        let qr = qr_text("http://192.168.1.5:5000");
        assert!(qr.lines().count() > 10, "expected many rows, got {qr:?}");
        assert!(qr.chars().any(|c| c != ' ' && c != '\n'), "expected marks");
    }
}
