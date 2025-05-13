use std::fmt::Display;

use strum::{EnumIter, EnumString, IntoEnumIterator};
use thiserror::Error;

use crate::{
	AppCtx,
	select_menu::{AfterRun, SelectMenu},
};

use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DocFileName {
	version: u64,
}

impl DocFileName {
	pub fn increment(&mut self) -> &mut Self {
		self.version += 1;
		self
	}
	pub fn is_doc_file(to_check: &std::path::Path) -> bool {
		to_check.is_file()
			&& to_check.file_name().is_some_and(|o_name| {
				o_name.to_str().is_some_and(|s_name| {
					let rest = match s_name.strip_prefix(DOC_FILE_NAME_START) {
						Some(r) => r,
						None => return false,
					};

					let version_str = match rest.strip_suffix(DOC_FILE_EXTENTION) {
						Some(vs) => vs,
						None => return false,
					};
					version_str.chars().all(|ch| ch.is_digit(10))
				})
			})
	}
}

#[derive(Debug, Error)]
pub enum ParseDocFileNameErr {
	#[error("invalid start to file name (expected {DOC_FILE_NAME_START}, found {found:?})")]
	BadStart { found: Box<str> },
	#[error("invalid end to file name (expected {DOC_FILE_EXTENTION}, found {found:?})")]
	BadEnd { found: Box<str> },
	#[error("invalid file version number (parse err: {inner:?}, found {found:?})")]
	ParseVersionNumber {
		found: Box<str>,
		inner: std::num::ParseIntError,
	},
}

impl TryFrom<&str> for DocFileName {
	type Error = ParseDocFileNameErr;

	fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
		let rest = value.strip_prefix(DOC_FILE_NAME_START).ok_or_else(|| {
			ParseDocFileNameErr::BadStart {
				found: Box::from(value),
			}
		})?;
		let version_str =
			rest.strip_suffix(DOC_FILE_EXTENTION)
				.ok_or_else(|| ParseDocFileNameErr::BadEnd {
					found: Box::from(value),
				})?;
		u64::from_str_radix(version_str, 10)
			.map_err(|ie| ParseDocFileNameErr::ParseVersionNumber {
				found: Box::from(version_str),
				inner: ie,
			})
			.map(|version| Self { version })
	}
}

impl Default for DocFileName {
	fn default() -> Self {
		Self { version: 0 }
	}
}

impl Display for DocFileName {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "requirement-v{}{DOC_FILE_EXTENTION}", self.version)
	}
}

pub const DOC_FILE_NAME_START: &'static str = "requirement-v";

pub const DOC_FILE_EXTENTION: &'static str = ".md";

#[derive(Debug, Copy, Clone, strum_macros::Display, EnumIter, EnumString)]
pub enum ProjectMenu {
	/// Makes search more efficiant after a purge
	#[strum(serialize = "Re-Number all IDs")]
	ReNumberAll,
	#[strum(serialize = "Build Requirement Document")]
	BuildDocs,
	#[strum(serialize = "Back to Main Menu")]
	Back,
}

impl SelectMenu for ProjectMenu {
	fn get_opts() -> Vec<Self> {
		Self::iter().collect()
	}

	fn run(&mut self, _ctx: &mut AppCtx) -> Result<()> {
		match self {
			ProjectMenu::ReNumberAll => todo!(),
			ProjectMenu::BuildDocs => todo!(),
			ProjectMenu::Back => todo!(),
		}
	}

	fn purpose(&self) -> &'static str {
		match self {
			ProjectMenu::ReNumberAll => "re-number records",
			ProjectMenu::BuildDocs => "build docs",
			ProjectMenu::Back => "go back to main menu",
		}
	}

	fn after(&self) -> AfterRun {
		match self {
			ProjectMenu::Back => AfterRun::GoBack,
			ProjectMenu::ReNumberAll => AfterRun::Continue,
			ProjectMenu::BuildDocs => todo!(),
		}
	}
}
