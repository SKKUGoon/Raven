use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};
use raven::config::Settings;
use raven::domain::venue::{default_venues, VenueId};
use std::collections::BTreeSet;
use std::io::{Error as IoError, ErrorKind};
use std::str::FromStr;

pub struct PipelineInputs {
    pub coin: String,
    pub quote: Option<String>,
    pub venue: Option<String>,
    pub venue_include: Vec<String>,
    pub venue_exclude: Vec<String>,
}

type ResolvedVenueSelection = (Option<String>, Vec<String>, Vec<String>);

pub struct PipelineInputResolver<'a> {
    settings: &'a Settings,
    action: &'a str,
    coin: Option<String>,
    quote: Option<String>,
    venue: Option<String>,
    venue_include: Vec<String>,
    venue_exclude: Vec<String>,
    interactive: bool,
}

impl<'a> PipelineInputResolver<'a> {
    pub fn new(settings: &'a Settings, action: &'a str) -> Self {
        Self {
            settings,
            action,
            coin: None,
            quote: None,
            venue: None,
            venue_include: Vec::new(),
            venue_exclude: Vec::new(),
            interactive: false,
        }
    }

    pub fn coin(mut self, coin: Option<String>) -> Self {
        self.coin = coin;
        self
    }

    pub fn quote(mut self, quote: Option<String>) -> Self {
        self.quote = quote;
        self
    }

    pub fn venue(mut self, venue: Option<String>) -> Self {
        self.venue = venue;
        self
    }

    pub fn venue_include(mut self, venue_include: Vec<String>) -> Self {
        self.venue_include = venue_include;
        self
    }

    pub fn venue_exclude(mut self, venue_exclude: Vec<String>) -> Self {
        self.venue_exclude = venue_exclude;
        self
    }

    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }
}

pub fn resolve_pipeline_inputs(
    resolver: PipelineInputResolver<'_>,
) -> Result<PipelineInputs, IoError> {
    let PipelineInputResolver {
        settings,
        action,
        coin,
        quote,
        venue,
        venue_include,
        venue_exclude,
        interactive,
    } = resolver;

    if !interactive {
        let coin = coin.ok_or_else(|| {
            IoError::new(
                ErrorKind::InvalidInput,
                format!("missing --coin for `{action}` (or use --interactive)"),
            )
        })?;
        return Ok(PipelineInputs {
            coin,
            quote,
            venue,
            venue_include,
            venue_exclude,
        });
    }

    let theme = ColorfulTheme::default();

    let (resolved_coin, resolved_quote) = resolve_coin_and_quote(&theme, coin, quote)?;
    let (resolved_venue, resolved_include, resolved_exclude) =
        resolve_venues(&theme, settings, venue, venue_include, venue_exclude)?;

    Ok(PipelineInputs {
        coin: resolved_coin,
        quote: resolved_quote,
        venue: resolved_venue,
        venue_include: resolved_include,
        venue_exclude: resolved_exclude,
    })
}

fn resolve_coin_and_quote(
    theme: &ColorfulTheme,
    coin: Option<String>,
    quote: Option<String>,
) -> Result<(String, Option<String>), IoError> {
    match (coin, quote) {
        (Some(coin), Some(quote)) => Ok((coin, Some(quote))),
        (Some(coin), None) => {
            let use_quote = Confirm::with_theme(theme)
                .with_prompt("Use COIN + QUOTE mode for this value?")
                .default(false)
                .interact()
                .map_err(prompt_error)?;
            if !use_quote {
                return Ok((coin, None));
            }
            let quote = Input::<String>::with_theme(theme)
                .with_prompt("Enter QUOTE asset (e.g. USDT)")
                .interact_text()
                .map_err(prompt_error)?;
            Ok((coin, Some(quote.trim().to_uppercase())))
        }
        (None, maybe_quote) => {
            if maybe_quote.is_some() {
                return Err(IoError::new(
                    ErrorKind::InvalidInput,
                    "received --quote without --coin",
                ));
            }

            let mode = Select::with_theme(theme)
                .with_prompt("How should ravenctl interpret your coin input?")
                .items([
                    "Venue symbol (e.g. BTCUSDT)",
                    "COIN + QUOTE assets (e.g. BTC + USDT)",
                ])
                .default(0)
                .interact()
                .map_err(prompt_error)?;

            if mode == 0 {
                let venue_symbol = Input::<String>::with_theme(theme)
                    .with_prompt("Enter venue symbol")
                    .interact_text()
                    .map_err(prompt_error)?;
                return Ok((venue_symbol.trim().to_string(), None));
            }

            let coin_asset = Input::<String>::with_theme(theme)
                .with_prompt("Enter COIN asset")
                .interact_text()
                .map_err(prompt_error)?;
            let quote_asset = Input::<String>::with_theme(theme)
                .with_prompt("Enter QUOTE asset")
                .interact_text()
                .map_err(prompt_error)?;
            Ok((
                coin_asset.trim().to_uppercase(),
                Some(quote_asset.trim().to_uppercase()),
            ))
        }
    }
}

fn resolve_venues(
    theme: &ColorfulTheme,
    settings: &Settings,
    venue: Option<String>,
    venue_include: Vec<String>,
    venue_exclude: Vec<String>,
) -> Result<ResolvedVenueSelection, IoError> {
    if venue.is_some() || !venue_include.is_empty() || !venue_exclude.is_empty() {
        return Ok((venue, venue_include, venue_exclude));
    }

    let choices = discover_venue_choices(settings);
    if choices.is_empty() {
        return Ok((venue, venue_include, venue_exclude));
    }

    let defaults = default_selected_indices(settings, &choices);
    let selected = MultiSelect::with_theme(theme)
        .with_prompt("Select one or more venues")
        .items(&choices)
        .defaults(&defaults)
        .interact()
        .map_err(prompt_error)?;

    if selected.is_empty() {
        return Err(IoError::new(
            ErrorKind::InvalidInput,
            "no venues selected in interactive mode",
        ));
    }

    let include = selected
        .into_iter()
        .map(|idx| choices[idx].clone())
        .collect::<Vec<_>>();
    Ok((None, include, Vec::new()))
}

fn discover_venue_choices(settings: &Settings) -> Vec<String> {
    let mut set = BTreeSet::new();
    for v in default_venues() {
        set.insert(v.as_wire());
    }
    for raw in &settings.routing.venue_include {
        if let Ok(v) = VenueId::from_str(raw) {
            set.insert(v.as_wire());
        }
    }
    for raw in &settings.routing.venue_exclude {
        if let Ok(v) = VenueId::from_str(raw) {
            set.insert(v.as_wire());
        }
    }
    set.into_iter().collect()
}

fn default_selected_indices(settings: &Settings, choices: &[String]) -> Vec<bool> {
    let mut selected = vec![false; choices.len()];
    let include: BTreeSet<String> = settings
        .routing
        .venue_include
        .iter()
        .filter_map(|raw| VenueId::from_str(raw).ok())
        .map(|v| v.as_wire())
        .collect();
    let exclude: BTreeSet<String> = settings
        .routing
        .venue_exclude
        .iter()
        .filter_map(|raw| VenueId::from_str(raw).ok())
        .map(|v| v.as_wire())
        .collect();

    for (idx, choice) in choices.iter().enumerate() {
        let is_default = if include.is_empty() {
            !exclude.contains(choice)
        } else {
            include.contains(choice) && !exclude.contains(choice)
        };
        selected[idx] = is_default;
    }
    selected
}

fn prompt_error(e: dialoguer::Error) -> IoError {
    IoError::other(format!("interactive prompt failed: {e}"))
}
