// TODO: Figure out what's required for batch printing (e.g.,
// can I just send the precursor bytes once, and then send multiple packed images?
// TODO: Figure out how to handle non-precut labels
// TODO: Figure out how to handle 'fruit' labels
// TODO: Implement templates with fixed font sizes and positions
// TODO: Implement preview feature
// TODO: Implement 'arbitrary image' feature

use std::{io::Write, ops::DerefMut, sync::Arc};

use advmac::MacAddr6;
use bluetooth_serial_port_async::BtAddr;
use clap::{Parser, Subcommand};
use d30::PrinterAddr;
use log::debug;
use snafu::{whatever, ResultExt, Whatever};
use tokio::sync::Mutex;

// ---------------------
// CLI Processing

#[derive(Debug, Parser)]
#[command(name = "d30")]
#[command(about = "A userspace Phomemo D30 controller.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long)]
    dry_run: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[clap(short_flag = 't')]
    PrintText(ArgsPrintText),
}

#[derive(clap::Args, Debug)]
struct ArgsPrintText {
    #[arg(short, long)]
    device: Option<d30::PrinterAddr>,
    text: String,
    #[arg(short, long)]
    #[arg(default_value = "40")]
    scale: f32,
}

// ---------------------
// End CLI Processing

struct State {
    dry_run: bool,
    d30_config: Option<d30::D30Config>,
}

impl State {
    fn new(args: &Cli) -> Self {
        Self {
            dry_run: args.dry_run,
            d30_config: None,
        }
    }
}

fn get_addr(
    state: &mut State,
    user_maybe_addr: Option<d30::PrinterAddr>,
) -> Result<MacAddr6, Whatever> {
    let addr: MacAddr6;
    match (user_maybe_addr, d30::D30Config::read_d30_config()) {
        // The case that the user has specified an address, and we have a config loaded
        // We must use config to attempt to resolve the address
        (Some(user_specified_addr), Ok(config)) => {
            let resolved_addr = config.resolve_addr(&user_specified_addr)?;
            addr = resolved_addr;
            state.d30_config = Some(config);
        }
        // The case that the user has specified an address, but we do NOT have a config
        // We must hope that the user gave us a fully quallified address & not a hostname
        (Some(user_specified_addr), Err(_)) => match user_specified_addr {
            PrinterAddr::MacAddr(user_addr) => {
                addr = user_addr;
            }
            PrinterAddr::PrinterName(name) => {
                whatever!(
                        "Cannot resolve \"{}\" because config file could not be retrieved.\n\
                        \tIf \"{}\" is meant to be an address rather than a device name, you should check your formatting,\n\
                        \tas it does not look like a valid MAC address.",
                        name, name
                    );
            }
        },
        // No address on CLI, but there IS a config!
        // Try to resolve from config
        (None, Ok(config)) => {
            addr = config
                .resolve_default()
                .with_whatever_context(|_| "Could not resolve default MAC address")?;
        }

        (None, Err(_)) => {
            whatever!("You did not correctly specify an address on command line or config file.")
        }
    }
    Ok(addr)
}

fn cmd_print(state: &mut State, args: &ArgsPrintText) -> Result<(), Whatever> {
    let addr = get_addr(state, args.device.clone())?;
    debug!("Generating image {} with scale {}", &args.text, &args.scale);
    let image = d30::generate_image(&args.text, args.scale)
        .with_whatever_context(|_| "Failed to generate image")?;
    let mut socket =
        bluetooth_serial_port_async::BtSocket::new(bluetooth_serial_port_async::BtProtocol::RFCOMM)
            .with_whatever_context(|_| "Failed to open socket")?;

    if !state.dry_run {
        socket
            .connect(BtAddr(addr.to_array()))
            .with_whatever_context(|_| "Failed to connect")?;
    }
    debug!("Init connection");
    if !state.dry_run {
        socket
            .write(d30::INIT_BASE_FLAT)
            .with_whatever_context(|_| "Failed to send magic init bytes")?;
    }
    let mut output = d30::IMG_PRECURSOR.to_vec();
    debug!("Extend output");
    if !state.dry_run {
        output.extend(d30::pack_image(&image));
    }
    debug!("Write output to socket");
    if !state.dry_run {
        socket
            .write(output.as_slice())
            .with_whatever_context(|_| "Failed to write to socket")?;
    }
    debug!("Flush socket");
    if !state.dry_run {
        socket
            .flush()
            .with_whatever_context(|_| "Failed to flush socket")?;
    }
    Ok(())
}

#[snafu::report]
#[tokio::main]
async fn main() -> Result<(), Whatever> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = Cli::parse();
    debug!("Args: {:#?}", &args);
    let mut app = Arc::new(Mutex::new(State::new(&args)));

    match &args.command {
        Commands::PrintText(args) => cmd_print(app.lock().await.deref_mut(), &args)
            .with_whatever_context(|_| "Could not complete print command")?,
    }

    Ok(())
}
