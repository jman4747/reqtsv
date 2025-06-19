use std::io::Write;
use std::path::PathBuf;

use argh::FromArgs;
use log::{LevelFilter, error, info};
use reqtsv_html_compiler::{BuildDocsErr, ProjectCtx, UserInputs, compile_html, hashed_table};
use reqtsv_lib::{Project, get_project_root};
use thiserror::Error;

fn main() -> Result<(), Error> {
	let reqtsv: ReqtsvHtml = argh::from_env();
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

	info!("Building requirements docs at: {:?}", &reqtsv.output);
	let project = get_project_root(reqtsv.project.as_path()).map_err(Error::ProjectRoot)?;
	let ctx = CtxImpl::from(project);
	compile_html(&ctx, reqtsv).map_err(Error::BuildDocs)
}

struct CtxImpl {
	project: Project,
	component_tlb_hash: Box<str>,
	requirement_tlb_hash: Box<str>,
}

impl From<Project> for CtxImpl {
	fn from(project: Project) -> Self {
		Self {
			component_tlb_hash: hashed_table(project.raw_components.as_bytes()),
			requirement_tlb_hash: hashed_table(project.raw_requirements.as_bytes()),
			project,
		}
	}
}

impl ProjectCtx for CtxImpl {
	fn get_project_title(&self) -> &str {
		&self.project.project_title
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
}

#[derive(Error, Debug)]
enum Error {
	#[error("Failed to build docs for project: {0:}")]
	BuildDocs(BuildDocsErr),
	#[error("Failed to open project: {0:}")]
	ProjectRoot(reqtsv_lib::GetProjectRootErr),
}

#[derive(FromArgs, Debug, PartialEq)]
/// TSV Requirements Tracker - HTML Compiler.
struct ReqtsvHtml {
	#[argh(switch)]
	/// print version number and exit
	version: bool,
	#[argh(option, short = 'p')]
	/// directory containing requirements project
	project: PathBuf,
	#[argh(option, short = 'o')]
	/// root directory of the html output (where index.html will go)
	output: PathBuf,
	#[argh(option, short = 'c')]
	/// css to use
	css: PathBuf,
	#[argh(switch, short = 'v')]
	/// verbose logging
	verbose: bool,
}

impl From<ReqtsvHtml> for UserInputs {
	fn from(val: ReqtsvHtml) -> Self {
		UserInputs {
			out_dir: val.output,
			css_path: val.css.into_boxed_path(),
		}
	}
}
