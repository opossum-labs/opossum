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

    /// destination directory of the report. if not defined, same directory as the filepath for the optical setup is used
    #[arg(short, long)]
    pub report_directory: String,
}


#[derive(Parser, StructOpt)]
pub struct PartialArgs {
    /// filepath of the opticscenery to read in 
    #[arg(short, long)]
    file_path: Option<String>,

    /// analyzer type that should be used to analyze the optical setup
    #[arg(short, long)]
    analyzer: Option<String>,

    /// destination directory of the report. if not defined, same directory as the filepath for the optical setup is used
    #[arg(short, long)]
    report_directory: Option<String>,
}

fn file_path_is_valid(path: &str) -> bool{
    Path::exists(Path::new(path)) && Path::is_file(Path::new(path)) && path.ends_with(".yaml")
}

fn eval_file_path(path: Option<String>, err_count:&mut usize) -> Result<String>{
    match path{
        Some(f) => {
            if file_path_is_valid(&f) {
                Ok(f)
            }
            else{
                user_prompt(&format!("Invalid file path: {}\nFile must be inside of an existing directory end end with .yaml!\n\nPlease insert valid file path!\n", f), "f", err_count)
            }
        },
        None => user_prompt("Insert path to optical-setup-description file:\n", "f", err_count)
    }
}

fn eval_analyzer_flag(analyzer_flag: Option<String>, err_count:&mut usize) -> Result<String>{
    match analyzer_flag{
        Some(flag) => {
            match flag.as_str() {
                "e" => Ok("energy".to_owned()),
                "p" => Ok("paraxial ray-tracing".to_owned()),
                _ => {
                    *err_count += 1;
                    user_prompt(&format!("Invalid analyzer flag: {}\n\nPlease choose an analyzer for your setup:\n", flag), "a", err_count)
                }
            }
        }
        None => user_prompt("Please choose an analyzer for your setup:\n", "a", err_count)       
    }
}

fn eval_report_directory(report_dir: Option<String>, err_count: &mut usize) -> Result<String>{
    match report_dir{
        Some(r) => {
            let r_path = Path::new(&r);
            if Path::exists(r_path){
                if r_path.ends_with("\\"){
                    Ok(r.to_owned())
                }
                else{
                    Ok(r + "\\")
                }
            }
            else if r == "" {
                Ok("".to_owned())
            }
            else{
                user_prompt(&format!("Invalid directory: {}\n\nPlease insert a valid directory or nothing to choose the same directory as the optical-setup file\n", r), "r", err_count)
            }
        },
        None => Ok("".to_owned())
    }
}

fn user_prompt(prompt: &str, check_flag: &str, err_count: &mut usize) -> Result<String>{
    if *err_count < 3{
        match check_flag{
            "f" => {
                let input: String = prompt_reply(prompt).unwrap();
                eval_file_path(Some(input), err_count)
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

                eval_analyzer_flag(Some(input), err_count)
            }
            "r" => 
            {
                let input: String = prompt_reply(prompt).unwrap();
                eval_report_directory(Some(input), err_count)
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

        let file_path = eval_file_path(part_args.file_path, &mut err_count)?;
        println!("{}", format!("Path to optical-setup file: {}\n", file_path));

        let analyzer = eval_analyzer_flag(part_args.analyzer, &mut err_count)?;
        println!("{}", format!("Chosen analysis: {}\n", analyzer));

        let report_directory = eval_report_directory(part_args.report_directory, &mut err_count)?;
        let report_directory = if report_directory.is_empty(){
            Path::parent(Path::new(&file_path))
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
        }
        else{
            report_directory
        };
        println!("{}", format!("Report directory: {}\n", report_directory));
        
        Ok(Args{ file_path: file_path, analyzer: analyzer, report_directory: report_directory})
    }
}
