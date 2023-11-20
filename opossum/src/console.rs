//! Handling the OPOSSUM CLI
//!
//! This module handles the command line parsing as well as basic information (e.g. help dialog, version information, etc.).
use crate::{
    analyzer::{AnalyzerType, RayTraceConfig},
    error::{OpmResult, OpossumError},
    get_version,
};
use std::io::{BufReader, BufWriter};

use clap::builder::Str;
use clap::{builder::OsStr, Parser};
use rprompt::prompt_reply_from_bufread;
use std::{io::Write, string::String};
use std::{
    io::{stdin, stdout, BufRead},
    path::{Path, PathBuf},
};
use strum::IntoEnumIterator;

/// Command line arguments for the OPOSSUM application.
pub struct Args {
    /// file path of the optical setup, which should be read in
    pub file_path: PathBuf,

    /// analyzer type that should be used to analyze the optical setup
    pub analyzer: AnalyzerType,

    /// destination directory of the report. if not defined, same directory as the filepath for the optical setup is used
    pub report_directory: PathBuf,
}

#[derive(Parser)]
#[command(author, version = Str::from(&get_version()), about, long_about = None)]
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

/// Checks if the passed file path is valid.
/// # Attributes
/// * `path`: Path to the file
/// # Returns
/// Returns True if the file path is valid, false otherwise
fn file_path_is_valid(path: &Path) -> bool {
    Path::exists(path) && Path::is_file(path) && path.extension() == Some(&OsStr::from("opm"))
}

fn eval_file_path_input(file_path: &str) -> Option<PathBuf> {
    let working_dir = std::env::current_dir().unwrap();
    println!("{}", working_dir.display());
    if file_path_is_valid(Path::new(file_path)) {
        Some(PathBuf::from(file_path))
    } else {
        None
    }
}

/// Evaluates if the passed analyzer string is valid.
/// # Attributes
/// * `analyzer_input`: String description of the analyzer
/// # Returns
/// * [`Option<AnalyzerType>`] as [`AnalyzerType::Energy`] or [`AnalyzerType::RayTrace`] depeneding on the input
/// * None if the analyzer string is invalid
fn eval_analyzer_input(analyzer_input: &str) -> Option<AnalyzerType> {
    match analyzer_input {
        "e" => Some(AnalyzerType::Energy),
        "r" => Some(AnalyzerType::RayTrace(RayTraceConfig::default())),
        _ => None,
    }
}

/// Evaluates if the passed report-directory string is valid.
/// # Attributes
/// * `report_path`: String description of the directory to the report
/// # Returns
/// * [`Option<PathBuf>`] with the defined directory string if valid or an empty path which will be replaced by the directory of the file path later on.
/// * None if the file-path string is invalid
fn eval_report_directory_input(report_path: &str) -> Option<PathBuf> {
    let r_path = Path::new(&report_path);
    if Path::exists(r_path) {
        Some(PathBuf::from(report_path))
    } else if report_path.is_empty() {
        Some(PathBuf::from(""))
    } else {
        None
    }
}

