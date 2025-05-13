use std::{fmt::Display, num::ParseIntError};

#[derive(Debug)]
pub struct Error {
	pub inner: Option<InnerErr>,
	pub msg: Box<str>,
	/// Use the line!() macro.
	pub line: u32,
	/// Use the file!() macro.
	pub file: Box<str>,
	/// Should we quit the whole program?
	pub severity: Severity,
}

impl Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self)?;
		if let Some(inner) = &self.inner {
			write!(f, " {:?}", inner)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
	/// Quit the program!
	Fatal,
	/// The failed operation may be retried.
	Retry,
}

impl Error {
	pub fn new(
		severity: Severity,
		inner: Option<InnerErr>,
		msg: String,
		line: u32,
		file: &str,
	) -> Self {
		Self {
			inner,
			msg: msg.into_boxed_str(),
			line,
			file: Box::from(file),
			severity,
		}
	}

	pub fn no_inner(severity: Severity, msg: String, line: u32, file: &str) -> Self {
		Self {
			inner: None,
			msg: msg.into_boxed_str(),
			line,
			file: Box::from(file),
			severity,
		}
	}
	pub fn inner(severity: Severity, inner: InnerErr, msg: String, line: u32, file: &str) -> Self {
		Self {
			inner: Some(inner),
			msg: msg.into_boxed_str(),
			line,
			file: Box::from(file),
			severity,
		}
	}

	pub fn retry_ok(&self) -> bool {
		matches!(self.severity, Severity::Retry)
	}

	pub fn fatal(&self) -> bool {
		matches!(self.severity, Severity::Fatal)
	}
}

#[derive(Debug)]
pub enum InnerErr {
	Inquire(inquire::error::InquireError),
	IO(std::io::Error),
	TomlDeserialize(toml::de::Error),
	Csv(csv::Error),
	BoxMsg(Box<str>),
	ParseInt(ParseIntError),
}

impl From<inquire::error::InquireError> for InnerErr {
	fn from(value: inquire::error::InquireError) -> Self {
		Self::Inquire(value)
	}
}

impl From<std::io::Error> for InnerErr {
	fn from(value: std::io::Error) -> Self {
		Self::IO(value)
	}
}

impl From<toml::de::Error> for InnerErr {
	fn from(value: toml::de::Error) -> Self {
		Self::TomlDeserialize(value)
	}
}

impl From<csv::Error> for InnerErr {
	fn from(value: csv::Error) -> Self {
		Self::Csv(value)
	}
}

impl From<Box<str>> for InnerErr {
	fn from(value: Box<str>) -> Self {
		Self::BoxMsg(value)
	}
}

impl From<String> for InnerErr {
	fn from(value: String) -> Self {
		Self::BoxMsg(value.into_boxed_str())
	}
}

impl From<&str> for InnerErr {
	fn from(value: &str) -> Self {
		Self::BoxMsg(Box::from(value))
	}
}

impl From<ParseIntError> for InnerErr {
	fn from(value: ParseIntError) -> Self {
		Self::ParseInt(value)
	}
}
