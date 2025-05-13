use std::{borrow::Cow, fmt::Display, io::Read, path::Path};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local};
use const_format::formatcp;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use strum_macros::EnumString;
use walkdir::DirEntry;

use crate::{
	AppCtx, EditFile, RecordStatus, RecordType, contains_any, create_draft_file, create_edit_file,
	delete_record, err_loc, escape_normalize_nl, file_list_chose,
	select_menu::{AfterRun, SelectMenu},
	update_record,
};

pub const COMPONENT_TABLE_NAME: &'static str = "component.tsv";
pub const COMPONENT_OLD_TABLE_NAME: &'static str = "components.old.tsv";
pub const COMPONENT_NEW_TABLE_NAME: &'static str = "components.new.tsv";
pub const COMPONENT_DRAFT_PREFIX: &'static str = "component_draft";
pub const COMPONENT_EDIT_PREFIX: &'static str = "component_edit";

pub trait ComponentMenuCtx: AsRef<Path> + AsMut<Vec<Component>> {}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComponentEdit {
	name: String,
	description: String,
	author: String,
}

impl Default for ComponentEdit {
	fn default() -> Self {
		Self {
			name: "write component name".into(),
			description: "write description".into(),
			author: "author name or initials here".into(),
		}
	}
}

impl EditFile for ComponentEdit {
	fn sanitize(self) -> Result<Self> {
		sanitize_component_edit(self)
	}

	fn fmt_as_draft(f: &mut impl std::fmt::Write) -> std::fmt::Result {
		writeln!(
			f,
			"# Do not include any tab characters in the document\n# Do not include any new-lines in the name field"
		)?;
		writeln!(f, "name = \"type name here\"\n")?;
		writeln!(
			f,
			"# If writing on multiple lines use triple quotes (e.g. \"\"\"stuff\"\"\")"
		)?;
		writeln!(
			f,
			"desciption = \"\"\"write description here\nuse more than one line if you want\"\"\"\n"
		)?;
		writeln!(f, "# Do not include any new-lines in the author field")?;
		writeln!(f, "author = \"author name here\"")
	}
}

impl From<&Component> for ComponentEdit {
	fn from(value: &Component) -> Self {
		Self {
			name: value.name.to_string(),
			description: value.description.to_string(),
			author: value.author.to_string(),
		}
	}
}

impl From<Component> for ComponentEdit {
	fn from(value: Component) -> Self {
		Self {
			name: value.name,
			description: value.description,
			author: value.author,
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Component {
	id: u64,
	name: String,
	description: String,
	creation_date: DateTime<Local>,
	status: RecordStatus,
	author: String,
}

impl RecordType for Component {
	type EditFile = ComponentEdit;

	fn get_id(&self) -> u64 {
		self.id
	}

	fn get_tbl_mut(app_ctx: &mut AppCtx) -> &mut Vec<Self> {
		&mut app_ctx.components
	}

	fn get_tbl(app_ctx: &AppCtx) -> &Vec<Self> {
		&app_ctx.components
	}

	fn write_table(app_ctx: &mut AppCtx) -> Result<()> {
		app_ctx.write_components()
	}

	fn update_from_edit(&mut self, ef: Self::EditFile) {
		self.name = ef.name;
		self.description = ef.description;
		self.author = ef.author;
	}

	fn set_accepted(&mut self) {
		self.status = RecordStatus::Accepted
	}

	fn check_for_conflict(&self, rhs: &Self::EditFile) -> Result<()> {
		if self.name == rhs.name {
			Err(anyhow!(
				"{} record with name: \"{}\"",
				err_loc!(),
				self.name
			))
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
			"# Do not include any tab characters in the document\n# Do not include any new-lines in the name field"
		)?;
		writeln!(f, "name = \"{}\"\n", self.name)?;
		writeln!(
			f,
			"# If writing on multiple lines use triple quotes (e.g. \"\"\"stuff\"\"\")"
		)?;
		write!(f, "desciption = \"")?;
		if self.description.contains("\\n") {
			write!(f, "\"\"")?;
			for line in self.description.split("\\n") {
				writeln!(f, "{}", line)?;
			}
			write!(f, "\"\"")?;
		} else {
			write!(f, "{}", self.description)?;
		}
		writeln!(f, "\"\n")?;
		writeln!(f, "# Do not include any new-lines in the author field")?;
		writeln!(f, "author = \"{}\"", self.author)
	}
}

impl Component {
	pub fn name(&self) -> &str {
		&self.name
	}
	pub fn status(&self) -> RecordStatus {
		self.status
	}
}

impl Ord for Component {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.id.cmp(&other.id)
	}
}

impl PartialOrd for Component {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.id.partial_cmp(&other.id)
	}
}

impl Display for Component {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		writeln!(f, "id = {}", self.id)?;
		writeln!(f, "name = \"{}\"", self.name)?;
		write!(f, "desciption = \"")?;
		if self.description.contains("\\n") {
			writeln!(f, "\"\"")?;
			for line in self.description.split("\\n") {
				writeln!(f, "{}", line)?;
			}
			writeln!(f, "\"\"")?;
		} else {
			write!(f, "{}", self.description)?;
			writeln!(f, "\"")?;
		}
		writeln!(f, "creation date = \"{}\"", self.creation_date)?;
		writeln!(f, "status = \"{}\"", self.status)?;
		writeln!(f, "author = \"{}\"", self.author)
	}
}

