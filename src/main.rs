use std::{
	fs::rename,
	path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use argh::FromArgs;
use const_format::formatcp;
use inline_colorization::*;
use reqtsv::{
	AppCtx, COLUMN_DELIMITER,
	component::{self, COMPONENT_TABLE_NAME, Component},
	err_loc, init_project, load_table,
	project::{self},
	requirement::{REQUIREMENT_TABLE_NAME, Requirement, RequirementMenu},
	select_menu::select_menu_loop,
};

fn main() -> Result<()> {
	let reqtsv: Reqtsv = argh::from_env();
	if reqtsv.version {
		println!("Version: {}", env!("CARGO_PKG_VERSION"));
		println!(
			"Built with Rust version: {}",
			env!("CARGO_PKG_RUST_VERSION")
		);
		return Ok(());
	}

	// let temp = tempdir::TempDir::new("reqtsv_example").unwrap();
	// let project_root = temp.path();
	let project_root = reqtsv.project.into_boxed_path();

	if reqtsv.init {
		println!("Creating new project at: {:?}", &project_root);
		init_project(&project_root).context("Failed to initialize project")?;
	}

	//serialize and verify both tables
	println!("Loading component table...");
	let component_tbl_path = project_root.join(COMPONENT_TABLE_NAME).into_boxed_path();

	let (component_file, raw_component_tbl) = load_table(component_tbl_path, true)?;

	let mut tsv_reader = csv::ReaderBuilder::new()
		.delimiter(COLUMN_DELIMITER)
		.terminator(csv::Terminator::Any(b'\n'))
		.from_reader(raw_component_tbl.as_bytes());

	let max_records = raw_component_tbl.chars().filter(|ch| *ch == '\n').count();
	let mut components: Vec<Component> = Vec::with_capacity(max_records);
	for res in tsv_reader
		.deserialize::<Component>()
		.map(|res| res.context(formatcp!("{} corrupt table entry", err_loc!())))
	{
		match res {
			Ok(record) => {
				components.push(record);
			}
			Err(e) => return Err(e),
		}
	}

	println!("Loading requirement table...");
	let requirement_tbl_path = project_root.join(REQUIREMENT_TABLE_NAME).into_boxed_path();

	let (requirement_file, raw_requirement_tbl) = load_table(requirement_tbl_path, true)?;

	let mut tsv_reader = csv::ReaderBuilder::new()
		.delimiter(COLUMN_DELIMITER)
		.terminator(csv::Terminator::Any(b'\n'))
		.from_reader(raw_requirement_tbl.as_bytes());

	let max_records = raw_requirement_tbl.chars().filter(|ch| *ch == '\n').count();
	let mut requirements: Vec<Requirement> = Vec::with_capacity(max_records);
	for res in tsv_reader
		.deserialize::<Requirement>()
		.map(|res| res.context(formatcp!("{} corrupt table entry", err_loc!())))
	{
		match res {
			Ok(record) => {
				requirements.push(record);
			}
			Err(e) => return Err(e),
		}
	}

	println!("Project Root: {:?}", &project_root);
	let component_new_path: Box<Path> = project_root.join("component.new.tsv").into_boxed_path();
	let requirement_new_path: Box<Path> =
		project_root.join("requirement.new.tsv").into_boxed_path();
	let mut app_ctx = AppCtx {
		components,
		requirements,
		project_root,
		component_file,
		requirement_file,
		component_new_path,
		requirement_new_path,
		updated_component: false,
		updated_requirement: false,
	};

	if let Err(e) = main_menu_loop(&mut app_ctx) {
		for e in e.chain() {
			eprintln!("{color_red}{e}{color_reset}")
		}
		eprintln!("{color_red}Exiting...{color_reset}")
	};

	let req_new = app_ctx.requirement_new_path;
	let comp_new = app_ctx.component_new_path;

	drop(app_ctx.component_file);
	drop(app_ctx.requirement_file);
	let project_root = app_ctx.project_root;
	if app_ctx.updated_requirement {
		let req_current = project_root.join(REQUIREMENT_TABLE_NAME);
		let req_old = project_root.join("requirement.old.tsv");
		// mv x.tsv x.old.tsv
		match rename(&req_current, &req_old).with_context(|| {
			format!(
				"{} can't move {:?} to {:?}",
				err_loc!(),
				&req_current,
				&req_old
			)
		}) {
			Err(e) => {
				// we want to try moving the other table so don't return on error here
				eprintln!("{color_red}{e}{color_reset}")
			}
			_ => {
				// mv x.new.tsv x.tsv
				rename(&req_new, &req_current).with_context(|| {
					format!(
						"{} can't move {:?} to {:?}",
						err_loc!(),
						&req_new,
						&req_current
					)
				})?;
			}
		}
		std::fs::remove_file(&req_old)
			.with_context(|| format!("{} can't delete {:?}", err_loc!(), &req_old))?;
	}
	if app_ctx.updated_component {
		let comp_current = project_root.join(COMPONENT_TABLE_NAME);
		let comp_old = project_root.join("component.old.tsv");
		// mv x.tsv x.old.tsv
		// we don't want to try to move new to current after this if this errors...
		// so return on error
		rename(&comp_current, &comp_old).with_context(|| {
			format!(
				"{} can't move {:?} to {:?}",
				err_loc!(),
				&comp_current,
				&comp_old
			)
		})?;
		// mv x.new.tsv x.tsv
		rename(&comp_new, &comp_current).with_context(|| {
			format!(
				"{} can't move {:?} to {:?}",
				err_loc!(),
				&comp_current,
				&comp_old
			)
		})?;
		std::fs::remove_file(&comp_old)
			.with_context(|| format!("{} can't delete {:?}", err_loc!(), &comp_old))?;
	}
	Ok(())
}

#[derive(FromArgs, Debug, PartialEq)]
/// TSV Requirements tracker.
struct Reqtsv {
	#[argh(switch)]
	/// print version number and exit
	version: bool,
	#[argh(positional)]
	/// directory containing project
	project: PathBuf,
	#[argh(switch, short = 'i')]
	/// initialize project and exit
	init: bool,
}

#[derive(Debug, Copy, Clone)]
enum MainMenu {
	Component,
	Project,
	Exit,
	Requirement,
}

impl AsRef<str> for MainMenu {
	fn as_ref(&self) -> &str {
		match self {
			MainMenu::Component => "Component",
			MainMenu::Exit => "Exit",
			MainMenu::Project => "Project",
			MainMenu::Requirement => "Requirement",
		}
	}
}

#[derive(Debug, Copy, Clone)]
enum DoNext {
	ComponentMenu,
	ProjectMenu,
	RequirementMenu,
	Exit,
	Loop,
}

impl MainMenu {
	pub fn visit(&self, selection: &str) -> bool {
		selection == self.as_ref()
	}
}

fn main_menu_loop(app_ctx: &mut AppCtx) -> Result<()> {
	loop {
		let state = main_menu()?;
		match state {
			DoNext::ComponentMenu => {
				select_menu_loop::<component::ComponentMenu>(app_ctx, "components")?;
			}
			DoNext::ProjectMenu => {
				select_menu_loop::<project::ProjectMenu>(app_ctx, "projects")?;
			}
			DoNext::RequirementMenu => {
				select_menu_loop::<RequirementMenu>(app_ctx, "requirements")?;
			}
			DoNext::Exit => {
				println!("Exiting...");
				return Ok(());
			}
			DoNext::Loop => {}
		}
	}
}

fn main_menu() -> Result<DoNext> {
	let options: Vec<&str> = vec![
		MainMenu::Requirement.as_ref(),
		MainMenu::Component.as_ref(),
		MainMenu::Project.as_ref(),
		MainMenu::Exit.as_ref(),
	];

	let ans: Result<Option<&str>, inquire::InquireError> =
		inquire::Select::new("What would you like to operate on?", options).prompt_skippable();

	match ans {
		Ok(Some(choice)) if MainMenu::Requirement.visit(choice) => Ok(DoNext::RequirementMenu),
		Ok(Some(choice)) if MainMenu::Component.visit(choice) => Ok(DoNext::ComponentMenu),
		Ok(Some(choice)) if MainMenu::Project.visit(choice) => Ok(DoNext::ProjectMenu),
		Ok(Some(choice)) if MainMenu::Exit.visit(choice) => Ok(DoNext::Exit),
		Err(iqe) => {
			Err(anyhow!(iqe).context(formatcp!("{} error prompting main menu", err_loc!())))
		}
		Ok(None) => Ok(DoNext::Exit),
		Ok(_choice) => Ok(DoNext::Loop),
	}
}
