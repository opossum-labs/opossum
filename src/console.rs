use clap::Parser;
use structopt::StructOpt;
use rprompt::prompt_reply;
use std::path::Path;
use crate::{analyzer::AnalyzerType, error::OpossumError};
use strum::IntoEnumIterator;

type Result<T> = std::result::Result<T, OpossumError>;

/// Simple program to greet a person
#[derive(Parser, Debug)]
pub struct Args {
    /// file path to buikd-filed of the optical setup, which should be read in 
    pub file_path: String,

    /// analyzer type that should be used to analyze the optical setup
    pub analyzer: String,

    /// destination directory of the report. if not defined, same directory as the filepath for the optical setup is used
    pub report_directory: String,
}


#[derive(Parser, StructOpt)]
#[command(author, version, about, long_about = None)]
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

fn eval_file_path_input(file_path: &str) -> Option<String>{
    if file_path_is_valid(file_path) {
        Some(file_path.to_owned())
    }
    else{
        None
    }
}

fn eval_analyzer_input(analyzer_input: &str) -> Option<String>{
    match analyzer_input{
        "e" => Some("energy".to_owned()),
        "p" => Some("paraxial ray-tracing".to_owned()),
        _ =>   None
    }
}

fn eval_report_directory_input(report_input: &str) -> Option<String>{
    let r_path = Path::new(&report_input);
    if Path::exists(r_path){
        if report_input.ends_with('\\'){
            Some(report_input.to_owned())
        }
        else{
            Some(report_input.to_owned() + "\\")
        }
    }
    else if report_input.is_empty() {
        Some("".to_owned())
    }
    else{
        None
    }
}

fn create_prompt_str(flag: &str, init_str: &str) -> Result<String>{
    let mut prompt_str = init_str.to_owned();
    match flag{
        "f" =>{
            Ok(prompt_str + "Please insert path to optical-setup-description file:\n")
        },
        "a" =>{
            for a_type in AnalyzerType::iter(){
                prompt_str += match a_type {
                    AnalyzerType::Energy => "e for energy analysis\n",
                    AnalyzerType::ParAxialRayTrace=> "p for paraxial ray-tracing analysis\n", 
                };
            }
            Ok(prompt_str)
        },
        "r" =>{
            Ok(prompt_str + "Please insert a report directory or nothing to select the same directory as the optical-setup file\n")
        },
        _ => Err(OpossumError::Console("Invalid flag type! Cannot create prompt string!".into()))
    }
}

fn get_args(func: fn(&str) -> Option<String>, input: Option<String>, arg_flag: &str) -> Result<String>{
    match input{
        Some(i)=>{
            let arg = func(&i);
            if arg.is_none(){
                let prompt_str = create_prompt_str(arg_flag, "Invalid input!\n")?;
                let input: String = prompt_reply(prompt_str).unwrap();
                get_args(func, Some(input), arg_flag)
            }
            else{
                arg.ok_or(OpossumError::Console("Could not extract argument!".into()))
            }
        },
        None => {
            let prompt_str = create_prompt_str(arg_flag, "")?;
            let input: String = prompt_reply(prompt_str).unwrap();
            get_args(func, Some(input), arg_flag)
        }
    }
}

fn get_parent_dir(path: &str) -> String{
    let parent_dir = Path::parent(Path::new(path))
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
    parent_dir + "\\"
}

impl TryFrom<PartialArgs> for Args{
    type Error = OpossumError;

    fn try_from(part_args: PartialArgs) -> Result<Args> {   

        let file_path = get_args(eval_file_path_input, part_args.file_path, "f")?;
        println!("Path to optical-setup file: {}\n", file_path);
        
        let analyzer = get_args(eval_analyzer_input, part_args.analyzer, "a")?;
        println!("Chosen analysis: {}\n", analyzer);

        let report_directory = get_args(eval_report_directory_input, part_args.report_directory, "r")?;
        let report_directory = if report_directory.is_empty(){
            get_parent_dir(&file_path)
        }
        else{
            report_directory
        };
        println!("Report directory: {}\n", report_directory);
        
        Ok(Args{ file_path, analyzer, report_directory})
    }
}


#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn file_path_is_valid_test() {
        let path_valid = ".\\files_for_testing\\CLI\\empty_yaml.yaml";
        let path_inexistent_file = ".\\files_for_testing\\CLI\\nonexistent.yaml";
        let path_inexistent_dir = ".\\files_for_testing\\this_dir_does_not_exist\\empty_yaml.yaml";
        let path_not_yaml = ".\\files_for_testing\\CLI\\is_not_a_yaml.txt";
        let path_is_dir = ".\\files_for_testing\\CLI\\";

        assert_eq!(file_path_is_valid(path_valid), true);
        assert_eq!(file_path_is_valid(path_inexistent_file), false);
        assert_eq!(file_path_is_valid(path_inexistent_dir), false);
        assert_eq!(file_path_is_valid(path_not_yaml), false);
        assert_eq!(file_path_is_valid(path_is_dir), false);
    }
    #[test]
    fn eval_file_path_input_test(){
        let path_valid = ".\\files_for_testing\\CLI\\empty_yaml.yaml";
        let path_inexistent_file = ".\\files_for_testing\\CLI\\nonexistent.yaml";
        let path_inexistent_dir = ".\\files_for_testing\\this_dir_does_not_exist\\empty_yaml.yaml";
        let path_not_yaml = ".\\files_for_testing\\CLI\\is_not_a_yaml.txt";
        let path_is_dir = ".\\files_for_testing\\CLI\\";

        assert_eq!(eval_file_path_input(path_valid), Some(path_valid.to_owned()));
        assert_eq!(eval_file_path_input(path_inexistent_file), None);
        assert_eq!(eval_file_path_input(path_inexistent_dir), None);
        assert_eq!(eval_file_path_input(path_not_yaml), None);
        assert_eq!(eval_file_path_input(path_is_dir), None);
        
    }
    #[test]
    fn eval_analyzer_input_test(){
        assert_eq!(eval_analyzer_input("e").unwrap(), "energy");
        assert_eq!(eval_analyzer_input("p").unwrap(), "paraxial ray-tracing");
    }
    #[test]
    fn eval_report_directory_input_test(){
        let dir_valid = ".\\files_for_testing\\CLI\\";
        let dir_valid2 = ".\\files_for_testing\\CLI";

        assert_eq!(eval_report_directory_input(""), Some("".to_owned()));
        assert_eq!(eval_report_directory_input(dir_valid), Some(dir_valid.to_owned()));
        assert_eq!(eval_report_directory_input(dir_valid2), Some(dir_valid.to_owned()));
        
    }
    #[test]
    fn get_parent_dir_test(){
        let path_valid = ".\\files_for_testing\\CLI\\empty_yaml.yaml".to_owned();
        assert_eq!(get_parent_dir(&path_valid), ".\\files_for_testing\\CLI\\");
    }
}