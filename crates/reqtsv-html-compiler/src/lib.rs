use std::{
	fmt::{Display, Write},
	fs::{File, OpenOptions, copy, create_dir},
	io::Write as _,
	path::{Path, PathBuf},
};

use chrono::{DateTime, Local};
use log::{debug, error, info};
use maud::{Markup, Render, html};
use out_dir::{
	OutDir, OutDirAllRequirements, OutDirCSS, OutDirComponents, OutDirComponentsDir, OutDirIndex,
};
use reqtsv_lib::{
	COMPONENT_HEADER, Component, REQUIREMENT_HEADER, Requirement, SaveFileError, save_file_strict,
};
use sha3::Digest;
use thiserror::Error;

pub mod out_dir;

#[derive(Error, Debug)]
pub enum InitProjectErr {
	#[error("Component table already exists at: {0:?}")]
	ComponentTblExists(Box<Path>),
	#[error("Requirement table already exists at: {0:?}")]
	RequirementTblExists(Box<Path>),
	#[error("can't create Component table at: {path:?} due to: {ioe}")]
	CreateComponentTbl {
		path: Box<Path>,
		#[source]
		ioe: std::io::Error,
	},
	#[error("can't create Requirement table at: {path:?} due to: {ioe}")]
	CreateRequirementTbl {
		path: Box<Path>,
		#[source]
		ioe: std::io::Error,
	},
	#[error("can't write Component table due to: {0:?}")]
	WriteComponentTbl(SaveFileError),
	#[error("can't write Requirement table due to: {0:?}")]
	WriteRequirementTbl(SaveFileError),
}

pub fn init_project(project_root: impl AsRef<Path>) -> Result<(), InitProjectErr> {
	// reqcsv.toml
	// reqcsv.lock
	// styles.css

	let component_path = project_root.as_ref().join(reqtsv_lib::COMPONENT_TABLE_NAME);
	if component_path.exists() {
		return Err(InitProjectErr::ComponentTblExists(
			component_path.into_boxed_path(),
		));
	}
	let requirement_path = project_root
		.as_ref()
		.join(reqtsv_lib::REQUIREMENT_TABLE_NAME);
	if requirement_path.exists() {
		return Err(InitProjectErr::RequirementTblExists(
			requirement_path.into_boxed_path(),
		));
	}

	let component_file = std::fs::OpenOptions::new()
		.read(true)
		.write(true)
		.truncate(true)
		.create(true)
		.open(&component_path)
		.map_err(|ioe| InitProjectErr::CreateComponentTbl {
			path: component_path.into_boxed_path(),
			ioe,
		})?;

	save_file_strict(component_file, COMPONENT_HEADER.as_bytes())
		.map_err(InitProjectErr::WriteComponentTbl)?;

	let requirement_file = std::fs::OpenOptions::new()
		.read(true)
		.write(true)
		.truncate(true)
		.create(true)
		.open(&requirement_path)
		.map_err(|ioe| InitProjectErr::CreateRequirementTbl {
			path: requirement_path.into_boxed_path(),
			ioe,
		})?;

	save_file_strict(requirement_file, REQUIREMENT_HEADER.as_bytes())
		.map_err(InitProjectErr::WriteRequirementTbl)?;

	Ok(())
}

pub trait ProjectCtx {
	fn get_project_title(&self) -> &str;
	fn get_requirement_tbl_hash(&self) -> &str;
	fn get_component_tbl_hash(&self) -> &str;
	fn get_components(&self) -> &[Component];
	fn get_requirements(&self) -> &[Requirement];
}

fn open_overwrite<P: AsRef<Path>>(path: P) -> Result<File, std::io::Error> {
	OpenOptions::new()
		.read(true)
		.write(true)
		.truncate(true)
		.create(true)
		.append(false)
		.open(path)
}

fn open_and_save(path: &Path, content: &str) -> Result<(), BuildDocsErr> {
	let mut file =
		open_overwrite(path).map_err(|e| BuildDocsErr::CreateOrReplace(e, path.into()))?;

	debug!("writing all {} bytes to: {:?}...", content.len(), path);
	file.write_all(content.as_bytes())
		.map_err(|e| BuildDocsErr::SaveFile(e, path.into()))
		.inspect_err(|e| error!("{e}"))
}

