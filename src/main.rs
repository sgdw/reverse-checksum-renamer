// https://doc.rust-lang.org/book/second-edition/ch01-00-introduction.html

mod utils;
mod file_verification;
mod par2_reader;
mod sfv_reader;

use std::fs;
use std::env;
use std::process;

use std::path::{Path, PathBuf};

const STATE_FILE_FOUND: u8 = 0;

fn main() {
    const VERSION_MAJ: u32 = 0;
    const VERSION_MIN: u32 = 1;

    let args: Vec<_> = env::args().collect();

    let mut source_file_path: Option<String> = None;
    let mut destination_file_path: Option<String> = None;
    let mut catalog_files: Vec<String> = Vec::new();
    
    let mut file_to_checksum: Option<String> = None;
    let mut file_to_decode: Option<String> = None;

    let mut do_fix_misnamed_catalog_files = false;
    let mut do_show_usage = false;

    let mut group_into_subfolder = false;
    let mut only_complete_sets = false;

    let mut verbose = false;
    let mut dry_run = false;

    if args.len() == 1 {
        do_show_usage = true;
    }

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

            } else if args[i] == "--show-catalog" || args[i] == "-p" {
                if i >= args.len() { die(&format!("Missing value for '{}' parameter", args[i]), 1); return; }
                file_to_decode = Some(args[i+1].to_string());
                skip = 1;

            } else if args[i] == "--checksum-file" {
                if i >= args.len() { die(&format!("Missing value for '{}' parameter", args[i]), 1); return; }
                file_to_checksum = Some(args[i+1].to_string());
                skip = 1;

            } else if args[i] == "--fix-catalog-files" || args[i] == "-f" {
                if i >= args.len() { die(&format!("Missing value for '{}' parameter", args[i]), 1); return; }
                do_fix_misnamed_catalog_files = true;

            } else if args[i] == "-c" {
                only_complete_sets = true;

            } else if args[i] == "-g" {
                group_into_subfolder = true;

            } else if args[i] == "-v" {
                verbose = true;

            } else if args[i] == "-d" {
                dry_run = true;

            } else if args[i] == "--help" {
                do_show_usage = true;

            } else {
                catalog_files.push(args[i].to_string());
                if verbose { println!("Adding catalog file: {:?}", args[i]); }
            }
        }
    }

    if verbose {
        println!("reverse-checksum-renamer V{}.{}", VERSION_MAJ, VERSION_MIN);
        par2_reader::set_verbose(verbose);
    }

    if do_show_usage {
        println!("Usage: reverse-checksum-renamer [-i <input>] [-o <output>] <SFV/PAR2 files>");
        println!("  -i  input folder");
        println!("  -o  output folder");
        println!("  -p  show referenced files in par2 or sfv file");
        println!("      (--show-catalog)");
        println!("  -f  find PAR2/SFV files and rename them");
        println!("      (--fix-catalog-files)");
        println!("  -c  only complete sets");
        println!("  -g  group into subfolders");
        println!("  -v  verbose");
        println!("  -d  dry run");
        println!("  --checksum-file print checksums of a file");
        
        return;
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
        let checksum_of_file = file_verification::get_checksum_from_file(&filepath, true).unwrap();
        println!("CRC32: {:x}", checksum_of_file.checksum_crc32.unwrap());
        println!("MD5:   {}", checksum_of_file.checksum_md5_as_str());
        println!("");
    }

    if file_to_decode.is_some() {
        let filepath = file_to_decode.unwrap();

        #[allow(unused_assignments)]
        let mut catalog_file: Option<file_verification::ChecksumCatalog> = None;
        
        catalog_file = match par2_reader::read_par2(&filepath) {
            Ok(_cf) => if _cf.valid { Some(_cf) } else { None },
            Err(_e) => None
        };

        if catalog_file.is_none() {
            catalog_file = match sfv_reader::read_sfv(&filepath) {
                Ok(_cf) => if _cf.valid { Some(_cf) } else { None },
                Err(_e) => None
            };

        }

        if catalog_file.is_none() {
            println!("Not a valid catalog file");
        } else {
            let catalog_file = catalog_file.unwrap();
            println!("Valid {} file having {} file references:", catalog_file.source_type, catalog_file.entries.len());
            for (i, entry) in catalog_file.entries.iter().enumerate() {
                println!("[{}] {:?}", i, entry.filename);
            }
        }
    }

    if do_fix_misnamed_catalog_files && source_file_path.is_some() {
        let file = source_file_path.as_ref().unwrap();
        fix_misnamed_catalog_files(&file, dry_run, verbose);
        return; // Always exit here, because if there were SFV files as input parameters 
                // from a glob, they might not be the same
    }

    let mut source_catalogs: Vec<file_verification::ChecksumCatalog> = Vec::new();

    if catalog_files.len() > 0 {
        for catalog_file_path in &catalog_files {
            println!("Reading {:?} ...", catalog_file_path);

            let src_type = file_verification::get_source_type_by_filename(&catalog_file_path);
            if src_type.is_some() {
                
                let catalog_opt = match src_type.unwrap() {
                    file_verification::SourceTypes::SFV  => sfv_reader::read_sfv(&catalog_file_path).ok(),
                    file_verification::SourceTypes::PAR2 => par2_reader::read_par2(&catalog_file_path).ok(),
                };

                if catalog_opt.is_some() {
                    let catalog = catalog_opt.unwrap();
                    println!("{:?} entries found in '{}':", catalog.entries.len(), catalog_file_path);
                    let mut i = 0;
                    for e in &catalog.entries {
                        i += 1;
                        let str_crc32 = e.checksum_crc32.map_or_else(|| "".to_string(), |v| format!("{:x}", v));
                        let str_md5   = e.checksum_md5.map_or_else(|| "".to_string(),|v| utils::byte_array_to_hex(&v));
                        println!("[{}] '{}' crc32:{} md5:{}", i, e.filename, str_crc32, str_md5);
                    }
                    println!("");
                    source_catalogs.push(catalog);
                }
            } else {
                println!("Filetype of {} is not recognized!", &catalog_file_path);
            }
        }
    }

    if source_file_path.is_some() && source_catalogs.len() > 0 {
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
            let mut existing_checksums = get_checksums_from_path(&source_file_path.unwrap());
            let destination_file_path = destination_file_path.unwrap();

            for mut catalog in source_catalogs {

                let recommendations = get_repair_recommendations(&mut existing_checksums, &mut catalog.entries);
                println!("");
                println!("Recommendations for {}:", catalog.source_file);
                let mut i = 0;
                for recommendation in &recommendations {
                    i+=1;
                    println!("[{}] {} -> {}", i, recommendation.source_file, recommendation.target_name);
                }

                println!("");
                if catalog_has_missing_files(&catalog) {
                    println!("Catalog {} has missing files!", catalog.source_file);
                    if only_complete_sets {
                        println!("Will not process files.");
                        continue;
                    }
                } else {
                    println!("Catalog {} is complete", catalog.source_file);
                }

                // let mut final_destination_path = Path::new(&destination_file_path);
                if group_into_subfolder {
                    let catalog_path = Path::new(&catalog.source_file);
                    
                    let mut catalog_filename = String::from(catalog_path.file_name().unwrap().to_str().unwrap());
                    catalog_filename.push_str("_FILES");
                    if verbose { println!("Subfolder name {:?}", catalog_filename); }

                    let new_final_destination_path = Path::new(&destination_file_path).join(&catalog_filename);

                    if !new_final_destination_path.exists() {
                        if dry_run {
                            println!("Would create directory '{:?}'", 
                                new_final_destination_path.to_str().ok_or_else(|| "Error showing path").unwrap());
                        } else {
                            if fs::create_dir(&new_final_destination_path).is_err() {
                                println!("Could not create directory '{}'", 
                                    new_final_destination_path.to_str().ok_or_else(|| "Error showing path").unwrap());
                            }
                        }
                    }
                    if verbose { println!("Will group into folder {:?}", new_final_destination_path); }
                    // Path::new(&new_final_destination_path)
                    repair_filenames(&mut existing_checksums, &mut catalog.entries, &new_final_destination_path, dry_run, verbose);

                } else {
                    // if verbose { println!("Not grouping results {:?}", final_destination_path); }
                    // Path::new(&destination_file_path)
                    let new_final_destination_path = Path::new(&destination_file_path);
                    if verbose { println!("GAHHH {:?}", new_final_destination_path); }
                    repair_filenames(&mut existing_checksums, &mut catalog.entries, &new_final_destination_path, dry_run, verbose);
                };

            }
        }
    }
}

