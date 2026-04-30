mod api;
mod commands;
mod config;
mod format;
mod http;
mod location;
mod places;
mod wizard;

use anyhow::{Context, Result};
use api::places::RankPreference;
use api::routes::TravelMode;
use clap::{Parser, Subcommand};
use config::Config;
use http::MapsApiError;
use owo_colors::OwoColorize;

#[derive(Parser, Debug)]
#[command(
    name = "gmaps",
    version,
    about = "A command-line interface for Google Maps Platform"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create or update settings via interactive wizard
    Init,

    /// Show current settings (API key is masked)
    Config,

    /// Manage saved places (favorite locations)
    Places {
        #[command(subcommand)]
        action: PlacesAction,
    },

    /// Geocode an address into latitude/longitude
    Geocode {
        address: Vec<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Number of results to display
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },

    /// Show current GPS location with address and place names
    Whereami {
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Number of results to display
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },

    /// Reverse-geocode coordinates to an address (e.g., gmaps reverse 40.7580,-73.9855)
    Reverse {
        latlng: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Number of results to display
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },

    /// Search nearby places
    Nearby {
        keyword: Vec<String>,

        /// Center point (lat,lng or address)
        #[arg(long)]
        location: Option<String>,

        /// Use GPS for current location
        #[arg(short = 'H', long)]
        here: bool,

        /// Search radius in meters (1..=50000)
        #[arg(long, default_value_t = 500)]
        radius: u32,

        /// Place type (e.g., restaurant, cafe, convenience_store)
        #[arg(long = "type")]
        place_type: Option<String>,

        /// Only currently open places (effective for keyword search)
        #[arg(long)]
        open_now: bool,

        /// Number of results to display (1..=20)
        #[arg(long, default_value_t = 10)]
        limit: u32,

        /// Sort order (used only without keyword)
        #[arg(long, value_enum, default_value_t = RankPreference::Distance)]
        rank: RankPreference,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Compute a route (origin/destination accept 'lat,lng' or an address)
    Route {
        origin: String,
        destination: String,

        /// Travel mode
        #[arg(short = 'm', long, value_enum, default_value_t = TravelMode::Driving)]
        mode: TravelMode,

        /// Departure time in RFC 3339 (e.g., 2026-04-30T18:00:00-04:00, only used for driving)
        #[arg(long)]
        depart: Option<String>,

        /// Waypoints separated by '|' (e.g., --waypoints "Penn Station|Times Square")
        #[arg(long)]
        waypoints: Option<String>,

        /// Show step-by-step directions
        #[arg(long)]
        steps: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum PlacesAction {
    /// Add a place by lat/lng, address, or -H GPS (overwrites if name exists)
    Add {
        /// Name of the place
        name: String,
        /// lat,lng (e.g., 40.7580,-73.9855) or address. Not needed when -H is used.
        #[arg(required_unless_present = "here", conflicts_with = "here")]
        location: Option<String>,
        /// Capture current GPS location
        #[arg(short = 'H', long)]
        here: bool,
        /// Optional note
        #[arg(long)]
        note: Option<String>,
    },
    /// List saved places
    List,
    /// Remove a place
    Remove { name: String },
}

fn require_config() -> Result<Config> {
    config::load()?.context("No configuration found. Run 'gmaps init' first.")
}

#[tokio::main]
async fn main() {
    if !cfg!(target_os = "macos") {
        eprintln!("{} This tool currently supports macOS only.", "[!]".red());
        std::process::exit(1);
    }
    if let Err(err) = run().await {
        print_error(&err);
        std::process::exit(1);
    }
}

/// Render any error escaping `run()`. If a `MapsApiError` is anywhere in the
/// chain we relabel it as "API error:" and surface the API-side `code` field
/// on a separate line, mirroring what the per-command `print_api_error`
/// helpers used to do but without scattering `process::exit(1)` calls across
/// the codebase.
fn print_error(err: &anyhow::Error) {
    let api = err.chain().find_map(|e| e.downcast_ref::<MapsApiError>());
    let label = if api.is_some() {
        "API error:"
    } else {
        "Error:"
    };
    eprintln!("{} {err:#}", label.red().bold());
    if let Some(c) = api.and_then(|a| a.code()) {
        eprintln!("  {} {c}", "code:".dimmed());
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            wizard::run_wizard().await?;
        }
        Commands::Config => commands::config::show()?,
        Commands::Places { action } => match action {
            PlacesAction::Add {
                name,
                location,
                here,
                note,
            } => {
                let config = require_config()?;
                commands::places::add(&config, &name, location.as_deref(), here, note).await?;
            }
            PlacesAction::List => commands::places::list()?,
            PlacesAction::Remove { name } => commands::places::remove(&name)?,
        },
        Commands::Geocode {
            address,
            json,
            limit,
        } => {
            let config = require_config()?;
            commands::geocode::run_geocode(&config, &address.join(" "), json, limit).await?;
        }
        Commands::Whereami { json, limit } => {
            let config = require_config()?;
            commands::whereami::run(&config, json, limit).await?;
        }
        Commands::Reverse {
            latlng,
            json,
            limit,
        } => {
            let config = require_config()?;
            commands::geocode::run_reverse(&config, &latlng, json, limit).await?;
        }
        Commands::Nearby {
            keyword,
            location,
            here,
            radius,
            place_type,
            open_now,
            limit,
            rank,
            json,
        } => {
            let config = require_config()?;
            commands::nearby::run(
                &config,
                commands::nearby::NearbyArgs {
                    keyword,
                    location,
                    here,
                    radius,
                    place_type,
                    open_now,
                    limit,
                    rank,
                    json,
                },
            )
            .await?;
        }
        Commands::Route {
            origin,
            destination,
            mode,
            depart,
            waypoints,
            steps,
            json,
        } => {
            let config = require_config()?;
            commands::route::run(
                &config,
                commands::route::RouteArgs {
                    origin,
                    destination,
                    mode,
                    depart,
                    waypoints,
                    steps,
                    json,
                },
            )
            .await?;
        }
    }

    Ok(())
}