#[derive(Debug)]
pub struct DisplayComponentPageName<'c>(&'c Component);

impl<'c> Display for DisplayComponentPageName<'c> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}-", self.0.id)?;
		let name_chars = self
			.0
			.name
			.chars()
			.map(|ch| if ch == ' ' { '_' } else { ch });

		for ch in name_chars {
			f.write_char(ch)?;
		}
		write!(f, ".html")
	}
}

#[derive(Error, Debug)]
pub enum BuildDocsErr {
	#[error("a file with the name of the output directory exists at: {0:?}")]
	FileConflict(Box<Path>),
	#[error("can't create output directory: {1:?}, source error: {0:}")]
	CreateOutDir(#[source] std::io::Error, Box<Path>),
	#[error("can't open or replace: {1:?}, source error: {0:}")]
	CreateOrReplace(#[source] std::io::Error, Box<Path>),
	#[error("can't save: {1:?}, source error: {0:}")]
	SaveFile(#[source] std::io::Error, Box<Path>),
	#[error("can't create components directory: {1:?}, source error: {0:}")]
	CreateComponentsDir(#[source] std::io::Error, Box<Path>),
	#[error("a file with the name \"components\" exists in the build directory at: {0:?}")]
	ComponentsDirFileConflict(Box<Path>),
	#[error("can't copy {1:?} to {2:?}, source error: {0:}")]
	CopyCss(#[source] std::io::Error, Box<Path>, Box<Path>),
}

#[derive(Debug)]
pub struct UserInputs {
	pub out_dir: PathBuf,
	pub css_path: Box<Path>,
}

pub fn compile_html<Ctx>(ctx: &Ctx, inputs: impl Into<UserInputs>) -> Result<(), BuildDocsErr>
where
	Ctx: ProjectCtx,
{
	let inputs: UserInputs = inputs.into();
	let mut out_dir = OutDir::from_pathbuf(inputs.out_dir);
	// find the old if it exists
	if out_dir.exists() {
		if out_dir.is_file() {
			let e = BuildDocsErr::FileConflict(out_dir.as_path().into());
			error!("{e}");
			return Err(e);
		}
		debug!("output directory exists at: {:?}", &out_dir);
		// check for components dir
		if out_dir.components_dir_is_file() {
			let e = BuildDocsErr::ComponentsDirFileConflict(out_dir.as_path().into());
			error!("{e}");
			return Err(e);
		}
	} else {
		info!("creating output directory at: {:?}", &out_dir);
		create_dir(&out_dir)
			.map_err(|ioe| BuildDocsErr::CreateOutDir(ioe, out_dir.clone_inner().into_boxed_path()))
			.inspect_err(|e| error!("{e}"))?;
	}

	// index.html
	info!("Building index.html");
	let index_path = OutDirIndex::from_parent(out_dir);
	let index_str = build_index(ctx);
	info!("Saving: {:?}", index_path.as_path());
	open_and_save(index_path.as_path(), &index_str).inspect_err(|e| error!("{e}"))?;

	info!("Building components.html");
	let component_path = OutDirComponents::from_parent(index_path.to_parent());
	let components_str = build_components(ctx);
	info!("Saving: {:?}", component_path.as_path());
	open_and_save(component_path.as_path(), &components_str).inspect_err(|e| error!("{e}"))?;

	info!("Building all-requirements.html");
	let all_requirements_str = build_all_requirements(ctx);
	let all_requirements_path = OutDirAllRequirements::from_parent(component_path.to_parent());
	info!("Saving: {:?}", all_requirements_path.as_path());
	open_and_save(all_requirements_path.as_path(), &all_requirements_str)
		.inspect_err(|e| error!("{e}"))?;

	// components/{component}.html
	let mut components_dir = OutDirComponentsDir::from_parent(all_requirements_path.to_parent());

	if !components_dir.exists() {
		info!("Createing component directory {:?}", &components_dir);
		create_dir(&components_dir)
			.map_err(|e| BuildDocsErr::CreateComponentsDir(e, components_dir.as_path().into()))
			.inspect_err(|e| error!("{e}"))?;
	}

	info!("Building component pages");
	let mut file_name_buf = String::with_capacity(256);
	for component in ctx.get_components() {
		// TODO: should I filter and delete on build or "clean" lazily?
		// don't filter and delete yet...
		write!(
			&mut file_name_buf,
			"{}",
			DisplayComponentPageName(component)
		)
		.unwrap();
		debug!("createing component file: {:?}", file_name_buf);

		components_dir
			.with_pushed(file_name_buf.as_str(), |path| {
				let component_str = build_a_component(ctx, component);
				open_and_save(path, &component_str)
			})
			.inspect_err(|e| error!("{e}"))?;

		file_name_buf.clear();
	}
	info!("Copying CSS");
	let css_out_path = OutDirCSS::from_parent(components_dir.to_parent());
	copy(&inputs.css_path, css_out_path.as_path())
		.map_err(|e| BuildDocsErr::CopyCss(e, inputs.css_path, css_out_path.as_path().into()))
		.inspect_err(|e| error!("{e}"))?;
	Ok(())
}

fn generic_root_page(body: Markup, title: &str, sub_title: Option<impl Render>) -> Box<str> {
	_generic_page(body, title, sub_title, "./index.html", "./styles.css")
}

fn generic_sub_page(body: Markup, title: &str, sub_title: Option<impl Render>) -> Box<str> {
	_generic_page(body, title, sub_title, "../index.html", "../styles.css")
}

fn _generic_page(
	body: Markup,
	title: &str,
	sub_title: Option<impl Render>,
	index_path: &str,
	style_sheet_path: &str,
) -> Box<str> {
	html! {
		(maud::DOCTYPE)
		meta charset="utf-8";
		title { (title) @if let Some(sub) = sub_title {" - " (sub)}}
		link rel="stylesheet" type="text/css" href=(style_sheet_path);
		body {
			p {a href=(index_path) { "Project Home" }}
			(body)
		}
		"\n"
	}
	.into_string()
	.into_boxed_str()
}

#[derive(Debug)]
pub struct RenderComponentPagePath<'c>(&'c Component);

impl<'c> Display for RenderComponentPagePath<'c> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "./components/{}-", self.0.id)?;
		let name_chars = self
			.0
			.name
			.chars()
			.map(|ch| if ch == ' ' { '_' } else { ch });

		for ch in name_chars {
			f.write_char(ch)?;
		}
		write!(f, ".html")
	}
}

impl<'c> Render for RenderComponentPagePath<'c> {
	fn render_to(&self, buffer: &mut String) {
		write!(buffer, "{}", &self).unwrap()
	}
}

pub fn build_a_component<Ctx>(ctx: &Ctx, component: &Component) -> Box<str>
where
	Ctx: ProjectCtx,
{
	struct SubTitle<'c>(&'c Component);
	impl<'c> Render for SubTitle<'c> {
		fn render(&self) -> Markup {
			let mut buffer = String::new();
			self.render_to(&mut buffer);
			maud::PreEscaped(buffer)
		}

