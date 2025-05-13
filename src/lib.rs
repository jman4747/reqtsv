use std::{
	borrow::Cow,
	fmt::Display,
	fs::{File, OpenOptions},
	io::{Read, Write},
	path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use component::{Component, ComponentMenuCtx};
use const_format::formatcp;
use requirement::Requirement;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use walkdir::{DirEntry, WalkDir};

pub mod component;
// pub mod error;
pub mod project;
pub mod requirement;
pub mod select_menu;

pub const COLUMN_DELIMITER: u8 = b'\t';
pub const RECORD_DELIMITER: u8 = b'\n';
pub const COMPONENT_HEADER: &'static str = "id\tname\tdescription\tcreation_date\tstatus\tauthor\n";
pub const REQUIREMENT_HEADER: &'static str = "id\tcomponent_id\tfunctional\tcreation_date\trequirement\tversion\tauthor\tpriority\tstatus\tstatus_justification\trisks\n";

#[derive(Debug)]
pub struct AppCtx {
	pub components: Vec<Component>,
	pub requirements: Vec<Requirement>,
	pub project_root: Box<Path>, // TODO: delete? field on in memory record
	pub component_file: File,
	pub requirement_file: File,
	pub component_new_path: Box<Path>,
	pub requirement_new_path: Box<Path>,
	pub updated_component: bool,
	pub updated_requirement: bool,
}

/// Puts "file!():line!():" e.g. "src/lib.rs:41:"
#[macro_export]
macro_rules! err_loc {
	() => {
		const_format::formatcp!("{}:{}:", file!(), line!())
	};
}

pub fn mut_record_by_id<R: RecordType>(records: &mut Vec<R>, id: u64) -> Result<&mut R> {
	if records.get(id as usize).is_some() {
		Ok(&mut records[id as usize])
	} else {
		match records.iter_mut().find(|rec| rec.get_id() == id) {
			Some(comp) => Ok(comp),
			None => Err(anyhow!(format!("{} no record at ID: {id}", err_loc!()))),
		}
	}
}

impl AppCtx {
	pub fn get_component_by_id(&mut self, id: u64) -> Result<&mut Component> {
		if self.components.get(id as usize).is_some() {
			Ok(&mut self.components[id as usize])
		} else {
			match self.components.iter_mut().find(|comp| comp.get_id() == id) {
				Some(comp) => Ok(comp),
				None => Err(anyhow!(format!("{} no component at ID: {id}", err_loc!()))),
			}
		}
	}

	fn wrtie_table<S>(&self, tbl_new_path: &Path, records: impl Iterator<Item = S>) -> Result<()>
	where
		S: Serialize,
	{
		let mut f_new = OpenOptions::new()
			.write(true)
			.create(true)
			.truncate(true)
			.open(tbl_new_path)
			.with_context(
				|| format!("{} can't create new file: {:?}", err_loc!(), tbl_new_path,),
			)?;

		let mut wtr = csv::WriterBuilder::new()
			.delimiter(COLUMN_DELIMITER)
			.has_headers(true)
			.terminator(csv::Terminator::Any(RECORD_DELIMITER))
			.from_writer(&mut f_new);

		for record in records {
			wtr.serialize(record).with_context(|| {
				format!(
					"{} can't write record to file: {:?}",
					err_loc!(),
					tbl_new_path
				)
			})?
		}

		drop(wtr);

		f_new
			.flush()
			.with_context(|| format!("{} Can't flush: {:?} to disk", err_loc!(), tbl_new_path))?;
		f_new.sync_all().with_context(|| {
			format!(
				"Can't sync file: {:?} to disk @ {}:{}",
				tbl_new_path,
				file!(),
				line!()
			)
		})
	}

	pub fn write_components(&mut self) -> Result<()> {
		self.wrtie_table(&self.component_new_path, self.components.iter())
			.context(formatcp!("{} can't write component table", err_loc!()))?;
		self.updated_component = true;
		Ok(())
	}

	fn write_requirements(&mut self) -> Result<()> {
		self.wrtie_table(&self.requirement_new_path, self.requirements.iter())
			.context(formatcp!("{} can't write requirement table", err_loc!()))?;
		self.updated_requirement = true;
		Ok(())
	}
}

impl AsRef<Path> for AppCtx {
	fn as_ref(&self) -> &Path {
		&self.project_root
	}
}

pub fn open_edit_file<EF>(entry: &DirEntry) -> Result<EF>
where
	EF: EditFile,
{
	// open file
	let mut file = std::fs::OpenOptions::new()
		.read(true)
		.write(false)
		.truncate(false)
		.create(false)
		.open(&entry.path())
		.with_context(|| format!("{} can't open edit file: {:?}", err_loc!(), &entry.path()))?;

	// read
	let mut buf = String::with_capacity(
		entry
			.metadata()
			.ok()
			.map(|m| m.len() as usize)
			.unwrap_or(4096),
	);
	file.read_to_string(&mut buf)
		.with_context(|| format!("{} can't read edit file: {:?}", err_loc!(), &entry.path()))?;
	// deserialize
	let edit = toml::from_str::<EF>(buf.as_str()).with_context(|| {
		format!(
			"{} bad file content format in: {:?}",
			err_loc!(),
			&entry.file_name()
		)
	})?;

	edit.sanitize()
}

impl AsMut<Vec<Component>> for AppCtx {
	fn as_mut(&mut self) -> &mut Vec<Component> {
		&mut self.components
	}
}

impl AsMut<Vec<Requirement>> for AppCtx {
	fn as_mut(&mut self) -> &mut Vec<Requirement> {
		&mut self.requirements
	}
}

impl ComponentMenuCtx for AppCtx {}

pub fn init_project(project_root: impl AsRef<Path>) -> Result<()> {
	let component_path = project_root.as_ref().join(component::COMPONENT_TABLE_NAME);
	if component_path.exists() {
		return Err(anyhow!(format!(
			"{} component table: {:?} exists",
			err_loc!(),
			&component_path
		)));
	}
	let requirement_path = project_root
		.as_ref()
		.join(requirement::REQUIREMENT_TABLE_NAME);
	if requirement_path.exists() {
		return Err(anyhow!(format!(
			"{} requirement table: {:?} exists",
			err_loc!(),
			&requirement_path
		)));
	}

	let component_file = std::fs::OpenOptions::new()
		.read(true)
		.write(true)
		.truncate(true)
		.create(true)
		.open(&component_path)
		.with_context(|| {
			format!(
				"{} can't create component table at: {:?}",
				err_loc!(),
				&component_path
			)
		})?;

	write_flush_sync(
		WriteFlushSync::Done(component_file),
		COMPONENT_HEADER.as_bytes(),
	)?;

	let requirement_file = std::fs::OpenOptions::new()
		.read(true)
		.write(true)
		.truncate(true)
		.create(true)
		.open(&requirement_path)
		.with_context(|| {
			format!(
				"{} can't create requirement table at: {:?}",
				err_loc!(),
				&component_path
			)
		})?;

	write_flush_sync(
		WriteFlushSync::Done(requirement_file),
		REQUIREMENT_HEADER.as_bytes(),
	)?;

	Ok(())
}

#[derive(Debug, Clone)]
pub enum FileListOpt {
	DirEntry(DirEntry),
	Cancel,
}

impl Display for FileListOpt {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			FileListOpt::DirEntry(dir_entry) => {
				write!(f, "{}", dir_entry.path().to_string_lossy())
			}
			FileListOpt::Cancel => write!(f, "Cancel"),
		}
	}
}