fn die(message: &str, exit_code: i32) {
    println!("{:?}", message);
    process::exit(exit_code);
}

#[derive(Debug)]
struct RenamingRecommendation {
    source_file: String,
    target_name: String,
}

fn fix_misnamed_catalog_files(path_s: &String, dry_run: bool, verbose: bool) -> u32 {
    let sfv_extension  = ".".to_owned() + sfv_reader::EXTENSION;
    let par2_extension = ".".to_owned() + par2_reader::EXTENSION;
    let mut renamed_files = 0;

    let res = get_files_from_path(path_s);
    if res.is_ok() {
        for file_path in res.unwrap() {
            
            let path = String::from(file_path.as_path().to_str().unwrap());
            let mut new_path: Option<String> = None;

            if verbose { println!("Checking '{}' ...", &path); }

            if sfv_reader::is_sfv(&path) {
                if !path.ends_with(&sfv_extension) {
                    // rename +.sfv
                    new_path = Some(path.clone() + &sfv_extension);
                } else {
                    if verbose { println!("Keep {:?} a SFV file", path); }
                }
            } else if par2_reader::is_par2(&path) {
                if !path.ends_with(&par2_extension) {
                    // rename +.par2
                    new_path = Some(path.clone() + &par2_extension);
                } else {
                    if verbose { println!("Keep {:?} a PAR2 file", path); }
                }
            } else {
                if path.ends_with(&sfv_extension) || path.ends_with(&par2_extension) {
                    // rename +_not
                    new_path = Some(path.clone() + "_not");
                } else {
                    if verbose { println!("Keep {:?} not a PAR2/SFV file", path); }
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

fn catalog_has_missing_files(catalog: &file_verification::ChecksumCatalog) -> bool {
    let ignore = [".nfo", ".txt", ".srr", ".sfv", ".par2"];
    for entry in &catalog.entries {
        for ext in ignore.iter() {
            if entry.filename.ends_with(ext) {
                continue;
            }
        }
        if !entry.has_state(STATE_FILE_FOUND) {
            return true;
        }
    }
    false
}

fn repair_filenames(
        mut source_checksums: &mut Vec<file_verification::ChecksumEntry>, 
        mut target_checksums: &mut Vec<file_verification::ChecksumEntry>, 
        destination_file_path: &Path, 
        dry_run: bool, verbose: bool,
        // only_complete_set: bool
        ) -> bool {

    let dest_path = Path::new(&destination_file_path);

    let recommendations = get_repair_recommendations(&mut source_checksums, &mut target_checksums);
    
    // if only_complete_set {
    //     for recommendation in &recommendations {
    //         let src = Path::new(&recommendation.source_file);
    //         if !src.exists() {
    //             return false;
    //         }
    //     }
    // }

    let mut to_do:     Vec<&RenamingRecommendation> = Vec::new();
    let mut push_back: Vec<&RenamingRecommendation> = Vec::new();

    recommendations.iter().for_each(|x| to_do.push(&x));

    let mut rename_count = 1;

    while to_do.len() > 0 && rename_count > 0 {
        rename_count = 0;

        for recommendation in &to_do {
            if verbose {
                println!("Recommend renaming '{}' to '{}'",
                    recommendation.source_file, recommendation.target_name);
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
                    if src.exists() {
                        std::fs::rename(src, dst).expect("Renaming failed!");
                    } else {
                        println!("Not renaming {:?}. File not found!", src.to_str());
                    }
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
    true
}

fn get_repair_recommendations(existing_checksums: &mut Vec<file_verification::ChecksumEntry>, target_checksums: &mut Vec<file_verification::ChecksumEntry>) -> Vec<RenamingRecommendation> {
    let mut recommendations: Vec<RenamingRecommendation> = Vec::new();

    for mut ecs in existing_checksums.iter_mut() {
        for mut tcs in target_checksums.iter_mut() {

            if tcs.valid {
                let crc32_matches = tcs.checksum_crc32.is_some() && ecs.checksum_crc32.is_some() && tcs.checksum_crc32 == ecs.checksum_crc32;
                let md5_matches   = tcs.checksum_md5.is_some()   && ecs.checksum_md5.is_some()   && tcs.checksum_md5 == ecs.checksum_md5;

                if crc32_matches || md5_matches {
                    recommendations.push(RenamingRecommendation {
                        source_file: ecs.path.clone(),
                        target_name: tcs.filename.clone(),
                    });
                    ecs.set_state(STATE_FILE_FOUND);
                    tcs.set_state(STATE_FILE_FOUND);
                }
            }
        }
    }
    recommendations
}

fn get_checksums_from_path(source_file_path: &String) -> Vec<file_verification::ChecksumEntry> {
    let existing_files = get_files_from_path(&source_file_path).unwrap();
    let mut existing_checksums: Vec<file_verification::ChecksumEntry> = Vec::new();
    let num_files = existing_files.len();

    for (i, existing_file) in existing_files.iter().enumerate() {
        let path = String::from(existing_file.as_path().to_str().unwrap());
        println!("[{} of {}] Checking file '{}' ...", i+1, num_files, path);
        let csf = file_verification::get_checksum_from_file(&path, true).unwrap();
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
