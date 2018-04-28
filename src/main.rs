// https://doc.rust-lang.org/book/second-edition/ch01-00-introduction.html

use std::env;
use std::process;

// use std::fs::read_dir;

use std::path::{Path, PathBuf};
// use std::collections::HashMap;

// use self::{ChecksumEntry, get_crc32_from_file, read_sfv};
// use self::ChecksumEntry;
// mod file_verification;

mod file_verification;

fn main() {
    const VERSION_MAJ: u32 = 0;
    const VERSION_MIN: u32 = 1;

    println!("reverse-checksum-renamer V{}.{}:\n", VERSION_MAJ, VERSION_MIN);

    let args: Vec<_> = env::args().collect();

    let mut path_of_files: String = String::new();
    let mut sfv_files: Vec<String> = Vec::new();
    let mut file_to_checksum: String = String::new();
    let mut verbose = false;
    let mut dry_run = false;

    let mut skip = 0;
    for i in 1 .. args.len() {
        if skip > 0 {
            skip = skip - 1;
        } else {
            if args[i] == "-o" {
                if i >= args.len() { die("Missing value for '-o' parameter", 1); return; }
                path_of_files = args[i+1].to_string();
                skip = 1;
            } else if args[i] == "-t" {
                if i >= args.len() { die("Missing value for '-o' parameter", 1); return; }
                file_to_checksum = args[i+1].to_string();
                skip = 1;
            } else if args[i] == "-v" {
                verbose = true;
            } else if args[i] == "-d" {
                dry_run = true;
            } else {
                sfv_files.push(args[i].to_string());
                if verbose { println!("Adding SFV file: {:?}", args[i]); }
            }
        }
    }

    if verbose {
        println!("Path of files: {:?}", path_of_files);
    }

    if file_to_checksum.len() > 0 {
        println!("Calculating checksum of '{}' ...", file_to_checksum);
        let crc32_of_file = file_verification::get_crc32_from_file(&file_to_checksum, true).unwrap();
        println!("CRC32 of file: '{}' is {:x}\n", file_to_checksum, crc32_of_file);
    }

    let mut target_checksums: Vec<file_verification::ChecksumEntry> = Vec::new();

    if sfv_files.len() > 0 {
        for sfv_file in &sfv_files {
            let checksums = file_verification::read_sfv(&sfv_file);
            println!("{:?} entries found in '{:?}':", checksums.len(), sfv_file);
            let mut i = 0;
            for e in &checksums {
                i += 1;
                println!("[{}] '{}' crc32:{:x}", i, e.filename, e.checksum_crc32);
            }
            println!("");
            checksums.into_iter().for_each(|c| target_checksums.push(c));
        }
    }

    if path_of_files.len() > 0 && target_checksums.len() > 0 {
        repair_filenames_in_path(&path_of_files, &target_checksums, dry_run, verbose);
    }
}

fn die(message: &str, exit_code: i32) {
    println!("{:?}", message);
    process::exit(exit_code);
}

fn repair_filenames_in_path(source_file_path: &String, target_checksums: &Vec<file_verification::ChecksumEntry>, dry_run: bool, verbose: bool) {
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
            let dst = src.with_file_name(&recommendation.target_name);
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

struct RenamingRecommendation {
    source_file: String,
    target_name: String,
    checksum_crc32: u32,
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

// Alternative module
// pub mod file_verification { <contents of file_verification.rs> }