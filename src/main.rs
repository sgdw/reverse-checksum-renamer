// https://doc.rust-lang.org/book/second-edition/ch01-00-introduction.html

use std::env;
use std::process;

use std::path::{Path, PathBuf};

mod file_verification;

fn main() {
    const VERSION_MAJ: u32 = 0;
    const VERSION_MIN: u32 = 1;

    println!("reverse-checksum-renamer V{}.{}:\n", VERSION_MAJ, VERSION_MIN);

    let args: Vec<_> = env::args().collect();

    let mut source_file_path: Option<String> = None;
    let mut destination_file_path: Option<String> = None;
    let mut sfv_files: Vec<String> = Vec::new();
    
    let mut file_to_checksum: Option<String> = None;
    let mut file_to_check_if_sfv: Option<String> = None;

    let mut do_fix_misnamed_sfv_files = false;

    let mut verbose = false;
    let mut dry_run = false;

    let mut skip = 0;
    for i in 1 .. args.len() {
        if skip > 0 {
            skip = skip - 1;
        } else {
            if args[i] == "-i" {
                if i >= args.len() { die(&format!("Missing value for '{}' parameter", args[i]), 1); return; }
                source_file_path = Some(args[i+1].to_string());
                skip = 1;

            } else if args[i] == "-o" {
                if i >= args.len() { die(&format!("Missing value for '{}' parameter", args[i]), 1); return; }
                destination_file_path = Some(args[i+1].to_string());
                skip = 1;

            } else if args[i] == "--test-sfv" {
                if i >= args.len() { die(&format!("Missing value for '{}' parameter", args[i]), 1); return; }
                file_to_check_if_sfv = Some(args[i+1].to_string());
                skip = 1;

            } else if args[i] == "--checksum-file" {
                if i >= args.len() { die(&format!("Missing value for '{}' parameter", args[i]), 1); return; }
                file_to_checksum = Some(args[i+1].to_string());
                skip = 1;

            } else if args[i] == "--fix-sfv-files" {
                if i >= args.len() { die(&format!("Missing value for '{}' parameter", args[i]), 1); return; }
                do_fix_misnamed_sfv_files = true;

            } else if args[i] == "-v" {
                verbose = true;

            } else if args[i] == "-d" {
                dry_run = true;

            } else if args[i] == "--help" {
                println!("Usage: reverse-checksum-renamer [-i <input>] [-o <output>] <SFV files>");
                println!("  -i  input folder");
                println!("  -o  output folder");
                println!("  --fix-sfv-files");
                println!("      find SFV files and rename them");
                println!("  --checksum-file");
                println!("      print checksums of files");
                println!("  --test-sfv");
                println!("  -v  verbose");
                println!("  -d  dry run");

                return;

            } else {
                sfv_files.push(args[i].to_string());
                if verbose { println!("Adding SFV file: {:?}", args[i]); }
            }
        }
    }

    if source_file_path.is_none() {
        source_file_path = Some(".".to_string());
    }

    if destination_file_path.is_none() {
        destination_file_path = Some(source_file_path.clone().unwrap());
    }

    if verbose { 
        println!("Source path: {:?}", source_file_path.as_ref().unwrap());
        println!("Destination path: {:?}", destination_file_path.as_ref().unwrap()); 
    }

    if file_to_checksum.is_some() {
        let filepath = file_to_checksum.unwrap();
        println!("Calculating checksum of '{}' ...", filepath);
        let crc32_of_file = file_verification::get_crc32_from_file(&filepath, true).unwrap();
        println!("CRC32 of file: '{}' is {:x}\n", filepath, crc32_of_file);
    }

    if file_to_check_if_sfv.is_some() {
        let filepath = file_to_check_if_sfv.unwrap();
        println!("Checking if '{}' is valid SFV file ...", filepath);
        let sfv_file = file_verification::read_sfv(&filepath);
        match sfv_file {
            Ok(_f)  => println!("Valid SFV file"),
            Err(_e) => println!("File not readable"),
        }        
    }

    if do_fix_misnamed_sfv_files && source_file_path.is_some() {
        let file = source_file_path.as_ref().unwrap();
        fix_misnamed_sfv_files(&file, dry_run, verbose);
        return; // Always exit here, because if there were SFV files as input parameters 
                // from a glob, they might not be the same
    }

    let mut target_checksums: Vec<file_verification::ChecksumEntry> = Vec::new();

    if sfv_files.len() > 0 {
        for sfv_file_path in &sfv_files {
            println!("Reading {:?} ...", sfv_file_path);
            let sfv_file_result = file_verification::read_sfv(&sfv_file_path);
            if sfv_file_result.is_ok() {
                let sfv_file = sfv_file_result.unwrap();
                println!("{:?} entries found in '{:?}':", sfv_file.entries.len(), sfv_file_path);
                let mut i = 0;
                for e in &sfv_file.entries {
                    i += 1;
                    println!("[{}] '{}' crc32:{:x}", i, e.filename, e.checksum_crc32);
                }
                println!("");
                sfv_file.entries.into_iter().for_each(|c| target_checksums.push(c));
            }
        }
    }

    if source_file_path.is_some() && target_checksums.len() > 0 {
        let mut paths_ok = true;

        if !Path::new(&source_file_path.as_ref().unwrap()).exists() {
            println!("Source path {:?} does not exist", source_file_path);
            paths_ok = paths_ok && false;
        }

        if !Path::new(&destination_file_path.as_ref().unwrap()).exists() {
            println!("Destination path {:?} does not exist", source_file_path);
            paths_ok = paths_ok && false;
        }

        if paths_ok {
            repair_filenames_in_path(&source_file_path.unwrap(), &destination_file_path.unwrap(), &target_checksums, dry_run, verbose);
        }
    }
}