impl From<DirEntry> for FileListOpt {
	fn from(inner: DirEntry) -> Self {
		Self::DirEntry(inner)
	}
}

// None = user canceled w/Esc`
fn file_list_chose(
	project_root: impl AsRef<Path>,
	file_filter: impl FnMut(&DirEntry) -> bool,
) -> Result<Option<DirEntry>> {
	let walker = WalkDir::new(project_root.as_ref())
		.min_depth(1)
		.max_depth(2)
		.into_iter();
	// find drafts
	let found: Vec<FileListOpt> = walker
		.filter_entry(file_filter)
		.filter_map(Result::ok)
		.map(FileListOpt::from)
		.collect();
	if found.is_empty() {
		return Err(anyhow!(format!(
			"{} no matching files in: {:?} to insert",
			err_loc!(),
			project_root.as_ref()
		)));
	}

	// prompt with drafts as list opts
	let ans: Option<FileListOpt> = inquire::Select::new("Select a file or cancel", found)
		.prompt_skippable()
		.context(formatcp!("{} can't prompt for file", err_loc!()))?;
	match ans {
		Some(FileListOpt::DirEntry(dir_entry)) => Ok(Some(dir_entry)),
		Some(FileListOpt::Cancel) => Ok(None),
		None => Ok(None),
	}
}

