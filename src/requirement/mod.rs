use std::borrow::Cow;
use std::fmt::Display;

use anyhow::anyhow;
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use const_format::formatcp;
use serde::Deserialize;
use serde::Serialize;
use strum::{Display, IntoEnumIterator};
use strum_macros::EnumIter;
use strum_macros::EnumString;

use crate::component::Component;
use crate::{
	AppCtx, EditFile, RecordStatus, RecordType,
	select_menu::{AfterRun, SelectMenu},
	update_record,
};
use crate::{contains_any, err_loc};
use crate::{create_draft_file, mut_record_by_id};
use crate::{create_edit_file, prompt_for_record_id};
use crate::{delete_record, escape_normalize_nl};

pub const REQUIREMENT_TABLE_NAME: &'static str = "requirement.tsv";
pub const REQUIREMENT_OLD_TABLE_NAME: &'static str = "requiremnt.old.tsv";
pub const REQUIREMENT_NEW_TABLE_NAME: &'static str = "requirement.new.tsv";
pub const REQUIREMENT_DRAFT_PREFIX: &'static str = "requirement_draft";
pub const REQUIREMENT_EDIT_PREFIX: &'static str = "requirement_edit";

#[derive(
	Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq, EnumString, Display, EnumIter,
)]
pub enum RequirementFunctional {
	#[strum(serialize = "Functional")]
	Functional,
	#[strum(serialize = "Non-Functional")]
	NonFunctional,
}

#[derive(
	Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Hash, Display, EnumIter,
)]
pub enum RequirementPriority {
	Mandated,
	High,
	Med,
	Low,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Requirement {
	id: u64,
	component_id: u64,
	title: String,
	functional: RequirementFunctional,
	creation_date: DateTime<Local>,
	requirement_text: String,
	version: usize,
	author: String,
	priority: RequirementPriority,
	status: RecordStatus,
	risks: String,
}

impl RecordType for Requirement {
	type EditFile = RequirementEdit;

	fn get_id(&self) -> u64 {
		self.id
	}

	fn get_tbl_mut(app_ctx: &mut AppCtx) -> &mut Vec<Self> {
		&mut app_ctx.requirements
	}

	fn get_tbl(app_ctx: &AppCtx) -> &Vec<Self> {
		&app_ctx.requirements
	}

	fn write_table(app_ctx: &mut AppCtx) -> Result<()> {
		app_ctx.write_requirements()
	}

	fn set_accepted(&mut self) {
		self.status = RecordStatus::Accepted
	}

	fn update_from_edit(&mut self, ef: Self::EditFile) {
		self.functional = ef.functional;
		self.title = ef.title;
		self.requirement_text = ef.requirement_text;
		self.version = self.version + 1;
		self.author = ef.author;
		self.priority = ef.priority;
		self.risks = ef.risks;
	}

	fn check_for_conflict(&self, rhs: &Self::EditFile) -> Result<()> {
		if self.requirement_text == rhs.requirement_text {
			Err(anyhow!(format!(
				"{} requirement with same text already exists at ID: {}",
				err_loc!(),
				self.id
			)))
		} else if self.title == rhs.title {
			Err(anyhow!(format!(
				"{} requirement with same title already exists at ID: {}",
				err_loc!(),
				self.id
			)))
		} else {
			Ok(())
		}
	}

	fn set_deleted(&mut self) {
		self.status = RecordStatus::Deleted;
	}

	fn get_status(&self) -> RecordStatus {
		self.status
	}

