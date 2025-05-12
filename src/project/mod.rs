use strum::{EnumIter, EnumString, IntoEnumIterator};

use crate::{
	AppCtx,
	select_menu::{AfterRun, SelectMenu},
};

use anyhow::Result;

#[derive(Debug, Copy, Clone, strum_macros::Display, EnumIter, EnumString)]
pub enum ProjectMenu {
	/// Makes search more efficiant
	#[strum(serialize = "Re-Number all IDs")]
	ReNumberAll,
	#[strum(serialize = "Back to Main Menu")]
	Back,
}

impl SelectMenu for ProjectMenu {
	fn get_opts() -> Vec<Self> {
		Self::iter().collect()
	}

	fn run(&mut self, _ctx: &mut AppCtx) -> Result<()> {
		todo!()
	}

	fn purpose(&self) -> &'static str {
		todo!()
	}

	fn after(&self) -> AfterRun {
		match self {
			ProjectMenu::Back => AfterRun::GoBack,
			ProjectMenu::ReNumberAll => AfterRun::Continue,
		}
	}
}