fn die(message: &str, exit_code: i32) {
    println!("{:?}", message);
    process::exit(exit_code);
}

struct RenamingRecommendation {
    source_file: String,
    target_name: String,
    checksum_crc32: u32,
}

fn fix_misnamed_sfv_files(path_s: &String, dry_run: bool, verbose: bool) -> u32 {
    let sfv_extension = ".sfv";
    let mut renamed_files = 0;

    let res = get_files_from_path(path_s);
    if res.is_ok() {
        for file_path in res.unwrap() {
            
            let path = String::from(file_path.as_path().to_str().unwrap());
            let mut new_path: Option<String> = None;

            if file_verification::is_sfv(&path) {
                if !path.ends_with(&sfv_extension) {
                    // rename +.sfv
                    new_path = Some(path.clone() + &sfv_extension);
                } else {
                    if verbose { println!("Keep {:?} a SFV file", path); }
                }
            } else {
                if path.ends_with(&sfv_extension) {
                    // rename +_not
                    new_path = Some(path.clone() + "_not");
                } else {
                    if verbose { println!("Keep {:?} not a SFV file", path); }
                }
            }

            if new_path.is_some() {
                let new_path = new_path.unwrap();
                if Path::new(&new_path).exists() {
                    println!("Will not rename {:?} to {:?} because target file already exists", path, new_path);
                } else {
                    println!("Rename {:?} to {:?}", path, new_path);
                    if !dry_run {
                        std::fs::rename(path, new_path).expect("Renaming failed!");
                        renamed_files += 1;
                    }
                }
            }

        }
    }
    renamed_files
}