	fn fmt_as_edit(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
		writeln!(
			f,
			"# Do not include any tab characters in the document\n# Do not include any new-lines in the title field"
		)?;
		writeln!(f, "title = \"{}\"\n", self.title)?;
		write!(f, "# Write only: ")?;
		let mut rp_iter = RequirementFunctional::iter().peekable();
		while let Some(var) = rp_iter.next() {
			write!(f, "\"{var}\"")?;
			if rp_iter.peek().is_some() {
				write!(f, ", ")?;
			} else {
				write!(f, "\n")?;
			}
		}
		writeln!(f, "functional = \"{}\"\n", self.functional)?;
		writeln!(
			f,
			"# If writing on multiple lines use triple quotes (e.g. \"\"\"stuff\"\"\")"
		)?;
		write!(f, "requirement_text = \"")?;
		if self.requirement_text.contains("\\n") {
			write!(f, "\"\"")?;
			for line in self.requirement_text.split("\\n") {
				writeln!(f, "{line}")?;
			}
			write!(f, "\"\"")?;
		} else {
			write!(f, "{}", self.requirement_text)?;
		}
		writeln!(f, "\"\n")?;
		writeln!(f, "# Do not include any new-lines in the author field")?;
		writeln!(f, "author = \"{}\"\n", self.author)?;
		write!(f, "# Write only: ")?;
		let mut rp_iter = RequirementPriority::iter().peekable();
		while let Some(var) = rp_iter.next() {
			write!(f, "\"{var}\"")?;
			if rp_iter.peek().is_some() {
				write!(f, ", ")?;
			} else {
				write!(f, "\n")?;
			}
		}
		writeln!(f, "priority = \"{}\"\n ", self.priority)?;
		writeln!(
			f,
			"# If writing on multiple lines use triple quotes (e.g. \"\"\"stuff\"\"\")"
		)?;
		write!(f, "risks = \"")?;
		if self.risks.contains("\\n") {
			write!(f, "\"\"")?;
			for line in self.risks.split("\\n") {
				writeln!(f, "{line}")?;
			}
			write!(f, "\"\"")?;
		} else {
			write!(f, "{}", self.risks)?;
		}
		writeln!(f, "\"")
	}
}

impl Display for Requirement {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		writeln!(f, "id = {}", self.id)?;
		writeln!(f, "component_id = {}", self.component_id)?;
		writeln!(f, "title = \"{}\"", self.title)?;
		writeln!(f, "functtional = \"{}\"", self.functional)?;
		writeln!(f, "creation_date = \"{}\"", self.creation_date)?;
		write!(f, "requirement_text = \"")?;
		if self.requirement_text.contains("\\n") {
			write!(f, "\"\"")?;
			for line in self.requirement_text.split("\\n") {
				writeln!(f, "{line}")?;
			}
			writeln!(f, "\"\"")?;
		} else {
			write!(f, "{}", self.requirement_text)?;
			writeln!(f, "\"")?;
		}
		writeln!(f, "version = \"{}\"", self.version)?;
		writeln!(f, "author = \"{}\"", self.author)?;
		writeln!(f, "priority = \"{}\"", self.priority)?;
		writeln!(f, "status = \"{}\"", self.status)?;
		write!(f, "risks = \"")?;
		if self.risks.contains("\\n") {
			write!(f, "\"\"")?;
			for line in self.risks.split("\\n") {
				writeln!(f, "{line}")?;
			}
			write!(f, "\"\"")?;
		} else {
			write!(f, "{}", self.risks)?;
		}
		writeln!(f, "\"")
	}
}

impl Ord for Requirement {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.id.cmp(&other.id)
	}
}

impl PartialOrd for Requirement {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.id.partial_cmp(&other.id)
	}
}

#[derive(Serialize, Deserialize, Eq, PartialEq)]
pub struct RequirementEdit {
	functional: RequirementFunctional,
	title: String,
	requirement_text: String,
	author: String,
	priority: RequirementPriority,
	status: RecordStatus,
	risks: String,
}

impl Default for RequirementEdit {
	fn default() -> Self {
		Self {
			functional: RequirementFunctional::Functional,
			title: "write title here".into(),
			requirement_text: "write requirement here".into(),
			author: "write author name here".into(),
			priority: RequirementPriority::Mandated,
			status: RecordStatus::Draft,
			risks: "write risks here".into(),
		}
	}
}