impl Component {
	pub fn max_len(&self) -> usize {
		self.name.len() + self.description.len() + self.author.len() + 5 + 27 + 8 + 128
	}
}

#[derive(Serialize, Deserialize, Eq, PartialEq)]
pub struct ComponentTomlDraft {
	name: String,
	description: String,
	author: String,
}

#[derive(Debug, Copy, Clone, strum_macros::Display, EnumIter, EnumString)]
pub enum ComponentMenu {
	#[strum(serialize = "Create New Draft")]
	NewDraft,
	#[strum(serialize = "Insert & Accept Draft")]
	Insert,
	#[strum(serialize = "Create an Edit File")]
	Edit,
	#[strum(serialize = "Update Using an Edit File")]
	Update,
	#[strum(serialize = "Delete")]
	Delete,
	#[strum(serialize = "Back to Main Menu")]
	Back,
}

impl SelectMenu for ComponentMenu {
	fn get_opts() -> Vec<Self> {
		Self::iter().collect()
	}

	fn run(&mut self, ctx: &mut AppCtx) -> Result<()> {
		match self {
			ComponentMenu::NewDraft => {
				create_draft_file::<ComponentEdit>(ctx, &COMPONENT_DRAFT_PREFIX)
			}
			ComponentMenu::Insert => insert_component_draft(ctx),
			ComponentMenu::Delete => delete_record::<Component>(ctx),
			ComponentMenu::Edit => create_edit_file::<Component>(ctx, &COMPONENT_EDIT_PREFIX),
			ComponentMenu::Update => update_record::<Component>(ctx, &COMPONENT_EDIT_PREFIX),
			ComponentMenu::Back => Ok(()),
		}
	}

	fn purpose(&self) -> &'static str {
		match self {
			ComponentMenu::NewDraft => "create draft",
			ComponentMenu::Insert => "insert component",
			ComponentMenu::Edit => "edit component",
			ComponentMenu::Update => "update component",
			ComponentMenu::Delete => "delete component",
			ComponentMenu::Back => "",
		}
	}

	fn after(&self) -> AfterRun {
		match self {
			ComponentMenu::Back => AfterRun::GoBack,
			_ => AfterRun::Continue,
		}
	}
}

