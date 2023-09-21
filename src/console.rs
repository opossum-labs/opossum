use clap::Parser;
use structopt::StructOpt;
use rprompt::prompt_reply;
use std::path::Path;
use crate::{analyzer::AnalyzerType, error::OpossumError};
use strum::IntoEnumIterator;

type Result<T> = std::result::Result<T, OpossumError>;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// file path to buikd-filed of the optical setup, which should be read in 
    #[arg(short, long)]
    pub file_path: String,

    /// analyzer type that should be used to analyze the optical setup
    #[arg(short, long)]
    pub analyzer: String,

    /// flag to define whether a report should be created or not
    #[arg(short, long)]
    report: Option<String>,

    /// path to the report destination. if not defined, same directory as the filepath for the optical setup is used
    #[arg(short, long)]
    path_to_report: Option<String>,
}


#[derive(Parser, StructOpt)]
pub struct PartialArgs {
    /// filepath of the opticscenery to read in 
    #[arg(short, long)]
    file_path: Option<String>,

    /// analyzer type that should be used to analyze the optical setup
    #[arg(short, long)]
    analyzer: Option<String>,

    /// flag to define whether a report should be created or not, defafult yes
    #[arg(short, long)]
    report: Option<String>,

    /// path to the report destination. if not defined, same directory as the filepath for the optical setup is used
    #[arg(short, long)]
    path_to_report: Option<String>,
}

fn user_prompt(prompt: &str, check_flag: &str, err_count: &mut usize) -> Result<String>{
    if *err_count < 3{
        match check_flag{
            "f" => {
                let input: String = prompt_reply(prompt).unwrap();
                if Path::exists(Path::new(&input)){
                    //todo: check for correct file extension
                    *err_count = 0;
                    Ok(input)
                }
                else{
                    *err_count += 1;
                    user_prompt(&format!("Invalid file path: {}\n\nPlease insert valid file path!\n", input), "f", err_count)
                }
            }
            "a" => 
            {
                let mut prompt_str = prompt.to_owned();
                for a_type in AnalyzerType::iter(){
                    prompt_str += match a_type {
                        AnalyzerType::Energy => "e for energy analysis\n",
                        AnalyzerType::ParAxialRayTrace=> "p for paraxial ray-tracing analysis\n", 
                    };
                }
                let input: String = prompt_reply(prompt_str).unwrap();

                match input.as_str() {
                    "e" => Ok("energy".to_owned()),
                    "p" => Ok("paraxial ray-tracing".to_owned()),
                    _ => {
                        *err_count += 1;
                        user_prompt(&format!("Invalid analyzer flag: {}\n\nPlease choose an analyzer for your setup:\n", input), "a", err_count)
                    }
                }
            }
            _ => {
                Err(OpossumError::Console("Wrong check flag! This type of flag is not defined!".into()))
            }
        }
    }
    else{
        Err(OpossumError::Console("Too many wrong inputs! Program exits! Please type \"opossum.exe -h\" for help!".into()))
    }

}

impl TryFrom<PartialArgs> for Args{
    type Error = OpossumError;

    fn try_from(part_args: PartialArgs) -> Result<Args> {
        let mut err_count: usize = 0;

        let file_path = match part_args.file_path{
            Some(f) => f,
            None => user_prompt("Insert path to optical-setup-description file:\n", "f", &mut err_count)?
        };
        println!("{}", format!("Path to optical-setup file: {}\n", file_path));

        let analyzer = match part_args.analyzer{
            Some(a) => a,
            None => user_prompt("Please choose an analyzer for your setup:\n", "a", &mut err_count)?
        };
        println!("{}", format!("Chosen analysis: {}\n", analyzer));
        
        Ok(Args{ file_path: file_path, analyzer: analyzer})
    }
}