pub fn prompt_for_record_id() -> Result<Option<u64>> {
	inquire::CustomType::<u64>::new("What ID are you looking for?")
		.with_formatter(&|i| format!("ID: {}", i))
		.with_error_message("Please type a valid integer")
		.with_help_message("Type the integer number ID of the table entry")
		.prompt_skippable()
		.context("Can't prompt for u64")
}

pub trait EditFile: DeserializeOwned + Default + Serialize {
	fn sanitize(self) -> Result<Self>;
	fn fmt_as_draft(f: &mut impl std::fmt::Write) -> std::fmt::Result;
}

pub trait RecordType:
	Sized + DeserializeOwned + Ord + std::fmt::Debug + Serialize + Display
where
	Self::EditFile: EditFile,
{
	type EditFile;
	fn get_id(&self) -> u64;
	fn get_tbl_mut(app_ctx: &mut AppCtx) -> &mut Vec<Self>;
	fn get_tbl(app_ctx: &AppCtx) -> &Vec<Self>;
	fn write_table(app_ctx: &mut AppCtx) -> Result<()>;
	fn check_for_conflict(&self, rhs: &Self::EditFile) -> Result<()>;
	fn fmt_as_edit(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result;
	fn set_accepted(&mut self);
	fn set_deleted(&mut self);
	fn get_status(&self) -> RecordStatus;
	fn update_from_edit(&mut self, ef: Self::EditFile);
}

pub fn ref_record_by_id<R: RecordType>(app_ctx: &AppCtx, id: u64) -> Option<&R> {
	R::get_tbl(app_ctx).iter().find(|r| r.get_id() == id)
}

pub fn atomic_file_update(
	current: impl AsRef<Path>,
	file_content: &[u8],
	old_file: Option<File>,
) -> Result<()> {
	// create new
	let (new, old) = {
		let mut current_buf_a = current.as_ref().to_path_buf();
		let mut current_buf_b = current_buf_a.clone();
		let mut current_name = current.as_ref().file_name().unwrap().to_os_string();
		current_name.push(".new");
		current_buf_a.set_file_name(&current_name);
		current_name.clear();
		current_name.push(current.as_ref().file_name().unwrap());
		current_name.push(".old");
		current_buf_b.set_file_name(current_name);
		(current_buf_a, current_buf_b)
	};

	let f_new = OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(&new)
		.with_context(|| format!("{} can't create new file at: {:?}", err_loc!(), &new))?;

	write_flush_sync(WriteFlushSync::Done(f_new), file_content)
		.with_context(|| format!("{} can't save new file at: {:?}", err_loc!(), &new))?;

	if let Some(file) = old_file {
		drop(file);
	}

	// mv x.tsv x.old.tsv
	std::fs::rename(&current, &old).with_context(|| {
		format!(
			"{} can't move {:?} to {:?}",
			err_loc!(),
			&current.as_ref(),
			&old
		)
	})?;
	// mv x.new.tsv x.tsv
	std::fs::rename(&new, &current).with_context(|| {
		format!(
			"{} can't move {:?} to {:?}",
			err_loc!(),
			&new,
			&current.as_ref()
		)
	})?;
	// delete x.old.tsv
	std::fs::remove_file(&old).with_context(|| format!("{} can't delete {:?}", err_loc!(), &old))
}

pub fn create_edit_file<R: RecordType>(ctx: &mut AppCtx, edit_prefix: &'static str) -> Result<()> {
	let id = match prompt_for_record_id()
		.context(formatcp!("{} can't prompt for record ID", err_loc!()))?
	{
		Some(id) => id,
		None => return Ok(()),
	};
	// find record
	let op_res = crate::ref_record_by_id::<R>(ctx, id);
	match op_res {
		Some(record) => {
			// create document
			let mut path = std::path::PathBuf::from(ctx.as_ref());
			path.push(format!("{edit_prefix}-{}.toml", record.get_id()));
			let file = std::fs::OpenOptions::new()
				.read(true)
				.write(true)
				.truncate(true)
				.create(true)
				.open(&path)
				.with_context(|| format!("{} can't create file: {:?}", err_loc!(), &path))?;
			println!("Creating component edit file at: {:?}", &path);

			let mut edit_file_string = String::with_capacity(4096);

			record
				.fmt_as_edit(&mut edit_file_string)
				.expect("infallible write to String");

			crate::write_flush_sync(
				crate::WriteFlushSync::Done(file),
				edit_file_string.as_bytes(),
			)
		}
		None => return Err(anyhow!(format!("{} no record at ID: {id}", err_loc!()))),
	}
}

pub fn update_record<R: RecordType>(ctx: &mut AppCtx, edit_prefix: &str) -> Result<()> {
	// list update files
	let edit_file_entry = match file_list_chose(ctx.as_ref(), |e| {
		e.file_type().is_file()
			&& e.file_name()
				.to_str()
				.is_some_and(|s| s.starts_with(edit_prefix) && s.ends_with(".toml"))
	})? {
		Some(efe) => efe,
		None => return Ok(()),
	};
	// get number portion of the file name
	let number_portion = edit_file_entry
		.file_name()
		.to_str()
		.expect("we filter to_str() on is_some_and() when searching for files")
		.strip_prefix(edit_prefix)
		.expect("we check that name starts with the edit prefix when searching")
		.strip_suffix(".toml")
		.expect("we check that name ends with .toml when searching");
	// parse number
	let id: u64 = number_portion.parse::<u64>().with_context(|| {
		format!(
			"{} edit file name has non-integer in number portion: \"{number_portion}\"",
			err_loc!()
		)
	})?;

	// load update file
	let edit_file: R::EditFile = open_edit_file(&edit_file_entry)
		.with_context(|| format!("{} can't get edit file", err_loc!()))?;

	// load table

	let records: &mut Vec<R> = R::get_tbl_mut(ctx);

	// check for conflict as long as we aren't looking at the same record
	if let Some(e) = records.iter().find_map(|c| {
		// don't check for conflict if this is the same record we are editing
		if c.get_id() == id {
			return None;
		}
		match c.check_for_conflict(&edit_file) {
			Ok(()) => None,
			Err(e) => Some(e),
		}
	}) {
		return Err(e);
	}

	// find in db
	let record = match records.get_mut(id as usize) {
		Some(c) => c,
		None => records
			.iter_mut()
			.find(|c| c.get_id() == id)
			.ok_or_else(|| anyhow!(format!("{} no record at ID: {id}", err_loc!())))?,
	};

	// update
	record.update_from_edit(edit_file);
	record.set_accepted();
	// ctx.write_components()
	R::write_table(ctx)
}

pub fn load_table(table_path: impl AsRef<Path>, write: bool) -> Result<(File, String)> {
	// open table

	let mut file = std::fs::OpenOptions::new()
		.read(true)
		.write(write)
		.append(write)
		.truncate(false)
		.create(false)
		.open(&table_path)
		.with_context(|| {
			format!(
				"{} can't open table file: {:?}",
				err_loc!(),
				table_path.as_ref()
			)
		})?;

	// load all
	let mut buf = String::with_capacity(
		file.metadata()
			.ok()
			.map(|m| m.len() as usize)
			.unwrap_or(1_000_000),
	);
	use std::io::Read as _;
	file.read_to_string(&mut buf).with_context(|| {
		format!(
			"{} can't read table file: {:?}",
			err_loc!(),
			table_path.as_ref()
		)
	})?;
	Ok((file, buf))
}

#[derive(Debug)]
pub enum WriteFlushSync<'file> {
	Done(File),
	NotDone(&'file mut File),
}

impl<'file> WriteFlushSync<'file> {
	pub fn with_inner<T, F>(&mut self, mut f: F) -> T
	where
		F: FnMut(&mut File) -> T,
	{
		match self {
			WriteFlushSync::Done(file) => f(file),
			WriteFlushSync::NotDone(file) => f(file),
		}
	}
}

pub fn write_flush_sync(mut file: WriteFlushSync, file_content: &[u8]) -> Result<()> {
	file.with_inner(|f| f.write_all(file_content))
		.context(formatcp!("{} can't write to file", err_loc!()))?;

	file.with_inner(|f| f.flush())
		.context(formatcp!("{} can't fulsh file to disk", err_loc!()))?;

	file.with_inner(|f| f.sync_all())
		.context(formatcp!("{} can't sync file to disk", err_loc!()))
}

/// Escaptes LF and CRLF to a literal backslash: '\' and 'n'.
pub fn escape_normalize_nl(input: &str) -> Cow<'_, str> {
	let mut input = Cow::Borrowed(input);
	if input.contains("\r\n") {
		input = Cow::Owned(input.replace("\r\n", "\\n"))
	}
	if input.contains('\n') {
		input = Cow::Owned(input.replace('\n', "\\n"))
	}
	input
}