		fn render_to(&self, buffer: &mut String) {
			write!(buffer, "Component: {} - {}", self.0.id, self.0.name).unwrap()
		}
	}
	// The component with links to each requirement, in order of ID.
	let requirements = ctx.get_requirements();
	let component_tbl_hash = ctx.get_component_tbl_hash();
	let requirements_tbl_hash = ctx.get_requirement_tbl_hash();
	let date = DateWrapper(&component.creation_date);
	let filtered = requirements
		.iter()
		.filter(|requriemnt| requriemnt.component_id == component.id);
	let body = html! {
		p {a href="../components.html" { "All Components" }}
		h1 { "ID: " (component.id) " - " (component.name)}
		p { span class="key" {"Components table hash: "} (component_tbl_hash)}
		p { span class="key" {"Requirements table hash: "} (requirements_tbl_hash)}
		p {span class="key" {"Status: "} span class="value" {(component.status)}}
		p {span class="key" {"Date Created: "} span class="value" {(date)}}
		p {span class="key" {"Author: "} span class="value" {(component.author)}}
		h2 {"Description"}
		p {(component.description)}
		h2 {"Requirements"}
		br;
		@for requirement in filtered {
			article id=(requirement.id) class="entry" {
				h2 { (requirement.id) " - " (requirement.title)}
				p {span class="key" {"Status: "} span class="value" {(requirement.status)}}
				p {span class="key" {"Version: "} span class="value" {(requirement.version)}}
				p {span class="key" {"Date Created: "} span class="value" {(date)}}
				p {span class="key" {"Author: "} span class="value" {(requirement.author)}}
				p {span class="key" {"Type: "} span class="value" {(requirement.functional)}}
				p {span class="key" {"Priority: "} span class="value" {(requirement.priority)}}
				h3 {"Requirement Text"}
				p {(requirement.requirement_text)}
				h3 {"Risks"}
				p {(requirement.risks)}
			}
		}
	};
	generic_sub_page(body, ctx.get_project_title(), Some(SubTitle(component)))
}

