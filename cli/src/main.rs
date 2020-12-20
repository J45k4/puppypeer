use clap::{App, Arg, SubCommand};
use simple_logger::SimpleLogger;

mod hash_utility;
mod metadata;
mod copy;
mod utility;

fn main() {
    SimpleLogger::new().init().unwrap();
    
    let matches = App::new("Epic shelter")
        .version("0.0.1")
        .subcommand(SubCommand::with_name("merge")
            .about("Merges two repositories")
            .arg(Arg::with_name("input-folder"))
            .arg(Arg::with_name("output-folder")))
        .subcommand(SubCommand::with_name("copy")
            .about("Copies input folder content to output folder")
            .arg(Arg::with_name("input-folder").long("input-folder").value_name("input folder").short("i"))
            .arg(Arg::with_name("output-folder").long("output-folder").value_name("output folder").short("o")))
        .subcommand(SubCommand::with_name("post")
            .about("Copies files to server with http post")
            .arg(Arg::with_name("url").long("url"))
            .arg(Arg::with_name("input-folder").long("input-folder")))
        .get_matches();

    match matches.subcommand_name().unwrap() {
        "merge" => {
            unimplemented!()
        }
        "copy" => {
            let args = matches.subcommand_matches("copy").unwrap();

            copy::exec_copy(args);
        }
        "post" => {
            unimplemented!()
        },
        _ => {
            panic!("What is this ??");
        }
    }
}