impl EditFile for RequirementEdit {
	fn sanitize(mut self) -> Result<Self> {
		if self.risks.contains('\t') {
			return Err(anyhow!(formatcp!(
				"{} risks contains one or more tab characters",
				err_loc!()
			)));
		}
		if self.requirement_text.contains('\t') {
			return Err(anyhow!(formatcp!(
				"{} requirement text contains one or more tab characters",
				err_loc!()
			)));
		}
		if contains_any(&['\n', '\t', '\r'], &self.author) {
			return Err(anyhow!(formatcp!(
				"{} author contains one or more tab or new line characters",
				err_loc!()
			)));
		}
		if contains_any(&['\n', '\t', '\r'], &self.title) {
			return Err(anyhow!(formatcp!(
				"{} title contains one or more tab or new line characters",
				err_loc!()
			)));
		}
		if let Cow::Owned(o) = escape_normalize_nl(&self.requirement_text) {
			self.risks = o
		}
		if let Cow::Owned(o) = escape_normalize_nl(&self.risks) {
			self.risks = o
		}
		Ok(self)
	}

	fn fmt_as_draft(f: &mut impl std::fmt::Write) -> std::fmt::Result {
		writeln!(
			f,
			"# Do not include any tab characters in the document\n# Do not include any new-lines in the title field"
		)?;
		writeln!(f, "title = \"type title here\"\n")?;
		write!(f, "# Write only: ")?;
		let mut rp_iter = RequirementFunctional::iter().peekable();
		while let Some(var) = rp_iter.next() {
			write!(f, "\"{var}\"")?;
			if rp_iter.peek().is_some() {
				write!(f, ", ")?;
			} else {
				write!(f, "\n")?;
			}
		}
		writeln!(f, "functional = \"\"\n")?;
		writeln!(
			f,
			"# If writing on multiple lines use triple quotes (e.g. \"\"\"stuff\"\"\")"
		)?;
		writeln!(
			f,
			"requirement_text = \"\"\"write requirement here\nuse more than one line if you want\"\"\"\n"
		)?;
		writeln!(f, "# Do not include any new-lines in the author field")?;
		writeln!(f, "author = \"type author name here\"\n")?;
		write!(f, "# Write only: ")?;
		let mut rp_iter = RequirementPriority::iter().peekable();
		while let Some(var) = rp_iter.next() {
			write!(f, "\"{var}\"")?;
			if rp_iter.peek().is_some() {
				write!(f, ", ")?;
			} else {
				write!(f, "\n")?;
			}
		}
		writeln!(f, "priority = \"\"\n ")?;
		writeln!(
			f,
			"# If writing on multiple lines use triple quotes (e.g. \"\"\"stuff\"\"\")"
		)?;
		writeln!(
			f,
			"risks = \"\"\"write risks here\nuse more than one line if you want\"\"\"\n"
		)
	}
}

#[derive(Debug, Copy, Clone, strum_macros::Display, EnumIter, EnumString)]
pub enum RequirementMenu {
	#[strum(serialize = "Create New Draft")]
	NewDraft,
	#[strum(serialize = "Insert & Accept Draft")]
	Insert,
	#[strum(serialize = "Change Component")]
	ChangeComponent,
	#[strum(serialize = "Create an Edit File")]
	Edit,
	#[strum(serialize = "Update Using an Edit File")]
	Update,
	#[strum(serialize = "Delete")]
	Delete,
	#[strum(serialize = "Back to Main Menu")]
	Back,
}

impl SelectMenu for RequirementMenu {
	fn get_opts() -> Vec<Self> {
		Self::iter().collect()
	}

	fn run(&mut self, ctx: &mut AppCtx) -> Result<()> {
		match self {
			RequirementMenu::NewDraft => {
				create_draft_file::<RequirementEdit>(ctx, &REQUIREMENT_DRAFT_PREFIX)
			}
			RequirementMenu::Insert => insert_requirement_draft(ctx, &REQUIREMENT_DRAFT_PREFIX),
			RequirementMenu::Edit => create_edit_file::<Requirement>(ctx, &REQUIREMENT_EDIT_PREFIX),
			RequirementMenu::ChangeComponent => change_component(ctx),
			RequirementMenu::Update => update_record::<Requirement>(ctx, &REQUIREMENT_EDIT_PREFIX),
			RequirementMenu::Delete => delete_record::<Requirement>(ctx),
			RequirementMenu::Back => Ok(()),
		}
	}