pub fn contains_any(to_find: &[char], search_str: &str) -> bool {
	search_str.chars().find(|ch| to_find.contains(ch)).is_some()
}

pub fn find_positions(to_find: &[char], search_str: &str) -> Option<Box<[(usize, char)]>> {
	let mut op_vec: Option<Vec<(usize, char)>> = None;
	for ch_idx in search_str
		.char_indices()
		.filter(|(_idx, ch)| to_find.contains(ch))
	{
		op_vec.get_or_insert(Vec::with_capacity(10)).push(ch_idx);
	}
	op_vec.map(|vec| vec.into_boxed_slice())
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
pub enum RecordStatus {
	Draft,
	Accepted,
	Deleted,
}

impl Display for RecordStatus {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			RecordStatus::Draft => write!(f, "Draft"),
			RecordStatus::Accepted => write!(f, "Accepted"),
			RecordStatus::Deleted => write!(f, "Deleted"),
		}
	}
}
pub fn create_draft_file<EF: EditFile>(ctx: &mut AppCtx, draft_prefix: &'static str) -> Result<()> {
	let (file_handle, file_path) =
		get_rand_file(&ctx.project_root, draft_prefix).map_err(|e| match e {
			Some(ioe) => {
				anyhow!(ioe).context(formatcp!("{} can't create new draft file", err_loc!()))
			}
			None => anyhow!(formatcp!(
				"{} somehow ran in to name conflicts several thousand times!?!? Please retry...",
				err_loc!()
			)),
		})?;

	println!("Creating draft at: {:?}", file_path);

	let mut buf = String::with_capacity(1024);

	EF::fmt_as_draft(&mut buf).expect("infallible write to String");

	crate::write_flush_sync(crate::WriteFlushSync::Done(file_handle), buf.as_bytes()).context(
		formatcp!("{} can't write default draft to disk", err_loc!()),
	)
}