fn repair_filenames_in_path(source_file_path: &String, destination_file_path: &String, 
        target_checksums: &Vec<file_verification::ChecksumEntry>, dry_run: bool, verbose: bool) {

    let dest_path = Path::new(&destination_file_path);

    let recommendations = get_repair_recommendations_by_path(&source_file_path, &target_checksums);

    let mut to_do:     Vec<&RenamingRecommendation> = Vec::new();
    let mut push_back: Vec<&RenamingRecommendation> = Vec::new();

    recommendations.iter().for_each(|x| to_do.push(&x));

    let mut rename_count = 1;

    while to_do.len() > 0 && rename_count > 0 {
        rename_count = 0;

        for recommendation in &to_do {
            if verbose {
                println!("Recommend renaming '{}' to '{}' because of matching checksum crc32:{:x}",
                    recommendation.source_file, recommendation.target_name, recommendation.checksum_crc32);
            }

            let src = Path::new(&recommendation.source_file);
            let dst = dest_path.join(&recommendation.target_name);

            if dst.exists() {
                if src == dst.as_path() {
                    println!("No need to rename {:?}", src.to_str().unwrap());
                } else {
                    println!("Will not rename {:?} because it will overwrite {:?}! Will try later.",
                        src.to_str().unwrap(),
                        dst.as_path().to_str().unwrap());
                    push_back.push(recommendation)
                }
            } else {
                rename_count += 1;
                if !dry_run {
                    println!("Renaming {:?} to {:?} ...", src.to_str(), dst.as_path().to_str());
                    std::fs::rename(src, dst).expect("Renaming failed!");
                } else {
                    println!("[dry run] Will rename {:?} to {:?}!",
                        src.to_str().unwrap(),
                        dst.as_path().to_str().unwrap());
                }
            }
        }

        if push_back.len() > 0 {
            println!("Try to rename {:?} pushed back files.", push_back.len());
        }

        to_do.clear();
        push_back.reverse(); // Important
        push_back.iter().for_each(|x| to_do.push(&x));

        if rename_count == 0 && to_do.len() > 0 {
            println!("Stuck in a renaming loop. {:?} files can not be renamed without having the same name! Abort!", to_do.len());
        }
    }
}

fn get_repair_recommendations_by_path(source_file_path: &String, target_checksums: &Vec<file_verification::ChecksumEntry>) -> Vec<RenamingRecommendation> {
    let existing_checksums = get_checksums_from_path(&source_file_path);
    return get_repair_recommendations(&existing_checksums, &target_checksums);
}

fn get_repair_recommendations(existing_checksums: &Vec<file_verification::ChecksumEntry>, target_checksums: &Vec<file_verification::ChecksumEntry>) -> Vec<RenamingRecommendation> {
    let mut recommendations: Vec<RenamingRecommendation> = Vec::new();
    for ecs in existing_checksums {
        for tcs in target_checksums {
            if tcs.valid && tcs.checksum_crc32 == ecs.checksum_crc32 {
                recommendations.push(RenamingRecommendation {
                    source_file: ecs.path.clone(),
                    target_name: tcs.filename.clone(),
                    checksum_crc32: ecs.checksum_crc32,
                });
            }
        }
    }
    recommendations
}

fn get_checksums_from_path(source_file_path: &String) -> Vec<file_verification::ChecksumEntry> {
    let existing_files = get_files_from_path(&source_file_path).unwrap();
    let mut existing_checksums: Vec<file_verification::ChecksumEntry> = Vec::new();

    for existing_file in existing_files {
        let path = String::from(existing_file.as_path().to_str().unwrap());
        let filename = String::from(existing_file.as_path().file_name().unwrap().to_str().unwrap());
        println!("Checking file '{}' ...", path);
        let crc32 = file_verification::get_crc32_from_file(&path, true).unwrap();
        let csf = file_verification::ChecksumEntry {
            filename: filename,
            path: path,
            checksum_crc32: crc32,
            valid: true,
        };
        existing_checksums.push(csf);
    }
    existing_checksums
}

fn get_files_from_path(path_s: &String) -> Result<Vec<PathBuf>, std::io::Error> {
    let path = Path::new(path_s);
    let paths = try!(std::fs::read_dir(path));

    // Warum funktioniert das nicht ... ?
    // paths.into_iter()
    //     .map(|x| x.unwrap().path())
    //     .filter(|x| x.is_dir())
    //     .collect()

    paths.into_iter()
        // .filter(|x| x.iter().map(|x| x.path().is_dir()).next().unwrap())
        .filter(|x| !x.as_ref().unwrap().path().is_dir())
        .map(|x| x.map(|p| p.path()))
        .collect()
}
