use std::collections::HashSet;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use dialoguer::{theme::ColorfulTheme, Confirm, Select};
use optional_struct::Applyable;

use super::{
	args::{Cli, Forecast},
	config::Config,
	localization::{ConfigLocales, Locales},
	location::Location,
	units::Units,
};

#[derive(Debug, Clone)]
pub struct Params {
	pub config: Config,
	pub texts: Locales,
	pub historical_weather: HashSet<NaiveDate>,
}

impl Params {
	pub async fn merge(config: &Config, args: &Cli) -> Result<Self> {
		let language = match &args.language {
			Some(lang) => lang.to_string(),
			_ => config.language.to_owned(),
		};

		let texts = Locales::get(&language).await?;

		if args.reset {
			Self::reset(&texts.config).await?;
			std::process::exit(1);
		}

		let units = Units::merge(&args.units, &config.units);

		let address = Location::resolve_input(args.address.as_deref().unwrap_or_default(), config, &texts).await?;

		let forecast = if args.forecast.contains(&Forecast::disable)
			|| (args.forecast.is_empty() && !args.historical_weather.is_empty())
		{
			HashSet::<Forecast>::new()
		} else if !args.forecast.is_empty() {
			args.forecast.iter().cloned().collect()
		} else {
			config.forecast.to_owned()
		};

		let historical_weather = if !args.historical_weather.is_empty() {
			args.historical_weather.iter().cloned().collect()
		} else {
			HashSet::<NaiveDate>::new()
		};

		let gui = config.gui.to_owned();

		Ok(Self {
			config: Config {
				language,
				forecast,
				units,
				address,
				gui,
			},
			texts,
			historical_weather,
		})
	}

	pub async fn handle_next(mut self, args: Cli, mut config_file: Config) -> Result<()> {
		if !args.save && !config_file.address.is_empty() {
			return Ok(());
		}

		// Restore time_indicator config setting in case it was disabled for a weekday / historical forecast.
		self.config.gui.graph.time_indicator = config_file.gui.graph.time_indicator;

		if config_file.address.is_empty() {
			// Prompt to save
			self.config.apply_to(&mut config_file);
			self.config = config_file;
			self.save_prompt(&args.address.unwrap_or_default()).await?;
		} else {
			// Handle explicit save call
			self.config.apply_to(&mut config_file);
			config_file.store().context("Error saving config file.")?;
			self.texts.store(&config_file.language);
		}

		Ok(())
	}

	async fn save_prompt(mut self, arg_address: &str) -> Result<()> {
		let mut items = vec![
			&self.texts.config.confirm,
			&self.texts.config.next_time,
			&self.texts.config.deny,
		];

		if arg_address.is_empty() || arg_address == "auto" {
			items.push(&self.texts.config.always_auto);
		}

		let selection = Select::with_theme(&ColorfulTheme::default())
			.with_prompt(&self.texts.config.save_as_default)
			.items(&items)
			.default(0)
			.interact()?;

		match selection {
			0 => {}
			1 => return Ok(()),
			2 => self.config.address = "arg_input".to_string(),
			3 => self.config.address = "auto".to_string(),
			_ => println!("{}", self.texts.config.no_selection),
		}

		self.config.store().context("Error saving config file.")?;
		self.texts.store(&self.config.language);

		Ok(())
	}

	pub async fn reset(t: &ConfigLocales) -> Result<()> {
		let confirmation = Confirm::with_theme(&ColorfulTheme::default())
			.with_prompt(&t.reset_config)
			.interact()?;

		if confirmation {
			let path = Config::get_path();

			std::fs::remove_dir_all(path.parent().unwrap()).with_context(|| "Error resetting config file.")?;
		}

		Ok(())
	}
}
