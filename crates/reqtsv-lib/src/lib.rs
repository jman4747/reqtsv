use std::fs::File;
use std::io::Write;
use std::path::Path;

use chrono::{DateTime, Local};
use log::Level;
use log::debug;
use log::error;
use log::info;
use log::log_enabled;
use serde::{Deserialize, Serialize};
use strum::Display;
use strum_macros::EnumIter;
use strum_macros::EnumString;
use thiserror::Error;

pub const COLUMN_DELIMITER: u8 = b'\t';
pub const RECORD_DELIMITER: u8 = b'\n';
pub const COMPONENT_HEADER: &'static str = "id\tname\tdescription\tcreation_date\tstatus\tauthor\n";
pub const REQUIREMENT_HEADER: &'static str = "id\tcomponent_id\tfunctional\tcreation_date\trequirement\tversion\tauthor\tpriority\tstatus\tstatus_justification\trisks\n";

pub const COMPONENT_TABLE_NAME: &'static str = "component.tsv";
pub const COMPONENT_OLD_TABLE_NAME: &'static str = "components.old.tsv";
pub const COMPONENT_NEW_TABLE_NAME: &'static str = "components.new.tsv";
pub const COMPONENT_DRAFT_PREFIX: &'static str = "component_draft";
pub const COMPONENT_EDIT_PREFIX: &'static str = "component_edit";

pub const REQUIREMENT_TABLE_NAME: &'static str = "requirement.tsv";
pub const REQUIREMENT_OLD_TABLE_NAME: &'static str = "requiremnt.old.tsv";
pub const REQUIREMENT_NEW_TABLE_NAME: &'static str = "requirement.new.tsv";
pub const REQUIREMENT_DRAFT_PREFIX: &'static str = "requirement_draft";
pub const REQUIREMENT_EDIT_PREFIX: &'static str = "requirement_edit";

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Hash, Display)]
pub enum RecordStatus {
	Draft,
	Accepted,
	Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Component {
	pub id: u64,
	pub name: String,
	pub description: String,
	pub creation_date: DateTime<Local>,
	pub status: RecordStatus,
	pub author: String,
}

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
	pub id: u64,
	pub component_id: u64,
	pub title: String,
	pub functional: RequirementFunctional,
	pub creation_date: DateTime<Local>,
	pub requirement_text: String,
	pub version: usize,
	pub author: String,
	pub priority: RequirementPriority,
	pub status: RecordStatus,
	pub risks: String,
}