/// Creates the prompt string that is displayed in the console, depending on the flag and if the passed input for the respective flag is valid
/// # Attributes
/// * `flag`:       Respective argument flag. "f" for file path of the optical setup, "a" for analyzer to be used and "r" for the report directory.
/// * `init_str`:   Prepended String. Used if some messages schould be displayed beforehand.
/// # Returns
/// * Returns an [`OpmResult<String>`] containing the prompt message.
/// # Errors
/// Errors if an invalid flag type has been used
fn create_prompt_str(flag: &str, init_str: &str) -> OpmResult<String> {
    let mut prompt_str = init_str.to_owned();
    match flag{
        "f" =>{
            Ok(prompt_str + "Please insert path to optical-setup-description file:\n")
        },
        "a" =>{
            for a_type in AnalyzerType::iter(){
                prompt_str += match a_type {
                    AnalyzerType::Energy => "e for energy analysis\n",
                    AnalyzerType::RayTrace(_) => "r for ray-tracing analysis\n", 
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

/// Extracts the arguments from the [`PartialArgs`] struct
/// # Attributes
/// * `func`:       Function to evaluate the input string of the given argument.
/// * `input`:      String-Option of the argument
/// * `arg_flag`:   Respective argument flag. "f" for file path of the optical setup, "a" for analyzer to be used and "r" for the report directory.
/// * `reader`:     Type that implements the `BufRead` trait to read from. May be stdin().lock() for user input or a `BufReader` from a static String for tests
/// * `writer`:     Type  that implements the Write trait to write into.
/// # Returns
/// * Returns an [`OpmResult<T>`] containing the extracted argument. The specific type of T depends on the used function.
/// # Errors
/// Returns an [`OpossumError::Console`] if func returns a non-None Option that creates an error. In theory not possible
fn get_args<T>(
    func: fn(&str) -> Option<T>,
    input: Option<&str>,
    arg_flag: &str,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
) -> OpmResult<T> {
    if let Some(i) = input {
        let arg = func(i);
        if arg.is_none() {
            let prompt_str = create_prompt_str(arg_flag, "Invalid input!\n")?;
            let input: String = prompt_reply_from_bufread(reader, writer, prompt_str).unwrap();
            get_args(func, Some(input.as_str()), arg_flag, reader, writer)
        } else {
            arg.ok_or_else(|| OpossumError::Console("Could not extract argument!".into()))
        }
    } else {
        let prompt_str = create_prompt_str(arg_flag, "")?;
        let input: String = prompt_reply_from_bufread(reader, writer, prompt_str).unwrap();
        get_args(func, Some(input.as_str()), arg_flag, reader, writer)
    }
}

/// Gets the parent directory of the passed file path
/// # Arguments
/// * `path`: Path to a file.
///
/// # Panics
/// Panics if no parent directory can be determined. In theory not possible, since the used `file_path` is only passed if it is valid.
fn get_parent_dir(path: &Path) -> PathBuf {
    let parent_dir = Path::parent(path).unwrap();
    PathBuf::from(parent_dir)
}

impl TryFrom<PartialArgs> for Args {
    type Error = OpossumError;

    fn try_from(part_args: PartialArgs) -> OpmResult<Self> {
        let mut reader = BufReader::new(stdin().lock());
        let mut writer = BufWriter::new(stdout().lock());
        //intro only shown when neither the help, nor the version flag is specified
        show_intro();

        let file_path = get_args(
            eval_file_path_input,
            part_args.file_path.as_deref(),
            "f",
            &mut reader,
            &mut writer,
        )?;
        println!("Path to optical-setup file: {}", file_path.display());

        let analyzer = get_args(
            eval_analyzer_input,
            part_args.analyzer.as_deref(),
            "a",
            &mut reader,
            &mut writer,
        )?;
        println!("Analyzer: {analyzer}");

        let report_directory = get_args(
            eval_report_directory_input,
            part_args.report_directory.as_deref(),
            "r",
            &mut reader,
            &mut writer,
        )?;
        drop(reader);
        let report_directory = if report_directory.as_os_str().is_empty() {
            get_parent_dir(&file_path)
        } else {
            report_directory
        };
        println!("Report directory: {}", report_directory.display());

        Ok(Self {
            file_path,
            analyzer,
            report_directory,
        })
    }
}

/// Creates the OPOSSUM logo as ASCII art.
/// # Returns
/// Returns the ASCII-art logo as String
#[must_use]
fn create_intro() -> String {
    let intro_str = format!(
        "{: ^119}\n",
        "Opossum - Open-source Optics Simulation System and Unified Modeler"
    );
    let intro_logo = "                                                            .:^
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
                                                      ^JPBBBBG5?.\n".to_owned();

    format!("{intro_logo}{intro_str}")
}

/// Show the OPOSSUM logo as ASCII art and the CLI version information.
/// Prints the created logo and version string to the console.
pub fn show_intro() {
    let intro = create_intro();
    let version_str = format!("{: ^119}\n", "version ".to_owned() + &get_version());
    println!("{intro}{version_str}");
}

#[cfg(test)]
mod test {
    use crate::analyzer::RayTraceConfig;
    use std::io::BufReader;

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
        assert_eq!(eval_analyzer_input("nothing_available"), None);
        assert_eq!(
            eval_analyzer_input("r").unwrap(),
            AnalyzerType::RayTrace(RayTraceConfig::default())
        );
    }
    #[test]
    fn eval_report_directory_input_test() {
        let dir_valid = "./files_for_testing/CLI";

        assert_eq!(eval_report_directory_input(""), Some(PathBuf::from("")));
        assert_eq!(
            eval_report_directory_input("non_existent_path/sill_not_existent/"),
            None
        );
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
    #[test]
    fn create_prompt_str_test() {
        assert_eq!(
            create_prompt_str("f", "test_str\r\n").unwrap(),
            "test_str\r\nPlease insert path to optical-setup-description file:\n"
        );
        assert_eq!(
            create_prompt_str("a", "test_str\r\n").unwrap(),
            "test_str\r\ne for energy analysis\nr for ray-tracing analysis\n"
        );
        assert_eq!(create_prompt_str("r", "test_str\r\n").unwrap(), "test_str\r\nPlease insert a report directory or nothing to select the same directory as the optical-setup file\n");
        assert!(create_prompt_str("invalid_flag", "").is_err());
    }
    #[test]
    fn intro_test() {
        let intro = create_intro();
        assert_eq!(intro, "                                                            .:^
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
                                                      ^JPBBBBG5?.
                          Opossum - Open-source Optics Simulation System and Unified Modeler                           \n")
    }
    #[test]
    fn try_from_args_test() {
        let path_valid = "./files_for_testing/CLI/opticscenery.opm".to_owned();
        let part_args = PartialArgs {
            file_path: Some(path_valid.clone()),
            analyzer: Some("e".to_owned()),
            report_directory: Some("".to_owned()),
        };

        let args = Args {
            file_path: PathBuf::from(path_valid.clone()),
            analyzer: AnalyzerType::Energy,
            report_directory: PathBuf::from(get_parent_dir(&PathBuf::from(path_valid.clone()))),
        };

        let args_from = Args::try_from(part_args).unwrap();

        assert_eq!(args.file_path, args_from.file_path);
        assert_eq!(args.analyzer, args_from.analyzer);
        assert_eq!(args.report_directory, args_from.report_directory);

        let part_args = PartialArgs {
            file_path: Some(path_valid.clone()),
            analyzer: Some("e".to_owned()),
            report_directory: Some("./files_for_testing/".to_owned()),
        };

        let args = Args {
            file_path: PathBuf::from(path_valid.clone()),
            analyzer: AnalyzerType::Energy,
            report_directory: PathBuf::from("./files_for_testing/"),
        };
        let args_from = Args::try_from(part_args).unwrap();
        assert_eq!(args.report_directory, args_from.report_directory);
    }

    #[test]
    fn get_args_test() {
        let correct_file_path = b"./files_for_testing/CLI/opticscenery.opm\r\n";
        let analyzer_energy_str = b"e\r\n";
        let analyzer_ray_str = b"r\r\n";
        let report_directory_path1 = b"./files_for_testing/\r\n";

        let mut writer = Vec::new();
        let mut reader = BufReader::new(&correct_file_path[..]);
        let file_path1 = get_args(
            eval_file_path_input,
            Some("./files_for_testing/CLI/opticscenery.opm"),
            "f",
            &mut reader,
            &mut writer,
        )
        .unwrap();
        let file_path_str1 = file_path1.to_str().unwrap();
        assert_eq!(file_path_str1, "./files_for_testing/CLI/opticscenery.opm");

        let mut reader = BufReader::new(&correct_file_path[..]);
        let file_path2 = get_args(
            eval_file_path_input,
            Some("./files_for_testing/CLI/not_an_opticscenery.opm"),
            "f",
            &mut reader,
            &mut writer,
        )
        .unwrap();
        let file_path_str2 = file_path2.to_str().unwrap();
        println!("{file_path_str2}");

        let mut reader = BufReader::new(&correct_file_path[..]);
        let file_path3 =
            get_args(eval_file_path_input, None, "f", &mut reader, &mut writer).unwrap();
        let file_path_str3 = file_path3.to_str().unwrap();
        assert_eq!(file_path_str3, "./files_for_testing/CLI/opticscenery.opm");

        let mut reader = BufReader::new(&analyzer_energy_str[..]);
        let analyzer1 = get_args(
            eval_analyzer_input,
            Some("e"),
            "a",
            &mut reader,
            &mut writer,
        )
        .unwrap();
        assert_eq!(analyzer1, AnalyzerType::Energy);

        let mut reader = BufReader::new(&analyzer_ray_str[..]);
        let analyzer2 = get_args(
            eval_analyzer_input,
            Some("r"),
            "a",
            &mut reader,
            &mut writer,
        )
        .unwrap();
        assert_eq!(analyzer2, AnalyzerType::RayTrace(RayTraceConfig::default()));

        let mut reader = BufReader::new(&analyzer_ray_str[..]);
        let analyzer2 = get_args(
            eval_analyzer_input,
            Some("not_an_analyzer"),
            "a",
            &mut reader,
            &mut writer,
        )
        .unwrap();
        assert_eq!(analyzer2, AnalyzerType::RayTrace(RayTraceConfig::default()));

        let mut reader = BufReader::new(&analyzer_ray_str[..]);
        let analyzer3 = get_args(eval_analyzer_input, None, "a", &mut reader, &mut writer).unwrap();
        assert_eq!(analyzer3, AnalyzerType::RayTrace(RayTraceConfig::default()));

        let mut reader = BufReader::new(&report_directory_path1[..]);
        let report_path1 = get_args(
            eval_report_directory_input,
            None,
            "a",
            &mut reader,
            &mut writer,
        )
        .unwrap();
        let report_path_str1 = report_path1.to_str().unwrap();
        assert_eq!(report_path_str1, "./files_for_testing/");

        let mut reader = BufReader::new(&report_directory_path1[..]);
        let report_path2 = get_args(
            eval_report_directory_input,
            Some("./files_for_testing/"),
            "a",
            &mut reader,
            &mut writer,
        )
        .unwrap();
        let report_path_str2 = report_path2.to_str().unwrap();
        assert_eq!(report_path_str2, "./files_for_testing/");

        let mut reader = BufReader::new(&report_directory_path1[..]);
        let report_path3 = get_args(
            eval_report_directory_input,
            Some("./files_for_not_testing/"),
            "a",
            &mut reader,
            &mut writer,
        )
        .unwrap();
        let report_path_str3 = report_path3.to_str().unwrap();
        assert_eq!(report_path_str3, "./files_for_testing/");
    }

    #[test]
    fn parser_test(){
        let arg_vec = vec![
            "opossum",
            "-f",
            "./files_for_testing/CLI/opticscenery.opm",
            "-a",
            "e",
            "-r",
            "./files_for_testing/",
        ];
        let part_args = PartialArgs::parse_from(arg_vec);
        let fpath = part_args.file_path.unwrap();
        let analyzer = part_args.analyzer.unwrap();
        let r_dir = part_args.report_directory.unwrap();

        assert_eq!(fpath, "./files_for_testing/CLI/opticscenery.opm");
        assert_eq!(analyzer, "e");
        assert_eq!(r_dir, "./files_for_testing/");
    }
}