#[derive(Debug)]
struct DateWrapper<'dt>(&'dt DateTime<Local>);

impl<'dt> From<&'dt DateTime<Local>> for DateWrapper<'dt> {
	fn from(value: &'dt DateTime<Local>) -> Self {
		Self(value)
	}
}

impl<'dt> Display for DateWrapper<'dt> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0.format("%Y-%m-%d"))
	}
}

impl<'dt> Render for DateWrapper<'dt> {
	fn render(&self) -> maud::Markup {
		let mut buffer = String::new();
		self.render_to(&mut buffer);
		maud::PreEscaped(buffer)
	}

	fn render_to(&self, buffer: &mut String) {
		write!(buffer, "{}", self.0.format("%Y-%m-%d")).unwrap()
	}
}

fn find_component_by_id(id: u64, components: &[Component]) -> Option<&Component> {
	components
		.get(id as usize)
		.and_then(|comp| if comp.id == id { Some(comp) } else { None })
		.or_else(|| components.iter().find(|comp| comp.id == id))
}

#[derive(Debug)]
struct OpCompNameRender<'c>(pub Option<&'c Component>);

impl<'c> Render for OpCompNameRender<'c> {
	fn render(&self) -> maud::Markup {
		let mut buffer = String::new();
		self.render_to(&mut buffer);
		maud::PreEscaped(buffer)
	}

	fn render_to(&self, buffer: &mut String) {
		match self.0 {
			Some(component) => write!(buffer, "{} - {}", component.id, component.name).unwrap(),
			None => write!(buffer, "Not Found").unwrap(),
		}
	}
}

pub fn build_all_requirements<Ctx>(ctx: &Ctx) -> Box<str>
where
	Ctx: ProjectCtx,
{
	let project_title = ctx.get_project_title();
	let requirements_tbl_hash = ctx.get_requirement_tbl_hash();
	let requirements: &[Requirement] = ctx.get_requirements();
	let components = ctx.get_components();

	let len = requirements.len();
	let body = html! {
			h1 { "Requirement Table Info" }
			p { span class="key" {"Requirements table hash: "} (requirements_tbl_hash)}
			p {"Number of Requirements: " (len)}
			h1 {"Requirement List"}
			@for requirement in requirements {
				@let date: DateWrapper = (&requirement.creation_date).into();
				@let op_component = find_component_by_id(requirement.component_id, components);
				@let comp_render = OpCompNameRender(op_component);
				article id=(requirement.id) class="entry" {
					h2 { (requirement.id) " - " (requirement.title)}
					p {span class="key" {"Status: "} span class="value" {(requirement.status)}}
					p {span class="key" {"Version: "} span class="value" {(requirement.version)}}
					p {span class="key" {"Date Created: "} span class="value" {(date)}}
					p {span class="key" {"Author: "} span class="value" {(requirement.author)}}
					@if let Some(component) = op_component {
						@let component_page_path = RenderComponentPagePath(component);
						p {span class="key" {"Component: "} a href=(component_page_path) {(comp_render)}}
					} @else {
						p {span class="key" {"Component: "} span class="value" {(comp_render)}}

					}
					p {span class="key" {"Type: "} span class="value" {(requirement.functional)}}
					p {span class="key" {"Priority: "} span class="value" {(requirement.priority)}}
					h3 {"Requirement Text"}
					p {(requirement.requirement_text)}
					h3 {"Risks"}
					p {(requirement.risks)}
				}
			}
	};
	generic_root_page(body, project_title, Some("All Requirements"))
}

