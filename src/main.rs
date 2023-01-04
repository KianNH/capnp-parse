use anyhow::Result;
use capnp::serialize;
use capnpc::codegen::GeneratorContext;
use capnpc::schema_capnp::node::WhichReader;
use capnpc::schema_capnp::value;
use capnpc::schema_capnp::*;
use clap::Parser;
use glob::glob;
use serde::{Serialize, Serializer};
use std::collections::{BTreeMap, HashMap};
use std::fs;

fn ordered_map<S>(value: &HashMap<String, String>, serializer: S) -> Result<S::Ok, S::Error>
where
	S: Serializer,
{
	let ordered: BTreeMap<_, _> = value.iter().collect();
	ordered.serialize(serializer)
}

#[derive(Serialize)]
struct Field {
	name: String,
	#[serde(serialize_with = "ordered_map")]
	annotations: HashMap<String, String>,
}

impl Field {
	fn add_annotation(
		&mut self,
		annotation: annotation::Reader,
		annotation_names: &HashMap<u64, String>,
	) -> Result<()> {
		let id = annotation.get_id();
		let name = annotation_names.get(&id);

		if let Some(actual_name) = name {
			let content = annotation.get_value()?;

			let value = match content.which()? {
				value::Void(..) => "true".to_string(),
				value::Text(txt) => txt?.to_string(),
				_ => "unhandled type".to_string(),
			};

			self.annotations.insert(actual_name.to_string(), value);
		}

		Ok(())
	}
}

#[derive(Serialize)]
struct Struct {
	name: String,
	fields: Vec<Field>,
}

impl Struct {
	fn add_field<T>(&mut self, name: &T)
	where
		T: ToString + ?Sized,
	{
		self.fields.push(Field {
			name: name.to_string(),
			annotations: HashMap::new(),
		})
	}
}

#[derive(Serialize)]
struct Enum {
	name: String,
	enumerants: Vec<Field>,
}

impl Enum {
	fn add_enumerant<T>(&mut self, name: &T)
	where
		T: ToString + ?Sized,
	{
		self.enumerants.push(Field {
			name: name.to_string(),
			annotations: HashMap::new(),
		})
	}
}

#[derive(Serialize)]
struct Interface {
	name: String,
	methods: Vec<Field>,
}

impl Interface {
	fn add_method<T>(&mut self, name: &T)
	where
		T: ToString + ?Sized,
	{
		self.methods.push(Field {
			name: name.to_string(),
			annotations: HashMap::new(),
		})
	}
}

#[derive(Serialize)]
struct Results {
	structs: Vec<Struct>,
	enums: Vec<Enum>,
	interfaces: Vec<Interface>,
	unk: Vec<String>,
}

impl Results {
	fn add_struct<T>(&mut self, name: &T)
	where
		T: ToString + ?Sized,
	{
		self.structs.push(Struct {
			name: name.to_string(),
			fields: vec![],
		})
	}

	fn get_current_struct(&self) -> usize {
		self.structs.len() - 1
	}

	fn add_enum<T>(&mut self, name: &T)
	where
		T: ToString + ?Sized,
	{
		self.enums.push(Enum {
			name: name.to_string(),
			enumerants: vec![],
		})
	}

	fn get_current_enum(&self) -> usize {
		self.enums.len() - 1
	}

	fn add_interface<T>(&mut self, name: &T)
	where
		T: ToString + ?Sized,
	{
		self.interfaces.push(Interface {
			name: name.to_string(),
			methods: vec![],
		})
	}

	fn get_current_interface(&self) -> usize {
		self.interfaces.len() - 1
	}

	fn add_unk<T>(&mut self, name: &T)
	where
		T: ToString + ?Sized,
	{
		self.unk.push(name.to_string())
	}
}

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
struct Args {
	/// A Glob - must be scoped to .capnp schemas
	#[arg(short, long, default_value = "./**/*.capnp")]
	glob: String,

	/// Filepath for the output JSON
	#[arg(short, long, default_value = "./output.json")]
	output: String,

	/// Filenames to exclude
	#[arg(short, long)]
	excludes: Option<Vec<String>>,
}

fn main() -> Result<()> {
	let args = Args::parse();

	let files = glob(&args.glob)?;

	let mut cmd = std::process::Command::new("/usr/local/bin/capnp");
	cmd.args(["compile", "-o", "-"]);

	for file in files.flatten() {
		let name = file
			.file_name()
			.map_or_else(String::new, |name| name.to_string_lossy().into_owned());

		if let Some(x) = &args.excludes {
			if x.contains(&name) {
				continue;
			}
		}

		cmd.arg(file.display().to_string());
	}

	cmd.stdout(std::process::Stdio::piped());
	let mut output = cmd.spawn()?;

	let message = serialize::read_message(
		output.stdout.take().unwrap(),
		capnp::message::ReaderOptions::new(),
	)?;

	let gen = GeneratorContext::new(&message)?;

	let mut results = Results {
		structs: vec![],
		enums: vec![],
		interfaces: vec![],
		unk: vec![],
	};
	let mut annotation_names: HashMap<u64, String> = HashMap::new();

	// initial pass to grab annotation names
	for node in gen.request.get_nodes()?.iter() {
		if let WhichReader::Annotation(_) = node.which()? {
			let node_name = node.get_display_name()?;
			let prefix_len = node.get_display_name_prefix_length() as usize;
			let annotation_name = node_name[prefix_len..].to_string();

			let id = node.get_id();
			annotation_names.insert(id, annotation_name);
		}
	}

	for node in gen.request.get_nodes()?.iter() {
		let node_name = node.get_display_name()?;

		match node.which()? {
			WhichReader::Struct(reader) => {
				println!("struct: {node_name}");
				results.add_struct(node_name);

				let idx = results.get_current_struct();
				let fields = reader.get_fields()?;

				for (i, field) in fields.iter().enumerate() {
					let field_name = field.get_name()?;

					println!("	field: {field_name}");
					results.structs[idx].add_field(field_name);

					let annotations = field.get_annotations()?;
					for annotation in annotations.iter() {
						results.structs[idx].fields[i].add_annotation(annotation, &annotation_names)?;
					}
				}
			}
			WhichReader::Enum(reader) => {
				println!("enum: {node_name}");
				results.add_enum(node_name);

				let idx = results.get_current_enum();
				let enumerants = reader.get_enumerants()?;

				for (i, enumerant) in enumerants.iter().enumerate() {
					let enumerant_name = enumerant.get_name()?;

					println!("	enumerant: {enumerant_name}");
					results.enums[idx].add_enumerant(enumerant_name);

					let annotations = enumerant.get_annotations()?;
					for annotation in annotations.iter() {
						results.enums[idx].enumerants[i].add_annotation(annotation, &annotation_names)?;
					}
				}
			}
			WhichReader::Interface(reader) => {
				println!("interface: {node_name}");
				results.add_interface(node_name);

				let idx = results.get_current_interface();
				let methods = reader.get_methods()?;

				for (i, method) in methods.iter().enumerate() {
					let method_name = method.get_name()?;

					println!("	method: {method_name}");
					results.interfaces[idx].add_method(method_name);

					let annotations = method.get_annotations()?;
					for annotation in annotations.iter() {
						results.interfaces[idx].methods[i].add_annotation(annotation, &annotation_names)?;
					}
				}
			}
			_ => results.add_unk(node_name),
		}
	}

	let json = serde_json::to_string_pretty(&results)?;

	fs::write(args.output, json)?;
	Ok(())
}
