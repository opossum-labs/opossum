use crate::{analyzer::AnalyzerType, error::OpossumError};
use chrono::DateTime;
use clap::{builder::OsStr, Parser};
use rprompt::prompt_reply;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use strum::IntoEnumIterator;

type Result<T> = std::result::Result<T, OpossumError>;

/// Command line arguments for the OPOSSUM application.
pub struct Args {
    /// file path of the optical setup, which should be read in
    pub file_path: PathBuf,

    /// analyzer type that should be used to analyze the optical setup
    pub analyzer: AnalyzerType,

    /// destination directory of the report. if not defined, same directory as the filepath for the optical setup is used
    pub report_directory: PathBuf,
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

fn file_path_is_valid(path: &Path) -> bool {
    Path::exists(path) && Path::is_file(path) && path.extension() == Some(&OsStr::from("opm"))
}

fn eval_file_path_input(file_path: &str) -> Option<PathBuf> {
    if file_path_is_valid(Path::new(file_path)) {
        Some(PathBuf::from(file_path))
    } else {
        None
    }
}

fn eval_analyzer_input(analyzer_input: &str) -> Option<AnalyzerType> {
    match analyzer_input {
        "e" => Some(AnalyzerType::Energy),
        "p" => Some(AnalyzerType::ParAxialRayTrace),
        _ => None,
    }
}

fn eval_report_directory_input(report_input: &str) -> Option<PathBuf> {
    let r_path = Path::new(&report_input);
    if Path::exists(r_path) {
        Some(PathBuf::from(report_input))
    } else if report_input.is_empty() {
        Some(PathBuf::from(""))
    } else {
        None
    }
}

fn create_prompt_str(flag: &str, init_str: &str) -> Result<String> {
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

fn get_analyzer_args(
    func: fn(&str) -> Option<AnalyzerType>,
    input: Option<String>,
    arg_flag: &str,
) -> Result<AnalyzerType> {
    match input {
        Some(i) => {
            let arg = func(&i);
            if arg.is_none() {
                let prompt_str = create_prompt_str(arg_flag, "Invalid input!\n")?;
                let input: String = prompt_reply(prompt_str).unwrap();
                get_analyzer_args(func, Some(input), arg_flag)
            } else {
                arg.ok_or(OpossumError::Console("Could not extract argument!".into()))
            }
        }
        None => {
            let prompt_str = create_prompt_str(arg_flag, "")?;
            let input: String = prompt_reply(prompt_str).unwrap();
            get_analyzer_args(func, Some(input), arg_flag)
        }
    }
}

fn get_path_args(
    func: fn(&str) -> Option<PathBuf>,
    input: Option<&str>,
    arg_flag: &str,
) -> Result<PathBuf> {
    match input {
        Some(i) => {
            let arg = func(i);
            if arg.is_none() {
                let prompt_str = create_prompt_str(arg_flag, "Invalid input!\n")?;
                let input: String = prompt_reply(prompt_str).unwrap();
                get_path_args(func, Some(&input), arg_flag)
            } else {
                arg.ok_or(OpossumError::Console("Could not extract argument!".into()))
            }
        }
        None => {
            let prompt_str = create_prompt_str(arg_flag, "")?;
            let input: String = prompt_reply(prompt_str).unwrap();
            get_path_args(func, Some(&input), arg_flag)
        }
    }
}
fn get_parent_dir(path: &Path) -> PathBuf {
    let parent_dir = Path::parent(path).unwrap();
    PathBuf::from(parent_dir)
}

impl TryFrom<PartialArgs> for Args {
    type Error = OpossumError;

    fn try_from(part_args: PartialArgs) -> Result<Args> {
        let file_path = get_path_args(eval_file_path_input, part_args.file_path.as_deref(), "f")?;
        println!("Path to optical-setup file: {}", file_path.display());

        let analyzer = get_analyzer_args(eval_analyzer_input, part_args.analyzer, "a")?;
        println!("Analyzer: {}", analyzer);

        let report_directory = get_path_args(
            eval_report_directory_input,
            part_args.report_directory.as_deref(),
            "r",
        )?;
        let report_directory = if report_directory.as_os_str().is_empty() {
            get_parent_dir(&file_path)
        } else {
            report_directory
        };
        println!("Report directory: {}", report_directory.display());

        Ok(Args {
            file_path,
            analyzer,
            report_directory,
        })
    }
}

pub fn show_intro() {
    println!("                                                            .:^
                                             ::   ......:::^^^^:.. .:.
                    :!?Y55YJ?!^.          ..:^^^^^^^^^^^^^^^^^^^^^^^^^^:..
                 .5#&&&&&&&&&&&&#P7.  .:^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^:..     :!J5PGGBBGPY!:
                ~&&&&#GGB#&&&&&&&&&&P?^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^!P#&&&&&&&&&&&&&&B!
                B&&&&&#G5YY5GB&&&&&&&&B7^^:^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^:^!P&&&&&&&&&&##B#&&&&&&J
                B&&&&&&&&BBG5?7JPB&&&&&&#PJ!^^^^^^^^^^^^^^^^^^^^^^^^^^^^:^~!JP&&&&&&#BP5YJJYPB&&&&&&#
                ^&&&&&&&&5!!J5Y^:^~5&&&&&&&&BP?~^^^^^^^^^^^^^^^^^^^^:^~75G#&&&&&&&P77J5PB#&&&&&&&&&&B
                 !&&&&BPPGP~::^^^!JG&&&&&&&&&&&#P7^^^^^^^^^^^^^^^^^~JG#&&&&&&&&&&B^^?J7J#&&&&&&&&&&&:
                  .P&&#BG55YY5PG#&&&&&&&&&&&&&&&&&G!^^^^^^^^^^^^^!P#&&&&&&&&&&&&&&?:^^?5G#&&&&&&&&#:
                    ^G&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&J^^^^^^^^^^^Y#&&&&&&&&&&&&&&&&#~:^7G#&&&&&&&#J
                      :Y#&&&&&&&&&&&&&&&&&&&&&&&&&&&&5^^^^^^^^^P&&&&&&&&&&&&&&&&&&&B7Y&&&&&&&&#?.
                         ^YB#&&&&&&&&&&&&&&&&&&&&&&&&&J^^^^^^^P&&&&&&&&&&&&&&&&&&&&&&&&&&&&#5^
                            :#&&&&&&&&&&&&&&&&&&&&&&&&&7^^^^:J&&&&&&&&&&&&&&&&&&&&&&&&&&B?^
                            ~&&&&&&&&&&#5?777YG#&&&&&&&B^^^^~#&&&&&&&&#BGPGB#&&&&&&&&&&&B
                            5&&&&&&&&&&~ . ~J~.:?#&&&&&&5::^G&&&&&&&P~:.7~...~P&&&&&&&&&&7
                            G&&&&&&&&&&Y....!Y7^ ?&&&&&&&57P&&&&&&&P....!PJ^. ?&&&&&&&&&&P
                            P&&&&&&&&&&&?...  ~~.P&&&&&&&&&&&&&&&&&G..... :5:J&&&&&&&&&&&B.
                            7&&&&&&&&&&&&P!:::^?B&&&&&&&&&&&&&&&&&&&G!:....!G&&&&&&&&&&&&P^^^:.
                      .::^^^^G&&&&&&&&&&&&&&#&&&&&&&&&&&&&&&&&&&&&&&&&#BGG#&&&&&&&&&&&&&#~^^^^^^:.
                   .:^^^^^^^^~G&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&B!^^^^^^^^^:.
             .^~~^!!~^^^^^^^^^^J#&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&#Y^^^^^^^^^^^^^.
          .JGB##BBBBBG57~^^^^^^:^J#&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&#5~:^^^^^^^^^^^^!!.
         7&&#BP55GBBBBBBGGPPPPP5?^^JB&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&G7^:^!!7?J?7!!7J5GBBBP?.
        ^&PY7~JPBBBGPBBBBBBBBBBB#Y^. ^P&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&G^  ^JPBBBBBBBBBBBBBBBBB#B7
        ..   ^G##B5J5BBBGGBBBBBBBP^    :G&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&G^   !PBBBGBBBBGBBBGBBBBGBBB#P.
     .::::.  P&#Y~7YBBBBYGBBBGBBBG~     :P&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&5.   :G&#B55BBBPJGBBB5BBBBPPG#&#:    .....
  ^YGBBGGBBB5#?   !G&#GP5P###5PB##7 :JGBBGG#&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&BGGBGJ.:&&PPP5B#BYJ5BBB5!5B#&P5GBB5   !BBBBB
 YBB5^.   :?BBP:  :&&B!  :P&&PPP#&7?BBP~.   :G&&&&&&&&&&&&&&&&&&&&&&&&&&P.   7BBP.Y::BB5#&G^.?B&#P  :J#&#BBJ    GBGBBB
YBB5        !BBG  .BBB^    ~&BBP.7JBBP        7&&&&&&&&&&&&&&&&&&&&&&&&#Y~:.        :BBGY#.   7&#G.  :BBP?BB:  ?BB~GBB
BBB!        .BBB^  PBB5???J5BBG^  5BB?         G#&&&&&&&######&&&&&&#?!YGBBBBGY!.   :BBG      .##G.  :BBY GB5 .BBJ.BBB
GBB?        .BBB:  PBBPYYYJJ7^    YBBY        .GBBG#&&#BBBBBBBB#&&#Y.    .:^!YBBB^  :BBG      :BBG   :BB5 !BB~JBB..BBB
~BBB~      .5BBY   PBB~           :BBB!      .YBB5 .P&PPBBBBBB5G&&. 7GG?      YBBY  .BBB:     ~BBG   :BB5  PBBBB? .BBB
 :5B#P?!!75BBG7    PB#~            :5B#GJ!!7YB#G?    BBB#####&###J  .5B#P?!!75BBP.   !BBBY7!7YBBG^   :#B5  ~BBBG. .BBB
   .!J5PP5Y7^      ?JY:              .~J5PP5Y7^      5BB#&@@&#5~.     :!Y5PP5Y7^      .~J5PPP5?~.    .JJ7   ?JY~   JJJ
                                                      7PGBBBGPJ~.
                                                         .^!?5BBB!
                                                    ^55J      !BBG
                                                     5BBP!^:^7GBB!
                                                      ^JPBBBBG5?.\n");
    println!(
        "{: ^119}",
        "Opossum - Open-source Optics Simulation System and Unified Modeler"
    );
    let timestamp = DateTime::parse_from_rfc3339(env!("VERGEN_GIT_COMMIT_TIMESTAMP")).unwrap();
    let version_string = format!(
        " version {} ({})",
        env!("VERGEN_GIT_DESCRIBE"),
        timestamp.format("%Y/%m/%d %H:%M")
    );
    println!("{: ^119}\n", version_string);
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn file_path_is_valid_test() {
        let path_valid = Path::new("./files_for_testing/CLI/opticscenery.opm");
        let path_inexistent_file = Path::new("./files_for_testing/CLI/nonexistent.opm");
        let path_inexistent_dir =
            Path::new("./files_for_testing/this_dir_does_not_exist/empty.opm");
        let path_not_yaml = Path::new("./files_for_testing/CLI/is_not_a_opm.txt");
        let path_is_dir = Path::new("./files_for_testing/CLI/");

        assert_eq!(file_path_is_valid(path_valid), true);
        assert_eq!(file_path_is_valid(path_inexistent_file), false);
        assert_eq!(file_path_is_valid(path_inexistent_dir), false);
        assert_eq!(file_path_is_valid(path_not_yaml), false);
        assert_eq!(file_path_is_valid(path_is_dir), false);
    }
    #[test]
    fn eval_file_path_input_test() {
        let path_valid = "./files_for_testing/CLI/opticscenery.opm";
        let path_inexistent_file = "./files_for_testing/CLI/nonexistent.opm";
        let path_inexistent_dir = "./files_for_testing/this_dir_does_not_exist/empty.opm";
        let path_not_yaml = "./files_for_testing/CLI/is_not_an_opm.txt";
        let path_is_dir = "./files_for_testing/CLI/";

        assert_eq!(
            eval_file_path_input(path_valid),
            Some(PathBuf::from(path_valid))
        );
        assert_eq!(eval_file_path_input(path_inexistent_file), None);
        assert_eq!(eval_file_path_input(path_inexistent_dir), None);
        assert_eq!(eval_file_path_input(path_not_yaml), None);
        assert_eq!(eval_file_path_input(path_is_dir), None);
    }
    #[test]
    fn eval_analyzer_input_test() {
        assert_eq!(eval_analyzer_input("e").unwrap(), AnalyzerType::Energy);
        assert_eq!(
            eval_analyzer_input("p").unwrap(),
            AnalyzerType::ParAxialRayTrace
        );
    }
    #[test]
    fn eval_report_directory_input_test() {
        let dir_valid = "./files_for_testing/CLI";

        assert_eq!(eval_report_directory_input(""), Some(PathBuf::from("")));
        assert_eq!(
            eval_report_directory_input(dir_valid),
            Some(PathBuf::from(dir_valid))
        );
    }
    #[test]
    fn get_parent_dir_test() {
        let path_valid = "./files_for_testing/CLI/empty_yaml.yaml".to_owned();
        assert_eq!(
            get_parent_dir(&PathBuf::from(path_valid)),
            PathBuf::from("./files_for_testing/CLI")
        );
    }
}