#[derive(Error, Debug)]
pub enum SaveFileError {
	#[error("can't write to file: {0}")]
	CantWriteAll(#[source] std::io::Error),
	#[error("can't flush file to disk: {0}")]
	CantFlush(#[source] std::io::Error),
	#[error("can't sync disk: {0}")]
	CantSync(#[source] std::io::Error),
}

pub fn save_file_strict(mut file: std::fs::File, file_content: &[u8]) -> Result<(), SaveFileError> {
	save_file_strict_mut(&mut file, file_content)
}

pub fn save_file_strict_mut(
	file: &mut std::fs::File,
	file_content: &[u8],
) -> Result<(), SaveFileError> {
	debug!("write all {} bytes", file_content.len());
	file.write_all(file_content)
		.map_err(|ioe| SaveFileError::CantWriteAll(ioe))
		.inspect_err(|e| error!("{e}"))?;

	debug!("flush to disk");
	file.flush()
		.map_err(|ioe| SaveFileError::CantFlush(ioe))
		.inspect_err(|e| error!("{e}"))?;

	debug!("sync disk");
	file.sync_data()
		.map_err(|ioe| SaveFileError::CantSync(ioe))
		.inspect_err(|e| error!("{e}"))
}

#[derive(Error, Debug)]
pub enum GetProjectRootErr {
	#[error("no Requirement table file in: {0:?}")]
	RequirementTblFile(Box<Path>),
	#[error("no Component table file in: {0:?}")]
	ComponentTblFile(Box<Path>),
	#[error("no Component or Requirement table files in: {0:?}")]
	BothTablesFiles(Box<Path>),

	#[error("can't load Requirements table: {0:}")]
	LoadRequirements(#[source] LoadTableError),
	#[error("can't load Components table: {0:}")]
	LoadComponents(#[source] LoadTableError),

	#[error("corrupt Requirement record: {0:}")]
	BadRequirementRecord(#[source] csv::Error),
	#[error("corrupt Component record: {0:}")]
	BadComponentRecord(#[source] csv::Error),
}

#[derive(Debug)]
pub struct Project {
	pub root: Box<Path>,
	pub components: Vec<Component>,
	pub raw_components: Box<str>,
	pub component_file: File,
	pub requirements: Vec<Requirement>,
	pub raw_requirements: Box<str>,
	pub requirement_file: File,
	pub project_title: Box<str>,
}

pub fn get_project_root(maybe_root: impl AsRef<Path>) -> Result<Project, GetProjectRootErr> {
	//serialize and verify both tables
	info!("Loading component table...");
	let component_tbl_path = maybe_root
		.as_ref()
		.join(COMPONENT_TABLE_NAME)
		.into_boxed_path();

	let (component_file, raw_components) = load_table(component_tbl_path, true)
		.map_err(|lte| GetProjectRootErr::LoadComponents(lte))?;

	let mut tsv_reader = csv::ReaderBuilder::new()
		.delimiter(COLUMN_DELIMITER)
		.terminator(csv::Terminator::Any(b'\n'))
		.from_reader(raw_components.as_bytes());

	let max_records = raw_components.chars().filter(|ch| *ch == '\n').count();
	let mut components: Vec<Component> = Vec::with_capacity(max_records);
	for res in tsv_reader.deserialize::<Component>() {
		match res {
			Ok(record) => {
				components.push(record);
			}
			Err(e) => return Err(GetProjectRootErr::BadComponentRecord(e)),
		}
	}

	if log_enabled!(Level::Debug) {
		debug!("found {} component records", components.len())
	}

	info!("Loading requirement table...");
	let requirement_tbl_path = maybe_root
		.as_ref()
		.join(REQUIREMENT_TABLE_NAME)
		.into_boxed_path();

	let (requirement_file, raw_requirements) = load_table(requirement_tbl_path, true)
		.map_err(|lte| GetProjectRootErr::LoadRequirements(lte))?;

	let mut tsv_reader = csv::ReaderBuilder::new()
		.delimiter(COLUMN_DELIMITER)
		.terminator(csv::Terminator::Any(b'\n'))
		.from_reader(raw_requirements.as_bytes());

	let max_records = raw_requirements.chars().filter(|ch| *ch == '\n').count();
	let mut requirements: Vec<Requirement> = Vec::with_capacity(max_records);
	for res in tsv_reader.deserialize::<Requirement>() {
		match res {
			Ok(record) => {
				requirements.push(record);
			}
			Err(e) => return Err(GetProjectRootErr::BadRequirementRecord(e)),
		}
	}

	if log_enabled!(Level::Debug) {
		debug!("found {} requirement records", requirements.len())
	}

	let root: Box<Path> = Box::from(maybe_root.as_ref());

	info!("Loaded project @: {:?}", &root);
	Ok(Project {
		root,
		components,
		requirements,
		component_file,
		requirement_file,
		raw_components,
		raw_requirements,
		// TODO: Need reqtsv.toml
		project_title: format!("TODO Placeholder Title").into_boxed_str(),
	})
}

#[derive(Error, Debug)]
pub enum LoadTableError {
	#[error("can't open table file: {0:}")]
	OpenTable(#[source] std::io::Error),
	#[error("cant't read table: {0:}")]
	ReadTable(#[source] std::io::Error),
}

fn load_table(
	table_path: impl AsRef<Path>,
	write: bool,
) -> Result<(File, Box<str>), LoadTableError> {
	// open table

	debug!("open table @: {:?}", table_path.as_ref());
	let mut file = std::fs::OpenOptions::new()
		.read(true)
		.write(write)
		.append(write)
		.truncate(false)
		.create(false)
		.open(&table_path)
		.map_err(|ioe| LoadTableError::OpenTable(ioe))?;

	// load all
	let mut buf = String::with_capacity(
		file.metadata()
			.ok()
			.map(|m| m.len() as usize)
			.unwrap_or(1_000_000),
	);
	use std::io::Read as _;
	debug!("read table content...");
	let loaded = file
		.read_to_string(&mut buf)
		.map_err(|ioe| LoadTableError::ReadTable(ioe))?;
	if log_enabled!(Level::Debug) {
		debug!("loaded {loaded} bytes")
	}
	Ok((file, buf.into_boxed_str()))
}

#[cfg(test)]
mod tests {}
