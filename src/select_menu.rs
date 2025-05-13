use std::fmt::Display;

use crate::{AppCtx, err_loc};
use anyhow::{Context, Result};
use inline_colorization::*;
use inquire::InquireError;

pub trait SelectMenu: Sized + Display + std::str::FromStr {
	fn get_opts() -> Vec<Self>;
	fn run(&mut self, ctx: &mut AppCtx) -> Result<()>;
	fn after(&self) -> AfterRun;
	/// the error message will say "Can't {purpose} due to {error}"
	fn purpose(&self) -> &'static str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterRun {
	Continue,
	GoBack,
}

fn select_menu<M>(menu_name: impl std::fmt::Display) -> Result<Option<M>>
where
	M: SelectMenu,
{
	let options: Vec<M> = M::get_opts();
	inquire::Select::new(
		format!("What would you like to do with {}?", &menu_name).as_str(),
		options,
	)
	.prompt_skippable()
	.with_context(|| format!("{} can't prompt {} menu", err_loc!(), &menu_name))
}

pub fn select_menu_loop<M>(ctx_in: &mut AppCtx, menu_name: impl std::fmt::Display) -> Result<()>
where
	M: SelectMenu,
{
	let mut ctx = ctx_in;
	loop {
		let mut operation: M = match select_menu::<M>(&menu_name)? {
			Some(m) => m,
			None => return Ok(()),
		};

		let purpose = operation.purpose();

		if let Err(e) = operation.run(&mut ctx) {
			match e.downcast_ref::<InquireError>() {
				Some(_) => return Err(e),
				None => {
					eprintln!("{color_red}Can't {purpose} due to:{color_reset}");
					for e in e.chain() {
						eprintln!("{color_red}{e}{color_reset}",)
					}
				}
			}
		}

		match operation.after() {
			AfterRun::Continue => {}
			AfterRun::GoBack => return Ok(()),
		}
	}
}
