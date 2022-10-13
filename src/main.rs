use std::collections::HashMap;
use std::error::Error;
use std::fs;
use capnp::serialize;
use capnpc::codegen::GeneratorContext;
use capnpc::schema_capnp::node::WhichReader;
use serde::Serialize;
use capnpc::schema_capnp::value;
use glob::glob;
use clap::Parser;

#[derive(Serialize)]
struct Field {
	name: String,
	annotations: HashMap<String, String>,
}

#[derive(Serialize)]
struct Struct {
	name: String,
	fields: Vec<Field>,
}

#[derive(Serialize)]
struct Enum {
	name: String,
	enumerants: Vec<Field>
}

#[derive(Serialize)]
struct Interface {
	name: String,
	methods: Vec<Field>
}

#[derive(Serialize)]
struct Results {
	structs: Vec<Struct>,
	enums: Vec<Enum>,
	interfaces: Vec<Interface>,
	unk: Vec<String>
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

fn main() -> Result<(), Box<dyn Error>> {
	let args = Args::parse();

	let files = glob(&args.glob)?;

	let mut cmd = std::process::Command::new("capnp");
	cmd.arg("compile").arg("-o").arg("-");

	for file in files.flatten() {
		// :(
		let name = file.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_else(|| "".into());

		if let Some(x) = &args.excludes {
			if x.contains(&name) { continue }
		}

		cmd.arg(file.display().to_string());
	}

	cmd.stdout(std::process::Stdio::piped());
	let mut output = cmd.spawn()?;

	let message = serialize::read_message(
		output.stdout.take().unwrap(),
		capnp::message::ReaderOptions::new()
	)?;

	let gen = GeneratorContext::new(&message)?;

	let mut results = Results {
		structs: vec![],
		enums: vec![],
		interfaces: vec![],
		unk: vec![]
	};
	let mut annotation_names: HashMap<String, String> = HashMap::new();

	// initial pass to grab annotation names
	for node in gen.request.get_nodes()?.iter() {
		if let WhichReader::Annotation(_) = node.which()? {
			let node_name = node.get_display_name()?;
			let prefix_len = node.get_display_name_prefix_length()  as usize;
			let annotation_name = node_name[prefix_len..].to_string();
			let id = node.get_id();
			annotation_names.insert(id.to_string(), annotation_name);
		}
	}

	for node in gen.request.get_nodes()?.iter() {
		let node_name = node.get_display_name()?.to_string();

		match node.which()? {
			WhichReader::Struct(reader) => {
				println!("struct: {node_name}");
				results.structs.push(
					Struct { name: node_name, fields: vec![] }
				);

				let idx = results.structs.len() - 1;
				let fields = reader.get_fields()?;

				for (i, field) in fields.iter().enumerate() {
					let field_name = field.get_name()?.to_string();

					println!("	field: {field_name}");
					results.structs[idx].fields.push(
						Field { name: field_name, annotations: HashMap::new() }
					);

					let annotations = field.get_annotations()?;
					for annotation in annotations.iter() {
						if let value::Text(t) = annotation.get_value()?.which()? {
							let id = annotation.get_id();
							let name = annotation_names.get(&id.to_string()).unwrap();
							let value = t?.parse().unwrap();

							results.structs[idx].fields[i].annotations.insert(name.to_string(), value);
						}
					}
				}
			}
			WhichReader::Enum(reader) => {
				println!("enum: {node_name}");
				results.enums.push(
					Enum { name: node_name, enumerants: vec![] }
				);

				let idx = results.enums.len() - 1;
				let enumerants = reader.get_enumerants()?;

				for (i, enumerant) in enumerants.iter().enumerate() {
					let enumerant_name = enumerant.get_name()?.to_string();

					println!("	enumerant: {enumerant_name}");
					results.enums[idx].enumerants.push(
						Field { name: enumerant_name, annotations: HashMap::new() }
					);

					let annotations = enumerant.get_annotations()?;
					for annotation in annotations.iter() {
						if let value::Text(t) = annotation.get_value()?.which()? {
							let id = annotation.get_id();
							let name = annotation_names.get(&id.to_string()).unwrap();
							let value = t?.parse().unwrap();

							results.enums[idx].enumerants[i].annotations.insert(name.to_string(), value);
						}
					}
				}
			}
			WhichReader::Interface(reader) => {
				println!("interface: {node_name}");
				results.interfaces.push(
					Interface { name: node_name, methods: vec![] }
				);

				let idx = results.interfaces.len() - 1;
				let methods = reader.get_methods()?;

				for (i, method) in methods.iter().enumerate() {
					let method_name = method.get_name()?.to_string();

					println!("	method: {method_name}");
					results.interfaces[idx].methods.push(
						Field { name: method_name, annotations: HashMap::new() }
					);

					let annotations = method.get_annotations()?;
					for annotation in annotations.iter() {
						if let value::Text(t) = annotation.get_value()?.which()? {
							let id = annotation.get_id();
							let name = annotation_names.get(&id.to_string()).unwrap();
							let value = t?.parse().unwrap();

							results.interfaces[idx].methods[i].annotations.insert(name.to_string(), value);
						}
					}
				}
			}
			// if we're not doing any processing, just throw it in unk
			_ => { results.unk.push(node_name) }
		}
	}

	let json = serde_json::to_string_pretty(&results).unwrap();

	fs::write(args.output, json)?;
	Ok(())
}