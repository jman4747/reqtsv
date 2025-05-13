use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct BuildDir(PathBuf);

impl BuildDir {
	pub fn from_pathbuf(p: PathBuf) -> Self {
		Self(p)
	}

	pub fn as_path(&self) -> &Path {
		self.0.as_path()
	}

	pub fn exists(&self) -> bool {
		self.0.exists()
	}

	pub fn is_file(&self) -> bool {
		self.0.is_file()
	}

	pub fn clone_inner(&self) -> PathBuf {
		self.0.clone()
	}

	pub fn components_dir_is_file(&mut self) -> bool {
		self.0.push("components");
		let is_file = self.0.is_file();
		self.0.pop();
		is_file
	}
}

impl AsRef<Path> for BuildDir {
	fn as_ref(&self) -> &Path {
		&self.0
	}
}

macro_rules! build_dir_file {
	($child_type:ident, $file_name:literal) => {
		#[derive(Debug)]
		pub struct $child_type(PathBuf);

		impl $child_type {
			pub fn as_path(&self) -> &Path {
				self.0.as_path()
			}

			pub fn from_build_dir(mut bd: BuildDir) -> Self {
				bd.0.push($file_name);
				Self(bd.0)
			}

			pub fn to_build_dir(mut self) -> BuildDir {
				self.0.pop();
				BuildDir(self.0)
			}
		}

		impl AsRef<Path> for $child_type {
			fn as_ref(&self) -> &Path {
				&self.0
			}
		}
	};
}

macro_rules! build_dir_sub {
	($child:ident, $file_name:literal) => {
		#[derive(Debug)]
		pub struct $child(PathBuf);

		impl $child {
			pub fn as_path(&self) -> &Path {
				self.0.as_path()
			}

			pub fn from_parent(mut parent: BuildDir) -> Self {
				parent.0.push($file_name);
				Self(parent.0)
			}

			pub fn to_parent(mut self) -> BuildDir {
				self.0.pop();
				BuildDir(self.0)
			}

			pub fn with_pushed<P: AsRef<Path>, F: FnMut(&Path) -> Out, Out>(
				&mut self,
				to_push: P,
				mut f: F,
			) -> Out {
				self.0.push(to_push.as_ref());
				let output = f(self.0.as_path());
				self.0.pop();
				output
			}

			pub fn exists(&self) -> bool {
				self.0.exists()
			}
		}
		impl AsRef<Path> for $child {
			fn as_ref(&self) -> &Path {
				&self.0
			}
		}
	};
}

build_dir_file!(BuildDirIndex, "index.html");
build_dir_file!(BuildDirComponents, "components.html");
build_dir_file!(BuildDirAllRequirements, "all-requirements.html");
build_dir_file!(BuildDirCSS, "styles.css");
build_dir_sub!(ComponentsDir, "components");