fn insert_component_draft(ctx: &mut AppCtx) -> Result<()> {
	// prompt with drafts as list opts
	let draft_file_entry = match file_list_chose(ctx.as_ref(), |e| {
		e.file_type().is_file()
			&& e.file_name()
				.to_str()
				.is_some_and(|s| s.starts_with(COMPONENT_DRAFT_PREFIX) && s.ends_with(".toml"))
	})? {
		Some(dfe) => dfe,
		None => return Ok(()),
	};

	// insert into component table...
	let draft = open_component_draft(&draft_file_entry)?;

	let components: &mut Vec<Component> = ctx.as_mut();
	// check for name conflict
	if let Some(e) = components.iter().find(|c| c.name == draft.name).map(|c| {
		anyhow!(format!(
			"{} component with name: \"{}\" already exists at ID: {}",
			err_loc!(),
			c.name,
			c.id
		))
	}) {
		return Err(e);
	}

	let id = components.iter().max().map(|c| c.id).unwrap_or(0);
	let name = draft.name.replace('\n', "\\n");
	println!("Inserted component: \"{name}\" at ID: {id}");
	components.push(Component {
		id,
		name,
		description: draft.description,
		creation_date: Local::now(),
		status: RecordStatus::Accepted,
		author: draft.author,
	});
	ctx.write_components()
}

fn open_component_draft(entry: &DirEntry) -> Result<ComponentTomlDraft> {
	// open file
	let mut component_file = std::fs::OpenOptions::new()
		.read(true)
		.write(false)
		.truncate(false)
		.create(false)
		.open(&entry.path())
		.with_context(|| {
			format!(
				"{} can't open component draft file: {:?}",
				err_loc!(),
				&entry.path()
			)
		})?;

	// read
	let mut buf = String::with_capacity(
		entry
			.metadata()
			.ok()
			.map(|m| m.len() as usize)
			.unwrap_or(4096),
	);
	component_file.read_to_string(&mut buf).with_context(|| {
		format!(
			"{} can't read component draft file: {:?}",
			err_loc!(),
			&entry.path()
		)
	})?;
	// deserialize
	let draft = toml::from_str::<ComponentTomlDraft>(buf.as_str()).with_context(|| {
		format!(
			"{} component file content format error for: {:?}",
			err_loc!(),
			&entry.file_name()
		)
	})?;

	sanitize_component_draft(draft).context(formatcp!(
		"{} component input contains illegal characters",
		err_loc!()
	))
}

/// Escape NL or CRNL to "\n" in description, error on NL or CRLF in name, and error on tab character anywhere.
fn sanitize_component_draft(mut draft: ComponentTomlDraft) -> Result<ComponentTomlDraft> {
	if contains_any(&['\n', '\r', '\t'], draft.name.as_str()) {
		return Err(anyhow!(formatcp!(
			"{} name contains one or more non-space whitespace characters",
			err_loc!()
		)));
	}
	if draft.description.contains('\t') {
		return Err(anyhow!(formatcp!(
			"{} description contains one or more tab characters",
			err_loc!()
		)));
	}
	if draft.author.contains('\t') {
		return Err(anyhow!(formatcp!(
			"{} author contains one or more tab characters",
			err_loc!()
		)));
	}

	if let Cow::Owned(o) = escape_normalize_nl(&draft.description) {
		draft.description = o
	}
	Ok(draft)
}

/// Escape NL or CRNL to "\n" in description, error on NL or CRLF in name, and error on tab character anywhere.
fn sanitize_component_edit(mut draft: ComponentEdit) -> Result<ComponentEdit> {
	if contains_any(&['\n', '\r', '\t'], draft.name.as_str()) {
		return Err(anyhow!(formatcp!(
			"{} name contains one or more non-space whitespace characters",
			err_loc!()
		)));
	}
	if draft.description.contains('\t') {
		return Err(anyhow!(formatcp!(
			"{} description contains one or more tab characters",
			err_loc!()
		)));
	}
	if contains_any(&['\n', '\r', '\t'], draft.author.as_str()) {
		return Err(anyhow!(formatcp!(
			"{} author contains one or more non-space whitespace characters",
			err_loc!()
		)));
	}
	if let Cow::Owned(o) = escape_normalize_nl(&draft.description) {
		draft.description = o
	}
	Ok(draft)
}
