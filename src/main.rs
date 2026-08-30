mod autostart;
mod input;
mod ui;

use std::net::{IpAddr, UdpSocket};
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use topcoat::asset::{AssetBundle, RouterBuilderAssetExt};
use topcoat::context::Cx;
use topcoat::router::{
    Body, HeaderValue, Method, Methods, Path, PathBuf, Route, RouteFuture, RouteId, Router, header,
    response::Response,
};
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

    /// Hide the console window (used by the autostart task).
    #[arg(long)]
    hidden: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Register a logon task so beam starts automatically at sign-in.
    ///
    /// Flags passed here are frozen into the task; re-run `beam install`
    /// after changing them. Idempotent.
    Install {
        /// Address to bind the autostart server to (default: 0.0.0.0).
        #[arg(long)]
        host: Option<String>,

        /// Port to bind the autostart server to (default: 5000).
        #[arg(long)]
        port: Option<u16>,

        /// Log input events instead of injecting real keystrokes.
        #[arg(long)]
        mock: bool,
    },
    /// Remove the logon task (and stop a task-started beam).
    Uninstall,
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

/// A `Route` serving one immutable file embedded in the binary.
///
/// Used for the small static files beam needs at fixed paths (PWA manifest,
/// icons) so they ship inside the single executable instead of riding the
/// asset bundle.
struct StaticRoute {
    id: RouteId,
    path: PathBuf,
    body: &'static [u8],
    content_type: &'static str,
}

impl StaticRoute {
    fn new(path: &'static str, body: &'static [u8], content_type: &'static str) -> Self {
        Self {
            id: RouteId::new(),
            path: Path::new(path).to_owned(),
            body,
            content_type,
        }
    }
}

impl Route for StaticRoute {
    fn id(&self) -> RouteId {
        self.id
    }

    fn methods(&self) -> Methods<'_> {
        Methods::Only(&[Method::GET])
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn handle<'cx>(&'cx self, _cx: &'cx Cx, _body: Body) -> RouteFuture<'cx> {
        Box::pin(async move {
            let mut response = Response::new(Body::from(self.body));
            let headers = response.headers_mut();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(self.content_type),
            );
            // Small files at fixed paths: revalidate cheaply instead of
            // long-lived caching, so an update reaches devices quickly.
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("max-age=300"),
            );
            Ok(response)
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let result = match &args.command {
        Some(Command::Install { host, port, mock }) => {
            autostart::install(&autostart::InstallOptions {
                host: host.clone(),
                port: *port,
                mock: *mock,
            })
        }
        Some(Command::Uninstall) => autostart::uninstall(),
        None => serve(&args).await,
    };

    if args.hidden
        && let Err(err) = &result
    {
        autostart::log_status(&format!("beam exited: {err:#}"));
    }
    result
}

async fn serve(args: &Args) -> anyhow::Result<()> {
    if args.hidden {
        autostart::hide_console();
    }

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
        .procedure(ui::open_url)
        .route(StaticRoute::new(
            "/beam.css",
            include_bytes!("../assets/beam.css"),
            "text/css",
        ))
        .route(StaticRoute::new(
            "/manifest.webmanifest",
            include_bytes!("../assets/manifest.json"),
            "application/manifest+json",
        ))
        .route(StaticRoute::new(
            "/icon-192.png",
            include_bytes!("../assets/icon-192.png"),
            "image/png",
        ))
        .route(StaticRoute::new(
            "/icon-512.png",
            include_bytes!("../assets/icon-512.png"),
            "image/png",
        ))
        .app_context(input)
        .app_context(HostInfo { url: url.clone() })
        .assets(AssetBundle::load().context(
            "asset bundle not found next to the executable; run `topcoat asset bundle` (or use `topcoat dev`)",
        )?)
        .build();

    let listener = TcpListener::bind((args.host.as_str(), args.port))
        .await
        .with_context(|| format!("binding {}:{} failed", args.host, args.port))?;

    let up = format!(
        "beam is up: {lan_ip}:{} (bound to {}:{})",
        args.port, args.host, args.port
    );
    if args.hidden {
        autostart::log_status(&up);
    } else {
        println!("{up}");
        println!("pair a phone by scanning:\n{}", qr_text(&url));
    }

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