pub fn build_components<Ctx>(ctx: &Ctx) -> Box<str>
where
	Ctx: ProjectCtx,
{
	let project_title = ctx.get_project_title();
	let components_table_hash = ctx.get_component_tbl_hash();
	let components = ctx.get_components();
	let len = components.len();
	let body = html! {
		h1 { "Component Table Info" }
		p { span class="key" {"Components table hash: "} (components_table_hash)}
		p {"Number of Components: " (len)}
		h1 {"Component List"}
		@for component in components {
			@let date: DateWrapper = (&component.creation_date).into();
			article id=(component.id) class="entry" {
				@let component_page_path = RenderComponentPagePath(component);
				h2 {
					a href=(component_page_path) { (component.id) " - " (component.name)}
				}
				p {span class="key" {"Status: "} span class="value" {(component.status)}}
				p {span class="key" {"Date Created: "} span class="value" {(date)}}
				p {span class="key" {"Author: "} span class="value" {(component.author)}}
				h3 {"Description"}
				p {(component.description)}
			}
		}
	};
	generic_root_page(body, project_title, Some("Components"))
}

pub fn build_index(ctx: &impl ProjectCtx) -> Box<str> {
	let project_title = ctx.get_project_title();
	let requirements_table_hash = ctx.get_requirement_tbl_hash();
	let components_table_hash = ctx.get_component_tbl_hash();
	let components = ctx.get_components();
	let body = html! {
			h1 { "Project Info" }
			p { span class="key" {"Title: "} (project_title) }
			a href="https://github.com/jman4747/reqtsv" { "Project Repository" }
			p { span class="key" {"Requirements Table Hash: "} (requirements_table_hash) }
			p { span class="key" {"Components Table Hash: "} (components_table_hash) }
			h1 { "Pages" }
			p { a href="./components.html" {"Components"}}
			p { a href="./all-requirements.html" {"Requirements"}}
			h2 {"Component Pages"}
			@for component in components {
				@let component_page_path = RenderComponentPagePath(component);
				p {
					a href=(component_page_path) { (component.id) " - " (component.name)}
				}
			}
	};
	let _n: Option<&str> = None;
	generic_root_page(body, project_title, _n)
}

