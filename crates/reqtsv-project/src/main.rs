use std::path::PathBuf;
use std::{io::Write, path::Path};

use argh::FromArgs;
use log::{LevelFilter, error, info};
use reqtsv_lib::{Project, get_project_root};
use reqtsv_project::{
	BuildDocsErr, InitProjectErr, ProjectCtx, build_docs, hashed_table, init_project,
};
use thiserror::Error;

fn main() -> Result<(), Error> {
	let reqtsv: ReqtsvProject = argh::from_env();
	env_logger::Builder::new()
		.format(|buf, record| {
			writeln!(
				buf,
				"{}:{} [{}] - {}",
				record.file().unwrap_or("unknown"),
				record.line().unwrap_or(0),
				record.level(),
				record.args()
			)
		})
		// order matters: the flag reqtsv.verbose will override the environment variable
		.parse_default_env()
		.filter_level(if reqtsv.verbose {
			LevelFilter::Trace
		} else {
			LevelFilter::Info
		})
		.init();

	if reqtsv.version {
		info!("Version: {}", env!("CARGO_PKG_VERSION"));
		info!(
			"Built with Rust version: {}",
			env!("CARGO_PKG_RUST_VERSION")
		);
		return Ok(());
	}

	let project_root = reqtsv.project.into_boxed_path();

	if reqtsv.init {
		info!("Creating new project at: {:?}", &project_root);
		init_project(&project_root).map_err(|e| Error::InitProject(e))?;
	}

	if reqtsv.build {
		info!("Building requirements docs at: {:?}/docs", &project_root);
		let project = get_project_root(&project_root).map_err(|gpre| Error::ProjectRoot(gpre))?;
		let mut ctx = CtxImpl::from(project);
		build_docs(&mut ctx).map_err(|e| Error::BuildDocs(e))?
	}

	Ok(())
}

struct CtxImpl {
	project: Project,
	component_tlb_hash: Box<str>,
	requirement_tlb_hash: Box<str>,
	css_path: Box<Path>,
}

impl From<Project> for CtxImpl {
	fn from(project: Project) -> Self {
		Self {
			component_tlb_hash: hashed_table(project.raw_components.as_bytes()),
			requirement_tlb_hash: hashed_table(project.raw_requirements.as_bytes()),
			css_path: project.root.join("styles.css").into_boxed_path(),
			project,
		}
	}
}

impl ProjectCtx for CtxImpl {
	fn get_project_title(&self) -> &str {
		"TODO Reqcsv Title"
	}

	fn get_project_root(&self) -> &std::path::Path {
		&self.project.root
	}

	fn get_requirement_tbl_hash(&self) -> &str {
		&self.requirement_tlb_hash
	}

	fn get_component_tbl_hash(&self) -> &str {
		&self.component_tlb_hash
	}

	fn get_components(&self) -> &[reqtsv_lib::Component] {
		&self.project.components
	}

	fn get_requirements(&self) -> &[reqtsv_lib::Requirement] {
		&self.project.requirements
	}

	fn get_css_path(&self) -> &std::path::Path {
		&self.css_path
	}
}

#[derive(Error, Debug)]
enum Error {
	#[error("Failed to initiallize project: {0:}")]
	InitProject(InitProjectErr),
	#[error("Failed to build docs for project: {0:}")]
	BuildDocs(BuildDocsErr),
	#[error("Failed to open project: {0:}")]
	ProjectRoot(reqtsv_lib::GetProjectRootErr),
}

#[derive(FromArgs, Debug, PartialEq)]
/// TSV Requirements Tracker - Project Commands.
struct ReqtsvProject {
	#[argh(switch)]
	/// print version number and exit
	version: bool,
	#[argh(positional)]
	/// directory containing requirements project
	project: PathBuf,
	#[argh(switch, short = 'i')]
	/// initialize project and exit
	init: bool,
	#[argh(switch, short = 'b')]
	/// build the specified project
	build: bool,
	#[argh(switch, short = 'v')]
	/// verbose logging
	verbose: bool,
}