	fn purpose(&self) -> &'static str {
		match self {
			RequirementMenu::NewDraft => "create requirement draft",
			RequirementMenu::Insert => "insert requirement",
			RequirementMenu::ChangeComponent => "change component",
			RequirementMenu::Edit => "edit requirement",
			RequirementMenu::Update => "update requirement",
			RequirementMenu::Delete => "delete requirement",
			RequirementMenu::Back => "",
		}
	}

	fn after(&self) -> AfterRun {
		match self {
			RequirementMenu::Back => AfterRun::GoBack,
			_ => AfterRun::Continue,
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct ComponentChose<'c> {
	id: u64,
	name: &'c str,
}

impl<'c> Display for ComponentChose<'c> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{} - {}", self.id, self.name)
	}
}

impl<'c> From<&'c Component> for ComponentChose<'c> {
	fn from(value: &'c Component) -> Self {
		Self {
			id: value.get_id(),
			name: value.name(),
		}
	}
}

fn change_component(ctx: &mut AppCtx) -> Result<()> {
	// pick requirement
	let req_id = match prompt_for_record_id()
		.with_context(|| formatcp!("{} can't prompt for requirement ID", err_loc!()))?
	{
		Some(id) => id,
		None => return Ok(()),
	};

	// get table for record type
	let requirements = &mut ctx.requirements;
	let requirement = mut_record_by_id(requirements, req_id)
		.context(formatcp!("{} can't find record", err_loc!()))?;

	// pick component
	let components: Vec<ComponentChose> = ctx
		.components
		.iter()
		.filter(|c| matches!(c.status(), RecordStatus::Accepted))
		.map(|c| ComponentChose::from(c))
		.collect();
	if components.is_empty() {
		return Err(anyhow!(formatcp!("{} there are no components", err_loc!())));
	}
	let selection = inquire::Select::new("Chose the component this requirement is for", components)
		.prompt_skippable()
		.map(|oc| oc.map(|c| c.id))
		.context(formatcp!("{} error prompting for component", err_loc!()))?;
	let component_id = match selection {
		Some(id) => id,
		None => return Ok(()),
	};

	requirement.component_id = component_id;
	ctx.write_requirements()
}

fn insert_requirement_draft(ctx: &mut AppCtx, draft_prefix: &'static str) -> Result<()> {
	// pick component
	let components: Vec<ComponentChose> = ctx
		.components
		.iter()
		.filter(|c| matches!(c.status(), RecordStatus::Accepted))
		.map(|c| ComponentChose::from(c))
		.collect();
	if components.is_empty() {
		return Err(anyhow!(formatcp!("{} there are no components", err_loc!())));
	}
	let selection = inquire::Select::new("Chose the component this requirement is for", components)
		.prompt_skippable()
		.map(|oc| oc.map(|c| c.id))
		.context(formatcp!("{} error prompting for component", err_loc!()))?;
	let component_id = match selection {
		Some(id) => id,
		None => return Ok(()),
	};
	// prompt with drafts as list opts
	let draft_file_entry = match crate::file_list_chose(ctx.as_ref(), |e| {
		e.file_type().is_file()
			&& e.file_name()
				.to_str()
				.is_some_and(|s| s.starts_with(draft_prefix) && s.ends_with(".toml"))
	})? {
		Some(dfe) => dfe,
		None => return Ok(()),
	};

	let draft_file = crate::open_edit_file::<RequirementEdit>(&draft_file_entry)?;

	// get table for record type
	let records: &mut Vec<Requirement> = Requirement::get_tbl_mut(ctx);

	// check for conflict as long as we aren't looking at the same record
	if let Some(e) = records
		.iter()
		.find_map(|c| match c.check_for_conflict(&draft_file) {
			Ok(()) => None,
			Err(e) => Some(e),
		}) {
		return Err(e);
	}

	// use max() here because Ord is based on the ID
	let id = records.iter().max().map(|c| c.get_id()).unwrap_or(0);

	let requirement = Requirement {
		id,
		component_id,
		title: draft_file.title,
		functional: draft_file.functional,
		creation_date: Local::now(),
		requirement_text: draft_file.requirement_text,
		version: 0,
		author: draft_file.author,
		priority: draft_file.priority,
		status: RecordStatus::Accepted,
		risks: draft_file.risks,
	};

	// insert into requirement table...
	records.push(requirement);
	println!("Inserted new requirement at ID: {id}");
	ctx.write_requirements()
}