pub fn hashed_table(raw_table: impl AsRef<[u8]>) -> Box<str> {
	let mut hasher = sha3::Sha3_256::new();
	hasher.update(raw_table.as_ref());
	let digest = hasher.finalize();
	let mut buf = [0; 64];
	Box::from(base16ct::upper::encode_str(digest.as_slice(), &mut buf).unwrap())
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::prelude::*;
	use reqtsv_lib::{RecordStatus, RequirementFunctional, RequirementPriority};

	#[test]
	fn test_table_hash() {
		assert_eq!(
			"8F8EAAD16CBF8722A2165B660D47FCFD8496A41C611DA758F3BB70F809F01EE3",
			hashed_table(b"0123456789").as_ref()
		)
	}

	#[test]
	fn test_build_index() {
		struct MockProject([Component; 1]);
		impl ProjectCtx for MockProject {
			fn get_project_title(&self) -> &str {
				"Reqcsv Title"
			}

			fn get_requirement_tbl_hash(&self) -> &str {
				"123"
			}

			fn get_component_tbl_hash(&self) -> &str {
				"ABC"
			}

			fn get_components(&self) -> &[Component] {
				&self.0
			}

			fn get_requirements(&self) -> &[Requirement] {
				todo!()
			}
		}
		let comp = Component {
			id: 0,
			name: "Comp A".into(),
			description: "Test A".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 16, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author A".into(),
		};
		let ctx = MockProject([comp]);
		let built = build_index(&ctx);
		let page = include_str!("./index.html");
		assert_eq!(
			page,
			built.as_ref(),
			"\nexpected:\n{}\nbuilt:\n{}\n",
			page,
			built.as_ref()
		)
	}

	#[test]
	fn test_build_components() {
		struct MockProject([Component; 2]);
		let comp_a = Component {
			id: 0,
			name: "Comp A".into(),
			description: "Test A".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 16, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author A".into(),
		};
		let comp_b = Component {
			id: 1,
			name: "Comp B".into(),
			description: "Test B".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 17, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author B".into(),
		};
		let components: [Component; 2] = [comp_a, comp_b];
		impl ProjectCtx for MockProject {
			fn get_project_title(&self) -> &str {
				"Reqcsv Title"
			}

			fn get_requirement_tbl_hash(&self) -> &str {
				"123"
			}

			fn get_component_tbl_hash(&self) -> &str {
				"ABC"
			}

			fn get_components(&self) -> &[Component] {
				&self.0
			}

			fn get_requirements(&self) -> &[Requirement] {
				todo!()
			}
		}
		let ctx = MockProject(components);
		let built = build_components(&ctx);
		let page = include_str!("./components.html");
		assert_eq!(
			page,
			built.as_ref(),
			"\nexpected:\n{}\nbuilt:\n{}\n",
			page,
			built.as_ref()
		)
	}

	#[test]
	fn test_build_all_requirements() {
		struct MockProject([Requirement; 2], [Component; 2]);
		let req_a = Requirement {
			id: 0,
			title: "Requirement A".into(),
			requirement_text: "Thing shall do A".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 16, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author A".into(),
			component_id: 0,
			functional: RequirementFunctional::Functional,
			version: 0,
			priority: RequirementPriority::Mandated,
			risks: "Risk A".into(),
		};
		let req_b = Requirement {
			id: 1,
			title: "Requirement B".into(),
			requirement_text: "Thing shall do B".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 17, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author B".into(),
			component_id: 1,
			functional: RequirementFunctional::Functional,
			version: 0,
			priority: RequirementPriority::Mandated,
			risks: "Risk B".into(),
		};
		let requirements: [Requirement; 2] = [req_a, req_b];
		let comp_a = Component {
			id: 0,
			name: "Comp A".into(),
			description: "Test A".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 16, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author A".into(),
		};
		let comp_b = Component {
			id: 1,
			name: "Comp B".into(),
			description: "Test B".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 17, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author B".into(),
		};
		let components: [Component; 2] = [comp_a, comp_b];
		impl ProjectCtx for MockProject {
			fn get_project_title(&self) -> &str {
				"Reqcsv Title"
			}

			fn get_requirement_tbl_hash(&self) -> &str {
				"123"
			}

			fn get_component_tbl_hash(&self) -> &str {
				"ABC"
			}

			fn get_components(&self) -> &[Component] {
				&self.1
			}

			fn get_requirements(&self) -> &[Requirement] {
				&self.0
			}
		}
		let ctx = MockProject(requirements, components);
		let built = build_all_requirements(&ctx);
		let page = include_str!("./all-requirements.html");
		assert_eq!(
			page,
			built.as_ref(),
			"\nexpected:\n{}\nbuilt:\n{}\n",
			page,
			built.as_ref()
		)
	}

	#[test]
	fn test_build_all_requirements_component_pos() {
		struct MockProject([Requirement; 2], [Component; 2]);
		let req_a = Requirement {
			id: 0,
			title: "Requirement A".into(),
			requirement_text: "Thing shall do A".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 16, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author A".into(),
			component_id: 0,
			functional: RequirementFunctional::Functional,
			version: 0,
			priority: RequirementPriority::Mandated,
			risks: "Risk A".into(),
		};
		let req_b = Requirement {
			id: 1,
			title: "Requirement B".into(),
			requirement_text: "Thing shall do B".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 17, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author B".into(),
			component_id: 1,
			functional: RequirementFunctional::Functional,
			version: 0,
			priority: RequirementPriority::Mandated,
			risks: "Risk B".into(),
		};
		let requirements: [Requirement; 2] = [req_a, req_b];
		let comp_a = Component {
			id: 0,
			name: "Comp A".into(),
			description: "Test A".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 16, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author A".into(),
		};
		let comp_b = Component {
			id: 1,
			name: "Comp B".into(),
			description: "Test B".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 17, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author B".into(),
		};
		// this is the key part of this test!
		let components: [Component; 2] = [comp_b, comp_a];
		impl ProjectCtx for MockProject {
			fn get_project_title(&self) -> &str {
				"Reqcsv Title"
			}

			fn get_requirement_tbl_hash(&self) -> &str {
				"123"
			}

			fn get_component_tbl_hash(&self) -> &str {
				"ABC"
			}

			fn get_components(&self) -> &[Component] {
				&self.1
			}

			fn get_requirements(&self) -> &[Requirement] {
				&self.0
			}
		}
		let ctx = MockProject(requirements, components);
		let built = build_all_requirements(&ctx);
		let page = include_str!("./all-requirements.html");
		assert_eq!(
			page,
			built.as_ref(),
			"\nexpected:\n{}\nbuilt:\n{}\n",
			page,
			built.as_ref()
		)
	}

	#[test]
	fn test_build_all_requirements_missing_component() {
		struct MockProject([Requirement; 2], [Component; 1]);
		let req_a = Requirement {
			id: 0,
			title: "Requirement A".into(),
			requirement_text: "Thing shall do A".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 16, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author A".into(),
			component_id: 0,
			functional: RequirementFunctional::Functional,
			version: 0,
			priority: RequirementPriority::Mandated,
			risks: "Risk A".into(),
		};
		let req_b = Requirement {
			id: 1,
			title: "Requirement B".into(),
			requirement_text: "Thing shall do B".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 17, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author B".into(),
			component_id: 1,
			functional: RequirementFunctional::Functional,
			version: 0,
			priority: RequirementPriority::Mandated,
			risks: "Risk B".into(),
		};
		let requirements: [Requirement; 2] = [req_a, req_b];
		let comp_a = Component {
			id: 0,
			name: "Comp A".into(),
			description: "Test A".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 16, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author A".into(),
		};
		// this is the key part of this test!
		let components: [Component; 1] = [comp_a];
		impl ProjectCtx for MockProject {
			fn get_project_title(&self) -> &str {
				"Reqcsv Title"
			}

			fn get_requirement_tbl_hash(&self) -> &str {
				"123"
			}

			fn get_component_tbl_hash(&self) -> &str {
				"ABC"
			}

			fn get_components(&self) -> &[Component] {
				&self.1
			}

			fn get_requirements(&self) -> &[Requirement] {
				&self.0
			}
		}
		let ctx = MockProject(requirements, components);
		let built = build_all_requirements(&ctx);
		let page = include_str!("./all-requirements-missing-component.html");
		assert_eq!(
			page,
			built.as_ref(),
			"\nexpected:\n{}\nbuilt:\n{}\n",
			page,
			built.as_ref()
		)
	}

	#[test]
	fn test_build_a_component() {
		struct MockProject([Requirement; 3]);
		let req_a = Requirement {
			id: 0,
			title: "Requirement A".into(),
			requirement_text: "Thing shall do A".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 16, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author A".into(),
			component_id: 0,
			functional: RequirementFunctional::Functional,
			version: 0,
			priority: RequirementPriority::Mandated,
			risks: "Risk A".into(),
		};
		let req_b = Requirement {
			id: 1,
			title: "Requirement B".into(),
			requirement_text: "Thing shall do B".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 17, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author B".into(),
			component_id: 1,
			functional: RequirementFunctional::Functional,
			version: 0,
			priority: RequirementPriority::Mandated,
			risks: "Risk B".into(),
		};
		let req_c = Requirement {
			id: 2,
			title: "Requirement C".into(),
			requirement_text: "Thing shall do c".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 18, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author C".into(),
			component_id: 0,
			functional: RequirementFunctional::Functional,
			version: 0,
			priority: RequirementPriority::Mandated,
			risks: "Risk C".into(),
		};
		let requirements: [Requirement; 3] = [req_a, req_b, req_c];
		let component = Component {
			id: 0,
			name: "Comp A".into(),
			description: "Test A".into(),
			creation_date: Local.with_ymd_and_hms(2025, 06, 16, 0, 0, 0).unwrap(),
			status: RecordStatus::Accepted,
			author: "Author A".into(),
		};
		// this is the key part of this test!
		impl ProjectCtx for MockProject {
			fn get_project_title(&self) -> &str {
				"Reqcsv Title"
			}

			fn get_requirement_tbl_hash(&self) -> &str {
				"123"
			}

			fn get_component_tbl_hash(&self) -> &str {
				"ABC"
			}

			fn get_components(&self) -> &[Component] {
				todo!()
			}

			fn get_requirements(&self) -> &[Requirement] {
				&self.0
			}
		}
		let ctx = MockProject(requirements);
		let built = build_a_component(&ctx, &component);
		let page = include_str!("./components/0-Comp_A.html");
		assert_eq!(
			page,
			built.as_ref(),
			"\nexpected:\n{}\nbuilt:\n{}\n",
			page,
			built.as_ref()
		)
	}
}