pub fn get_rand_file(
	project_root: &std::path::Path,
	prefix: &'static str,
) -> Result<(std::fs::File, Box<std::path::Path>), Option<std::io::Error>> {
	const ALPHABET: &'static str = "_+=^~0123456789abcdefghigklmnopqrstufwxyz";
	const NUM_RAND_CHARS: usize = 12;
	const NUM_RETRIES: usize = NUM_RAND_CHARS * ALPHABET.len() * 10;
	const EXTENTION: &'static str = ".toml";

	use rand::seq::IteratorRandom;

	let mut _open_opts = std::fs::OpenOptions::new();
	let open_opts = _open_opts
		.write(true)
		.read(true)
		.truncate(false)
		.create_new(true);
	let file_name_len = prefix.len() + 1 + NUM_RAND_CHARS + EXTENTION.len();

	let mut file_name: String = String::with_capacity(file_name_len);
	let mut new_path: PathBuf =
		PathBuf::with_capacity(project_root.as_os_str().len() + file_name_len);
	new_path.push(project_root);
	for _ in 0..NUM_RETRIES {
		file_name.push_str(&prefix);
		file_name.push('-');
		for _ in 0..NUM_RAND_CHARS {
			file_name.push(ALPHABET.chars().choose(&mut rand::rng()).unwrap())
		}
		file_name.push_str(&EXTENTION);
		new_path.push(&file_name);
		let open_attempt = open_opts.open(&new_path);
		match open_attempt {
			Ok(f) => return Ok((f, new_path.into_boxed_path())),
			Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
				file_name.clear();
				new_path.pop();
			}
			Err(e) => return Err(Some(e)),
		}
	}
	Err(None)
}

pub fn delete_record<R>(ctx: &mut AppCtx) -> Result<()>
where
	R: RecordType,
{
	let op_id =
		prompt_for_record_id().context(formatcp!("{} can't prompt for record ID", err_loc!(),))?;
	match op_id {
		Some(id) => {
			println!("Attempting to delete record at ID: {id}...");
			// find it
			let records = R::get_tbl_mut(ctx);
			let record = mut_record_by_id(records, id)
				.context(formatcp!("{} can't find record", err_loc!(),))?;

			//TODO: serialize to TOML
			let toml_record = toml::to_string_pretty(&record).unwrap();

			println!("Found record: {}", toml_record);

			if let RecordStatus::Deleted = record.get_status() {
				return Err(anyhow!(formatcp!(
					"{} component already deleted",
					err_loc!(),
				)));
			}
			let ans = inquire::Confirm::new("Are you sure you want to delete that?")
				.with_default(false)
				.prompt()
				.context(formatcp!("{} can't prompt for author name", err_loc!()))?;
			match ans {
				true => {
					println!("Deleting...");
					record.set_deleted();
					R::write_table(ctx)
				}
				false => {
					println!("canceling...");
					Ok(())
				}
			}
		}
		None => Ok(()),
	}
}
